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
ALLOWED_HOSTS = frozenset(('api.anthropic.com', 'platform.claude.com', 'claude.ai'))
ASK_FLAGS = ['-p', '--model', 'haiku', '--safe-mode', '--no-session-persistence',
             '--tools', '', '--output-format', 'json', '--setting-sources', 'project,local',
             '--strict-mcp-config', '--mcp-config', '{"mcpServers":{}}',
             '--permission-mode', 'dontAsk', '--max-budget-usd', '0.05']


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
        owner = self

        class Handler(socketserver.BaseRequestHandler):
            def handle(self):
                owner.handle_connection(self.request)

        class Server(socketserver.ThreadingTCPServer):
            daemon_threads = False
            block_on_close = True

        self._server = Server(('127.0.0.1', 0), Handler)
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
            while not self._stopped.is_set():
                ready, _, _ = select.select([child, upstream], [], [], 0.2)
                for source in ready:
                    chunk = source.recv(65536)
                    if not chunk:
                        return
                    (upstream if source is child else child).sendall(chunk)
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
    try:
        path.mkdir(mode=0o700, parents=True, exist_ok=True)
    except FileExistsError:
        raise ValueError('That profile folder is a link or not a folder.') from None
    # lstat checks the chosen folder itself, without following a link.
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode):
        raise ValueError('That profile folder is a link or not a folder.')
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
            return Path(os.path.realpath(path))
    raise ValueError("Claude Code isn't installed: no claude on PATH or in the usual places.")


def child_environment(profile, run, port, sign_in):
    user = pwd.getpwuid(os.getuid())
    proxy = 'http://127.0.0.1:%d' % port
    env = {'HOME': user.pw_dir, 'USER': user.pw_name, 'LOGNAME': user.pw_name,
           'PATH': '/usr/bin:/bin:/usr/sbin:/sbin',
           'LANG': os.environ.get('LANG', 'en_US.UTF-8'),
           'CLAUDE_CONFIG_DIR': str(profile), 'CLAUDE_CODE_TMPDIR': str(run),
           'TMPDIR': str(run) + '/', 'HTTPS_PROXY': proxy, 'https_proxy': proxy,
           'CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC': '1'}
    if sign_in:
        env['BROWSER'] = '/usr/bin/false'
    return env


def command(claude, profile, run, port, action):
    argv = ['/usr/bin/sandbox-exec', '-f', str(PROFILE)]
    for key, value in [('PROGRAM_DIR', claude.parent), ('PROFILE_DIR', profile),
                       ('RUN_DIR', run), ('PROXY_PORT', port),
                       ('SIGN_IN', 'yes' if action == 'sign-in' else 'no')]:
        argv.extend(['-D', '%s=%s' % (key, value)])
    actions = {'version': ['--version'], 'status': ['auth', 'status', '--json'],
               'sign-in': ['auth', 'login', '--claudeai'], 'ask': ASK_FLAGS,
               'sign-out': ['auth', 'logout']}
    return argv + [str(claude)] + actions[action]


def kill_group(child):
    try:
        os.killpg(child.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    child.wait()


def run_child(claude, profile, run, port, action):
    sign_in = action == 'sign-in'
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
                        print(line, end='', flush=True)
                copier = threading.Thread(target=copy_lines, daemon=True)
                copier.start()
                child.wait(timeout=timeout)
                copier.join(timeout=1)
                return subprocess.CompletedProcess(argv, child.returncode, '', '')
            stdout, stderr = child.communicate(
                input='Reply with the single word: ok' if action == 'ask' else '', timeout=timeout)
            return subprocess.CompletedProcess(argv, child.returncode, stdout, stderr)
        except BaseException:
            kill_group(child)
            if copier is not None:
                copier.join(timeout=1)
            raise


def failure(ran):
    print("Claude Code doesn't run inside the sandbox (exit %d)." % ran.returncode)
    lines = (ran.stderr or '').strip().splitlines()
    if lines:
        print(lines[-1].replace(pwd.getpwuid(os.getuid()).pw_dir, '~')[:200])
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
        print("Sign in on Claude's page. If no browser opens, copy the link Claude Code prints below.", flush=True)
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
        print('Claude Code said: ' + result[:200])
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
    try:
        profile = prepare_profile(args.profile)
        claude = find_claude(Path(pwd.getpwuid(os.getuid()).pw_dir))
        run = Path(tempfile.mkdtemp(prefix='voice-sandbox-')).resolve()
        run.chmod(0o700)
        proxy = EgressProxy()
        proxy.start()
        return perform(args.action, claude, profile, run, proxy.port)
    except ValueError as error:
        print(str(error))
        return 1
    except subprocess.TimeoutExpired:
        print('Claude Code took too long inside the sandbox.')
        return 1
    except (OSError, KeyboardInterrupt):
        print('The sandbox check could not finish.')
        return 1
    finally:
        if proxy is not None:
            proxy.stop()
        if run is not None:
            shutil.rmtree(run)
        print_counts(proxy)


if __name__ == '__main__':
    raise SystemExit(main())
