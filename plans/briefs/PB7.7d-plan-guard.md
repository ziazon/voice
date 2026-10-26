# PB7.7d brief: stop for the day when Claude Code says the plan is nearly used up

Written by Claude (lane V1), 2026-10-26. Spec: lane V's `PB7.7d` entry, rescoped by V-Q8's answer
(option 1): dollars only for v1 (PB7.7c, landed), plus a guard from the plan. Read the repo's
`AGENTS.md` first, then `builder/agent/src/claude.rs` (all of it), `builder/agent/src/ledger.rs`
(all of it), `builder/agent/src/text.rs`, `builder/agent/src/child.rs` (`Ran`),
`builder/agent/tests/common/mod.rs`, `builder/agent/tests/call.rs`, `builder/agent/tests/cap.rs`,
`builder/api/src/assistants.rs` (`claude_message`), `builder/api/src/building.rs` (`call_failure`,
`build_assistant_in`) and `builder/api/tests/building.rs` (how its stub `claude` is written and
patched with `h.stub(...)`).

Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`); security rules:
`~/.codex/skills/security-and-hardening/SKILL.md`; review bar: `~/.codex/skills/code-quality/SKILL.md`;
the one new sentence: `~/.codex/skills/plain-language/SKILL.md`. Run cargo with
`CARGO_NET_OFFLINE=true` and `-j 2`. Do not commit. Build no UI, no DTO change, no percentage
setting, no new app command.

## The problem

The app's only limit is in dollars, and Claude Code's dollar figure is a local estimate that doesn't
track a Pro or Max plan. Claude Code does tell a headless caller when the plan is nearly used up: in
`--output-format stream-json` (with `--verbose`, which `-p` requires for stream-json) it writes a line

```json
{"type":"rate_limit_event","rate_limit_info":{"status":"allowed_warning","resetsAt":1793003600,"rateLimitType":"five_hour","utilization":0.91},"uuid":"…","session_id":"…"}
```

when the plan's status changes. `status` is one of `allowed`, `allowed_warning` (nearing a limit) and
`rejected` (the limit is hit). Sources: `https://code.claude.com/docs/en/agent-sdk/python`
(`RateLimitEvent`, `RateLimitInfo`: "Emitted when rate limit status changes (for example, from
"allowed" to "allowed_warning")"), and Claude Code 2.1.292's own code, which builds the line as
`{type:"rate_limit_event",rate_limit_info:…,uuid,session_id}` with camelCase keys inside
`rate_limit_info` (only `status` is read here). `--output-format json`, which the app uses today,
prints only the final result object, so it never carries this event. The stream's last `result`
line is that same result object (`type`, `subtype`, `is_error`, `result`, `total_cost_usd`,
`usage`, `structured_output`): `https://code.claude.com/docs/en/headless` ("The last line of the
stream is a `result` message") and the TypeScript SDK's `SDKResultMessage`.

## Decisions (made, do not reopen)

1. **The call reads the stream.** In `Claude::call`, replace `"--output-format", "json"` with
   `"--output-format", "stream-json", "--verbose"` (in that order, at the same place in the argv).
   Nothing else in the argv changes. Update `expected_argv` in `builder/agent/tests/call.rs` to match.
2. **Reading stdout (`judge`).** stdout is newline-delimited JSON. Split on `\n`; skip lines that are
   empty or only white space. Parse each remaining line with `serde_json`.
   - **The plan's status** is the worst `rate_limit_info.status` over every line that parses as a
     JSON object with `"type": "rate_limit_event"`, where worst means `rejected` over
     `allowed_warning` over anything else. Only the two exact strings `"allowed_warning"` and
     `"rejected"` count; a missing `rate_limit_info`, a missing or non-string `status`, `allowed` or
     any other value is ignored. It is collected from every line that parses, even when the output
     as a whole is unreadable (below). Keep it as `Option<&'static str>` (`Some("allowed_warning")`,
     `Some("rejected")` or `None`).
   - **The reply** is the last line that parses as an object with `"type": "result"`. If any
     non-blank line is not a JSON object, or there is no result line, the outcome is `unreadable`
     with `text::FAILED`, as a non-object stdout is today. Everything after that (usage, cost,
     signed-out, `is_error`, `structured_output`) is read from the reply exactly as today.
   - Existing stubs print a single one-line result object; that is a valid stream of one line and
     must keep working unchanged.
   - `Ran::Stopped` and `Ran::CutOff` carry no stdout: their plan status is `None` (a known limit,
     below).
3. **A rejected plan explains a failed call.** When the plan status is `Some("rejected")` and the
   call would otherwise end `error`, `no-output` or `unreadable`, its outcome is `plan-limit` and
   its error is `text::PLAN_STOPPED`. `signed-out` keeps its own outcome and sentence. An `ok` call
   stays `ok` whatever the status: its answer was already paid for, so the caller gets it.
4. **The ledger keeps it.** `Line` (in `ledger.rs`) gains `pub plan: Option<&'a str>`; `judged`
   carries the plan status and `call` puts it on the call's ledger line. `render` writes a `"plan"`
   key holding that exact string only when it is `Some`; lines without a plan status look exactly as
   today (no `"plan"` key, no `null`). The `started` line has none.
5. **Stopped for the day.** `parse_line` also reads `plan`: absent → not stopped; the string
   `"allowed_warning"` or `"rejected"` → stopped; any other value (another string, a number, null)
   → the line is not a ledger line, so the ledger reads as damaged and nothing spends (fail closed,
   as a bad `cost_usd` does today). A private `fn plan_stopped_today(&self) -> Result<bool>` on
   `Session` is true when any of today's lines (same local `day` as `recorded_today` uses) is
   stopped. `Session::start` checks it first, under the same `held()` guard and before the cap
   check: when stopped it returns `Error::Invalid(text::PLAN_STOPPED.into())`, writes no ledger
   line, and the stub `claude` never runs. Lines from earlier days never stop a call. Keep one pass
   over the ledger in `start` if you can (for example, `parse_line` returning a small struct or a
   tuple `(day, cost, stopped)`), and no copy of the day logic.
6. **The sentence.** In `builder/agent/src/text.rs`:
   `pub const PLAN_STOPPED: &str = "Your Claude plan is nearly used up, so Persona Builder has stopped for today and left the rest for you. Run the build again tomorrow to carry on.";`
7. **It reaches the screens.** In `builder/api/src/assistants.rs` `claude_message`, add
   `text::PLAN_STOPPED` to the list of agent sentences passed through as they are. Nothing else in
   the api changes: `call_failure` already falls through to `claude_message` when the dollar limit
   isn't reached, and the build, the interview draft and the refresh all fail through it.
8. **The file's source comment.** Add to the doc comment at the top of `claude.rs` one line naming
   the two sources in "The problem" (the Python SDK page's `RateLimitEvent`, read 2026-10-26, and
   the headless page's stream-json section), in the style of the existing lines.

## Tests (red first; in `builder/agent/tests/`, a new `plan.rs` using `common`)

Add a helper in `common/mod.rs`, `pub fn rate_limit_line(status: &str) -> String`, that renders the
event line shown in "The problem" with the given `status`, and build multi-line replies by joining
lines with `\n` (end with `\n`). Give each test its own `Scratch`. Each must fail before the change
for the reason its name says:

1. `a_call_reads_the_result_from_a_stream` — reply = a `{"type":"system","subtype":"init"}` line, an
   `{"type":"assistant","message":{}}` line, then `ok_reply(&output)`: the call returns `output`,
   the usage and `0.0123`; the ledger line is `ok` with no `plan` key.
2. `a_warning_keeps_this_answer_and_stops_the_next_call` — `rate_limit_line("allowed_warning")`
   then `ok_reply`: the call returns `Ok` with its output; its ledger line has
   `"plan": "allowed_warning"`; a second `call` on the same session returns
   `Err` with `text::PLAN_STOPPED`, `stub.argv(1)` is `None`, and the ledger still has one line.
3. `a_rejected_plan_explains_the_failed_call` — `rate_limit_line("rejected")` then a result with
   `is_error: true`, `result: "You've hit your limit"`, `total_cost_usd: 0.0`: `Err` with
   `text::PLAN_STOPPED`; the ledger line's outcome is `plan-limit`, `plan` is `"rejected"`; the next
   call is refused without running.
4. `allowed_and_unknown_statuses_change_nothing` — events with `allowed`, `"weird"`, a missing
   `status` and a missing `rate_limit_info`, then `ok_reply`: `Ok`, no `plan` key, a second call
   runs (`stub.argv(1)` is `Some`).
5. `the_worst_status_wins` — `allowed_warning`, then `allowed`, then `ok_reply`: the line's `plan`
   is `"allowed_warning"`; with `rejected` then `allowed_warning`, it is `"rejected"`.
6. `the_stop_lasts_only_the_day` — a session on `fixed_clock()` records a warning; a second session
   (drop the first) on a clock one day (86 400 s) later makes a call that runs.
7. `a_warning_in_unreadable_output_still_stops` — `rate_limit_line("allowed_warning")` then the line
   `not json`: `Err` with `text::FAILED`, outcome `unreadable`, `plan` `"allowed_warning"`; the next
   call is refused with `PLAN_STOPPED`.
8. `a_stream_without_a_result_is_unreadable` — only a `{"type":"system","subtype":"init"}` line:
   `Err(text::FAILED)`, outcome `unreadable`.
9. `the_last_result_line_is_the_reply` — two result lines, the first `ok_reply` with output A, the
   second with output B: the call returns B.
10. `an_unknown_plan_value_in_the_ledger_stops_spending` — write a ledger line by hand (copy the
    shape of an existing ledger line in `cap.rs`, today's `day`, plus `"plan": "maybe"`): `spent_today`
    errs with `text::ledger_damaged(...)` and a call does not run. Repeat with `"plan": null`.

In `builder/api/tests/building.rs`, one test,
`a_build_stops_for_the_day_when_claude_code_says_the_plan_is_nearly_used`: patch the stub so every
call prints `rate_limit_line`'s JSON (status `allowed_warning`) on its own line before its normal
result; `build_assistant_in` fails with exactly `persona_builder_agent::text::PLAN_STOPPED`, and the
stub ran once (count the calls the way the file's other tests do, or read `runs.jsonl`: one line,
`"plan": "allowed_warning"`).

Mutation checks, each made with a `cp` backup and restored with `cp` (never `git checkout`), and
each must turn at least one test red: (a) drop the `"--verbose"` argument; (b) make `start` ignore
the plan stop; (c) treat `allowed` as stopping; (d) take the first result line instead of the last;
(e) accept any `plan` value in `parse_line`. Report each mutation and the test that caught it.

## Done means

- `bash scripts/ci-local.sh` passes in the worktree (it runs fmt, clippy with `-D warnings`, every
  test and the UI checks).
- No real person's name, handle or text in any file or fixture; the fixtures are invented.
- Report: the files changed, each test and its red reason, the mutation results, and anything in
  this brief you could not do as written (with why).

## Known limits (for the record, not to fix)

- A call that is stopped or cut off has no stdout, so a warning sent during it is not seen; the next
  call that finishes reads it again only if Claude Code sends it again (it sends on a change).
- The stop is per assistant (each has its own ledger): a second assistant learns it on its own first
  finished call.
- Claude Code reports the plan's status only when it changes, so the guard is as early as Claude
  Code's own warning level, which is not documented.
