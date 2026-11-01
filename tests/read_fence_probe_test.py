"""Offline checks of the read fence, using invented files and a fake Claude Code."""
import contextlib
import fnmatch
import io
import json
import os
import re
import signal
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'scripts'))
import read_fence_probe as probe

REPO = Path(__file__).resolve().parent.parent


class ContractTests(unittest.TestCase):
    def test_rules_match_rust_in_order(self):
        rust = (REPO / 'builder/core/src/deny.rs').read_text()
        names = re.findall(r'"([^"]+)"', rust.split('const NAMES:')[1].split(';')[0])
        sequence_text = rust.split('const SEQUENCES:')[1].split(';')[0]
        strings = re.findall(r'"([^"]+)"', sequence_text)
        sequences = list(zip(strings[::2], strings[1::2]))
        self.assertEqual(list(probe.NAMES), names)
        self.assertEqual([tuple(s.lower() for s in pair) for pair in probe.SEQUENCES], sequences)
        self.assertEqual(probe.SEQUENCES[2], ('Library', 'Keychains'))
        expected = []
        for pattern in names + ['/'.join(pair) for pair in probe.SEQUENCES]:
            expected.extend(['Read(//**/%s)' % pattern, 'Read(//**/%s/**)' % pattern])
        self.assertEqual(len(expected), 34)
        self.assertEqual(probe.deny_rules(), expected)
        self.assertEqual(probe.relative_rules(), [r.replace('//**/', '**/') for r in expected])

    def test_exact_flags(self):
        flags = ['-p', '--model', 'haiku', '--no-session-persistence', '--output-format',
                 'stream-json', '--verbose', '--strict-mcp-config', '--mcp-config',
                 '{"mcpServers":{}}', '--tools', 'Read,Grep,Glob', '--permission-mode',
                 'manual', '--permission-prompts', 'none', '--max-budget-usd', '0.10']
        self.assertEqual(probe.FENCE_FLAGS, flags)
        for forbidden in ['Bash', 'Write', 'Edit', 'WebFetch', 'WebSearch',
                          'bypassPermissions', 'auto', '--dangerously-skip-permissions',
                          '--allow-dangerously-skip-permissions']:
            self.assertNotIn(forbidden, ' '.join(probe.FENCE_FLAGS))
        for settings in [None, Path('/fixture/settings.json')]:
            expected = ['/program/claude'] + flags + ['--add-dir', '/fixture/root']
            if settings:
                expected += ['--settings', str(settings)]
            expected += ['--debug-file', '/fixture/log']
            self.assertEqual(probe.fence_argv(Path('/program/claude'), Path('/fixture'),
                                             settings, Path('/fixture/log')), expected)

    def test_environment_is_built_from_nothing(self):
        poison = dict.fromkeys(['ANTHROPIC_API_KEY', 'ANTHROPIC_BASE_URL',
                               'CLAUDE_CODE_USE_BEDROCK', 'CLAUDECODE', 'HTTPS_PROXY'], 'poison')
        for optional in [{}, {'CLAUDE_CONFIG_DIR': '/invented/config',
                              'CLAUDE_CODE_OAUTH_TOKEN': 'invented-token'}]:
            with patch.dict(os.environ, dict(poison, **optional), clear=True):
                actual = probe.child_environment()
            import pwd
            user = pwd.getpwuid(os.getuid())
            expected = {'HOME': user.pw_dir, 'USER': user.pw_name, 'LOGNAME': user.pw_name,
                        'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'LANG': 'en_US.UTF-8',
                        'CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC': '1'}
            expected.update(optional)
            self.assertEqual(actual, expected)
        with patch.dict(os.environ, {'LANG': 'C'}):
            self.assertEqual(probe.child_environment()['LANG'], 'C')

    def test_debug_evidence_is_exact(self):
        rules = probe.deny_rules()
        prefix = "[DEBUG] Applying permission update: Adding 34 deny rule(s) to destination 'flagSettings': "
        line = prefix + json.dumps(rules)
        self.assertTrue(probe.settings_applied('earlier\n' + line + '\nlater', rules))
        for bad in ['', line.replace('34', '33', 1), prefix + json.dumps(rules[::-1]),
                    prefix + 'not JSON', line.replace('flagSettings', 'userSettings'),
                    line + ' trailing text']:
            with self.subTest(line=bad):
                self.assertFalse(probe.settings_applied(bad, rules))

    def test_fixture_and_canary_coverage(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder).resolve()
            words = probe.make_fixture(base, True)
            self.assertEqual(len(probe.CANARIES), 24)
            self.assertEqual(len(words), 24)
            paths = [path for key, path in probe.CANARIES]
            for name in probe.NAMES:
                self.assertTrue(any(any(fnmatch.fnmatch(part.lower(), name) for part in Path(p).parts)
                                    for p in paths), name)
            for sequence in probe.SEQUENCES:
                self.assertTrue(any('/'.join(sequence).lower() in p.lower() for p in paths), sequence)
            ordinary = [word for key, word in words.items() if key not in ['alias.md', 'out-link.md']]
            self.assertEqual(len(ordinary), len(set(ordinary)))
            for word in ordinary:
                self.assertRegex(word, r'^fence[0-9a-f]{12}$')
            for path in base.rglob('*'):
                if not path.is_symlink():
                    self.assertEqual(stat.S_IMODE(path.stat().st_mode),
                                     0o700 if path.is_dir() else 0o600, str(path.relative_to(base)))
            self.assertEqual(stat.S_IMODE(base.stat().st_mode), 0o700)
            self.assertEqual((base / 'root/alias.md').resolve(), base / 'root/.env')
            self.assertEqual((base / 'root/out-link.md').resolve(), base / 'outside/far.md')
            self.assertEqual(words['alias.md'], words['.env'])
            self.assertEqual(words['out-link.md'], words['outside/far.md'])
            self.assertEqual(json.loads((base / 'settings.json').read_text()),
                             {'permissions': {'deny': probe.deny_rules()}})
            self.assertTrue((base / 'elsewhere/relative.json').is_file())
            self.assertEqual(json.loads((base / 'elsewhere/relative.json').read_text()),
                             {'permissions': {'deny': probe.relative_rules()}})
            self.assertFalse((base / 'relative.json').exists())
            self.assertIsInstance(json.loads((base / 'broken.json').read_text())['permissions']['deny'], str)
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            self.assertEqual(probe.make_fixture(base, False), {})
            self.assertEqual(list((base / 'root').iterdir()), [])
        for error in [OSError('invented failure'), KeyboardInterrupt()]:
            seen = []
            def fail(action, claude, base, words):
                seen.append(base)
                self.assertEqual(base, base.resolve())
                raise error
            output = io.StringIO()
            with patch.object(probe, 'find_claude', return_value=Path('/fake')), \
                    patch.object(probe, 'perform', side_effect=fail), contextlib.redirect_stdout(output):
                self.assertEqual(probe.main(['run']), 1)
            self.assertEqual(output.getvalue(), 'The probe could not finish.\n')
            self.assertEqual(len(seen), 1)
            self.assertFalse(seen[0].exists())
        seen = []
        def completed(action, claude, base, words):
            seen.append(base)
            print('Printed before cleanup.')
            return 0
        output = io.StringIO()
        with patch.object(probe, 'find_claude', return_value=Path('/fake')), \
                patch.object(probe, 'perform', side_effect=completed), \
                patch.object(probe.shutil, 'rmtree', side_effect=OSError('invented failure')), \
                contextlib.redirect_stdout(output):
            self.assertEqual(probe.main(['check']), 1)
        self.addCleanup(probe.shutil.rmtree, seen[0])
        self.assertTrue(output.getvalue().startswith("Printed before cleanup.\nThe probe's folder could not be removed: "))

    def test_second_communicate_is_bounded_after_timeout(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            probe.make_fixture(base, False)
            child = MagicMock(returncode=-9)
            child.communicate.side_effect = [subprocess.TimeoutExpired('fake', 180),
                                             subprocess.TimeoutExpired('fake', 10)]
            launch = MagicMock()
            launch.__enter__.return_value = child
            with patch.object(probe.subprocess, 'Popen', return_value=launch), \
                    patch.object(probe, 'kill_group') as kill:
                turn = probe.run_turn(Path('/fake'), base, 'read', 'prompt', None)
            self.assertTrue(turn.timed_out)
            self.assertEqual(turn.stdout, '')
            self.assertEqual(child.communicate.call_args.kwargs, {'timeout': 10})
            kill.assert_called_once_with(child)

    def test_signal_handlers_interrupt_and_restore(self):
        for sig in [signal.SIGTERM, signal.SIGHUP]:
            with self.subTest(signal=sig):
                previous = {s: signal.getsignal(s) for s in [signal.SIGTERM, signal.SIGHUP]}
                seen = []
                def interrupted(action, claude, base, words):
                    seen.append(base)
                    handler = signal.getsignal(sig)
                    self.assertTrue(callable(handler))
                    self.assertIsNot(handler, previous[sig])
                    handler(sig, None)
                    self.fail('the signal handler did not interrupt')
                output = io.StringIO()
                with patch.object(probe, 'find_claude', return_value=Path('/fake')), \
                        patch.object(probe, 'perform', side_effect=interrupted), \
                        contextlib.redirect_stdout(output):
                    self.assertEqual(probe.main(['check']), 1)
                self.assertEqual(output.getvalue(), 'The probe could not finish.\n')
                self.assertFalse(seen[0].exists())
                for restored, handler in previous.items():
                    self.assertIs(signal.getsignal(restored), handler)

    def test_safe_text_removes_unicode_controls_and_base_alias(self):
        controls = ''.join(chr(n) for start, end in [(0x202a, 0x202e), (0x2066, 0x2069),
                                                    (0x200b, 0x200f), (0xfeff, 0xfeff)]
                           for n in range(start, end + 1))
        base = Path('/private/var/tmp/voice-fence-invented')
        for text, folder, expected in [
                ('before' + controls + 'after', None, 'beforeafter'),
                (str(base) + '/root /var/tmp/voice-fence-invented/logs', base,
                 '<folder>/root <folder>/logs')]:
            with self.subTest(expected=expected):
                self.assertEqual(probe.safe_text(text, folder), expected)

    def test_turn_transport_timeout_and_cost(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            probe.make_fixture(base, False)
            child = MagicMock()
            child.pid = 123456
            child.returncode = 7
            events = [{'type': 'system', 'subtype': 'init', 'tools': ['Read']},
                      {'type': 'result', 'total_cost_usd': 0.0123}]
            stdout = '\n'.join(json.dumps(e) for e in events) + '\nplain canary'
            child.communicate.return_value = (stdout, 'private stderr')
            launch_context = MagicMock()
            launch_context.__enter__.return_value = child
            log = base / 'logs/read-1.log'
            log.write_text('measured log')
            with patch.object(probe.subprocess, 'Popen', return_value=launch_context) as launch, \
                    patch.object(probe, 'kill_group') as kill:
                turn = probe.run_turn(Path('/fake'), base, 'read', 'prompt', base / 'settings.json')
            self.assertIsNotNone(turn)
            self.assertEqual(turn.events, events)
            self.assertEqual(turn.stdout, stdout)
            self.assertEqual(turn.exit_code, 7)
            self.assertEqual(turn.init, events[0])
            self.assertEqual(turn.result, events[1])
            self.assertEqual(turn.log, 'measured log')
            self.assertEqual(turn.cost, 0.0123)
            self.assertFalse(turn.timed_out)
            if str(base).startswith('/private/var/'):
                self.assertTrue(probe.same_path(str(base).replace('/private/var/', '/var/', 1), base))
            self.assertFalse(probe.same_path(None, base))
            child.communicate.assert_called_once_with(input='prompt', timeout=180)
            kill.assert_called_once_with(child)
            self.assertEqual(launch.call_args.kwargs['cwd'], base / 'work')
            self.assertEqual(launch.call_args.kwargs['env'], probe.child_environment())
            self.assertTrue(launch.call_args.kwargs['start_new_session'])
            self.assertEqual(launch.call_args.kwargs['stdin'], subprocess.PIPE)
            self.assertEqual(launch.call_args.kwargs['stderr'], subprocess.PIPE)
            for cost in [-1, 'wrong', float('nan'), float('inf'), None, True]:
                with self.subTest(cost=cost):
                    self.assertEqual(probe.parse_turn(json.dumps({'type': 'result',
                                                               'total_cost_usd': cost}), 0, '').cost, 0)
            child.communicate.side_effect = [subprocess.TimeoutExpired('fake', 180), ('partial', '')]
            with patch.object(probe.subprocess, 'Popen', return_value=launch_context), \
                    patch.object(probe, 'kill_group') as kill:
                turn = probe.run_turn(Path('/fake'), base, 'read', 'prompt', None)
            self.assertTrue(turn.timed_out)
            kill.assert_called_once_with(child)


FAKE = r'''#!/usr/bin/env python3
import json
import sys
from pathlib import Path

plan_path = Path(__file__).with_name('plan.json')
plan = json.loads(plan_path.read_text())
args = sys.argv[1:]
if args == ['--version']:
    print(plan.get('version', '2.1.292 (Claude Code)'))
    print(plan.get('stderr', ''), file=sys.stderr)
    sys.exit(plan.get('version_exit', 0))
if args == ['auth', 'status', '--json']:
    print(json.dumps(plan.get('auth', {'loggedIn': True, 'email': 'sam@example.com'})))
    sys.exit(plan.get('auth_exit', 0))
prompt = sys.stdin.read()
log = Path(args[args.index('--debug-file') + 1])
name = log.stem.rsplit('-', 1)[0]
attempt = int(log.stem.rsplit('-', 1)[1])
settings = Path(args[args.index('--settings') + 1]) if '--settings' in args else None
if settings and not plan.get('no_log') and name != plan.get('no_log_turn') and (settings.name != 'broken.json' or plan.get('broken_good') or plan.get('broken_one_rule')):
    rules = json.loads(settings.read_text())['permissions']['deny']
    if settings.name == 'broken.json':
        rules = json.loads(settings.with_name('settings.json').read_text())['permissions']['deny']
    if settings.name == 'broken.json' and plan.get('broken_one_rule'):
        rules = ['Read(//**/.env)']
    log.write_text("[DEBUG] Applying permission update: Adding %d deny rule(s) to destination 'flagSettings': " % len(rules) + json.dumps(rules))
def emit(event):
    print(json.dumps(event))
if not plan.get('no_init') and name != plan.get('no_init_turn'):
    emit({'type': 'system', 'subtype': 'init', 'tools': plan.get('tools', ['Glob', 'Grep', 'Read']),
          'permissionMode': plan.get('mode', 'default'), 'mcp_servers': plan.get('mcp', [])})
denials = []
root = Path(args[args.index('--add-dir') + 1])
if name in ['read', 'relative', 'baseline']:
    paths = [Path(line) for line in prompt.splitlines() if line.startswith('/')]
    trace = plan_path.with_name('trace.jsonl')
    with trace.open('a') as out:
        out.write(json.dumps({'name': name, 'attempt': attempt, 'paths': list(map(str, paths))}) + '\n')
    for path in paths:
        key = str(path.relative_to(root)) if root in path.parents else 'outside/far.md'
        if key == plan.get('never') or (key == plan.get('retry') and attempt == 1):
            continue
        used_path = str(path)
        if plan.get('realpath'):
            used_path = str(path.parent) + '/./' + path.name
        emit({'type': 'assistant', 'message': {'content': [
            {'type': 'tool_use', 'name': 'Read', 'input': {'file_path': used_path}}]}})
        if key == 'notes.md' or name in ['relative', 'baseline'] or key == plan.get('leak'):
            emit({'type': 'user', 'message': {'content': [
                {'type': 'tool_result', 'content': path.read_text()}]}})
        if key != 'notes.md' and key != plan.get('other_refusal'):
            denials.append({'tool_name': 'Read', 'tool_input': {'file_path': used_path}})
        if key == plan.get('other_refusal'):
            emit({'type': 'user', 'message': {'content': [
                {'type': 'tool_result', 'is_error': True, 'content': 'invented refusal'}]}})
if name == 'read' and attempt == 2 and plan.get('later_leak'):
    print((root / plan['later_leak']).read_text())
if name in ['grep', 'glob'] and not plan.get('skip_' + name):
    emit({'type': 'assistant', 'message': {'content': [
        {'type': 'tool_use', 'name': name.title(), 'input': {'path': str(root)}}]}})
    if name == 'grep':
        text = (root / 'notes.md').read_text()
        if plan.get('grep_leak'):
            text += (root / plan['grep_leak']).read_text()
        emit({'type': 'user', 'message': {'content': [{'type': 'tool_result', 'content': text}]}})
    else:
        print(str(root / '.env'))
        if plan.get('glob_leak'):
            print((root / plan['glob_leak']).read_text())
result = {'type': 'result', 'permission_denials': denials, 'total_cost_usd': 0.001}
if plan.get('error'):
    result.update(is_error=True, result=plan['error'].replace('<base>', str(root.parent)))
emit(result)
sys.exit(plan.get('exit', 0))
'''


@unittest.skipUnless(Path('/usr/bin/python3').exists(), '/usr/bin/python3 does not exist')
class FakeTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name).resolve()
        self.fake = self.folder / 'claude'
        self.fake.write_text(FAKE)
        self.fake.chmod(0o700)
        self.config = self.folder / 'config'
        self.config.mkdir()
        (self.config / 'settings.json').write_text('{}')

    def invoke(self, action='check', plan=None, settings=None):
        (self.folder / 'plan.json').write_text(json.dumps(plan or {}))
        if settings is not None:
            (self.config / 'settings.json').write_text(json.dumps(settings))
        output = io.StringIO()
        with patch.object(probe, 'find_claude', return_value=self.fake), \
                patch.dict(os.environ, {'CLAUDE_CONFIG_DIR': str(self.config),
                                        'CLAUDE_CODE_OAUTH_TOKEN': 'invented-private-token'}), \
                contextlib.redirect_stdout(output):
            code = probe.main([action])
        text = output.getvalue()
        self.assertNotIn('invented-private-token', text)
        self.assertNotIn('sam@example.com', text)
        import pwd
        self.assertNotIn(pwd.getpwuid(os.getuid()).pw_dir, text)
        self.assertNotRegex(text, r'fence[0-9a-f]{12}')
        self.assertNotRegex(text, r'/[^\s]*voice-fence-')
        return code, text

    def test_broken_settings_with_one_applied_rule_fails(self):
        code, text = self.invoke(plan={'broken_one_rule': True})
        self.assertIn('A broken settings file is caught: FAIL', text)
        self.assertEqual(code, 1)

    def test_check_measured_contract(self):
        code, text = self.invoke()
        self.assertEqual(code, 0)
        for line in ['Claude Code 2.1.292.', 'Signed in: yes.',
                     'This Mac\'s own settings widen reads: no.',
                     'Tools offered: Glob, Grep, Read: pass.', 'MCP servers: none: pass.',
                     'Permission mode: default: pass.',
                     'The deny rules were applied (34 rules): pass.',
                     'A broken settings file is caught: pass.']:
            self.assertIn(line, text)
        for plan, expected in [({'tools': ['Bash', 'Glob', 'Grep', 'Read']}, 'Bash'),
                               ({'mode': 'dontAsk'}, 'Permission mode: default: FAIL'),
                               ({'mcp': [{}]}, 'MCP servers: none: FAIL'),
                               ({'no_log': True}, 'the deny rules were not applied'),
                               ({'broken_good': True}, 'FAIL (the probe could not tell it apart'),
                               ({'no_init': True}, "Claude Code didn't start a turn (exit 0).")]:
            with self.subTest(plan=plan):
                code, text = self.invoke(plan=plan)
                self.assertEqual(code, 1)
                self.assertIn(expected, text)
                if 'no_init' not in plan:
                    self.assertIn('FAIL', text)
        code, text = self.invoke(plan={'no_init_turn': 'broken', 'exit': 1})
        self.assertEqual(code, 0)
        self.assertIn('A broken settings file is caught: pass.', text)
        for logged_in in [True, False, None, 'yes', 1]:
            code, text = self.invoke(plan={'auth': {'loggedIn': logged_in, 'email': 'sam@example.com'},
                                           'auth_exit': 1})
            self.assertEqual(code, 0)
            if type(logged_in) is bool:
                self.assertIn('Signed in: %s.' % ('yes' if logged_in else 'no'), text)
            else:
                self.assertIn("Claude Code's sign-in status could not be read.", text)
        for version in ['2.1', '2.1.292', '2.1.292.0']:
            code, text = self.invoke(plan={'version': version})
            self.assertEqual(code, 0)
            self.assertIn('Claude Code %s.' % version, text)
        for version in ['', '2', '2.1.2.3.4', '2..1', '2.1.', 'v2.1', '2.1-beta', '２.１']:
            code, text = self.invoke(plan={'version': version})
            self.assertEqual(code, 1)
            self.assertIn("Claude Code didn't start (exit 0).", text)

    def test_settings_report_counts_extras_without_values(self):
        cases = [
            ({'hooks': {'PreToolUse': [{'hooks': [{'command': 'invented-command'}]}, {}],
                        'Stop': [{}]}}, '3 hooks'),
            ({'env': {'FIRST': 'invented-env-value', 'SECOND': 'other-value'}},
             '2 environment settings'),
            ({'apiKeyHelper': '/invented/helper'}, 'a key helper'),
            ({'enabledPlugins': {'one': True, 'two': False, 'three': True, 'four': 1}},
             '2 plugins turned on'),
            ({}, 'nothing else'),
        ]
        combined = {}
        for settings, expected in cases:
            with self.subTest(expected=expected):
                code, text = self.invoke(settings=settings)
                self.assertEqual(code, 0)
                self.assertIn("This Mac's own settings also bring: %s." % expected, text)
                for private in ['invented-command', 'invented-env-value', 'other-value',
                                '/invented/helper']:
                    self.assertNotIn(private, text)
            combined.update(settings)
        code, text = self.invoke(settings=combined)
        self.assertEqual(code, 0)
        self.assertIn("This Mac's own settings also bring: 3 hooks, 2 environment settings, "
                      "a key helper, 2 plugins turned on.", text)

    def test_settings_report_lists_only_allowed_values(self):
        for permissions, expected in [
                ({'defaultMode': 'dontAsk'}, 'a default mode (dontAsk)'),
                ({'defaultMode': 'private-mode-value'}, 'a default mode (another value)'),
                ({'additionalDirectories': ['/private/invented', '/second']}, '2 added folders'),
                ({'allow': ['Read(/private/invented)', 'Grep', 'Glob(**)', 'Bash']},
                 '3 allow rules for Read, Grep or Glob'),
                ({'defaultMode': 'manual'}, 'widen reads: no.'),
                ({'defaultMode': 'default'}, 'widen reads: no.'),
                ({'defaultMode': 3, 'allow': [None], 'additionalDirectories': []}, 'widen reads: no.')]:
            with self.subTest(permissions=permissions):
                code, text = self.invoke(settings={'permissions': permissions})
                self.assertEqual(code, 0)
                self.assertIn(expected, text)
                self.assertNotIn('private-mode-value', text)
                self.assertNotIn('/private/invented', text)
        permissions = {'defaultMode': 'plan', 'additionalDirectories': ['/invented'], 'allow': ['Read']}
        code, text = self.invoke(settings={'permissions': permissions})
        self.assertIn('yes (a default mode (plan), 1 added folders, 1 allow rules for Read, Grep or Glob).', text)
        for content in ['not JSON', '[]']:
            (self.config / 'settings.json').write_text(content)
            code, text = self.invoke()
            self.assertEqual(code, 0)
            self.assertIn("This Mac's own settings could not be read.", text)
        (self.config / 'settings.json').unlink()
        code, text = self.invoke()
        self.assertIn("This Mac's own settings widen reads: no.", text)
        with patch.object(probe.Path, 'read_text', side_effect=PermissionError()):
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                probe.own_settings_report()
            self.assertEqual(output.getvalue(), "This Mac's own settings could not be read.\n")

    def test_run_classification_and_retries(self):
        code, text = self.invoke('run')
        self.assertEqual(code, 0)
        self.assertIn('Result: the fence held.', text)
        self.assertIn('Read, notes.md: read: pass.', text)
        for key, _ in probe.CANARIES[1:]:
            self.assertIn('Read, %s: denied: pass.' % key, text)
        self.assertIn('Grep over the folder: pass.', text)
        self.assertIn('Glob listed 1 of the 22 denied names (names only, no contents).', text)
        self.assertIn('A rule written without // (**/.env*) did not hold for a folder added with --add-dir.', text)
        self.assertIn('Without the deny rules, .env in the folder was read.', text)
        self.assertIn('Cost: $0.0070.', text)
        for plan, expected, wanted in [
                ({'leak': '.npmrc'}, 'Read, .npmrc: read: FAIL.', 1),
                ({'retry': '.npmrc'}, 'Read, .npmrc: denied: pass.', 0),
                ({'never': '.npmrc'}, 'Read, .npmrc: not tried: FAIL.', 1),
                ({'other_refusal': '.npmrc'}, 'Read, .npmrc: refused another way: FAIL.', 1),
                ({'grep_leak': '.netrc'}, 'Grep over the folder: FAIL (.netrc).', 1),
                ({'skip_grep': True}, 'Grep over the folder: FAIL (not tried).', 1),
                ({'realpath': True}, 'Result: the fence held.', 0),
                ({'skip_glob': True}, 'Glob: not tried.', 0)]:
            with self.subTest(plan=plan):
                trace = self.folder / 'trace.jsonl'
                trace.unlink(missing_ok=True)
                code, text = self.invoke('run', plan)
                self.assertEqual(code, wanted)
                self.assertIn(expected, text)
                if wanted:
                    self.assertIn('Result: the fence did not hold, or the check could not tell (1 lines failed).', text)
                records = [json.loads(line) for line in trace.read_text().splitlines()]
                reads = [r for r in records if r['name'] == 'read']
                self.assertEqual(len(reads), 3 if 'never' in plan else 2 if 'retry' in plan else 1)
                for later in reads[1:]:
                    self.assertEqual(len(later['paths']), 1)
                    self.assertTrue(later['paths'][0].endswith('/.npmrc'))
        code, text = self.invoke('run', {'no_log_turn': 'read'})
        self.assertEqual(code, 1)
        for key, _ in probe.CANARIES:
            self.assertIn('Read, %s: the deny rules were not applied: FAIL.' % key, text)
        code, text = self.invoke('run', {'no_log_turn': 'grep'})
        self.assertEqual(code, 1)
        self.assertIn('Grep over the folder: FAIL (the deny rules were not applied).', text)
        code, text = self.invoke('run', {'no_log_turn': 'relative'})
        self.assertEqual(code, 0)
        self.assertIn('A rule written without // (**/.env*): the deny rules were not applied.', text)
        code, text = self.invoke('run', {'error': 'invented error <base> fence123456abcdef \x1b ' + str(Path.home())})
        self.assertEqual(text.count('Claude Code said:'), 7)
        self.assertNotIn('\x1b', text)
        self.assertEqual(code, 0)
        real_run_turn = probe.run_turn
        def timeout_read(claude, base, name, prompt, settings, attempt=1):
            if name == 'read':
                return probe.parse_turn('', -9, '', True)
            return real_run_turn(claude, base, name, prompt, settings, attempt)
        with patch.object(probe, 'run_turn', side_effect=timeout_read):
            code, text = self.invoke('run')
        self.assertEqual(code, 1)
        self.assertIn('Read, notes.md: timed out: FAIL.', text)
        self.assertIn('(24 lines failed)', text)

    def test_failed_version_stops_and_scrubs_last_stderr_line(self):
        import pwd
        home = pwd.getpwuid(os.getuid()).pw_dir
        code, text = self.invoke(plan={'version_exit': 7, 'stderr': 'first\n' + home + '/oops ' + 'x' * 300})
        self.assertEqual(code, 1)
        self.assertEqual(text.splitlines()[0], "Claude Code didn't start (exit 7).")
        self.assertTrue(text.splitlines()[1].startswith('~/oops'))
        self.assertEqual(len(text.splitlines()[1]), 200)
        self.assertNotIn('first', text)

    def test_run_finishes_check_before_making_canary_files(self):
        original = probe.check
        def inspect(claude, base):
            self.assertEqual(list((base / 'root').iterdir()), [])
            self.assertEqual(list((base / 'outside').iterdir()), [])
            return original(claude, base)
        with patch.object(probe, 'check', side_effect=inspect):
            code, text = self.invoke('run', {'auth': {'loggedIn': False}, 'auth_exit': 1})
        self.assertEqual(code, 1)
        self.assertIn('Signed in: no.', text)
        self.assertIn('Sign in to Claude Code first', text)
        for plan in [{'auth': {}}, {'no_log': True}, {'tools': ['Bash']}]:
            with patch.object(probe, 'make_canaries') as make:
                code, text = self.invoke('run', plan)
            self.assertEqual(code, 1)
            make.assert_not_called()

    def test_glob_denied_contents_fail(self):
        code, text = self.invoke('run', {'glob_leak': '.netrc'})
        self.assertIn("Glob turn: a denied file's contents appeared (.netrc): FAIL.", text)
        self.assertIn('Glob listed 1 of the 22 denied names (names only, no contents).', text)
        self.assertEqual(code, 1)
        self.assertIn('(1 lines failed)', text)

    def test_each_symlink_and_target_must_be_tried(self):
        for key in ['alias.md', 'out-link.md', '.env', 'outside/far.md']:
            with self.subTest(key=key):
                code, text = self.invoke('run', {'never': key})
                self.assertIn('Read, %s: not tried: FAIL.' % key, text)
                self.assertEqual(code, 1)
                self.assertIn('(1 lines failed)', text)

    def test_later_try_cannot_hide_an_already_denied_words_leak(self):
        code, text = self.invoke('run', {'retry': '.npmrc', 'later_leak': '.netrc'})
        self.assertEqual(code, 1)
        self.assertIn('Read, .netrc: read: FAIL.', text)


class WiringTests(unittest.TestCase):
    def test_ci_runs_probe_after_sandbox_probe(self):
        self.assertRegex((REPO / 'scripts/ci-local.sh').read_text(),
                         r'for t in [^;]*claude_sandbox_probe read_fence_probe; do')


if __name__ == '__main__':
    unittest.main()
