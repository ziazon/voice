"""Offline checks of the sandbox probe, using invented replies and local sockets."""
import contextlib
import io
import json
import os
import socket
import stat
import subprocess
import sys
import tempfile
import threading
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'scripts'))
import claude_sandbox_probe as probe


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


class ProbeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.profile = self.root / 'profile'
        self.run = self.root / 'run'
        self.run.mkdir()

    def test_environment_is_built_from_nothing(self):
        poison = {key: 'poison' for key in ['ANTHROPIC_API_KEY', 'CLAUDE_CODE_OAUTH_TOKEN',
                  'ANTHROPIC_BASE_URL', 'CLAUDE_CODE_USE_BEDROCK', 'CLAUDECODE']}
        poison['HTTPS_PROXY'] = 'http://elsewhere'
        with patch.dict(os.environ, dict(poison, LANG='C'), clear=True):
            env = probe.child_environment(self.profile, self.run, 1234, False)
            self.assertEqual(set(env), {'HOME', 'USER', 'LOGNAME', 'PATH', 'LANG',
                             'CLAUDE_CONFIG_DIR', 'CLAUDE_CODE_TMPDIR', 'TMPDIR', 'HTTPS_PROXY',
                             'https_proxy', 'CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC'})
            self.assertFalse(set(poison.values()) & set(env.values()))
            self.assertEqual(env['PATH'], '/usr/bin:/bin:/usr/sbin:/sbin')
            self.assertEqual(env['TMPDIR'], str(self.run) + '/')
            self.assertEqual(env['CLAUDE_CODE_TMPDIR'], str(self.run))
            self.assertEqual(env['HTTPS_PROXY'], 'http://127.0.0.1:1234')
            self.assertEqual(env['https_proxy'], env['HTTPS_PROXY'])
            self.assertEqual(env['LANG'], 'C')
            signed = probe.child_environment(self.profile, self.run, 1234, True)
            self.assertEqual(signed, dict(env, BROWSER='/usr/bin/false'))
        with patch.dict(os.environ, {}, clear=True):
            self.assertEqual(probe.child_environment(self.profile, self.run, 1, False)['LANG'], 'en_US.UTF-8')

    def test_exact_commands(self):
        claude = self.root / 'program' / 'claude'
        prefix = ['/usr/bin/sandbox-exec', '-f', str(probe.PROFILE),
                  '-D', 'PROGRAM_DIR=' + str(claude.parent),
                  '-D', 'PROFILE_DIR=' + str(self.profile), '-D', 'RUN_DIR=' + str(self.run),
                  '-D', 'PROXY_PORT=4321', '-D']
        suffixes = {'version': ['--version'], 'status': ['auth', 'status', '--json'],
                    'sign-in': ['auth', 'login', '--claudeai'], 'sign-out': ['auth', 'logout'],
                    'ask': ['-p', '--model', 'haiku', '--safe-mode', '--no-session-persistence',
                            '--tools', '', '--output-format', 'json', '--setting-sources',
                            'project,local', '--strict-mcp-config', '--mcp-config',
                            '{"mcpServers":{}}', '--permission-mode', 'dontAsk',
                            '--max-budget-usd', '0.05']}
        for action, suffix in suffixes.items():
            self.assertEqual(probe.command(claude, self.profile, self.run, 4321, action),
                             prefix + ['SIGN_IN=' + ('yes' if action == 'sign-in' else 'no'),
                                       str(claude)] + suffix)

    def fake(self, folder):
        folder.mkdir(parents=True, exist_ok=True)
        program = folder / 'claude'
        program.write_text('#!/bin/sh\nexit 0\n')
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
        self.assertIn("copy the link Claude Code prints below.", output)
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
        fake.write_text('#!/bin/sh\nprintf "Sign in at https://example.com/login\\n"\n')
        output = io.StringIO()
        real_popen = subprocess.Popen
        with patch.object(probe, 'command', return_value=[str(fake)]), \
                patch.object(probe.subprocess, 'Popen', wraps=real_popen) as launch, \
                contextlib.redirect_stdout(output):
            ran = probe.run_child(fake, self.profile, self.run, 1, 'sign-in')
        self.assertEqual(ran.returncode, 0)
        self.assertIsNone(launch.call_args.kwargs['stdin'])
        self.assertEqual(launch.call_args.kwargs['env']['BROWSER'], '/usr/bin/false')
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
        with patch.object(probe, 'command', return_value=[str(fake)]) as command:
            ran = probe.run_child(fake, self.profile, self.run, 1234, 'ask')
        self.assertEqual(ran.returncode, 0)
        command.assert_called_once_with(fake, self.profile, self.run, 1234, 'ask')


if __name__ == '__main__':
    unittest.main()
