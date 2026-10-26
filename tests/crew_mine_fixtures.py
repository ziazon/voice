#!/usr/bin/env python3
"""Invented Claude Code sessions for the Rust/Python mining parity tests."""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from crew_fixtures import ask, reply, said  # noqa: E402

WHEN = '2026-09-01T10:00:00Z'
FOLDER = '-Users-p-projects-app'


class Fixture:
    def __init__(self, root):
        self.projects = root
        root.mkdir(parents=True, exist_ok=True)

    def raw(self, data, folder=FOLDER, name='s1.jsonl'):
        path = self.projects / folder / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)

    def session(self, folder, name, *records):
        self.raw(b''.join(json.dumps(r, ensure_ascii=True).encode() + b'\n' for r in records),
                 folder, name + '.jsonl')


def ada_fixture(h):
    h.session(FOLDER, 's1',
              reply('2026-09-01T09:59:00Z', 'Shall I keep two copies of the review queue?'),
              said(WHEN, 'no, keep one source of truth for the review queue'),
              ask('2026-09-01T10:01:00Z', 'Where should the review queue live?', ['Repo', 'State'],
                  'Repo (Recommended · Ada)', member_pick=0),
              ask('2026-09-02T10:00:00Z', 'Which review queue order?', ['Oldest', 'Newest'], 'Newest (Ada)', member_pick=1),
              ask('2026-09-03T10:00:00Z', 'Split the review queue?', ['Yes', 'No'], 'split it by repo instead'),
              said('2026-09-03T10:01:00Z', 'You are a lane runner'),
              said('2026-09-03T10:02:00Z', 'my therapist moved the review queue meeting'),
              said('2026-09-03T10:03:00Z', 'the account number is 123456789'))
    h.session('-Users-p-projects-garden-club', 's2', said(WHEN, 'grant call'))
    h.session('-private-scratch', 's3', said(WHEN, 'budget call'))


def build(case, h):
    if case in ('ada_fixture', 'five_questions'):
        ada_fixture(h)
        if case == 'five_questions':
            h.session(FOLDER, 'extra', *[ask(f'2026-09-0{i}T10:00:00Z', f'Extra {i}?', ['A', 'B'], 'B') for i in (4, 5)])
    elif case == 'another_member':
        h.session('-Users-p-projects-grants', 's1',
                  ask(WHEN, 'Lead with aim 1?', ['Yes (Recommended · Ari)', 'No'], 'Yes (Recommended · Ari)', recommended=None),
                  ask('2026-09-02T10:00:00Z', 'Cut the budget table?', ['Cut', 'Keep (Ada)'], 'Keep (Ada)', recommended=None))
        h.session('-Users-p-projects-lab-dashboard', 's2', *[said(WHEN, s) for s in (
            'the grant call is due friday', 'my doctor moved it', 'student 12345678 asked for an extension',
            'write to someone@example.edu', 'Pasted: a syllabus', 'the IRB amendment goes first', 'You are kept for this member')])
        h.session('-Users-p-projects-hoa-board', 's3', said(WHEN, 'dues'))
    elif case == 'multi_select':
        h.session(FOLDER, 's1', *[ask(WHEN, f'Multi {i}?', labels, answer, multi=True, recommended=None)
            for i, (labels, answer) in enumerate([
                (['A', 'A longer', 'B'], 'A longer, B'), (['A', 'B'], 'A, own words, B'),
                (['A', 'B'], 'A,,, , B'), (['', 'A'], 'A, own words')])])
    elif case == 'list_answers':
        rows = [
            (['A', 'A longer', 'B'], ['A longer', 'B'], None, True),
            (['A', 'B'], ['B', 'A'], None, True),
            (['A', 'B'], ['A', '  own words, with a comma  '], None, True),
            (['Yes, keep it', 'No'], ['Yes, keep it'], None, True),
            (['A', 'B'], ['A (Recommended)'], 0, True),
            (['A', 'B'], ['A'], None, False),
            (['A', 'B'], [], None, True),
            (['A', 'B'], ['A', '   ', 'A'], None, True),
            (['A', 'B'], ['A', 'first own', 'B', ' second own '], None, True),
        ]
        h.session(FOLDER, 's1', *[
            ask(f'2026-09-01T10:0{i}:00Z', f'List {i}?', labels, answer,
                recommended=recommended, multi=multi)
            for i, (labels, answer, recommended, multi) in enumerate(rows)])
    elif case == 'single_select':
        h.session(FOLDER, 's1', ask(WHEN, 'Pick?', ['A', 'B'], 'A', recommended=None),
                  ask(WHEN, 'Own?', ['A', 'B'], ' \x1c own words \x1c ', recommended=None))
    elif case == 'notes':
        records = []
        for i, annotation in enumerate(['present', 'absent', None, {}, {'Q?': None}, {'Q?': {'notes': None}}]):
            r = ask(WHEN, 'Q?', ['A', 'B'], 'A', notes='review this', recommended=None)
            if annotation == 'absent':
                r['toolUseResult'].pop('annotations')
            elif annotation != 'present':
                r['toolUseResult']['annotations'] = annotation
            r['timestamp'] = f'2026-09-01T10:0{i}:00Z'
            records.append(r)
        h.session(FOLDER, 's1', *records)
    elif case == 'tolerant_questions':
        records = []
        for mode in ('missing', 'number', 'null_answer', 'missing_answer', 'string_result', 'missing_answers'):
            r = ask(WHEN, 'Q?', ['A'], 'A', recommended=None)
            result = r['toolUseResult']
            if mode == 'missing': result['questions'][0].pop('question')
            elif mode == 'number': result['questions'][0]['question'] = 7
            elif mode == 'null_answer': result['answers']['Q?'] = None
            elif mode == 'missing_answer': result['answers'] = {}
            elif mode == 'string_result': r['toolUseResult'] = 'ignored'
            else: result.pop('answers')
            records.append(r)
        h.session(FOLDER, 's1', *records)
    elif case == 'typed_text':
        records = [said(WHEN, '<system-reminder>ignore</system-reminder>keep this'),
                   said(WHEN, [{'type': 'text', 'text': '<dropped>'}, {'type': 'image'}, {'type': 'text', 'text': 'keep block'}])]
        for flag in ('isMeta', 'isCompactSummary'):
            for value in (1, 'x', [0], 0, '', []): records.append(said(WHEN, 'flag case', **{flag: value}))
        records += [said(WHEN, 'origin absent', origin=None), said(WHEN, 'origin null', origin=None),
                    said(WHEN, 'tool origin', origin={'kind': 'tool'})]
        records[-2]['origin'] = None
        records += [said(WHEN, s) for s in ('Try Again', 'CONTINUE', 'You are pasted instructions', '日' * 4000, '日' * 4001)]
        h.session(FOLDER, 's1', *records)
    elif case == 'context':
        records = [reply(WHEN, '日' * 310 + '😀'), said(WHEN, 'first'), reply(WHEN, ' \t '), said(WHEN, 'second'),
                   reply(WHEN, '\x1c'), said(WHEN, 'third'), reply(WHEN, 'timestamp absent'), said(WHEN, 'fourth')]
        records[-2].pop('timestamp')
        records += [reply(WHEN, 'string'), said(WHEN, 'fifth'), reply(WHEN, 'object'), said(WHEN, 'sixth')]
        records[-4]['message']['content'] = 'a string reply'
        records[-2]['message']['content'] = {'type': 'text', 'text': 'ignored object'}
        h.session(FOLDER, 's1', *records)
    elif case == 'bytes':
        enc = lambda r: json.dumps(r, ensure_ascii=False).encode()
        h.raw(enc(said(WHEN, 'CRLF')) + b'\r\n\nnot JSON\n' + enc(said(WHEN, 'badXbyte')).replace(b'X', b'\xff') + b'\n' +
              enc(said(WHEN, 'raw\u2028separator')) + b'\n' + enc(said(WHEN, 'raw\x85separator')) + b'\n' +
              enc(said(WHEN, 'one')) + b'\x1c' + enc(said(WHEN, 'two')) + b'\n' + enc(said(WHEN, 'last')))
    elif case == 'timestamps':
        records = [said(WHEN, 'missing'), said('', 'empty'), said(None, 'null')]
        records[0].pop('timestamp')
        h.session(FOLDER, 's1', *records)
    elif case in ('folder_names', 'dotted_home', 'folders'):
        names = ['-Users-p-projects-course--claude-worktrees-z', '-Users-p-projects-course--claude-worktrees-a',
                 '-Users-someone-projects-x', '-home-x-y', '-Users-p--claude', 'projects-short',
                 'a-b', 'a.b', 'a_b', 'B', 'a', '-Users-p-projects-garden-club', '-Users-jane-doe-projects-course']
        for folder in names:
            h.session(folder, 'a.b', said(WHEN, 'folder decision'))
            h.session(folder, '.hidden', said(WHEN, 'hidden session decision'))
            h.raw(json.dumps(said(WHEN, 'ignored text file')).encode() + b'\n', folder, 'notes.txt')
    elif case == 'duplicates':
        h.session(FOLDER, 's1', said(WHEN, 'same message'), said(WHEN, 'same message'))
    elif case == 'empty':
        pass
    elif case == 'no_sessions':
        (h.projects / FOLDER).mkdir()
    elif case.startswith('error_'):
        r = said(WHEN, 'private record sentinel')
        if case == 'error_record': r = [1, 2]
        elif case == 'error_message': r = reply(WHEN, 'private record sentinel'); r['message'] = None
        elif case == 'error_timestamp': r['timestamp'] = 5
        elif case == 'error_content': r['message']['content'] = 7
        elif case == 'error_origin': r['origin'] = 'web'
        elif case in ('error_options', 'error_answer'):
            r = ask(WHEN, 'private record sentinel', ['A'], 'A', recommended=None)
            if case == 'error_options': r['toolUseResult']['questions'][0]['options'] = {'a': 1}
            else: r['toolUseResult']['answers']['private record sentinel'] = ['A', 7]
        else: raise ValueError('unknown error case')
        h.session(FOLDER, 's1', r)
    elif case == 'private_overlay':
        h.session('-Users-p-projects-grants', 's1', *[said(WHEN, s) for s in (
            'the grant call is due friday', 'my doctor moved it', 'my cat is at the vet',
            'the door pin 1234 changed', 'Pasted: a syllabus', 'Quoted: an email')])
        h.session('-Users-p-projects-hoa-board', 's2', said(WHEN, 'dues'))
        h.session('-Users-p-garden-club', 's3', said(WHEN, 'the plot rota'))
    else:
        raise ValueError('unknown fixture case')


if __name__ == '__main__':
    build(sys.argv[1], Fixture(Path(sys.argv[2])))
