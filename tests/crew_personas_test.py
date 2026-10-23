#!/usr/bin/env python3
"""Decision cards, per-person splits, blind backtests and the panel, on invented people only."""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from crew_fixtures import assert_cli_error

SCRIPT = Path(__file__).resolve().parents[1] / 'scripts/crew_personas.py'
CATEGORIES = ['scope', 'process', 'architecture']


def card(cid, date, stances, outcome='a', category='scope', options=('Ship it all', 'Ship half'), **extra):
    return {'id': cid, 'date': date, 'category': category, 'source': 'card',
            'question': f'Question {cid}: should the bake sale move indoors?',
            'context': 'The forecast was mixed and the hall was free.',
            'options': [{'id': chr(97 + i), 'text': text} for i, text in enumerate(options)],
            'stances': stances, 'outcome': outcome, 'asked': [], 'agent_drafted': [],
            'link': f'https://example.org/thread/{cid}', **extra}


def series(n, person='Sam', start_month=1):
    """n cards for one person, one a month from 2024-01, picking a, b, a, b..."""
    out = []
    for i in range(n):
        year, month = 2024 + (start_month - 1 + i) // 12, (start_month - 1 + i) % 12 + 1
        out.append(card(f'c{i:02d}', f'{year}-{month:02d}-15', {person: 'ab'[i % 2], 'Alex': 'a'}))
    return out


class Personas(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.categories = self.root / 'categories.json'
        self.categories.write_text(json.dumps(CATEGORIES))

    def write(self, name, value):
        path = self.root / name
        path.write_text(json.dumps(value) if not isinstance(value, str) else value)
        return path

    def run_cli(self, *args):
        return subprocess.run([sys.executable, '-I', str(SCRIPT), *map(str, args)],
                              capture_output=True, text=True, cwd=self.root)

    def split(self, cards, person='Sam', *extra):
        path = self.write('cards.json', {'cards': cards})
        proc = self.run_cli('split', '--cards', path, '--person', person, *extra)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        return path, json.loads(proc.stdout)

    # check

    def test_check_accepts_valid_cards(self):
        path = self.write('cards.json', {'cards': series(3)})
        proc = self.run_cli('check', '--cards', path, '--categories', self.categories)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stdout.strip(), '3 cards, 0 problems')

    def test_check_reports_each_broken_rule(self):
        bad = [
            card('dup', '2024-01-01', {'Sam': 'a'}),
            card('dup', '2024-01-02', {'Sam': 'a'}),
            card('stance', '2024-01-03', {'Sam': 'z'}),
            card('one-option', '2024-01-04', {'Sam': 'a'}, options=('Only one',)),
            card('category', '2024-01-05', {'Sam': 'a'}, category='weather'),
            card('outcome', '2024-01-06', {'Sam': 'a'}, outcome='q'),
            card('date', '2024-13-01', {'Sam': 'a'}),
            card('leak', '2024-01-07', {'Sam': 'a'}, options=('Move the bake sale indoors', 'Stay out')),
            card('asker', '2024-01-08', {'Sam': 'a'}, asked=[{'by': 'Robin', 'about': 'the hall key'}]),
            card('drafted', '2024-01-09', {'Sam': 'a'}, agent_drafted=['Jo']),
            card('source', '2024-01-10', {'Sam': 'a'}, source='simulated'),
        ]
        bad[7]['context'] = 'Someone said move the bake sale indoors before it rained.'
        path = self.write('cards.json', {'cards': bad})
        proc = self.run_cli('check', '--cards', path, '--categories', self.categories)
        self.assertEqual(proc.returncode, 1)
        for needle in ('dup: duplicate id', 'stance: Sam has no option z', 'one-option: 2 to 4 options',
                       'category: unknown category weather', 'outcome: outcome q', 'date: date',
                       'leak: context repeats option a', 'asker: Robin asked but has no stance',
                       'drafted: Jo is agent_drafted but has no stance', 'source: source simulated'):
            self.assertIn(needle, proc.stdout)
        self.assertEqual(proc.stdout.strip().splitlines()[-1], '11 cards, 10 problems')

    def test_check_refuses_a_file_that_is_not_cards(self):
        path = self.write('cards.json', [1, 2])
        proc = self.run_cli('check', '--cards', path)
        self.assertEqual(proc.returncode, 2)
        assert_cli_error(self, proc.stderr, 'crew_personas: ')

    # split

    def test_split_holds_out_the_newest_share_with_a_floor(self):
        cards = series(24)
        _, result = self.split(cards)
        self.assertEqual(result['heldout'], [f'c{i:02d}' for i in range(14, 24)])  # 25% of 24 is 6; floor 10
        self.assertEqual(result['train'], [f'c{i:02d}' for i in range(14)])
        _, result = self.split(series(60))
        self.assertEqual(len(result['heldout']), 15)
        self.assertEqual(result['heldout'][0], 'c45')

    def test_split_before_date_holds_out_everything_from_that_day(self):
        _, result = self.split(series(60), 'Sam', '--before', '2024-06-15')
        self.assertEqual(result['train'], ['c00', 'c01', 'c02', 'c03', 'c04'])
        self.assertEqual(len(result['heldout']), 55)

    def test_split_skips_asked_only_drafted_and_absent(self):
        cards = series(24)
        cards[3]['stances']['Sam'] = 'asked-only'
        cards[4]['agent_drafted'] = ['Sam']
        del cards[5]['stances']['Sam']
        cards[6]['stances']['Sam'] = 'own'
        _, result = self.split(cards)
        both = result['train'] + result['heldout']
        self.assertNotIn('c03', both)
        self.assertNotIn('c04', both)
        self.assertNotIn('c05', both)
        self.assertIn('c06', both)
        self.assertEqual(len(both), 21)

    def test_split_too_thin_for_a_backtest(self):
        path = self.write('cards.json', {'cards': series(19)})
        proc = self.run_cli('split', '--cards', path, '--person', 'Sam')
        self.assertEqual(proc.returncode, 3)
        self.assertIn('too few cards for Sam: 19 (need 20)', proc.stderr)

    # the ground-truth guard

    def test_split_refuses_cards_that_are_not_ground_truth(self):
        cards = series(24)
        cards[0]['source'] = 'simulated'
        path = self.write('cards.json', {'cards': cards})
        proc = self.run_cli('split', '--cards', path, '--person', 'Sam')
        self.assertEqual(proc.returncode, 2)
        self.assertIn('c00: source simulated is not ground truth', proc.stderr)

    def test_distil_in_gives_training_cards_and_refuses_simulated_evidence(self):
        path, result = self.split(series(24))
        split = self.write('split.json', result)
        proc = self.run_cli('distil-in', '--cards', path, '--split', split)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        rows = [json.loads(line) for line in proc.stdout.splitlines()]
        self.assertEqual([r['id'] for r in rows], result['train'])
        self.assertEqual(rows[0]['stance'], 'a')
        self.assertEqual(rows[0]['outcome_text'], 'Ship it all')
        good = self.write('reads.jsonl', json.dumps({'source': 'principal-read', 'text': 'Patient and careful.'}))
        proc = self.run_cli('distil-in', '--cards', path, '--split', split, '--evidence', good)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(json.loads(proc.stdout.splitlines()[-1])['source'], 'principal-read')
        panel = self.write('panel.jsonl', json.dumps({'source': 'simulated', 'seat': 'Alex', 'pick': 'a'}))
        proc = self.run_cli('distil-in', '--cards', path, '--split', split, '--evidence', panel)
        self.assertEqual(proc.returncode, 2)
        self.assertIn('panel.jsonl line 1: source simulated is not ground truth', proc.stderr)
        self.assertEqual(proc.stdout, '')

    # questions and score

    def test_questions_are_blind(self):
        cards = series(24)
        cards[20]['asked'] = [{'by': 'Alex', 'about': 'who holds the key'}]
        path, result = self.split(cards)
        proc = self.run_cli('questions', '--cards', path, '--split', self.write('split.json', result))
        self.assertEqual(proc.returncode, 0, proc.stderr)
        rows = [json.loads(line) for line in proc.stdout.splitlines()]
        self.assertEqual([r['id'] for r in rows], result['heldout'])
        self.assertEqual(set(rows[0]), {'id', 'date', 'category', 'question', 'context', 'options'})

    def test_score_tiers_categories_baselines_and_interval(self):
        cards = series(20)
        for c in cards[10:]:
            c['outcome'] = c['stances']['Sam']  # Sam always went with the outcome on held-out cards
        cards[19]['category'] = 'process'
        path, result = self.split(cards)
        self.assertEqual(result['heldout'], [f'c{i:02d}' for i in range(10, 20)])
        predictions = {f'c{i:02d}': {'pick': 'ab'[i % 2], 'confidence': 'high'} for i in range(10, 19)}
        predictions['c19'] = {'pick': 'a', 'confidence': 'low'}  # Sam picked b
        proc = self.run_cli('score', '--cards', path, '--split', self.write('split.json', result),
                            '--predictions', self.write('p.json', predictions), '--json')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        s = json.loads(proc.stdout)
        self.assertEqual(s['overall']['hits'], 9)
        self.assertEqual(s['overall']['n'], 10)
        self.assertEqual([round(x, 2) for x in s['overall']['interval']], [0.6, 0.98])
        self.assertEqual(s['overall']['uniform'], 0.5)
        self.assertEqual(s['overall']['sides_with_outcome'], 1.0)
        self.assertEqual(s['tiers']['high'], {'n': 9, 'hits': 9, 'accuracy': 1.0})
        self.assertEqual(s['categories']['scope']['high']['hits'], 9)
        self.assertEqual(s['categories']['process']['low'], {'n': 1, 'hits': 0, 'accuracy': 0.0})
        self.assertEqual(s['live'], ['scope'])
        proc = self.run_cli('score', '--cards', path, '--split', self.write('split.json', result),
                            '--predictions', self.write('p.json', predictions))
        self.assertIn('Overall: 9 of 10 (90%, 95% interval 60–98%)', proc.stdout)
        self.assertIn('| scope | 9/9 | – | – | yes |', proc.stdout)

    def test_score_needs_every_prediction_and_a_known_tier(self):
        path, result = self.split(series(20))
        split = self.write('split.json', result)
        proc = self.run_cli('score', '--cards', path, '--split', split, '--predictions', self.write('p.json', {}))
        self.assertEqual(proc.returncode, 2)
        self.assertIn('no prediction for c10', proc.stderr)
        predictions = {i: {'pick': 'a', 'confidence': 'sure'} for i in result['heldout']}
        proc = self.run_cli('score', '--cards', path, '--split', split, '--predictions', self.write('p.json', predictions))
        self.assertEqual(proc.returncode, 2)
        self.assertIn('confidence must be one of high, medium, low', proc.stderr)

    def test_own_counts_only_against_an_own_stance(self):
        cards = series(20)
        cards[10]['stances']['Sam'] = 'own'
        path, result = self.split(cards)
        predictions = {i: {'pick': 'own', 'confidence': 'medium'} for i in result['heldout']}
        proc = self.run_cli('score', '--cards', path, '--split', self.write('split.json', result),
                            '--predictions', self.write('p.json', predictions), '--json')
        self.assertEqual(json.loads(proc.stdout)['overall']['hits'], 1)

    # similar

    def test_similar_finds_training_cards_and_never_held_out_ones(self):
        cards = series(20)
        cards[2]['question'] = 'Should the choir rehearse on Tuesdays or Thursdays?'
        cards[15]['question'] = 'Should the choir rehearse on Tuesdays or Thursdays this term?'
        path, result = self.split(cards)
        proc = self.run_cli('similar', '--cards', path, '--person', 'Sam', '--split',
                            self.write('split.json', result), '--query', 'choir rehearse Thursdays', '--json')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        hits = json.loads(proc.stdout)
        self.assertEqual([h['id'] for h in hits], ['c02'])
        self.assertEqual(hits[0]['argued'], 'Ship it all')
        self.assertEqual(hits[0]['went'], 'Ship it all')
        # Without a split, every card is searchable: the held-out c15 now ranks too.
        proc = self.run_cli('similar', '--cards', path, '--person', 'Sam', '--query', 'choir rehearse Thursdays')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertIn('thread/c15', proc.stdout)
        # Each line leads with the card id, the handle a panel seat cites as evidence.
        self.assertIn('- c15 · ', proc.stdout)
        self.assertIn('argued: Ship half · went: Ship it all', proc.stdout)

    # the panel

    def panel_files(self):
        gold = card('gold', '2026-03-01', {'Sam': 'a', 'Alex': 'b', 'Jo': 'b', 'Robin': 'asked-only'},
                    outcome='unresolved', options=('Indoors', 'Outdoors', 'Cancel'),
                    asked=[{'by': 'Robin', 'about': 'who pays for the hall'}])
        cards = self.write('cards.json', {'cards': series(20) + [gold]})

        def seat(name, rnd, pick, agree=(), disagree=(), evidence=('c01',)):
            return {'seat': name, 'round': rnd, 'source': 'simulated', 'pick': pick, 'confidence': 'medium',
                    'reasons': [{'text': 'past pattern', 'evidence': list(evidence)}],
                    'asks': ['who pays'], 'expects_agree': list(agree), 'expects_disagree': list(disagree)}
        answers = [seat('Sam', 1, 'a', disagree=['Alex']), seat('Alex', 1, 'b', agree=['Jo']),
                   seat('Jo', 1, 'a'), seat('Robin', 1, 'c'),
                   seat('Sam', 2, 'a'), seat('Alex', 2, 'b'), seat('Jo', 2, 'b'), seat('Robin', 2, 'c')]
        return cards, gold, answers

    def test_panel_check_and_sheet(self):
        cards, gold, answers = self.panel_files()
        path = self.write('answers.json', answers)
        proc = self.run_cli('panel-check', '--answers', path, '--cards', cards, '--question', 'gold')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        proc = self.run_cli('panel-sheet', '--answers', path, '--cards', cards, '--question', 'gold')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertIn('Round 2', proc.stdout)
        self.assertIn('- a) Indoors: Sam', proc.stdout)
        self.assertIn('- b) Outdoors: Alex, Jo', proc.stdout)
        self.assertIn('simulated', proc.stdout)

    def test_panel_check_refuses_bad_answers(self):
        cards, _, answers = self.panel_files()
        answers[0]['source'] = 'card'
        answers[1]['pick'] = 'z'
        answers[2]['reasons'][0]['evidence'] = ['nope']
        answers[3]['confidence'] = 'sure'
        del answers[7]
        proc = self.run_cli('panel-check', '--answers', self.write('a.json', answers), '--cards', cards,
                            '--question', 'gold')
        self.assertEqual(proc.returncode, 1)
        for needle in ('Sam round 1: source must be simulated', 'Alex round 1: pick z',
                       'Jo round 1: evidence nope is not a card', 'Robin round 1: confidence sure',
                       'round 2 is missing Robin'):
            self.assertIn(needle, proc.stdout)

    def test_panel_check_refuses_evidence_from_the_question_itself(self):
        cards, _, answers = self.panel_files()
        answers[0]['reasons'][0]['evidence'] = ['gold']
        proc = self.run_cli('panel-check', '--answers', self.write('a.json', answers), '--cards', cards,
                            '--question', 'gold')
        self.assertEqual(proc.returncode, 1)
        self.assertIn('Sam round 1: evidence gold is the question itself', proc.stdout)

    def test_panel_score_against_the_gold_card(self):
        cards, _, answers = self.panel_files()
        proc = self.run_cli('panel-score', '--answers', self.write('a.json', answers), '--cards', cards,
                            '--question', 'gold', '--json')
        self.assertEqual(proc.returncode, 0, proc.stderr)
        s = json.loads(proc.stdout)
        # Round 1: Sam a (hit), Alex b (hit), Jo a (gold b, miss); Robin is asked-only, so unscored.
        self.assertEqual(s['1']['stances'], {'n': 3, 'hits': 2, 'accuracy': 0.6667})
        # Pairs among scored seats: Sam-Alex differ (gold differ: hit), Sam-Jo same (gold differ: miss),
        # Alex-Jo differ (gold same: miss).
        self.assertEqual(s['1']['pairs'], {'n': 3, 'hits': 1, 'accuracy': 0.3333})
        self.assertEqual(s['2']['stances']['hits'], 3)
        self.assertEqual(s['2']['pairs']['hits'], 3)
        self.assertEqual(s['1']['unscored'], ['Robin'])
        self.assertEqual(s['asked'], [{'by': 'Robin', 'about': 'who pays for the hall'}])


if __name__ == '__main__':
    unittest.main()
