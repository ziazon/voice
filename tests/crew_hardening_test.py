"""Regression checks for public-kit input validation and private output handling."""
import contextlib
import io
import json
import os
import re
import struct
import subprocess
import sys
import unittest
import zipfile
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import crew_decisions_test as base_tests
from crew_decisions_test import ADA, Harness
from crew_fixtures import assert_cli_error
from crew_sources_test import EXPORT
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'scripts'))
import crew_decisions as crew
import slack_mine


class Hardening(unittest.TestCase):
    def setUp(self):
        self.h = Harness()
        self.addCleanup(self.h.dir.cleanup)
        self.member = self.h.member(ADA)

    def test_member_with_ceilings_still_loads(self):
        self.h.projects.mkdir()
        self.member = self.h.member(dict(ADA, ceilings={'mail': 'anthropic',
                                                      'claude-history': 'local-only'}))
        proc = self.h.crew(self.member, 'folders', '--projects', str(self.h.projects))
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(json.loads(self.h.read('member.json'))['ceilings'],
                         {'mail': 'anthropic', 'claude-history': 'local-only'})

    def test_merge_preserves_unicode_line_separators(self):
        record = crew.message_record('s', '2026-09-01', 'kit', 'scope\u2028next\u0085last', '')
        source = self.h.root / 'source.jsonl'
        output = self.h.root / 'merged.jsonl'
        crew.write_jsonl(source, [record])
        proc = self.h.crew(self.member, 'merge', '--in', str(source), '--out', str(output))
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(crew.read_jsonl(output)[0]['text'], record['text'])

    def test_bad_config_shapes_are_one_line_errors(self):
        for filename, value in [('member.json', []), ('member.json', None), ('private.json', []),
                                ('private.json', {'filters': {'pasted_prefixes': [3]}})]:
            with self.subTest(filename=filename, value=value):
                self.h.member(ADA)
                (self.h.root / 'private.json').unlink(missing_ok=True)
                self.h.write(filename, value)
                proc = self.h.crew(self.member, 'folders', '--projects', str(self.h.projects))
                self.assertEqual(proc.returncode, 1)
                assert_cli_error(self, proc.stderr, "crew_decisions:")
        for value in ['Pasted: ', [4], None]:
            with self.subTest(prefixes=value):
                config = json.loads(json.dumps(ADA))
                config['filters']['pasted_prefixes'] = value
                (self.h.root / 'private.json').unlink(missing_ok=True)
                self.h.member(config)
                proc = self.h.crew(self.member, 'folders')
                self.assertEqual(proc.returncode, 1)
                self.assertIn('list of strings', proc.stderr)
                assert_cli_error(self, proc.stderr, "crew_decisions:")

    def test_empty_answer_scores_as_a_miss(self):
        answer = {'id': 'q', 'outcome': 'no_rec', 'picks': [], 'multiSelect': False}
        result = crew.score(crew.load_member(ADA), [answer], {}, {'q': {'pick': 'Yes', 'confidence': 'low'}})
        self.assertEqual(result['overall']['hits'], 0)

    def test_project_name_uses_current_home(self):
        for home in ['/Users/jane.doe', '/home/jane', '/Users/jane-doe']:
            with self.subTest(home=home), patch.object(Path, 'home', return_value=Path(home)):
                # Claude Code turns every character but letters and digits into "-".
                prefix = re.sub(r'[^A-Za-z0-9]', '-', home)
                self.assertEqual(crew.project_name(prefix + '-projects-course--claude-worktrees-test'), 'course')
                self.assertEqual(crew.project_name(prefix + '--claude'), '.claude')

    def test_project_name_strips_another_macs_home_too(self):
        with patch.object(Path, 'home', return_value=Path('/Users/someone')):
            self.assertEqual(crew.project_name('-Users-z-projects-app'), 'app')
            self.assertEqual(crew.project_name('-home-jo-notes'), 'notes')

    def test_folders_breaks_project_name_ties_by_folder_name(self):
        names = ['-Users-p-projects-course--claude-worktrees-z',
                 '-Users-p-projects-course--claude-worktrees-a']
        folders = [self.h.projects / name for name in names]
        for folder in folders:
            folder.mkdir(parents=True)
        output = io.StringIO()
        with patch.object(Path, 'iterdir', return_value=iter(folders)), \
                patch.object(Path, 'home', return_value=Path('/Users/p')), \
                contextlib.redirect_stdout(output):
            crew.cmd_folders(crew.load_member(ADA), SimpleNamespace(projects=self.h.projects))
        printed_names = [line.split('\t')[-1] for line in output.getvalue().splitlines()[:-1]]
        self.assertEqual(printed_names, sorted(names))

    def test_project_fields_are_filtered(self):
        member = crew.load_member(ADA)
        for project, reason in [('my therapist', 'personal'), ('123456789', 'secret')]:
            with self.subTest(project=project):
                record = crew.message_record('s', 'now', project, 'a safe decision', '')
                self.assertEqual(crew.privacy(member, record), reason)

    def test_zip_limits_before_reading(self):
        path = self.h.root / 'export.zip'
        with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_DEFLATED) as z:
            z.writestr('conversations.json', ' ' * 100000)
        proc = self.h.crew(self.member, 'claudeai', 'inspect', '--export', str(path))
        self.assertEqual(proc.returncode, 1)
        self.assertIn('compression ratio', proc.stderr)
        assert_cli_error(self, proc.stderr, "crew_decisions:")
        # Forge only the central directory's declared size; no large allocation is needed.
        data = bytearray(path.read_bytes())
        central = data.index(b'PK\x01\x02')
        struct.pack_into('<I', data, central + 24, 2 * 1024**3 + 1)
        path.write_bytes(data)
        proc = self.h.crew(self.member, 'claudeai', 'inspect', '--export', str(path))
        self.assertEqual(proc.returncode, 1)
        self.assertIn('2 GB', proc.stderr)
        assert_cli_error(self, proc.stderr, "crew_decisions:")

    def test_inspect_counts_files_without_names(self):
        path = self.h.root / 'export.zip'
        with zipfile.ZipFile(path, 'w') as z:
            z.writestr('conversations.json', json.dumps(EXPORT))
            z.writestr('private-client-name.txt', '')
        proc = self.h.crew(self.member, 'claudeai', 'inspect', '--export', str(path))
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertIn('files: 2', proc.stdout)
        self.assertNotIn('private-client-name', proc.stdout)

    def test_private_outputs_replace_symlinks_and_use_private_modes(self):
        victim = self.h.root / 'victim'
        victim.write_text('untouched')
        target = self.h.root / 'output'
        target.symlink_to(victim)
        # A stale predictable tmp symlink must never be followed either.
        target.with_suffix('.tmp').symlink_to(victim)
        crew.write_jsonl(target, [{'text': 'private'}])
        self.assertEqual(victim.read_text(), 'untouched')
        self.assertFalse(target.is_symlink())
        self.assertEqual(target.stat().st_mode & 0o777, 0o600)
        nested = self.h.root / 'new' / 'nested' / 'review.tsv'
        crew.write_review(nested, [])
        self.assertEqual(nested.parent.stat().st_mode & 0o777, 0o700)
        self.assertEqual(nested.parent.parent.stat().st_mode & 0o777, 0o700)
        self.assertEqual(nested.stat().st_mode & 0o777, 0o600)

    def test_exclusive_tmp_collision_keeps_existing_file(self):
        target = self.h.root / 'output'
        stale = self.h.root / '.output.collision.tmp'
        stale.write_text('existing private data')
        with patch.object(crew.uuid, 'uuid4') as unique:
            unique.return_value.hex = 'collision'
            with self.assertRaises(FileExistsError):
                crew.write_jsonl(target, [])
        self.assertEqual(stale.read_text(), 'existing private data')
        self.assertFalse(target.exists())

    def test_empty_chunks_still_create_private_output_directory(self):
        (self.h.root / 'empty.jsonl').write_text('')
        output = self.h.root / 'chunks/nested'
        proc = self.h.crew(self.member, 'chunk', '--corpus', 'empty.jsonl', '--out-dir', str(output))
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertTrue(output.is_dir())
        self.assertEqual(output.stat().st_mode & 0o777, 0o700)
        self.assertEqual(output.parent.stat().st_mode & 0o777, 0o700)

    def test_record_and_pending_write_private_files(self):
        log = self.h.root / 'logs/nested/live.jsonl'
        for _ in range(2):
            proc = self.h.crew(self.member, 'record', '--log', str(log), '--work', 'plan.md',
                               '--kind', 'decided', '--category', 'scope', '--tier', 'high',
                               '--question', 'Keep this?', '--ada-pick', 'Yes')
            self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(len(log.read_text().splitlines()), 2)
        self.assertEqual(log.stat().st_mode & 0o777, 0o600)
        self.assertEqual(log.parent.stat().st_mode & 0o777, 0o700)
        queues = self.h.root / 'queues/project'
        queues.mkdir(parents=True)
        (queues / 'queue.md').write_text('🎛️ CONTROL-Q example · Which order?\n')
        self.h.projects.mkdir()
        seen = self.h.root / 'seen/nested/seen.json'
        args = ('pending', '--projects', str(self.h.projects), '--queues', str(queues.parent),
                '--seen', str(seen))
        proc = self.h.crew(self.member, *args)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(len(proc.stdout.splitlines()), 1)
        self.assertEqual(seen.stat().st_mode & 0o777, 0o600)
        self.assertEqual(seen.parent.stat().st_mode & 0o777, 0o700)
        self.assertEqual(self.h.crew(self.member, *args).stdout, '')

    def pending_queue(self, text):
        queues = self.h.root / 'queues/project'
        queues.mkdir(parents=True, exist_ok=True)
        (queues / 'queue.md').write_text(text)
        member = self.h.member(dict(ADA, queue_answerer='Desk'))
        seen = self.h.root / 'seen.json'
        seen.unlink(missing_ok=True)
        proc = self.h.crew(member, 'pending', '--projects', str(self.h.projects),
                           '--queues', str(queues.parent), '--seen', str(seen))
        self.assertEqual(proc.returncode, 0, proc.stderr)
        return [json.loads(line) for line in proc.stdout.splitlines()]

    def test_pending_plain_body_stops_at_blank_and_extracts_options(self):
        events = self.pending_queue(
            '  🎛️ CONTROL-Q BODY · Which order?\n'
            '**Choose the order.**  \n'
            'Keep `code` verbatim.\n'
            '  a) Oldest first  \n'
            'b) Newest first ← Ada (high)\n'
            'One last detail.\n \t\nMore prose outside the block.\n')
        self.assertEqual(len(events), 1)
        self.assertEqual(events[0]['body'], '**Choose the order.**\nKeep `code` verbatim.\n'
                         '  a) Oldest first\nb) Newest first ← Ada (high)\nOne last detail.')
        self.assertEqual(events[0]['options'], ['Oldest first', 'Newest first ← Ada (high)'])

    def test_pending_body_stops_at_next_control_header(self):
        events = self.pending_queue('🎛️ CONTROL-Q FIRST · Pick?\nFirst body\n'
                                    '🎛️ CONTROL-Q SECOND · Pick?\nSecond body\n')
        self.assertEqual([e['key'] for e in events], ['FIRST', 'SECOND'])
        self.assertEqual([e['body'] for e in events], ['First body', 'Second body'])

    def test_pending_heading_body_keeps_blanks_until_any_heading(self):
        events = self.pending_queue('### 🎛️ CONTROL-Q HEADING · Pick?\nFirst paragraph\n\n'
                                    'a) Keep `this`\n\n \t\n## Next section\nOutside\n')
        self.assertEqual(len(events), 1)
        self.assertEqual(events[0]['body'], 'First paragraph\n\na) Keep `this`')
        self.assertEqual(events[0]['options'], ['Keep `this`'])

    def test_pending_heading_body_trims_leading_blank(self):
        events = self.pending_queue('### 🎛️ CONTROL-Q H · t\n\ntext\n')
        self.assertEqual(events[0]['body'], 'text')

    def test_pending_heading_body_trims_edges_keeps_interior_blank(self):
        events = self.pending_queue('### 🎛️ CONTROL-Q H2 · t\n\none\n\ntwo\n\n')
        self.assertEqual(events[0]['body'], 'one\n\ntwo')

    def test_pending_plain_body_keeps_seven_hash_dial_line(self):
        events = self.pending_queue('🎛️ CONTROL-Q HASHES · t\none\n####### 🎛️ x\ntwo\n')
        self.assertEqual(events[0]['body'], 'one\n####### 🎛️ x\ntwo')

    def test_pending_body_stops_at_any_dial_line(self):
        for header in ('🎛️ CONTROL-Q K · t', '### 🎛️ CONTROL-Q K · t'):
            for stop in ('🎛️ Park status', '### 🎛️ Park status',
                         '  🎛️ Park status', '  ### 🎛️ Park status', '🎛️ CONTROL-Z BAD'):
                with self.subTest(header=header, stop=stop):
                    events = self.pending_queue(header + '\nasked\n' + stop + '\nnot\n')
                    self.assertEqual(len(events), 1)
                    self.assertEqual(events[0]['body'], 'asked')

    def test_pending_heading_body_stops_at_indented_heading(self):
        events = self.pending_queue('### 🎛️ CONTROL-Q H · x\nbody\n  ## Next\noutside\n')
        self.assertEqual(events[0]['body'], 'body')

    def test_pending_private_later_body_line_is_withheld(self):
        private = 'my therapist'
        events = self.pending_queue('🎛️ CONTROL-Q LATER · x\nSafe first line\n'
                                    + private + '\na) Safe last line\n')
        self.assertEqual(len(events), 1)
        self.assertIsNone(events[0]['body'])
        self.assertTrue(events[0]['body_withheld'])
        self.assertEqual(events[0]['options'], [])
        self.assertNotIn(private, json.dumps(events))

    def test_pending_heading_body_stops_at_plain_control_header(self):
        events = self.pending_queue('### 🎛️ CONTROL-Q H1 · x\none\n\ntwo\n'
                                    '🎛️ CONTROL-Q P2 · y\n')
        self.assertEqual([e['key'] for e in events], ['H1', 'P2'])
        self.assertEqual(events[0]['body'], 'one\n\ntwo')

    def test_pending_options_require_space_and_text(self):
        events = self.pending_queue('🎛️ CONTROL-Q OPTIONS · x\na)\nb)x\nc) real\n')
        self.assertEqual(events[0]['options'], ['real'])

    def test_pending_indented_heading_header_keeps_blanks(self):
        events = self.pending_queue('  ### 🎛️ CONTROL-Q IH · x\nbody\n\nmore\n## End\n')
        self.assertEqual(events[0]['body'], 'body\n\nmore')

    def test_pending_heading_body_heading_levels(self):
        for stop in ('# End', '###### End'):
            with self.subTest(stop=stop):
                events = self.pending_queue('### 🎛️ CONTROL-Q LEVEL · x\nbody\n'
                                            '####### Deep\n##Next\n' + stop + '\noutside\n')
                self.assertEqual(events[0]['body'], 'body\n####### Deep\n##Next')

    def test_pending_answer_carries_body(self):
        events = self.pending_queue(
            '🎛️ CONTROL-Q ANSWER · 2026-10-23T10:00Z · session abc123 (Autonomous on) · attended\n'
            '🎛️ CONTROL-A ANSWER · 2026-10-23T11:00Z · Desk · answered by Sam · attended\n'
            '**Sam picked (b):** Newest first\n1. Apply the order.\n\nOutside\n')
        self.assertEqual(len(events), 1)
        self.assertEqual(events[0]['kind'], 'control-a')
        self.assertEqual(events[0]['session'], 'abc123')
        self.assertEqual(events[0]['body'], '**Sam picked (b):** Newest first\n1. Apply the order.')

    def test_pending_private_body_is_withheld_but_event_is_emitted(self):
        for kind in ('Q', 'A'):
            for private in ('my therapist', '123456789'):
                with self.subTest(kind=kind, pattern='personal' if private.startswith('my') else 'secret'):
                    header = '🎛️ CONTROL-Q PRIVATE · Pick?\n'
                    if kind == 'A':
                        header = (
                            '🎛️ CONTROL-Q PRIVATE · session abc123 (Autonomous on)\n'
                            '🎛️ CONTROL-A PRIVATE · 2026-10-23 · Desk · answered by Sam\n')
                    events = self.pending_queue(header + 'a) ' + private + '\n')
                    self.assertEqual(len(events), 1)
                    self.assertEqual(events[0]['kind'], 'control-' + kind.lower())
                    self.assertIsNone(events[0]['body'])
                    self.assertTrue(events[0]['body_withheld'])
                    self.assertEqual(events[0].get('options', []), [])
                    self.assertNotIn(private, json.dumps(events))

    def test_pending_header_at_eof_has_empty_body(self):
        events = self.pending_queue('🎛️ CONTROL-Q EMPTY · Pick?')
        self.assertEqual(events[0]['body'], '')
        self.assertEqual(events[0]['options'], [])

    def test_pending_body_cut_at_eof(self):
        events = self.pending_queue('🎛️ CONTROL-Q EOF · Pick?\nFirst line\na) Last line  ')
        self.assertEqual(events[0]['body'], 'First line\na) Last line')
        self.assertEqual(events[0]['options'], ['Last line'])

    def test_queue_answers_come_only_from_the_configured_answerer(self):
        queues = self.h.root / 'queues'
        (queues / 'project').mkdir(parents=True)
        (queues / 'project/queue.md').write_text(
            '🎛️ CONTROL-Q ORDER · 2026-10-23T10:00Z · session abc123 (Autonomous on) · attended\n'
            '🎛️ CONTROL-A ORDER · 2026-10-23T11:00Z · Other · answered by Sam · attended\n'
            '🎛️ CONTROL-A ORDER · 2026-10-23T12:00Z · Desk · answered by Sam · attended\n')
        for answerer, keys in [(None, []), ('Desk', ['ORDER']), ('Other', ['ORDER'])]:
            with self.subTest(answerer=answerer):
                config = dict(ADA, **({'queue_answerer': answerer} if answerer else {}))
                answers = crew.answered_escalations(crew.load_member(config), queues)
                self.assertEqual([a['key'] for a in answers], keys)
                if answerer:
                    self.assertIn(f'· {answerer} ·', answers[0]['answer'])
        self.h.write('member.json', dict(ADA, queue_answerer=3))
        proc = self.h.crew(self.member, 'folders', '--projects', str(self.h.projects))
        self.assertEqual(proc.returncode, 1)
        assert_cli_error(self, proc.stderr, "crew_decisions:")

    def test_text_block_consumers_keep_strings_and_only_text_blocks(self):
        for content in ['plain text', [{'type': 'text', 'text': 'plain text'}, {'type': 'image'}]]:
            record = {'type': 'user', 'origin': {'kind': 'human'}, 'message': {'content': content}}
            self.assertEqual(crew.typed_text(crew.load_member(ADA), record), 'plain text')
            self.assertEqual(crew.assistant_text(record), 'plain text')
            self.assertTrue(crew.opens_a_turn(record))
            self.assertEqual(crew.chat_text({'content': content}), 'plain text')

    def test_shared_secret_shapes_drop_tokens_but_keep_ordinary_words(self):
        template = json.loads((crew.Path(crew.__file__).parent.parent / 'crew/_template/member.json').read_text())
        patterns = [re.compile(slack_mine.DEFAULT_SECRET, re.I), re.compile(template['filters']['secret'], re.I)]
        tokens = ['sk-' + 'a' * 20, 'sk_live_' + 'a' * 16, 'sk_test_' + 'a' * 16,
                  'ghp_' + 'a' * 20, 'xoxb-example', 'xapp-example', 'AKIA' + 'A' * 16,
                  'eyJ' + 'a' * 10 + '.' + 'b' * 10, 'AIza' + 'a' * 30, 'hf_' + 'a' * 30,
                  'glpat-' + 'a' * 20, 'npm_' + 'a' * 30, '-----BEGIN', 'password', 'passwd', '123456789']
        for i, pattern in enumerate(patterns):
            for n, token in enumerate(tokens):
                with self.subTest(pattern=i, shape=n):
                    self.assertIsNotNone(pattern.search(token))
                    member = crew.load_member({**ADA, 'filters': {**ADA['filters'], 'secret': pattern.pattern}})
                    record = crew.message_record('s', 'now', 'work', token, '')
                    self.assertEqual(crew.privacy(member, record), 'secret')
            for word in ['desk-123', 'task-9', 'npm_lifecycle', 'hf_word', 'AKIA']:
                with self.subTest(pattern=i, word=word):
                    self.assertIsNone(pattern.search(word))
        self.assertIn(slack_mine.DEFAULT_SECRET, template['filters']['secret'])


class TrackedPrivacy(unittest.TestCase):
    setUp = base_tests.PrivacyCheck.setUp
    check = base_tests.PrivacyCheck.check
    append = base_tests.PrivacyCheck.append

    def test_git_checks_only_tracked_files_under_dir(self):
        root = self.h.root
        subprocess.run(['git', 'init', '-q', str(root)], check=True)
        (root / '.gitignore').write_text('ignored/\n')
        self.append('tracked.md', 'keep this public')
        subprocess.run(['git', '-C', str(root), 'add', 'crew', '.gitignore'], check=True)
        self.append('untracked.md', 'password')
        self.append('ignored/private.txt', 'password')
        (root / 'outside.md').write_text('password')
        self.assertEqual(self.check().returncode, 0)
        self.append('tracked.md', 'password')
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        self.assertIn('tracked.md', proc.stdout)
        self.assertNotIn('untracked.md', proc.stdout)

    def test_fallback_skips_os_junk(self):
        (self.dir / '.DS_Store').write_bytes(b'\xff')
        self.assertEqual(self.check().returncode, 0)


if __name__ == '__main__':
    unittest.main()
