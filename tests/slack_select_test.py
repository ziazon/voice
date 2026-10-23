#!/usr/bin/env python3
"""Select by participant ids, then batch transcripts in timestamp order."""
import json
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from crew_fixtures import assert_cli_error

SCRIPT = Path(__file__).resolve().parents[1] / 'scripts/slack_select.py'


class SlackSelect(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.work = self.root / 'work'
        episodes = self.work / 'episodes/2026'
        episodes.mkdir(parents=True)
        self.config = self.root / 'config.json'
        self.cfg = {'workspace_url': 'https://example.slack.com', 'principal': 'U0SAM',
                    'people': {'U0SAM': 'sam', 'U0ALEX': 'alex'},
                    'select': {'min_score': 10, 'pairs': [
                        {'ids': ['U0SAM', 'U0JO'], 'min_score': 0},
                        {'ids': ['U0SAM', 'U0ALEX'], 'min_score': 8}]}}
        # All names are identical, so only the ids can distinguish these cases.
        index = [
            ('C0:6.000000', ['U0SAM', 'U0ALEX'], 8),
            ('C0:5.000000', ['U0SAM', 'U0JO'], 0),
            ('C0:4.000000', ['U0SAM', 'U0ALEX'], 7.99),
            ('C0:3.000000', ['U0SAM'], 9.99),
            ('C0:2.000000', ['U0JO'], 10),
            ('C0:1.000000', ['U0SAM', 'U0JO'], -0.01),
        ]
        (self.work / 'episodes/index.json').write_text(json.dumps([
            {'id': eid, 'ids': ids, 'score': score, 'people': ['Sam', 'Alex']}
            for eid, ids, score in index]))
        (episodes / '000.txt').write_text('\n'.join(
            f'=== EPISODE {eid} · fictional transcript\nSam: We should discuss this.\n'
            for eid, _, _ in index))

    def run_select(self, limit=60000):
        self.config.write_text(json.dumps(self.cfg))
        before = {p: p.read_bytes() for p in self.root.rglob('*')
                  if p.is_file() and self.work not in p.parents}
        proc = subprocess.run([sys.executable, '-I', str(SCRIPT), '--config',
                               str(self.config), str(self.work)] + ([] if limit is None else [str(limit)]),
                              capture_output=True, text=True, cwd=self.root)
        after = {p: p.read_bytes() for p in self.root.rglob('*')
                 if p.is_file() and self.work not in p.parents}
        self.assertEqual(after, before, 'wrote outside the work directory')
        return proc

    def test_pairs_thresholds_and_order_with_private_batches(self):
        proc = self.run_select(1)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stdout.strip(), '3 chosen, 3 found, 3 batches')
        out = self.work / 'record-batches'
        self.assertEqual(stat.S_IMODE(out.stat().st_mode), 0o700)
        files = sorted(out.glob('*.txt'))
        self.assertEqual([p.name for p in files], ['000.txt', '001.txt', '002.txt'])
        for path, eid in zip(files, ['C0:2.000000', 'C0:5.000000', 'C0:6.000000']):
            self.assertTrue(path.read_text().startswith(f'=== EPISODE {eid} '))
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)

    def test_default_rule_and_final_partial_batch(self):
        del self.cfg['select']
        proc = self.run_select()
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stdout.strip(), '1 chosen, 1 found, 1 batches')
        self.assertIn('C0:2.000000', (self.work / 'record-batches/000.txt').read_text())

    def test_omitted_batch_chars_uses_config(self):
        self.cfg['batch_chars'] = 1
        proc = self.run_select(None)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stdout.strip(), '3 chosen, 3 found, 3 batches')

    def test_work_directory_in_git_tree_is_refused(self):
        (self.root / '.git').write_text('gitdir: fictional')
        proc = self.run_select()
        self.assertEqual(proc.returncode, 2, proc.stderr)
        assert_cli_error(self, proc.stderr, "error:")
        self.assertIn('git work tree', proc.stderr)
        self.assertFalse((self.work / 'record-batches').exists())

    def test_invalid_rule_is_one_line_and_writes_nothing(self):
        self.cfg['select']['pairs'][0]['typo'] = True
        proc = self.run_select()
        self.assertEqual(proc.returncode, 2)
        assert_cli_error(self, proc.stderr, "error:")
        self.assertIn('select pairs', proc.stderr)
        self.assertFalse((self.work / 'record-batches').exists())


if __name__ == '__main__':
    unittest.main()
