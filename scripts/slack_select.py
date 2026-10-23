"""Pick episodes for the record pass and batch them, oldest first.

Usage: python3 -I scripts/slack_select.py --config CFG <work dir> [batch chars]
Batch chars defaults to config batch_chars.
Writes <work>/record-batches/NNN.txt (0600) and prints the counts.
"""
import argparse
import json
import re
import sys
from pathlib import Path

# Isolated Python omits the script directory; import only this sibling tool.
# Imports must not leave bytecode beside the tools or outside the work directory.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from slack_mine import load_config, private_write, validate_work


def pick(episode, rule):
    if episode['score'] >= rule['min_score']:
        return True
    return any(set(pair['ids']) <= set(episode['ids'])
               and episode['score'] >= pair['min_score'] for pair in rule['pairs'])


def main(config, work: Path, limit: int):
    validate_work(work)
    if limit is None:
        limit = config['batch_chars']
    if limit <= 0:
        raise ValueError('batch chars must be greater than zero')
    index = json.loads((work / 'episodes/index.json').read_text())
    chosen = {episode['id'] for episode in index if pick(episode, config['select'])}
    blocks = {}
    for path in sorted((work / 'episodes').glob('*/*.txt')):
        for block in re.split(r'(?m)^(?==== EPISODE )', path.read_text()):
            match = re.match(r'=== EPISODE (\S+) ', block)
            if match and match.group(1) in chosen:
                blocks[match.group(1)] = block.strip() + '\n'
    dated = sorted(blocks.items(), key=lambda item: float(item[0].split(':')[1]))
    out = work / 'record-batches'
    batch, size, number = [], 0, 0
    for _, block in dated:
        batch.append(block)
        size += len(block)
        if size >= limit:
            private_write(work, out / f'{number:03d}.txt', '\n'.join(batch))
            batch, size, number = [], 0, number + 1
    if batch:
        private_write(work, out / f'{number:03d}.txt', '\n'.join(batch))
        number += 1
    print(len(chosen), 'chosen,', len(blocks), 'found,', number, 'batches')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config', required=True, type=Path)
    parser.add_argument('work', type=Path)
    parser.add_argument('batch_chars', type=int, nargs='?')
    args = parser.parse_args()
    try:
        main(load_config(args.config.expanduser()), args.work.expanduser(), args.batch_chars)
    except (ValueError, OSError) as error:
        print('error: ' + ' '.join(str(error).splitlines()), file=sys.stderr)
        sys.exit(2)
