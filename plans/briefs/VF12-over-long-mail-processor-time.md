# VF12: the over-long mail test measures processor time, not the clock

Test only. One file: `tests/crew_extra_sources_test.py`. Commit nothing. Standard library only.
Skills: `~/.codex/skills/tdd-skill/SKILL.md` (red first), `~/.codex/skills/code-quality/SKILL.md`.

## The failure

`Mail.test_an_over_long_mail_is_skipped_before_the_filters_run` mines one 120,000-character sent mail
through a whole `crew sources mine` run (`self.mine(...)`, which starts the CLI as a subprocess and waits
for it) and asserts `time.monotonic() - started < 5`. At load 21 to 32 on an 8-core Mac it took 7.0 s
inside `scripts/ci-local.sh` and 8.5 s alone, so it fails though nothing is wrong: the clock counts the
time the subprocess spent waiting for a turn on the processor.

The bound guards one thing: commit `ba6b6fb` skips a mail longer than `MAX_MESSAGE_CHARS` before the
secret and personal filters run, because those patterns run slow on a long token and Python 3.9's regex
engine never yields to the `crew_mail.budget` timer. A slow pattern costs processor time; a busy computer
costs only clock time.

## The change

In that test, replace the clock with the processor time the mining run's subprocesses used:

```python
import resource  # at the top, with the other imports, in alphabetical order

def test_an_over_long_mail_is_skipped_before_the_filters_run(self):
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    stdout, records = self.mine([...unchanged...])
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    used = (after.ru_utime - before.ru_utime) + (after.ru_stime - before.ru_stime)
    # Processor time, not the clock: a slow pattern costs it, a busy computer doesn't.
    self.assertLess(used, 5, f"the mining run used {used:.1f} s of processor time")
    ...the two existing assertions, unchanged...
```

`RUSAGE_CHILDREN` counts only children the test process has waited for; `self.mine` waits for the
CLI before it returns. Keep the same 5-second bound. Remove the now-unused `time` import only if
nothing else in the file uses it (it is used elsewhere today: keep it). Change no other test, no
assertion text other than this one, and no file under `scripts/`.

## Proof (report each)

1. **Red, the failure as found:** before your change, run
   `python3 tests/crew_extra_sources_test.py Mail.test_an_over_long_mail_is_skipped_before_the_filters_run`
   and report the result and time. Never start programs to load the computer; the reviewer reruns red and
   green on the busy host. If it passes for you, say so plainly.
2. **Green:** after the change, the same command passes. Report the processor time
   the run used (add a temporary `print`, then remove it).
3. **Mutation:** copy `scripts/crew_decisions.py` to a backup with `cp`, delete the four-line
   `if len("\n\n".join(paragraphs)) > MAX_MESSAGE_CHARS: return "too long"` block and its comment, run
   the test, report which assertion failed, then restore with `cp` and confirm with `cmp` that the file
   is byte-for-byte the original. Never `git checkout`.
4. `python3 tests/crew_extra_sources_test.py` (the whole file) passes, and
   `git diff --stat` shows only this test file.
