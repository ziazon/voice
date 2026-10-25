"""Invented members and files for the real-Python/Rust parity checks of the build's steps.

`python3 -I tests/crew_steps_fixtures.py <case> <member> <state dir>` writes the member config
(`member.json`) and the case's input files into the state dir, and prints the runs to compare as one
JSON list: `{"cmd": …, "args": […]}`, where an argument `@in:NAME` is an input file in the state
dir, `@out:NAME` an output (the Python writes `py-NAME` beside the Rust's `NAME`) and `@doc:NAME` a
file both read by its absolute path.
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'scripts'))
import crew_decisions as crew  # noqa: E402
from crew_decisions_test import ADA, ARI  # noqa: E402

CODEBOOK = {"categories": ["scope", "methods", "writing-voice", "other"],
            "domains": ["research", "grants", "teaching", "other"],
            "escalates": ["send-external", "submit", "privacy"],
            "signals": ["ruling", "correction", "direction", "ack", "other"],
            "groups": {"g1-work": ["scope", "methods"], "g2-words": ["writing-voice", "other"],
                       "g3-later": ["methods"]}}


def member_config(which):
    config = dict(ADA if which == 'ada' else ARI)
    config['state_dir'] = f"~/.claude/state/{config['slug']}"
    config['codebook'] = CODEBOOK
    return config


def question(m, word, day, text, options, answer, recommended=0, member_pick=None, multi=False, notes=None,
             header='Scope', project='kit', long=False):
    """A question as `mine` writes it, its labels marked with the member's own mark word."""
    labels = [f'{label} (Recommended · {word})' if i == recommended and i == member_pick else
              f'{label} (Recommended)' if i == recommended else
              f'{label} ({word})' if i == member_pick else label for i, label in enumerate(options)]
    when = f'2026-09-{day:02d}T10:00:00.000Z'
    result = {'questions': [{'question': text, 'header': header, 'multiSelect': multi,
                             'options': [{'label': label, 'description': (f'about {label} ' * (20 if long else 1)).strip()
                                          if i else ''} for i, label in enumerate(labels)]}],
              'answers': {text: answer.replace('{word}', word)}}
    if notes:
        result['annotations'] = {text: {'notes': notes}}
    return crew.questions_in(m, {'timestamp': when, 'toolUseResult': result}, f's{day}', project)[0]


def message(day, text, project='kit', hour=11):
    return crew.message_record(f's{day}', f'2026-09-{day:02d}T{hour}:00:00.000Z', project, text,
                               'Shall I keep the review queue as it is?')


def corpus(m, word):
    q = lambda *a, **k: question(m, word, *a, **k)  # noqa: E731
    records = [
        q(1, 'Keep two copies of the review queue?', ['One copy', 'Two copies'],
          'One copy (Recommended · {word})', member_pick=0),
        q(2, 'Trim the methods section by half?', ['Trim', 'Keep'], 'Keep', long=True),
        q(3, 'Which reviewers should see the draft?', ['Wren', 'Sam', 'Lee'], 'Wren (Recommended), Lee',
          multi=True, header='Review'),
        q(4, 'Name the new folder?', ['notes', 'drafts'], 'notes', recommended=None),
        q(5, 'Ship the review queue change today?', ['Ship', 'Wait'], 'Wait until the partner review is done'),
        q(6, 'Run the slow tests before the release?', ['Run all', 'Run the fast ones'], 'Run all',
          recommended=1, member_pick=1, notes='the release can wait a day for the slow tests'),
        q(7, 'Garder le café à côté de la file, 日本 😀?', ['Oui', 'Non'], 'Oui (Recommended)', project='café'),
        q(8, 'Keep the review queue small this week?', ['Small', 'Large'], 'Small (Recommended)'),
        q(9, 'Merge the two review queues into one?', ['Merge', 'Keep apart'], 'Keep apart ({word})',
          member_pick=1),
        q(10, 'Which checks gate the release review?', ['Lint', 'Tests', 'Docs'], 'Tests (Recommended), Docs',
          recommended=1, multi=True),
        q(11, 'Split the queue by partner?', ['Split', 'Keep one'], 'Split (Recommended)'),
        q(12, 'Delay the queue review to Friday?', ['Delay', 'Keep Monday'], 'ask the partner first'),
        q(13, 'Send the grant summary to the partner?', ['Send', 'Hold'], 'Send (Recommended · {word})',
          member_pick=0, header='Send'),
        q(14, 'Which sections go in the partner summary?', ['Aims', 'Budget', 'Team'], 'Budget, Team',
          multi=True),
        message(1, 'always keep the review queue small and the partner note first in every reply'),
        message(2, 'ok go'),
        message(2, 'eight words exactly about the partner review queue', hour=12),
        message(3, 'the release should never wait more than one day for a review of the queue'),
        message(4, 'le café du matin reste à côté de la file de revue 日本 😀 toujours et encore'),
        message(5, 'fix the typo in the second aim before you send anything to the partner please'),
        message(9, 'Thanks'),
        message(12, 'keep the partner summary short: the budget and the team, nothing about the aims'),
        message(13, 'write "keep\\the\tqueue" — with the quote marks — in the reply to the partner today'),
    ]
    records.sort(key=lambda r: (r['timestamp'], r['id']))
    crew.mark_split(records, 0.4, None)
    return records


def write_jsonl(path, records):
    path.write_text(''.join(json.dumps(r, ensure_ascii=False) + '\n' for r in records))


def labels_for(records, held_reason=False):
    categories = CODEBOOK['categories']
    out = {}
    for n, r in enumerate(records):
        label = {'category': categories[n % len(categories)], 'domain': CODEBOOK['domains'][n % 4]}
        if r['split'] == 'train':
            label['reason'] = f'keeps the partner need up front, case {n}'
            label.update({'escalates': ['submit'] if n % 3 == 0 else []} if r['kind'] == 'question'
                         else {'signal': CODEBOOK['signals'][n % 5]})
        elif held_reason:
            label['reason'] = 'peeked'
        out[r['id']] = label
    return out


def predictions_for(m, records):
    out = {}
    tiers = ['high', 'medium', 'low']
    for n, r in enumerate(x for x in records if x['kind'] == 'question' and x['split'] == 'heldout'):
        tier = tiers[n % 3]
        if r['outcome'] == 'own_words':
            pick = 'own' if n % 2 == 0 else r['options'][0]['label']
        elif r['multiSelect']:
            pick = [p + ' (Recommended)' for p in r['picks']] if n % 2 == 0 else r['picks'][:1]
        elif n % 4 == 3:
            pick = r['options'][-1]['label']
        else:
            pick = crew.bare(m, r['picks'][0]) + '  ' if r['picks'] else 'none'
        out[r['id']] = {'pick': pick, 'confidence': tier, 'why': 'the partner comes first'}
    return out


def answers_of(m, records):
    held = [r for r in records if r['kind'] == 'question' and r['split'] == 'heldout']
    return [{k: r[k] for k in crew.answer_fields(m)} for r in held]


def shares_case(m, state):
    """Shares and percentages at their rounding edges: 1/8, 3/8, 1/32 and 1/200."""
    records = []
    def add(category, took, total):
        for i in range(total):
            records.append({'id': f'question:{category}-{i:03d}', 'kind': 'question',
                            'timestamp': f'2026-08-{1 + i % 28:02d}T09:00:00.000Z', 'project': 'kit',
                            'header': 'Scope', 'question': f'Pick {i}?', 'multiSelect': False,
                            'options': [{'label': 'A (Recommended)', 'description': ''},
                                        {'label': 'B', 'description': ''}],
                            'recommended': 'A (Recommended)', m.pick_field: None,
                            'picks': ['A (Recommended)' if i < took else 'B'], 'own_words': '', 'notes': '',
                            'outcome': 'took_rec' if i < took else 'other_option', 'split': 'train'})
    add('eighth', 1, 8)
    add('three-eighths', 3, 8)
    add('thirty-second', 1, 32)
    add('two-hundredth', 1, 200)
    write_jsonl(state / 'decisions.jsonl', records)
    (state / 'labels.json').write_text(json.dumps({r['id']: {'category': r['id'].split(':')[1].rsplit('-', 1)[0]}
                                                   for r in records}))
    answers = [{'id': r['id'], 'recommended': r['recommended'], m.pick_field: None, 'picks': r['picks'],
                'own_words': '', 'notes': '', 'outcome': r['outcome'], 'multiSelect': False} for r in records]
    write_jsonl(state / 'answers.jsonl', answers)
    (state / 'predictions.json').write_text(json.dumps(
        {r['id']: {'pick': 'A', 'confidence': ['high', 'medium', 'low'][i % 3]} for i, r in enumerate(records)}))
    return [{'cmd': 'tally', 'args': ['--corpus', '@in:decisions.jsonl', '--labels', '@in:labels.json']},
            {'cmd': 'tally', 'args': ['--corpus', '@in:decisions.jsonl', '--labels', '@in:labels.json', '--json']},
            {'cmd': 'score', 'args': ['--answers', '@in:answers.jsonl', '--labels', '@in:labels.json',
                                      '--predictions', '@in:predictions.json']},
            {'cmd': 'score', 'args': ['--answers', '@in:answers.jsonl', '--labels', '@in:labels.json',
                                      '--predictions', '@in:predictions.json', '--json']}]


def round_files(state):
    questions = {'questions': [
        {'n': 1, 'question': 'Lead with the partner need?', 'header': 'Aims',
         'options': [{'label': 'Yes', 'description': 'the need first'}, {'label': 'No'}], 'recommended': 'A'},
        {'n': 2, 'question': 'Which parts go first?', 'multiSelect': True,
         'options': [{'label': 'Aims'}, {'label': 'Budget'}, {'label': 'Team', 'description': ''}],
         'recommended': 'B'},
        {'n': 3, 'question': 'Send it this week?', 'options': [{'label': 'Send'}, {'label': 'Wait'}]},
        {'n': 4, 'question': 'Keep the café line, 日本 😀?', 'options': [{'label': 'Keep'}, {'label': 'Cut'}],
         'recommended': None},
        {'n': 5, 'question': 'Tell the team about my therapist and my doctor?',
         'options': [{'label': 'Tell'}, {'label': 'Not'}]},
        {'n': 6, 'question': 'Use the account 1234567890?', 'options': [{'label': 'Use'}, {'label': 'Skip'}]},
        {'n': 12, 'question': 'Ask again next month?', 'options': [{'label': 'Ask'}, {'label': 'Stop'}],
         'recommended': 'B'},
    ]}
    answers = {'answered': '2026-10-02', 'answers': {
        '1': {'pick': 'A', 'notes': 'the need sets the tone'},
        '2': {'pick': ['C', 'B', 'C']},
        '3': {'own_words': '  wait for the partner  '},
        '4': {'pick': ['A'], 'own_words': '   '},
        '5': {'pick': 'A'},
        '6': {'pick': 'B'},
        '12': {'own_words': '\u3000 \t'},
    }}
    (state / 'round-questions.json').write_text(json.dumps(questions, ensure_ascii=False))
    (state / 'round-answers.json').write_text(json.dumps(answers))


def base_case(m, word, state):
    records = corpus(m, word)
    write_jsonl(state / 'decisions.jsonl', records)
    labels = labels_for(records)
    (state / 'labels.json').write_text(json.dumps(labels, ensure_ascii=False))
    (state / 'labels-peeked.json').write_text(json.dumps(labels_for(records, held_reason=True)))
    missing = dict(labels)
    missing.pop(next(i for i, r in zip(labels, records) if r['split'] == 'train'))
    missing['question:extra'] = {'category': 'scope'}
    (state / 'labels-missing.json').write_text(json.dumps(missing))
    unicode = {i: dict(l, category=['café', '日本', '😀', 'Zed', 'apple'][n % 5]) for n, (i, l) in enumerate(labels.items())}
    (state / 'labels-unicode.json').write_text(json.dumps(unicode, ensure_ascii=False))
    write_jsonl(state / 'answers.jsonl', answers_of(m, records))
    (state / 'predictions.json').write_text(json.dumps(predictions_for(m, records), ensure_ascii=False))

    train = [r for r in records if r['split'] == 'train']
    held = [{'kind': 'question', **{k: r[k] for k in crew.QUESTION_FIELDS}}
            for r in records if r['split'] == 'heldout' and r['kind'] == 'question']
    write_jsonl(state / 'chunk-train.jsonl', train[:6])
    write_jsonl(state / 'chunk-held.jsonl', held)
    chunk_labels = {r['id']: labels[r['id']] for r in train[:6]}
    (state / 'chunk-labels.json').write_text(json.dumps(chunk_labels))
    ids = list(chunk_labels)
    bad = {i: dict(l) for i, l in chunk_labels.items()}
    bad.pop(ids[0])
    bad['question:extra'] = {'category': 'scope'}
    bad[ids[1]]['category'] = 'nope'
    bad[ids[1]]['domain'] = 'nope'
    bad[ids[2]]['reason'] = ' '.join(['word'] * 26)
    bad[ids[3]]['reason'] = '   '
    bad[ids[4]]['reason'] = 7
    for i, r in zip(ids, train[:6]):
        if r['kind'] == 'question' and i != ids[0]:
            bad[i]['escalates'] = ['submit', 'nope']
        elif r['kind'] == 'message' and i != ids[0]:
            bad[i]['signal'] = 'nope'
    (state / 'chunk-labels-bad.json').write_text(json.dumps(bad))
    write_jsonl(state / 'chunk-all.jsonl', train)
    (state / 'chunk-labels-all-bad.json').write_text(json.dumps(
        {r['id']: {'category': 'nope', 'domain': 'nope', 'escalates': ['nope'], 'signal': 'nope'} for r in train}))
    many = {f'question:x{n:02d}': {'category': 'nope'} for n in range(45)}
    (state / 'chunk-labels-many.json').write_text(json.dumps(many))
    (state / 'held-labels.json').write_text(json.dumps({r['id']: {'category': 'scope', 'domain': 'grants'}
                                                         for r in held}))
    (state / 'held-labels-reason.json').write_text(json.dumps(
        {r['id']: {'category': 'scope', 'domain': 'grants', 'reason': 'peeked'} for r in held}))

    (state / 'doc.md').write_text('# Decisions\n\nAlways keep the Review queue small, and the partner note '
                                  "first. Don't wait.\n")
    (state / 'doc-long.md').write_text(' '.join(r.get('text') or r['question'] for r in records))
    (state / 'doc-options.md').write_text('We wrote it about Keep about Keep about Keep about Keep.\n')
    (state / 'doc-clean.md').write_text('# Decisions\n\nPrefer one short queue; ask before sending.\n')
    write_jsonl(state / 'more.jsonl', [message(20, 'prefer one short queue ask before sending anything at all')])
    round_files(state)

    corpus_in = ['--corpus', '@in:decisions.jsonl']
    runs = [
        {'cmd': 'chunk', 'args': [*corpus_in, '--out-dir', '@out:chunks', '--size', '4']},
        {'cmd': 'chunk', 'args': [*corpus_in, '--out-dir', '@out:chunks', '--size', '300']},
        {'cmd': 'chunk', 'args': [*corpus_in, '--out-dir', '@out:one', '--size', '1']},
        {'cmd': 'check-labels', 'args': ['--chunk', '@in:chunk-train.jsonl', '--labels', '@in:chunk-labels.json',
                                         '--mode', 'train']},
        {'cmd': 'check-labels', 'args': ['--chunk', '@in:chunk-train.jsonl',
                                         '--labels', '@in:chunk-labels-bad.json', '--mode', 'train']},
        {'cmd': 'check-labels', 'args': ['--chunk', '@in:chunk-train.jsonl',
                                         '--labels', '@in:chunk-labels-many.json', '--mode', 'train']},
        {'cmd': 'check-labels', 'args': ['--chunk', '@in:chunk-all.jsonl',
                                         '--labels', '@in:chunk-labels-all-bad.json', '--mode', 'train']},
        {'cmd': 'check-labels', 'args': ['--chunk', '@in:chunk-held.jsonl', '--labels', '@in:held-labels.json',
                                         '--mode', 'heldout']},
        {'cmd': 'check-labels', 'args': ['--chunk', '@in:chunk-held.jsonl',
                                         '--labels', '@in:held-labels-reason.json', '--mode', 'heldout']},
        {'cmd': 'check-labels', 'args': ['--chunk', '@in:chunk-train.jsonl', '--labels', '@in:chunk-labels.json',
                                         '--mode', 'heldout']},
        {'cmd': 'tally', 'args': [*corpus_in, '--labels', '@in:labels.json']},
        {'cmd': 'tally', 'args': [*corpus_in, '--labels', '@in:labels.json', '--json']},
        {'cmd': 'tally', 'args': [*corpus_in, '--labels', '@in:labels-unicode.json']},
        {'cmd': 'tally', 'args': [*corpus_in, '--labels', '@in:labels-unicode.json', '--json']},
        {'cmd': 'tally', 'args': [*corpus_in, '--labels', '@in:labels-missing.json']},
        {'cmd': 'tally', 'args': [*corpus_in, '--labels', '@in:labels-peeked.json']},
        {'cmd': 'distil-in', 'args': [*corpus_in, '--labels', '@in:labels.json', '--out-dir', '@out:distil']},
        {'cmd': 'score', 'args': ['--answers', '@in:answers.jsonl', '--labels', '@in:labels.json',
                                  '--predictions', '@in:predictions.json']},
        {'cmd': 'score', 'args': ['--answers', '@in:answers.jsonl', '--labels', '@in:labels-unicode.json',
                                  '--predictions', '@in:predictions.json', '--json']},
        {'cmd': 'score', 'args': ['--answers', '@in:answers.jsonl', '--labels', '@in:labels.json',
                                  '--predictions', '@in:labels.json']},
        {'cmd': 'shingle', 'args': ['--doc', '@doc:doc.md', '--corpus', '@in:decisions.jsonl']},
        {'cmd': 'shingle', 'args': ['--doc', '@doc:doc.md', '--corpus', '@in:decisions.jsonl', '--n', '3']},
        {'cmd': 'shingle', 'args': ['--doc', '@doc:doc-clean.md', '--corpus', '@in:decisions.jsonl']},
        {'cmd': 'shingle', 'args': ['--doc', '@doc:doc-clean.md', '--corpus', '@in:decisions.jsonl',
                                    '@in:more.jsonl', '--n', '4']},
        {'cmd': 'shingle', 'args': ['--doc', '@doc:doc.md', '--corpus', '@in:decisions.jsonl', '--n', '1']},
        {'cmd': 'shingle', 'args': ['--doc', '@doc:doc-options.md', '--corpus', '@in:decisions.jsonl']},
        {'cmd': 'shingle', 'args': ['--doc', '@doc:doc-long.md', '--corpus', '@in:decisions.jsonl', '--n', '1']},
        {'cmd': 'round', 'args': ['--name', 'pilot', '--questions', '@in:round-questions.json',
                                  '--answers', '@in:round-answers.json', '--out', '@out:decisions-round-pilot.jsonl']},
        {'cmd': 'round', 'args': ['--name', 'p-2', '--questions', '@in:round-questions.json',
                                  '--answers', '@in:round-answers.json', '--heldout', '--out', '@out:held.jsonl']},
    ]
    for query in ['review queue partner', 'Keep the REVIEW queue small?', 'café file revue toujours',
                  'release tests slow', 'nothing in common here', 'queue', 'ada always keep',
                  'eight words partner']:
        for extra in [[], ['--json'], ['--k', '1'], ['--k', '2', '--json']]:
            runs.append({'cmd': 'similar', 'args': [*corpus_in, '--query', query, *extra]})
    return runs


def empty_case(m, state):
    (state / 'decisions.jsonl').write_text('')
    (state / 'labels.json').write_text('{}')
    (state / 'answers.jsonl').write_text('')
    (state / 'predictions.json').write_text('{}')
    (state / 'doc.md').write_text('')
    corpus_in = ['--corpus', '@in:decisions.jsonl']
    return [{'cmd': 'chunk', 'args': [*corpus_in, '--out-dir', '@out:chunks']},
            {'cmd': 'tally', 'args': [*corpus_in, '--labels', '@in:labels.json']},
            {'cmd': 'tally', 'args': [*corpus_in, '--labels', '@in:labels.json', '--json']},
            {'cmd': 'distil-in', 'args': [*corpus_in, '--labels', '@in:labels.json', '--out-dir', '@out:distil']},
            {'cmd': 'score', 'args': ['--answers', '@in:answers.jsonl', '--labels', '@in:labels.json',
                                      '--predictions', '@in:predictions.json']},
            {'cmd': 'score', 'args': ['--answers', '@in:answers.jsonl', '--labels', '@in:labels.json',
                                      '--predictions', '@in:predictions.json', '--json']},
            {'cmd': 'similar', 'args': [*corpus_in, '--query', 'review queue']},
            {'cmd': 'similar', 'args': [*corpus_in, '--query', 'review queue', '--json']},
            {'cmd': 'shingle', 'args': ['--doc', '@doc:doc.md', '--corpus', '@in:decisions.jsonl']}]


def main(case, which, state):
    state = Path(state)
    state.mkdir(parents=True, exist_ok=True)
    config = member_config(which)
    (state / 'member.json').write_text(json.dumps(config))
    m = crew.load_member(config)
    word = config['mark_word']
    if case == 'base':
        runs = base_case(m, word, state)
    elif case == 'empty':
        runs = empty_case(m, state)
    elif case == 'shares':
        runs = shares_case(m, state)
    else:
        raise ValueError(case)
    print(json.dumps(runs, ensure_ascii=False))


if __name__ == '__main__':
    main(*sys.argv[1:4])
