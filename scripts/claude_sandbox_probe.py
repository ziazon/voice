"""Check your Claude Code inside the macOS sandbox with a separate profile.

Only ``ask`` spends a little of your Claude usage. The probe never prints a reply
or a credential. During ``sign-in`` it copies Claude Code's own sign-in output,
including its link, to your terminal; it never reads or prints the login file.
"""
import argparse
import ipaddress
import json
import math
import os
from pathlib import Path
import pwd
import re
import select
import shutil
import signal
import socket
import socketserver
import stat
import subprocess
import tempfile
import threading
import time

PROFILE = Path(__file__).resolve().parent.parent / 'builder/agent/sandbox/claude-code.sb'
HEADER_TIMEOUT = 10
TUNNEL_IDLE_TIMEOUT = 120
ALLOWED_HOSTS = frozenset(('api.anthropic.com', 'platform.claude.com', 'claude.ai'))
ASK_FLAGS = ['-p', '--model', 'haiku', '--safe-mode', '--no-session-persistence',
             '--tools', '', '--output-format', 'json',
             '--strict-mcp-config', '--mcp-config', '{"mcpServers":{}}',
             '--permission-mode', 'dontAsk', '--max-budget-usd', '0.05']


MAX_LINK = 4096
# Claude Code 2.1.292's compiled client id, measured in the link handed to BROWSER.
CLAUDE_CODE_CLIENT_ID = '9d1c250a-e61b-44d9-88ed-5944d1962f5e'
HELPER_SCRIPT = '#!/bin/bash\numask 077\nprintf \'%s\\n\' "$1" > "${0%/*}/sign-in-link"\n'


def valid_link(link):
    if not (1 <= len(link) <= MAX_LINK
            and all(0x21 <= ord(char) <= 0x7e and char not in '"<>\\^`{|}#' for char in link)
            and link.startswith('https://platform.claude.com/oauth/authorize?')):
        return False
    query = {}
    for pair in link.split('?', 1)[1].split('&'):
        key, separator, value = pair.partition('=')
        if not separator or re.fullmatch(r'[A-Za-z0-9_]+', key) is None or key in query:
            return False
        decoded = bytearray()
        index = 0
        while index < len(value):
            if value[index] == '%':
                digits = value[index + 1:index + 3]
                if len(digits) != 2 or any(char not in '0123456789abcdefABCDEF' for char in digits):
                    return False
                decoded.append(int(digits, 16))
                index += 3
            else:
                decoded.append(ord(value[index]))
                index += 1
        query[key] = bytes(decoded)
    if (query.get('client_id') != CLAUDE_CODE_CLIENT_ID.encode('ascii')
            or query.get('response_type') != b'code'):
        return False
    callback = query.get('redirect_uri', b'')
    return re.fullmatch(rb'http://localhost:([1-9][0-9]{0,4})/callback', callback) is not None and int(
        callback[len(b'http://localhost:'):-len(b'/callback')]) <= 65535


def open_sign_in_link(run):
    path = run / 'sign-in-link'
    try:
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    except FileNotFoundError:
        return None
    except OSError:
        return False
    pending = False
    try:
        if not stat.S_ISREG(os.fstat(descriptor).st_mode):
            os.close(descriptor)
            return False
        with os.fdopen(descriptor, 'rb') as source:
            data = source.read(MAX_LINK + 2)
        pending = not data.endswith(b'\n') and len(data) <= MAX_LINK
        if pending:
            return None
    finally:
        if not pending:
            try:
                path.unlink()
            except OSError:
                pass
    try:
        link = data[:-1].decode('utf-8') if data.endswith(b'\n') else ''
    except UnicodeDecodeError:
        return False
    if not valid_link(link):
        return False
    subprocess.run(['/usr/bin/open', link], check=False)
    print("Opened Claude's page in your browser.", flush=True)
    return True


def host_name(target):
    """Keep only a host name safe to show in the connection counts."""
    host = target.rsplit(':', 1)[0] if ':' in target else target
    host = host.strip('[]').lower().removesuffix('.')
    try:
        return str(ipaddress.ip_address(host))
    except ValueError:
        if len(host) <= 253 and re.fullmatch(r'[a-z0-9-]+(?:\.[a-z0-9-]+)*', host):
            return host
        return '(invalid)'


def terminal_text(text):
    """Drop terminal controls, keeping only tab from the control ranges."""
    return re.sub(r'[\x00-\x08\x0a-\x1f\x7f-\x9f]', '', text)


class ProxyServer(socketserver.ThreadingTCPServer):
    daemon_threads = False
    block_on_close = True

    def __init__(self, owner, bind_and_activate=True):
        self.owner = owner
        self._slots = threading.BoundedSemaphore(16)

        class Handler(socketserver.BaseRequestHandler):
            def handle(self):
                owner.handle_connection(self.request)

        super().__init__(('127.0.0.1', 0), Handler, bind_and_activate=bind_and_activate)

    def process_request(self, request, client_address):
        # Reserve before starting a thread, so incomplete headers cannot grow
        # either the socket set or the number of worker threads without bound.
        if not self._slots.acquire(blocking=False):
            self.owner._count('(other)', False)
            self.shutdown_request(request)
            return
        try:
            super().process_request(request, client_address)
        except BaseException:
            self._slots.release()
            raise

    def process_request_thread(self, request, client_address):
        try:
            super().process_request_thread(request, client_address)
        finally:
            self._slots.release()


class EgressProxy:
    """Let CONNECT requests reach only the three Claude hosts on port 443."""

    def __init__(self, connect=socket.create_connection):
        self.connect = connect
        self._counts = {}
        self._lock = threading.Lock()
        self._stopped = threading.Event()
        self._sockets = set()
        self._server = None
        self._thread = None

    def start(self):
        self._server = ProxyServer(self)
        self.port = self._server.server_address[1]
        self._thread = threading.Thread(target=self._server.serve_forever,
                                        kwargs={'poll_interval': 0.05}, daemon=True)

        self._thread.start()

    def stop(self):
        self._stopped.set()
        if self._thread is not None and self._thread.is_alive():
            self._server.shutdown()
            self._thread.join()
        with self._lock:
            for sock in self._sockets:
                try:
                    sock.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass
        # Join request handlers too, including a dial still reaching its timeout.
        if self._server is not None:
            self._server.server_close()

    def counts(self):
        with self._lock:
            return {host: tuple(count) for host, count in self._counts.items()}

    def _count(self, host, allowed):
        with self._lock:
            if host not in self._counts and sum(name != '(other)' for name in self._counts) >= 32:
                host = '(other)'
            count = self._counts.setdefault(host, [0, 0])
            count[0 if allowed else 1] += 1

    @staticmethod
    def _answer(child, code):
        messages = {200: 'Connection established', 403: 'Forbidden',
                    405: 'Method Not Allowed', 502: 'Bad Gateway'}
        child.sendall(('HTTP/1.1 %d %s\r\n\r\n' % (code, messages[code])).encode('ascii'))

    def handle_connection(self, child):
        upstream = None
        host = '(invalid)'
        counted = False
        with self._lock:
            self._sockets.add(child)
        try:
            deadline = time.monotonic() + HEADER_TIMEOUT
            data = b''
            complete = False
            while not self._stopped.is_set():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    break
                child.settimeout(remaining)
                try:
                    chunk = child.recv(4096)
                except socket.timeout:
                    break
                if not chunk:
                    break
                data += chunk
                end = data.find(b'\r\n\r\n')
                if end >= 0:
                    complete = end + 4 <= 8192
                    break
                if len(data) > 8192:
                    break
            line = data.split(b'\r\n', 1)[0].decode('ascii', errors='replace')
            words = line.split(' ')
            if len(words) >= 2:
                host = host_name(words[1])
            code = 403
            if complete and words[0] != 'CONNECT':
                code = 405
            valid = (complete and len(words) == 3 and words[0] == 'CONNECT'
                     and words[2] in ('HTTP/1.0', 'HTTP/1.1')
                     and re.fullmatch(r'[A-Za-z0-9.-]+:443', words[1]) is not None
                     and host in ALLOWED_HOSTS)
            self._count(host, valid)
            counted = True
            if not valid:
                self._answer(child, code)
                return
            try:
                upstream = self.connect((host, 443), timeout=10)
            except OSError:
                self._answer(child, 502)
                return
            with self._lock:
                self._sockets.add(upstream)
            if self._stopped.is_set():
                return
            self._answer(child, 200)
            child.settimeout(10)
            upstream.settimeout(10)
            tail = data[end + 4:]
            if tail:
                upstream.sendall(tail)
            last_byte = time.monotonic()
            while not self._stopped.is_set():
                remaining = TUNNEL_IDLE_TIMEOUT - (time.monotonic() - last_byte)
                if remaining <= 0:
                    return
                ready, _, _ = select.select([child, upstream], [], [], min(0.2, remaining))
                for source in ready:
                    chunk = source.recv(65536)
                    if not chunk:
                        return
                    (upstream if source is child else child).sendall(chunk)
                    last_byte = time.monotonic()
        except OSError:
            if not counted:
                self._count(host, False)
        finally:
            with self._lock:
                self._sockets.discard(child)
                self._sockets.discard(upstream)
            if upstream is not None:
                upstream.close()


def prepare_profile(value):
    path = Path(value)
    if not path.is_absolute():
        raise ValueError('Use an absolute path for the profile folder.')
    home = Path(pwd.getpwuid(os.getuid()).pw_dir).resolve()
    resolved = path.resolve()
    if resolved == home or resolved in home.parents:
        raise ValueError('Use a throwaway folder for the profile, not your home folder.')
    try:
        path.mkdir(mode=0o700, parents=True, exist_ok=True)
    except FileExistsError:
        raise ValueError('That profile folder is a link or not a folder.') from None
    # lstat checks the chosen folder itself, without following a link.
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode):
        raise ValueError('That profile folder is a link or not a folder.')
    marker = path / '.voice-sandbox-probe'
    if any(path.iterdir()):
        if marker.is_symlink() or not marker.is_file():
            raise ValueError("That folder isn't empty and wasn't made by this probe. Use a new folder.")
    else:
        with marker.open('x'):
            pass
        marker.chmod(0o600)
    path.chmod(0o700)
    return path.resolve()


def fallbacks(home):
    return [home / '.local/bin/claude', Path('/opt/homebrew/bin/claude'),
            Path('/usr/local/bin/claude')]


def find_claude(home):
    candidates = [Path(folder) / 'claude' for folder in os.environ.get('PATH', '').split(os.pathsep)
                  if Path(folder).is_absolute()]
    for path in candidates + fallbacks(home):
        if path.is_file() and os.access(path, os.X_OK):
            program = path.resolve()
            with program.open('rb') as source:
                header = source.read(4)
            if header not in (b'\xcf\xfa\xed\xfe', b'\xca\xfe\xba\xbe', b'\xca\xfe\xba\xbf'):
                raise ValueError("This Claude Code is an older kind that runs as a script, which Voice can't run inside its sandbox. Install the current Claude Code (https://code.claude.com/docs/en/setup), then try again.")
            return program
    raise ValueError("Claude Code isn't installed: no claude on PATH or in the usual places.")


def child_environment(profile, run, port, sign_in):
    user = pwd.getpwuid(os.getuid())
    proxy = 'http://127.0.0.1:%d' % port
    language = os.environ.get('LANG', '')
    if not re.fullmatch(r'[A-Za-z0-9_.@-]{1,64}', language):
        language = 'en_US.UTF-8'
    env = {'HOME': user.pw_dir, 'USER': user.pw_name, 'LOGNAME': user.pw_name,
           'PATH': '/usr/bin:/bin:/usr/sbin:/sbin',
           'LANG': language,
           'CLAUDE_CONFIG_DIR': str(profile), 'CLAUDE_CODE_TMPDIR': str(run),
           'TMPDIR': str(run) + '/', 'HTTPS_PROXY': proxy, 'https_proxy': proxy,
           'CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC': '1', 'DISABLE_AUTOUPDATER': '1'}
    if sign_in:
        env['BROWSER'] = str(run / 'open-sign-in')
    return env


def command(claude, profile, run, port, action):
    claude = claude.resolve()
    argv = ['/usr/bin/sandbox-exec', '-f', str(PROFILE)]
    for key, value in [('PROGRAM', claude), ('PROFILE_DIR', profile),
                       ('RUN_DIR', run), ('PROXY_PORT', port),
                       ('SIGN_IN', 'yes' if action == 'sign-in' else 'no')]:
        argv.extend(['-D', '%s=%s' % (key, value)])
    actions = {'version': ['--version'], 'status': ['auth', 'status', '--json'],
               'sign-in': ['auth', 'login', '--console'], 'ask': ASK_FLAGS,
               'sign-out': ['auth', 'logout']}
    return argv + [str(claude), '--setting-sources', 'project,local'] + actions[action]


def kill_group(child):
    try:
        os.killpg(child.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    child.wait()


def run_child(claude, profile, run, port, action):
    sign_in = action == 'sign-in'
    if sign_in:
        descriptor = os.open(run / 'open-sign-in', os.O_WRONLY | os.O_CREAT | os.O_EXCL
                             | os.O_NOFOLLOW, 0o700)
        with os.fdopen(descriptor, 'w', encoding='utf-8') as helper:
            os.fchmod(helper.fileno(), 0o700)
            helper.write(HELPER_SCRIPT)
    argv = command(claude, profile, run, port, action)
    timeout = 600 if sign_in else (120 if action == 'ask' else 30)
    with subprocess.Popen(argv, cwd=run, env=child_environment(profile, run, port, sign_in),
                          stdin=None if sign_in else subprocess.PIPE,
                          stdout=subprocess.PIPE,
                          stderr=subprocess.STDOUT if sign_in else subprocess.PIPE,
                          text=True, encoding='utf-8', errors='replace',
                          start_new_session=True) as child:
        copier = None
        try:
            if sign_in:
                def copy_lines():
                    for line in child.stdout:
                        print(terminal_text(line), end='\n' if line.endswith('\n') else '', flush=True)
                copier = threading.Thread(target=copy_lines, daemon=True)
                copier.start()
                deadline = time.monotonic() + timeout
                while True:
                    if open_sign_in_link(run) is False:
                        print("Claude Code handed over a link Voice won't open.", flush=True)
                        return subprocess.CompletedProcess(argv, 1, '', '')
                    if child.poll() is not None:
                        return subprocess.CompletedProcess(argv, child.returncode, '', '')
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        raise subprocess.TimeoutExpired(argv, timeout)
                    try:
                        child.wait(timeout=min(0.2, remaining))
                    except subprocess.TimeoutExpired:
                        pass
            stdout, stderr = child.communicate(
                input='Reply with the single word: ok' if action == 'ask' else '', timeout=timeout)
            return subprocess.CompletedProcess(argv, child.returncode, stdout, stderr)
        finally:
            kill_group(child)
            if copier is not None:
                copier.join(timeout=1)


def failure(ran):
    print("Claude Code doesn't run inside the sandbox (exit %d)." % ran.returncode)
    lines = (ran.stderr or '').strip().split('\n')
    if lines:
        print(terminal_text(lines[-1]).replace(pwd.getpwuid(os.getuid()).pw_dir, '~')[:200])
    return 1


def read_object(text):
    try:
        value = json.loads(text)
        return value if isinstance(value, dict) else {}
    except (ValueError, TypeError):
        return {}


def perform(action, claude, profile, run, port):
    def call(kind):
        return run_child(claude, profile, run, port, kind)

    def check_status():
        version = call('version')
        words = version.stdout.split()
        number = words[0] if words else ''
        if version.returncode or not re.fullmatch(r'[0-9]+(?:\.[0-9]+){1,3}', number):
            failure(version)
            return None
        status = call('status')
        data = read_object(status.stdout)
        if type(data.get('loggedIn')) is not bool:
            if status.returncode:
                failure(status)
            else:
                print("Claude Code's sign-in status could not be read.")
            return None
        print('Claude Code %s runs inside the sandbox.' % number)
        print('Signed in inside the sandbox: %s.' % ('yes' if data['loggedIn'] else 'no'))
        return data['loggedIn']

    if action == 'status':
        return 0 if check_status() is not None else 1
    if action == 'sign-in':
        print("Sign in on Claude's page, which opens in your browser.", flush=True)
        ran = call('sign-in')
        signed_in = check_status()
        kept = (profile / '.credentials.json').is_file()
        print("The login is %sin the profile folder's own file." % ('kept ' if kept else 'not '))
        return 0 if ran.returncode == 0 and signed_in is True else 1
    if action == 'sign-out':
        ran = call('sign-out')
        if ran.returncode:
            return failure(ran)
        print('Signed out inside the sandbox.')
        return 0
    ran = call('ask')
    data = read_object(ran.stdout)
    result = data.get('result')
    answered = ran.returncode == 0 and data.get('is_error') is False and isinstance(result, str)
    ok = answered and result.strip().lower() == 'ok'
    cost = data.get('total_cost_usd', 0)
    if type(cost) not in (int, float) or not math.isfinite(cost) or cost < 0:
        cost = 0
    print('Answered: %s.' % ('yes' if answered else 'no'))
    print('It said ok: %s.' % ('yes' if ok else 'no'))
    print('Cost: $%.4f.' % cost)
    if data.get('is_error') is True and isinstance(result, str):
        print('Claude Code said: ' + terminal_text(result)[:200])
    return 0 if ok else 1


def print_counts(proxy):
    counts = proxy.counts() if proxy is not None else {}
    if not counts:
        print('Network: nothing.')
    for host, (allowed, refused) in sorted(counts.items()):
        print('Network: %s allowed %d, refused %d.' % (host, allowed, refused))


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', required=True)
    parser.add_argument('action', choices=['status', 'sign-in', 'ask', 'sign-out'])
    args = parser.parse_args(argv)
    run = None
    proxy = None
    result = 1
    try:
        profile = prepare_profile(args.profile)
        claude = find_claude(Path(pwd.getpwuid(os.getuid()).pw_dir))
        run = Path(tempfile.mkdtemp(prefix='voice-sandbox-')).resolve()
        run.chmod(0o700)
        proxy = EgressProxy()
        proxy.start()
        result = perform(args.action, claude, profile, run, proxy.port)
    except ValueError as error:
        print(str(error))
    except subprocess.TimeoutExpired:
        print('Claude Code took too long inside the sandbox.')
    except (OSError, KeyboardInterrupt):
        print('The sandbox check could not finish.')
    finally:
        if proxy is not None:
            proxy.stop()
        print_counts(proxy)
        if run is not None:
            try:
                shutil.rmtree(run)
            except OSError:
                path = terminal_text(str(run)).replace(pwd.getpwuid(os.getuid()).pw_dir, '~')
                print('The run folder could not be removed: %s.' % path)
                result = 1
    return result


if __name__ == '__main__':
    raise SystemExit(main())
