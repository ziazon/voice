"""Invented corpora for the real-Python/Rust merge and split parity checks."""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from crew_fixtures import ask
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / 'scripts'))
import crew_decisions as crew


def question(n):
    from crew_decisions_test import ADA
    member = crew.load_member(ADA)
    record = ask(f'2026-09-0{n}T10:00:00Z', 'Which scope?', ['Small', 'Large'], 'Small (Recommended)')
    q = list(crew.questions_in(member, record, f's{n}', 'kit'))[0]
    q['ari_pick'] = None
    return q


def message(n, text='Keep the scope small'):
    return crew.message_record(f's{n}', f'2026-09-0{n}T10:01:00Z', 'kit', text, '')


def write_case(case, root):
    root.mkdir(parents=True, exist_ok=True)
    files = {'a.jsonl': [question(n) for n in range(1, 4)] + [message(1)]}
    flags = ['--in', 'a.jsonl']
    if case == 'two_machines':
        files['b.jsonl'] = [question(2), question(4), question(5)]
        flags += ['b.jsonl', 'a.jsonl']
    elif case == 'interview_train':
        files['decisions-interview.jsonl'] = [question(4), {**question(5), 'split': 'heldout'}]
        flags += ['--train', 'decisions-interview.jsonl']
    elif case in ('rounds_keep_split', 'rounds_only'):
        files['decisions-round-a.jsonl'] = [{**question(4), 'split': 'train'}]
        files['decisions-round-b.jsonl'] = [{**question(5), 'split': 'heldout'}]
        if case == 'rounds_only':
            files.pop('a.jsonl')
            flags = []
        flags += ['--rounds', 'decisions-round-a.jsonl', 'decisions-round-b.jsonl']
    elif case == 'messages_only':
        files['a.jsonl'] = [message(1), message(2)]
    elif case == 'refilter':
        files['a.jsonl'] += [message(2, 'my doctor changed the scope'),
                            message(3, '1234567'), message(2, 'my doctor changed the scope')]
    elif case == 'ties':
        files['a.jsonl'] = [{**question(1), 'id': key} for key in ['z', 'é', 'a', '😀']]
    elif case == 'unicode':
        files['a.jsonl'] += [message(2, '\u2028\u0085\u001c"\\é日😀' + ''.join(map(chr, range(32))))]
    elif case == 'shapes':
        files['a.jsonl'][0] = {'extra': [True, False, None, {'n': [0, -3, 18446744073709551615]}],
                              **files['a.jsonl'][0]}
    elif case not in ('whitespace_lines', 'bare_cr'):
        raise ValueError(case)
    for name, records in files.items():
        text = ''.join(json.dumps(r, ensure_ascii=False) + '\n' for r in records)
        if case == 'shapes':
            text = text.replace('"extra":', '"extra": "old", "extra":', 1)
        if case == 'whitespace_lines':
            text = '\r\n  \x1c\r\n' + text.rstrip('\n').replace('\n', '\r\n')
        if case == 'bare_cr':
            lines = text.split('\n')
            text = lines[0] + '\r\r' + lines[1] + '\r\n' + lines[2] + '\n' + lines[3] + '\n'
        (root / name).write_bytes(text.encode())
    return flags


if __name__ == '__main__':
    print(json.dumps(write_case(sys.argv[1], Path(sys.argv[2]))))
