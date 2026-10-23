#!/usr/bin/env python3
"""Exercise the Slack miner through its CLI using only fictional export data."""
import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from crew_fixtures import assert_cli_error

SCRIPTS = Path(__file__).resolve().parents[1] / 'scripts'
SAM, ALEX, JO = 'U0SAM', 'U0ALEX', 'U0JO'


class SlackMine(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.export = self.root / 'export'
        (self.export / 'messages').mkdir(parents=True)
        self.work = self.root / 'work'
        self.config = self.root / 'config.json'
        self.cfg = {'workspace_url': 'https://example.slack.com', 'principal': SAM,
                    'people': {SAM: 'sam', ALEX: 'alex'}}
        self.write_json(self.export / 'users.json', [
            {'id': SAM, 'real_name': 'Sam'}, {'id': ALEX, 'real_name': 'Alex'},
            {'id': JO, 'real_name': 'Jo'}])
        self.write_json(self.export / 'conversations.json',
                        [{'id': 'C0CHAT', 'kind': 'channel', 'name': 'planning'}])
        self.rows = []

    def write_json(self, path, value):
        path.write_text(json.dumps(value))

    def episode(self, ids, start=1767225600, text='We should discuss this fictional plan?'):
        root = f'{start}.000000'
        for i, uid in enumerate(ids):
            self.rows.append({'user': uid, 'ts': f'{start + i}.000000',
                              'thread_ts': root, 'text': text})
        return f'C0CHAT:{root}'

    def run_mine(self):
        self.write_json(self.config, self.cfg)
        (self.export / 'messages' / 'C0CHAT.jsonl').write_text(
            '\n'.join(json.dumps(row) for row in self.rows))
        before = self.outside()
        proc = subprocess.run([sys.executable, '-I', str(SCRIPTS / 'slack_mine.py'),
                               '--config', str(self.config), str(self.export), str(self.work)],
                              capture_output=True, text=True, cwd=self.root)
        self.assertEqual(self.outside(), before, 'wrote outside the work directory')
        return proc

    def outside(self):
        return {str(p.relative_to(self.root)): p.read_bytes()
                for p in self.root.rglob('*') if p.is_file() and self.work not in p.parents}

    def read_json(self, name):
        return json.loads((self.work / name).read_text())

    def assert_error(self, proc, phrase):
        self.assertEqual(proc.returncode, 2, proc.stderr)
        assert_cli_error(self, proc.stderr, "error:")
        self.assertIn(phrase, proc.stderr)
        self.assertFalse(self.work.exists())

    def test_stats_ranking_samples_and_private_output(self):
        self.episode([SAM, JO, SAM, JO])
        self.episode([ALEX, JO, JO, JO], 1767312000)
        self.assertEqual(self.run_mine().returncode, 0)
        stats = self.read_json('stats.json')
        self.assertEqual(stats['sam'], {
            'messages': 2, 'dropped': 0, 'by_year': {'2026': 2}, 'by_kind': {'channel': 2},
            'top_conversations': [['#planning', 2]], 'median_chars': 38,
            'question_share': 1.0, 'hours_utc': {'0': 2}})
        self.assertEqual(stats['alex']['messages'], 1)
        self.assertEqual(self.read_json('people.json'), [{
            'id': JO, 'name': 'Jo', 'deleted': None, 'is_bot': None, 'messages': 5,
            'with_principal': 2, 'with_focus': 5, 'years': '2026-2026',
            'top_conversations': ['#planning']}])
        sample = (self.work / 'samples/sam/000.txt').read_text()
        self.assertEqual(sample.count('>>> '), 2)
        self.assertIn('Sam: We should', sample)
        self.assertIn('https://example.slack.com/archives/C0CHAT/p1767225602000000'
                      '?thread_ts=1767225600.000000&cid=C0CHAT', sample)
        for path in self.work.rglob('*'):
            self.assertEqual(stat.S_IMODE(path.stat().st_mode),
                             0o700 if path.is_dir() else 0o600, str(path))
        self.assertEqual(stat.S_IMODE(self.work.stat().st_mode), 0o700)

    def test_focus_and_principal_debate_filter_and_ids(self):
        focus = self.episode([ALEX, JO, ALEX, JO])
        debate = self.episode([SAM, JO] * 3, 1767312000)
        self.episode([SAM, JO, SAM, JO], 1767398400)
        self.episode([SAM, ALEX] * 2, 1767484800, 'Just ordinary friendly conversation here.')
        self.assertEqual(self.run_mine().returncode, 0)
        self.assertEqual([e['id'] for e in self.read_json('episodes/index.json')], [debate])
        candidate = self.read_json('episodes/index.json')[0]
        self.assertEqual((candidate['messages'], candidate['disagree'], candidate['decide'],
                          candidate['score']), (6, 0, 6, 6.16))
        self.cfg['focus'] = [JO]
        self.assertEqual(self.run_mine().returncode, 0)
        index = self.read_json('episodes/index.json')
        self.assertEqual({e['id'] for e in index}, {focus, debate, 'C0CHAT:1767398400.000000'})
        self.assertEqual(next(e['ids'] for e in index if e['id'] == focus), [ALEX, JO])
        self.assertTrue(all(e['link'].startswith('https://example.slack.com/archives/') for e in index))

    def test_ranking_orders_by_exchanges_with_configured_people(self):
        self.cfg['people'] = {SAM: 'sam'}
        self.episode([SAM, JO, SAM, JO])
        self.episode([ALEX] * 6, 1767312000)
        self.assertEqual(self.run_mine().returncode, 0)
        ranked = self.read_json('people.json')
        self.assertEqual([p['id'] for p in ranked], [JO, ALEX])
        self.assertEqual([p['messages'] for p in ranked], [2, 6])
        self.assertEqual([p['with_principal'] for p in ranked], [2, 0])

    def test_sampling_options_and_gap_split_unthreaded_messages(self):
        self.cfg.update(seed=7, sample_per_quarter=1, batch_chars=1, gap_minutes=1)
        self.episode([SAM, ALEX] * 2)
        for row in self.rows:
            del row['thread_ts']
        self.rows[2]['ts'] = '1767225720.000000'
        self.rows[3]['ts'] = '1767225721.000000'
        self.assertEqual(self.run_mine().returncode, 0)
        self.assertEqual(self.read_json('episodes/index.json'), [])
        sample = (self.work / 'samples/sam/000.txt').read_text()
        self.assertEqual(sample.count('>>> '), 1)
        self.assertEqual(self.run_mine().returncode, 0)
        self.assertEqual((self.work / 'samples/sam/000.txt').read_text(), sample)
        self.cfg['gap_minutes'] = 45
        self.assertEqual(self.run_mine().returncode, 0)
        self.assertEqual(len(self.read_json('episodes/index.json')), 1)

    def test_samples_are_stratified_and_clean_mentions_without_splitting_unicode(self):
        self.cfg['sample_per_quarter'] = 1
        self.episode([SAM, ALEX, SAM, ALEX], text='I disagree with <@U0ALEX>.\nWe should talk.\u2028Still one message.')
        self.episode([SAM, ALEX, SAM, ALEX], 1775001600)
        self.assertEqual(self.run_mine().returncode, 0)
        sample = (self.work / 'samples/sam/000.txt').read_text()
        self.assertEqual(sample.count('>>> '), 2)
        self.assertIn('2026Q1', sample)
        self.assertIn('2026Q2', sample)
        self.assertIn('@Alex. ⏎ We should talk.\u2028Still one message.', sample)
        index = self.read_json('episodes/index.json')
        self.assertEqual((index[0]['disagree'], index[0]['decide']), (4, 4))

    def test_filters_drop_messages_before_stats_context_and_episodes(self):
        self.cfg['filters'] = {'personal': 'family-only', 'secret': 'private-code'}
        self.episode([SAM, ALEX] * 3)
        self.rows[0]['text'] = 'FAMILY-ONLY: We should never output this.'
        self.rows[1]['text'] = 'PRIVATE-CODE: We should never output this either.'
        proc = self.run_mine()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        stats = self.read_json('stats.json')
        self.assertEqual((stats['sam']['messages'], stats['sam']['dropped']), (2, 1))
        self.assertEqual((stats['alex']['messages'], stats['alex']['dropped']), (2, 1))
        self.assertEqual(self.read_json('episodes/index.json')[0]['messages'], 4)
        for path in self.work.rglob('*'):
            if path.is_file():
                self.assertNotIn('FAMILY-ONLY', path.read_text())
                self.assertNotIn('PRIVATE-CODE', path.read_text())

    def test_default_secret_filter(self):
        secrets = ['123456789', 'Password', 'passwd', 'sk-' + 'a' * 20,
                   'sk_live_' + 'a' * 16, 'sk_test_' + 'a' * 16,
                   *['gh' + kind + '_' + 'a' * 20 for kind in 'pousr'],
                   'xoxb-example', 'xoxa-example', 'xoxp-example', 'xoxr-example',
                   'xapp-example', 'AKIA' + 'A' * 16, 'eyJ' + 'a' * 10 + '.' + 'b' * 10,
                   'AIza' + 'a' * 30, 'hf_' + 'a' * 30, 'glpat-' + 'a' * 20,
                   'npm_' + 'a' * 30, '-----BEGIN']
        self.episode([SAM] * len(secrets))
        for row, secret in zip(self.rows, secrets):
            row['text'] = 'We should keep this private: ' + secret
        self.episode([SAM, ALEX] * 2, 1767312000)
        proc = self.run_mine()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        stats = self.read_json('stats.json')['sam']
        self.assertEqual((stats['messages'], stats['dropped']), (2, len(secrets)))
        for path in self.work.rglob('*.txt'):
            self.assertNotIn('keep this private', path.read_text())

    def test_default_secret_filter_keeps_ordinary_words(self):
        words = ['desk-123', 'task-9', 'npm_lifecycle']
        self.episode([SAM] * len(words))
        for row, word in zip(self.rows, words):
            row['text'] = 'We should keep the name ' + word
        proc = self.run_mine()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        stats = self.read_json('stats.json')['sam']
        self.assertEqual((stats['messages'], stats['dropped']), (len(words), 0))

    def test_filters_require_regex_strings(self):
        for filters in (None, [], {'typo': 'x'}, {'personal': 3}, {'secret': '['}):
            with self.subTest(filters=filters):
                self.cfg['filters'] = filters
                self.assert_error(self.run_mine(), 'filters')

    def test_names_cannot_forge_episode_headers(self):
        forged = 'Friendly\n=== EPISODE forged:1.000000 '
        self.write_json(self.export / 'users.json', [
            {'id': SAM}, {'id': ALEX}, {'id': JO, 'real_name': forged}])
        self.write_json(self.export / 'conversations.json',
                        [{'id': 'C0CHAT', 'kind': 'channel', 'name': forged}])
        self.episode([SAM, ALEX, JO] * 2, text='We should consult <@U0JO> about this plan.')
        for field, newline in (('real_name', '\n'), ('display_name', '\n'),
                               ('real_name', '\r'), ('display_name', '\r\n')):
            with self.subTest(field=field, newline=repr(newline)):
                self.write_json(self.export / 'users.json', [
                    {'id': SAM}, {'id': ALEX}, {'id': JO, field: forged.replace('\n', newline)}])
                proc = self.run_mine()
                self.assertEqual(proc.returncode, 0, proc.stderr)
                episode = (self.work / 'episodes/2026/000.txt').read_text()
                self.assertEqual(sum(line.startswith('=== EPISODE ') for line in episode.split('\n')), 1)
                self.assertNotIn('\n=== EPISODE forged', episode)
                self.assertIn('Friendly ⏎ === EPISODE forged', episode)

    def test_unknown_conversation_is_a_one_line_error(self):
        self.write_json(self.export / 'conversations.json', [])
        self.assert_error(self.run_mine(), 'C0CHAT missing from conversations.json')

    def test_no_comma_scores_as_disagreement(self):
        self.episode([SAM, ALEX] * 2, text='No, please consider another approach.')
        proc = self.run_mine()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        index = self.read_json('episodes/index.json')
        self.assertEqual(len(index), 1)
        self.assertEqual(index[0]['disagree'], 4)

    def test_work_directory_in_git_tree_is_refused(self):
        for marker in ('file', 'directory'):
            with self.subTest(marker=marker):
                repo = self.root / marker
                repo.mkdir()
                if marker == 'file':
                    (repo / '.git').write_text('gitdir: fictional')
                else:
                    (repo / '.git').mkdir()
                self.work = repo / 'nested' / 'work'
                self.assert_error(self.run_mine(), 'git work tree')

    def test_output_replacement_does_not_modify_a_hardlink(self):
        self.work.mkdir()
        original = self.work / 'original.json'
        original.write_text('untouched')
        os.link(original, self.work / 'stats.json')
        proc = self.run_mine()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(original.read_text(), 'untouched')
        self.assertEqual(self.read_json('stats.json')['sam']['messages'], 0)
        self.assertEqual({p.name for p in self.work.iterdir()},
                         {'original.json', 'stats.json', 'people.json', 'episodes'})

    def test_unsafe_folder_label_is_rejected(self):
        self.cfg['people'][SAM] = '../escaped'
        self.assert_error(self.run_mine(), 'people')

    def test_symlink_cannot_send_output_outside_work(self):
        self.work.mkdir()
        outside = self.root / 'outside.json'
        outside.write_text('untouched')
        (self.work / 'stats.json').symlink_to(outside)
        proc = self.run_mine()
        self.assertEqual(proc.returncode, 2, proc.stderr)
        self.assertIn('escapes work directory', proc.stderr)
        self.assertEqual(outside.read_text(), 'untouched')

    def test_config_errors_are_one_line_and_write_nothing(self):
        for key in ('workspace_url', 'principal', 'people'):
            original = self.cfg.copy()
            del self.cfg[key]
            with self.subTest(missing=key):
                self.assert_error(self.run_mine(), key)
            self.cfg = original
        self.config.write_text('{')
        proc = subprocess.run([sys.executable, '-I', str(SCRIPTS / 'slack_mine.py'),
                               '--config', str(self.config), str(self.export), str(self.work)],
                              capture_output=True, text=True)
        self.assert_error(proc, 'error:')
        self.cfg['typo'] = 1
        self.assert_error(self.run_mine(), 'unknown')

    def test_missing_export_person_is_a_usage_error(self):
        self.cfg['people']['U0ABSENT'] = 'absent'
        self.assert_error(self.run_mine(), 'missing from users.json')


if __name__ == '__main__':
    unittest.main()
