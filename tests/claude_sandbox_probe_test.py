"""Offline checks of the sandbox probe, using invented replies and local sockets."""
import contextlib
import io
import json
import os
import re
import socket
import stat
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'scripts'))
import claude_sandbox_probe as probe


SIGN_IN_LINK = 'https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e&response_type=code&redirect_uri=http%3A%2F%2Flocalhost%3A59244%2Fcallback&scope=user%3Aprofile&code_challenge=x&code_challenge_method=S256&state=y'


class ProxyTests(unittest.TestCase):
    def setUp(self):
        self.calls = []
        self.peers = []

        def connect(address, timeout):
            self.calls.append((address, timeout))
            upstream, peer = socket.socketpair()
            self.peers.append(peer)
            return upstream

        self.proxy = probe.EgressProxy(connect=connect)
        self.addCleanup(self.proxy.stop)
        self.addCleanup(lambda: [peer.close() for peer in self.peers])

    def request(self, line, headers=b'\r\n'):
        child, accepted = socket.socketpair()
        child.settimeout(2)
        def serve():
            with accepted:
                self.proxy.handle_connection(accepted)
        worker = threading.Thread(target=serve, daemon=True)
        worker.start()
        self.addCleanup(lambda: worker.join(timeout=2))
        self.addCleanup(child.close)
        child.sendall(line + b'\r\n' + headers)
        reply = b''
        while b'\r\n\r\n' not in reply:
            reply += child.recv(4096)
        return child, reply

    def test_allowed_hosts_and_bidirectional_relay(self):
        for host in ['api.anthropic.com', 'PLATFORM.CLAUDE.COM.', 'claude.ai']:
            with self.subTest(host=host):
                child, reply = self.request(('CONNECT ' + host + ':443 HTTP/1.1').encode())
                self.assertIn(b'200 Connection established', reply)
                peer = self.peers[-1]
                peer.settimeout(2)
                child.sendall(b'from child')
                self.assertEqual(peer.recv(100), b'from child')
                peer.sendall(b'from upstream')
                self.assertEqual(child.recv(100), b'from upstream')
                child.close()
        self.assertEqual(self.calls, [((host, 443), 10) for host in
                                     ['api.anthropic.com', 'platform.claude.com', 'claude.ai']])
        self.assertEqual(self.proxy.counts(), {host: (1, 0) for host in
                                             ['api.anthropic.com', 'platform.claude.com', 'claude.ai']})

    def test_refusals_never_dial(self):
        cases = [
            (b'CONNECT elsewhere.example:443 HTTP/1.1', 403, 'elsewhere.example'),
            (b'CONNECT 1.1.1.1:443 HTTP/1.1', 403, '1.1.1.1'),
            (b'CONNECT [::1]:443 HTTP/1.1', 403, '::1'),
            (b'CONNECT ::1:443 HTTP/1.1', 403, '::1'),
            (b'CONNECT api.anthropic.com:80 HTTP/1.1', 403, 'api.anthropic.com'),
            (b'CONNECT api.anthropic.com HTTP/1.1', 403, 'api.anthropic.com'),
            (b'CONNECT api.anthropic.com:443 HTTP/1.1 extra', 403, 'api.anthropic.com'),
            (b'CONNECT user@api.anthropic.com:443 HTTP/1.1', 403, '(invalid)'),
            (b'GET api.anthropic.com:443 HTTP/1.1', 405, 'api.anthropic.com'),
            (b'CONNECT api.anthropic.com:443 HTTP/2', 403, 'api.anthropic.com'),
        ]
        expected = {}
        for line, code, host in cases:
            with self.subTest(line=line):
                _, reply = self.request(line)
                self.assertIn(str(code).encode(), reply.split(b'\r\n')[0])
                expected[host] = (0, expected.get(host, (0, 0))[1] + 1)
        self.assertEqual(self.calls, [])
        self.assertEqual(self.proxy.counts(), expected)

    def test_oversized_headers(self):
        _, reply = self.request(b'CONNECT api.anthropic.com:443 HTTP/1.1', b'X: ' + b'x' * 8192 + b'\r\n\r\n')
        self.assertIn(b'403', reply)
        self.assertEqual(self.calls, [])
        self.assertEqual(self.proxy.counts(), {'api.anthropic.com': (0, 1)})

    def test_incomplete_headers_have_a_whole_block_deadline(self):
        with patch.object(probe, 'HEADER_TIMEOUT', 0.1):
            _, reply = self.request(b'CONNECT claude.ai:443 HTTP/1.1', b'X: unfinished')
        self.assertIn(b'403', reply)
        self.assertEqual(self.calls, [])

    def test_bytes_after_headers_reach_upstream(self):
        child, reply = self.request(b'CONNECT claude.ai:443 HTTP/1.1', b'\r\nfirst bytes')
        self.assertIn(b'200', reply)
        self.peers[-1].settimeout(2)
        self.assertEqual(self.peers[-1].recv(100), b'first bytes')
        child.close()

    def test_counts_bound_distinct_names(self):
        for index in range(40):
            self.request(('CONNECT host%d.example:443 HTTP/1.1' % index).encode())
        counts = self.proxy.counts()
        self.assertEqual(len(counts), 33)
        self.assertEqual(counts['(other)'], (0, 8))
        self.assertEqual(sum(n for _, n in counts.values()), 40)

    def test_idle_tunnel_closes_but_traffic_resets_deadline(self):
        with patch.object(probe, 'TUNNEL_IDLE_TIMEOUT', 1.0, create=True):
            child, reply = self.request(b'CONNECT claude.ai:443 HTTP/1.1')
            self.assertIn(b'200', reply)
            peer = self.peers[-1]
            peer.settimeout(2)
            for _ in range(6):
                time.sleep(0.25)
                peer.sendall(b'x')
                self.assertEqual(child.recv(1), b'x')
            self.assertEqual(child.recv(1), b'')

    def test_server_closes_excess_connections_and_reuses_slots(self):
        # Drive the real server dispatch with socketpairs: no TCP bind is needed.
        server = probe.ProxyServer(self.proxy, bind_and_activate=False)
        self.addCleanup(server.server_close)
        self.addCleanup(self.proxy.stop)
        clients = []
        self.addCleanup(lambda: [client.close() for client in clients])
        def dispatch():
            client, accepted = socket.socketpair()
            client.settimeout(2)
            clients.append(client)
            server.process_request(accepted, ('127.0.0.1', 1))
            return client
        for _ in range(16):
            client = dispatch()
            client.sendall(b'CONNECT claude.ai:443 HTTP/1.1\r\n\r\n')
            self.assertIn(b'200', client.recv(4096))
        extra = dispatch()
        self.assertEqual(extra.recv(1), b'')
        self.assertEqual(sum(n for _, n in self.proxy.counts().values()), 1)
        clients[0].close()
        first_worker = server._threads[0]
        first_worker.join(timeout=2)
        self.assertFalse(first_worker.is_alive())
        client = dispatch()
        client.sendall(b'CONNECT claude.ai:443 HTTP/1.1\r\n\r\n')
        self.assertIn(b'200', client.recv(4096))
        self.proxy.stop()


class ProbeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.profile = self.root / 'profile'
        self.run = self.root / 'run'
        self.run.mkdir()

    def test_profile_refuses_home_and_ancestors_without_changing_them(self):
        home = self.root / 'home'
        home.mkdir(mode=0o755)
        user = MagicMock(pw_dir=str(home))
        with patch.object(probe.pwd, 'getpwuid', return_value=user):
            for path in [home, self.root, Path('/')]:
                with self.subTest(path=path):
                    mode = path.stat().st_mode
                    with self.assertRaisesRegex(ValueError, 'Use a throwaway folder'):
                        probe.prepare_profile(str(path))
                    self.assertEqual(path.stat().st_mode, mode)
            alias = self.root / 'alias'
            alias.symlink_to(home, target_is_directory=True)
            with self.assertRaises(ValueError):
                probe.prepare_profile(str(alias / '..'))

    def test_profile_marks_empty_and_new_folders_and_refuses_unmarked_data(self):
        for name, exists in [('new', False), ('empty', True)]:
            folder = self.root / name
            if exists:
                folder.mkdir()
            self.assertEqual(probe.prepare_profile(str(folder)), folder.resolve())
            self.assertTrue((folder / '.voice-sandbox-probe').is_file())
            (folder / 'data').write_text('canary')
            self.assertEqual(probe.prepare_profile(str(folder)), folder.resolve())
        self.profile.mkdir(mode=0o755)
        (self.profile / 'data').write_text('canary')
        mode = self.profile.stat().st_mode
        with self.assertRaisesRegex(ValueError, "That folder isn't empty and wasn't made by this probe"):
            probe.prepare_profile(str(self.profile))
        self.assertEqual(self.profile.stat().st_mode, mode)
        (self.profile / '.voice-sandbox-probe').symlink_to(self.profile / 'data')
        with self.assertRaises(ValueError):
            probe.prepare_profile(str(self.profile))

    def test_controls_removed_from_failure_and_error_reply(self):
        controls = ''.join(chr(n) for n in range(32) if n != 9) + chr(127) + ''.join(chr(n) for n in range(128, 160))
        result, output = self.capture('status', [subprocess.CompletedProcess([], 1, '', 'bad' + controls.replace('\n', '') + '\tend')])
        self.assertEqual(result, 1)
        self.assertIn('bad\tend', output)
        for char in controls:
            if char != '\n':
                self.assertNotIn(char, output)
        _, output = self.capture('ask', [self.reply({'is_error': True, 'result': 'bad' + controls + '\tend'})])
        self.assertIn('Claude Code said: bad\tend', output)

    def test_normal_child_exit_also_kills_group_then_waits(self):
        child = MagicMock(pid=123456, returncode=0)
        child.communicate.return_value = ('ok', '')
        popen = MagicMock()
        popen.__enter__.return_value = child
        events = []
        with patch.object(probe.subprocess, 'Popen', return_value=popen), \
                patch.object(probe.os, 'killpg', side_effect=lambda *args: events.append('kill')):
            child.wait.side_effect = lambda: events.append('wait')
            ran = probe.run_child(Path('/program/claude'), self.profile, self.run, 1, 'ask')
        self.assertEqual(ran.stdout, 'ok')
        self.assertEqual(events, ['kill', 'wait'])

    def test_descendants_cannot_outlive_a_normally_finished_child(self):
        for action in ['ask', 'sign-in']:
            with self.subTest(action=action):
                marker = self.root / ('late-' + action)
                descendant = ('import pathlib, time; time.sleep(0.3); '
                              'pathlib.Path(%r).write_text("canary")' % str(marker))
                parent = ('import subprocess, sys; subprocess.Popen([sys.executable, "-c", %r], '
                          'stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)' % descendant)
                with patch.object(probe, 'command', return_value=[sys.executable, '-c', parent]):
                    ran = probe.run_child(Path('/program/claude'), self.profile, self.run, 1, action)
                self.assertEqual(ran.returncode, 0)
                time.sleep(0.5)
                self.assertFalse(marker.exists())

    def test_removal_failure_follows_counts_and_sets_exit_one(self):
        output = io.StringIO()
        with patch.object(probe, 'find_claude', return_value=Path('/program/claude')), \
                patch.object(probe, 'perform', return_value=0), \
                patch.object(probe, 'EgressProxy') as proxy_class, \
                patch.object(probe.shutil, 'rmtree', side_effect=OSError('invented failure')), \
                patch.object(probe.tempfile, 'mkdtemp', return_value=str(self.run)), \
                patch.object(probe.pwd, 'getpwuid', return_value=MagicMock(pw_dir=str(self.root))), \
                contextlib.redirect_stdout(output):
            proxy_class.return_value.counts.return_value = {}
            result = probe.main(['--profile', str(self.profile), 'status'])
        self.assertEqual(result, 1)
        lines = output.getvalue().splitlines()
        self.assertEqual(lines[0], 'Network: nothing.')
        self.assertEqual(lines[1], 'The run folder could not be removed: ~/run.')

    def test_environment_is_built_from_nothing(self):
        poison = {key: 'poison' for key in ['ANTHROPIC_API_KEY', 'CLAUDE_CODE_OAUTH_TOKEN',
                  'ANTHROPIC_BASE_URL', 'CLAUDE_CODE_USE_BEDROCK', 'CLAUDECODE']}
        poison['HTTPS_PROXY'] = 'http://elsewhere'
        with patch.dict(os.environ, dict(poison, LANG='C'), clear=True):
            env = probe.child_environment(self.profile, self.run, 1234, False)
            self.assertEqual(set(env), {'HOME', 'USER', 'LOGNAME', 'PATH', 'LANG',
                             'CLAUDE_CONFIG_DIR', 'CLAUDE_CODE_TMPDIR', 'TMPDIR', 'HTTPS_PROXY',
                             'https_proxy', 'CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC',
                             'DISABLE_AUTOUPDATER'})
            self.assertFalse(set(poison.values()) & set(env.values()))
            self.assertEqual(env['PATH'], '/usr/bin:/bin:/usr/sbin:/sbin')
            self.assertEqual(env['TMPDIR'], str(self.run) + '/')
            self.assertEqual(env['CLAUDE_CODE_TMPDIR'], str(self.run))
            self.assertEqual(env['HTTPS_PROXY'], 'http://127.0.0.1:1234')
            self.assertEqual(env['https_proxy'], env['HTTPS_PROXY'])
            self.assertEqual(env['LANG'], 'C')
            self.assertEqual(env['DISABLE_AUTOUPDATER'], '1')
            signed = probe.child_environment(self.profile, self.run, 1234, True)
            self.assertEqual(signed, dict(env, BROWSER=str(self.run / 'open-sign-in')))
        with patch.dict(os.environ, {}, clear=True):
            self.assertEqual(probe.child_environment(self.profile, self.run, 1, False)['LANG'], 'en_US.UTF-8')

    def test_language_matches_the_launch_rules(self):
        for value in ['C', 'en_US.UTF-8', 'en_US.UTF-8@variant', 'a' * 64]:
            with self.subTest(value=value), patch.dict(os.environ, {'LANG': value}, clear=True):
                env = probe.child_environment(self.profile, self.run, 1, False)
                self.assertEqual(env['LANG'], value)
        for value in ['', 'a' * 65, 'en US', 'C\n', 'é', 'C/', 'C+', 'C\t']:
            with self.subTest(value=value), patch.dict(os.environ, {'LANG': value}, clear=True):
                env = probe.child_environment(self.profile, self.run, 1, False)
                self.assertEqual(env['LANG'], 'en_US.UTF-8')
        with patch.dict(os.environ, {}, clear=True):
            env = probe.child_environment(self.profile, self.run, 1, False)
            self.assertEqual(env['LANG'], 'en_US.UTF-8')

    def test_exact_commands(self):
        claude = self.root / 'program' / 'claude'
        prefix = ['/usr/bin/sandbox-exec', '-f', str(probe.PROFILE),
                  '-D', 'PROGRAM=' + str(claude.resolve()),
                  '-D', 'PROFILE_DIR=' + str(self.profile), '-D', 'RUN_DIR=' + str(self.run),
                  '-D', 'PROXY_PORT=4321', '-D']
        suffixes = {'version': ['--version'], 'status': ['auth', 'status', '--json'],
                    'sign-in': ['auth', 'login', '--console'], 'sign-out': ['auth', 'logout'],
                    'ask': ['-p', '--model', 'haiku', '--safe-mode', '--no-session-persistence',
                            '--tools', '', '--output-format', 'json', '--strict-mcp-config', '--mcp-config',
                            '{"mcpServers":{}}', '--permission-mode', 'dontAsk',
                            '--max-budget-usd', '0.05']}
        for action, suffix in suffixes.items():
            self.assertEqual(probe.command(claude, self.profile, self.run, 4321, action),
                             prefix + ['SIGN_IN=' + ('yes' if action == 'sign-in' else 'no'),
                                       str(claude.resolve()), '--setting-sources', 'project,local'] + suffix)

    def fake(self, folder):
        folder.mkdir(parents=True, exist_ok=True)
        program = folder / 'claude'
        program.write_bytes(b'\xcf\xfa\xed\xfe')
        program.chmod(0o700)
        return program

    def test_finding_claude(self):
        first = self.fake(self.root / 'first')
        second = self.fake(self.root / 'second')
        self.fake(self.root / 'relative')
        with patch.dict(os.environ, {'PATH': 'relative:' + str(first.parent) + ':' + str(second.parent)}):
            self.assertEqual(probe.find_claude(self.root), first)
        first.chmod(0o600)
        with patch.dict(os.environ, {'PATH': str(first.parent) + ':' + str(second.parent)}):
            self.assertEqual(probe.find_claude(self.root), second)
        link = self.root / 'links'
        link.mkdir()
        (link / 'claude').symlink_to(second)
        with patch.dict(os.environ, {'PATH': str(link)}):
            self.assertEqual(probe.find_claude(self.root), second)
        fallback = self.fake(self.root / '.local/bin')
        with patch.dict(os.environ, {'PATH': ''}):
            self.assertEqual(probe.find_claude(self.root), fallback)
        self.assertEqual(probe.fallbacks(self.root), [fallback, Path('/opt/homebrew/bin/claude'),
                                                    Path('/usr/local/bin/claude')])
        with patch.dict(os.environ, {'PATH': 'relative'}), patch.object(probe, 'fallbacks', return_value=[]):
            with self.assertRaisesRegex(ValueError, "Claude Code isn't installed"):
                probe.find_claude(self.root)

    def test_valid_link_matches_the_launch_rules(self):
        check = getattr(probe, 'valid_link', None)
        self.assertTrue(callable(check), 'the probe needs one link check')
        prefix = 'https://claude.com/cai/oauth/authorize?'
        for link in [SIGN_IN_LINK, SIGN_IN_LINK.replace(prefix, 'https://claude.ai/oauth/authorize?'),
                     SIGN_IN_LINK + 'x' * (4096 - len(SIGN_IN_LINK))]:
            self.assertTrue(check(link), link)
        for link in ['', 'http://claude.com/cai/oauth/authorize?a',
                     'https://claude.com.example/cai/oauth/authorize?a',
                     'https://claude.com/cai/oauth/authorize@x?a',
                     'https://claude.com/cai/oauth/authorizex?a',
                     SIGN_IN_LINK + 'x' * (4097 - len(SIGN_IN_LINK))]:
            link = link.replace('?a', '?' + SIGN_IN_LINK.split('?', 1)[1])
            self.assertFalse(check(link), link)
        for char in ' \t\n"<>\\^`{|}é':
            self.assertFalse(check(SIGN_IN_LINK + char), repr(char))

    def test_link_query_keys_are_nonempty_and_plain(self):
        self.assertTrue(probe.valid_link(SIGN_IN_LINK))
        for extra in ['client%5Fid=other',
                      'redirect%5Furi=https%3A%2F%2Fexample.com%2Fcallback',
                      '=x', 'client_id+=other']:
            with self.subTest(extra=extra):
                self.assertFalse(probe.valid_link(SIGN_IN_LINK + '&' + extra))

    def test_link_requires_the_claude_code_client_and_local_callback(self):
        for port in ['1', '65535']:
            self.assertTrue(probe.valid_link(SIGN_IN_LINK.replace('59244', port)))
        self.assertTrue(probe.valid_link(SIGN_IN_LINK.replace('code=true', 'code=a+b=c')))
        self.assertTrue(probe.valid_link(SIGN_IN_LINK.replace('response_type=code', 'response_type=%63ode')))
        invalid = [
            SIGN_IN_LINK.replace('9d1c250a-e61b-44d9-88ed-5944d1962f5e', 'other'),
            SIGN_IN_LINK + '&client_id=other',
            SIGN_IN_LINK + '&response_type=code',
            SIGN_IN_LINK + '&redirect_uri=http%3A%2F%2Flocalhost%3A1%2Fcallback',
            SIGN_IN_LINK + '&state=z',
            SIGN_IN_LINK.replace('&redirect_uri=http%3A%2F%2Flocalhost%3A59244%2Fcallback', ''),
            SIGN_IN_LINK.replace('&client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e', ''),
            SIGN_IN_LINK.replace('&response_type=code', ''),
            SIGN_IN_LINK.replace('response_type=code', 'response_type=token'),
            SIGN_IN_LINK + '&broken',
            SIGN_IN_LINK + '#fragment',
        ]
        for callback in ['https://example.com/callback', 'http://localhost.example.com:1/callback',
                         'http://localhost:0/callback', 'http://localhost:65536/callback',
                         'http://localhost:1/callback/x', 'http://127.0.0.1:1/callback',
                         'http://localhost:01/callback', 'http://localhost:000001/callback',
                         'http://localhost:+1/callback', 'http://localhost:/callback']:
            invalid.append(SIGN_IN_LINK.replace('http%3A%2F%2Flocalhost%3A59244%2Fcallback', callback))
        for escape in ['%', '%2', '%GG']:
            invalid.append(SIGN_IN_LINK + '&extra=' + escape)
        with patch.object(probe.subprocess, 'run') as opened:
            for link in invalid:
                with self.subTest(link=link):
                    self.assertFalse(probe.valid_link(link))
                    (self.run / 'sign-in-link').write_text(link + '\n')
                    self.assertFalse(probe.open_sign_in_link(self.run))
            opened.assert_not_called()

    def test_link_handoff_refuses_links_and_non_regular_files(self):
        check = getattr(probe, 'open_sign_in_link', None)
        self.assertTrue(callable(check))
        path = self.run / 'sign-in-link'
        with patch.object(probe.subprocess, 'run') as opened:
            self.assertIsNone(check(self.run))
            target = self.root / 'elsewhere'
            target.write_text(SIGN_IN_LINK + '\n')
            path.symlink_to(target)
            self.assertFalse(check(self.run))
            path.unlink()
            path.mkdir()
            self.assertFalse(check(self.run))
            path.rmdir()
            os.mkfifo(path)
            self.assertFalse(check(self.run))
            opened.assert_not_called()

    def test_link_handoff_waits_for_the_helper_to_finish_writing(self):
        path = self.run / 'sign-in-link'
        link = SIGN_IN_LINK
        with patch.object(probe.subprocess, 'run') as opened:
            for data in [b'', link.encode(), b'x' * probe.MAX_LINK]:
                with self.subTest(size=len(data)):
                    path.write_bytes(data)
                    self.assertIsNone(probe.open_sign_in_link(self.run))
                    self.assertEqual(path.read_bytes(), data)
                    opened.assert_not_called()
            path.write_bytes(link.encode())
            self.assertIsNone(probe.open_sign_in_link(self.run))
            with path.open('ab') as helper:
                helper.write(b'\n')
            self.assertTrue(probe.open_sign_in_link(self.run))
            opened.assert_called_once_with(['/usr/bin/open', link], check=False)
            self.assertFalse(path.exists())

    def test_helper_matches_rust_and_is_private(self):
        helper = getattr(probe, 'HELPER_SCRIPT', None)
        self.assertIsInstance(helper, str)
        rust = (probe.PROFILE.parent.parent / 'src/sandbox.rs').read_text()
        constant = re.search(r'(?:pub )?const HELPER_SCRIPT: &str =\s*("(?:\\.|[^"\\])*");', rust)
        self.assertIsNotNone(constant)
        self.assertEqual(helper, json.loads(constant.group(1)))
        fake = self.fake(self.root / 'helper')
        fake.write_text('#!/bin/sh\nexit 0\n')
        with patch.object(probe, 'command', return_value=[str(fake)]):
            probe.run_child(fake, self.profile, self.run, 1, 'sign-in')
        script = self.run / 'open-sign-in'
        self.assertEqual(script.read_text(), helper)
        self.assertEqual(stat.S_IMODE(script.stat().st_mode), 0o700)

    def test_sign_in_opens_only_a_valid_helper_link(self):
        link = SIGN_IN_LINK
        for value, accepted in [(link + '\n', True), ('https://example.com/\n', False),
                                ('x' * (probe.MAX_LINK + 1), False),
                                (link + 'x' * 4096 + '\n', False),
                                (b'\xff\n', False), (link + '\n\n', False)]:
            with self.subTest(value=repr(value)[:60]):
                (self.run / 'open-sign-in').unlink(missing_ok=True)
                child = MagicMock(pid=123456, returncode=0, stdout=io.StringIO('Claude output\n'))
                child.poll.side_effect = [None, 0]
                data = value.encode() if isinstance(value, str) else value
                def deliver_link(timeout=None):
                    if timeout is not None:
                        (self.run / 'sign-in-link').write_bytes(data)
                child.wait.side_effect = deliver_link
                popen = MagicMock()
                popen.__enter__.return_value = child
                output = io.StringIO()
                with patch.object(probe.subprocess, 'Popen', return_value=popen), \
                        patch.object(probe.os, 'killpg') as kill, \
                        patch.object(probe.subprocess, 'run') as opened, contextlib.redirect_stdout(output):
                    ran = probe.run_child(Path('/program/claude'), self.profile, self.run, 1, 'sign-in')
                kill.assert_called_once_with(child.pid, probe.signal.SIGKILL)
                self.assertEqual(child.wait.call_args_list[0].kwargs, {'timeout': 0.2})
                if accepted:
                    opened.assert_called_once_with(['/usr/bin/open', link], check=False)
                    self.assertEqual(ran.returncode, 0)
                    self.assertIn("Opened Claude's page in your browser.", output.getvalue())
                else:
                    opened.assert_not_called()
                    self.assertNotEqual(ran.returncode, 0)
                    self.assertIn("Claude Code handed over a link Voice won't open.", output.getvalue())
                self.assertFalse((self.run / 'sign-in-link').exists())
                self.assertIn('Claude output', output.getvalue())

    def test_finding_claude_refuses_non_native_programs(self):
        program = self.fake(self.root / 'native')
        for header in [b'#!/bin/sh\n', b'xx', b'\x7fELF', b'\xfe\xed\xfa\xcf']:
            with self.subTest(header=header), patch.dict(os.environ, {'PATH': str(program.parent)}):
                program.write_bytes(header)
                with self.assertRaisesRegex(ValueError, "This Claude Code is an older kind") as error:
                    probe.find_claude(self.root)
                self.assertEqual(str(error.exception), "This Claude Code is an older kind that runs as a script, which Voice can't run inside its sandbox. Install the current Claude Code (https://code.claude.com/docs/en/setup), then try again.")
        for header in [b'\xcf\xfa\xed\xfe', b'\xca\xfe\xba\xbe', b'\xca\xfe\xba\xbf']:
            with self.subTest(header=header), patch.dict(os.environ, {'PATH': str(program.parent)}):
                program.write_bytes(header)
                self.assertEqual(probe.find_claude(self.root), program.resolve())

    def test_profile_folder(self):
        self.assertEqual(probe.prepare_profile(str(self.profile)), self.profile)
        self.assertEqual(stat.S_IMODE(self.profile.stat().st_mode), 0o700)
        self.profile.chmod(0o777)
        probe.prepare_profile(str(self.profile))
        self.assertEqual(stat.S_IMODE(self.profile.stat().st_mode), 0o700)
        link = self.root / 'link'
        link.symlink_to(self.profile)
        file = self.root / 'file'
        file.write_text('canary')
        for path in [link, file]:
            with self.assertRaisesRegex(ValueError, 'That profile folder is a link or not a folder.'):
                probe.prepare_profile(str(path))
        with self.assertRaisesRegex(ValueError, 'absolute'):
            probe.prepare_profile('relative')

    def capture(self, action, replies):
        output = io.StringIO()
        with contextlib.redirect_stdout(output), patch.object(probe, 'run_child', side_effect=replies):
            result = probe.perform(action, Path('/program/claude'), self.profile, self.run, 1)
        return result, output.getvalue()

    def reply(self, value, code=0, stderr=''):
        return subprocess.CompletedProcess([], code, json.dumps(value), stderr)

    def test_status_prints_only_version_and_logged_in(self):
        result, output = self.capture('status', [subprocess.CompletedProcess([], 0, '2.0 (Claude Code)\n', ''),
                                               self.reply({'loggedIn': True, 'email': 'sam@example.com'})])
        self.assertEqual(result, 0)
        self.assertEqual(output, 'Claude Code 2.0 runs inside the sandbox.\nSigned in inside the sandbox: yes.\n')
        self.assertNotIn('sam@example.com', output)

    def test_status_accepts_signed_out_json_with_exit_one(self):
        fake = self.fake(self.root / 'status')
        fake.write_text('#!/bin/sh\nif [ "$1" = version ]; then\n'
                        '  printf "2.1.292 (Claude Code)\\n"\nelse\n'
                        '  printf \'{"loggedIn": false}\\n\'\n  exit 1\nfi\n')
        output = io.StringIO()
        with patch.object(probe, 'command', side_effect=lambda *args: [str(fake), args[-1]]), \
                contextlib.redirect_stdout(output):
            result = probe.perform('status', fake, self.profile, self.run, 1)
        self.assertEqual(result, 0)
        self.assertEqual(output.getvalue(), 'Claude Code 2.1.292 runs inside the sandbox.\n'
                         'Signed in inside the sandbox: no.\n')

    def test_status_validates_version(self):
        for version in ['2.1', '2.1.292', '2.1.292.0']:
            with self.subTest(version=version):
                result, output = self.capture('status', [
                    subprocess.CompletedProcess([], 0, version + ' (Claude Code)\n', ''),
                    self.reply({'loggedIn': True})])
                self.assertEqual(result, 0)
                self.assertIn('Claude Code ' + version + ' runs inside the sandbox.', output)
        for version in ['', '2', '2.1.2.3.4', '2..1', '2.1.', 'v2.1', '2.1-beta', '２.１']:
            with self.subTest(version=version):
                result, output = self.capture('status', [
                    subprocess.CompletedProcess([], 0, version, ''),
                    self.reply({'loggedIn': True})])
                self.assertEqual(result, 1)
                self.assertIn("Claude Code doesn't run inside the sandbox (exit 0).", output)

    def test_ask_summaries(self):
        for value, code, expected, result in [
            ({'result': ' OK\n', 'is_error': False, 'total_cost_usd': 0.01234}, 0,
             'Answered: yes.\nIt said ok: yes.\nCost: $0.0123.\n', 0),
            ({'result': 'private reply', 'is_error': False, 'total_cost_usd': 0}, 0,
             'Answered: yes.\nIt said ok: no.\nCost: $0.0000.\n', 1),
            ({'result': 'Not logged in · Please run /login', 'is_error': True}, 1,
             'Answered: no.\nIt said ok: no.\nCost: $0.0000.\nClaude Code said: Not logged in · Please run /login\n', 1),
        ]:
            with self.subTest(value=value):
                self.assertEqual(self.capture('ask', [self.reply(value, code)]), (result, expected))
        result, output = self.capture('ask', [subprocess.CompletedProcess([], 0, 'not JSON: private reply', '')])
        self.assertEqual(result, 1)
        self.assertNotIn('private reply', output)
        self.assertIn('Answered: no.', output)

    def test_failure_masks_home_and_limits_line(self):
        import pwd
        home = pwd.getpwuid(os.getuid()).pw_dir
        result, output = self.capture('status', [subprocess.CompletedProcess([], 7, '', 'first\n' + home + '/oops ' + 'x' * 300)])
        self.assertEqual(result, 1)
        self.assertIn('(exit 7)', output)
        self.assertTrue(output.splitlines()[1].startswith('~/oops'))
        self.assertEqual(len(output.splitlines()[1]), 200)

    def test_sign_out(self):
        self.assertEqual(self.capture('sign-out', [self.reply({})]), (0, 'Signed out inside the sandbox.\n'))

    def test_run_folder_removed_after_exception_and_counts_printed(self):
        seen = []
        def fail(action, claude, profile, run, port):
            seen.append(run)
            self.assertEqual(stat.S_IMODE(run.stat().st_mode), 0o700)
            self.assertEqual(run, run.resolve())
            raise OSError('invented failure')
        output = io.StringIO()
        with patch.object(probe, 'find_claude', return_value=Path('/program/claude')), \
                patch.object(probe, 'perform', side_effect=fail), \
                patch.object(probe, 'EgressProxy') as proxy_class, contextlib.redirect_stdout(output):
            proxy_class.return_value.counts.return_value = {}
            self.assertEqual(probe.main(['--profile', str(self.profile), 'status']), 1)
        self.assertEqual(len(seen), 1)
        self.assertFalse(seen[0].exists())
        self.assertIn('Network: nothing.', output.getvalue())

    def test_sign_in_streams_only_claude_output_then_reports_login_location(self):
        self.profile.mkdir()
        (self.profile / '.credentials.json').write_text('canary')
        replies = [self.reply({}), subprocess.CompletedProcess([], 0, '2.0', ''),
                   self.reply({'loggedIn': True, 'email': 'sam@example.com'})]
        result, output = self.capture('sign-in', replies)
        self.assertEqual(result, 0)
        self.assertTrue(output.startswith("Sign in on Claude's page, which opens in your browser.\n"))
        self.assertIn("The login is kept in the profile folder's own file.", output)
        self.assertNotIn('canary', output)
        self.assertNotIn('sam@example.com', output)

    def test_sign_in_requires_status_to_confirm_login(self):
        replies = [self.reply({}), subprocess.CompletedProcess([], 0, '2.0', ''),
                   self.reply({'loggedIn': False})]
        result, output = self.capture('sign-in', replies)
        self.assertIn('Signed in inside the sandbox: no.', output)
        self.assertEqual(result, 1)

    def test_child_launch_settings_and_timeout_kill_the_group(self):
        child = MagicMock()
        child.pid = 123456
        child.communicate.side_effect = subprocess.TimeoutExpired('fake', 120)
        popen = MagicMock()
        popen.__enter__.return_value = child
        with patch.object(probe.subprocess, 'Popen', return_value=popen) as launch, \
                patch.object(probe.os, 'killpg') as kill:
            with self.assertRaises(subprocess.TimeoutExpired):
                probe.run_child(Path('/program/claude'), self.profile, self.run, 1, 'ask')
        self.assertTrue(launch.call_args.kwargs['start_new_session'])
        self.assertEqual(launch.call_args.kwargs['cwd'], self.run)
        self.assertEqual(launch.call_args.kwargs['env'],
                         probe.child_environment(self.profile, self.run, 1, False))
        child.communicate.assert_called_once_with(input='Reply with the single word: ok', timeout=120)
        kill.assert_called_once_with(child.pid, probe.signal.SIGKILL)
        child.wait.assert_called_once_with()

    def test_sign_in_inherits_stdin_and_copies_output(self):
        fake = self.fake(self.root / 'login')
        fake.write_text('#!/bin/sh\nprintf "\\033Sign in at https://example.com/login\\177\\n"\n')
        output = io.StringIO()
        real_popen = subprocess.Popen
        with patch.object(probe, 'command', return_value=[str(fake)]), \
                patch.object(probe.subprocess, 'Popen', wraps=real_popen) as launch, \
                contextlib.redirect_stdout(output):
            ran = probe.run_child(fake, self.profile, self.run, 1, 'sign-in')
        self.assertEqual(ran.returncode, 0)
        self.assertIsNone(launch.call_args.kwargs['stdin'])
        self.assertEqual(launch.call_args.kwargs['env']['BROWSER'], str(self.run / 'open-sign-in'))
        self.assertEqual(output.getvalue(), 'Sign in at https://example.com/login\n')

    def test_counts_are_sorted_and_contain_only_hosts(self):
        proxy = MagicMock()
        proxy.counts.return_value = {'claude.ai': (2, 1), 'api.anthropic.com': (1, 0)}
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            probe.print_counts(proxy)
        self.assertEqual(output.getvalue(), 'Network: api.anthropic.com allowed 1, refused 0.\n'
                         'Network: claude.ai allowed 2, refused 1.\n')

    def test_run_child_uses_private_cwd_environment_and_prompt(self):
        fake = self.fake(self.root / 'fake')
        fake.write_text('#!/bin/sh\nexit 0\n')
        with patch.object(probe, 'command', return_value=[str(fake)]) as command:
            ran = probe.run_child(fake, self.profile, self.run, 1234, 'ask')
        self.assertEqual(ran.returncode, 0)
        command.assert_called_once_with(fake, self.profile, self.run, 1234, 'ask')


if __name__ == '__main__':
    unittest.main()
