# PB8.7a fix-up 4: read CLAUDE.md's Voice blocks in one pass

From the repeat Verifier (Opus) on `0ba9a77`. Same skills and rules as `plans/briefs/PB8.7a-look-before-making.md`.
Commit nothing. Red test first where the brief says so. Change only `builder/core/src/existing.rs`.

1. **One pass over the end markers** (`find_lines`). Fix-up 3 made each `<!-- voice:begin <id> -->` line search
   every later line for its own end, so the work grows with the square of the line count: a `CLAUDE.md` near
   the 4 MiB cap made only of unclosed begin lines (distinct ids) stalls `voice new` and the app's
   `new_assistant` for a long time.
   - Before the loop, build one map from each end id to the ascending line indices where it appears: a line
     whose trimmed text is `<!-- voice:end <id> -->` (strip the prefix `<!-- voice:end ` and the suffix ` -->`,
     exactly as the begin side does today). `std::collections::HashMap<&str, Vec<usize>>` is fine.
   - For a begin with id `<id>` at `index`, `block_end` is the first index in that id's list greater than
     `index` (`partition_point`), or `None` when there is none.
   - Everything else stays exactly as it is: an unclosed begin hides nothing, an end of another id closes
     nothing, begin and end lines are never counted, the block's end line is skipped.
   - Red first: `many_unclosed_voice_begins_are_read_in_one_pass`. Write a `CLAUDE.md` of 120,000 lines
     `<!-- voice:begin a<n> -->` (n = 0..120000, distinct ids, no ends) and a last line `- Ask Ari (/ari)`;
     check the file is under 4 MiB; time `f.find()` with `std::time::Instant`; assert `lines == [120001]` and
     the time is under 10 seconds. It must fail on today's code by the time assertion (expect it to take a
     minute or more there), then pass.
   - `a_reused_block_id_pairs_each_begin_with_its_next_end`: lines `<!-- voice:begin x -->`, `- Ask Ari (/ari)`,
     `<!-- voice:end x -->`, `- Ask Ari (/ari)`, `<!-- voice:begin x -->`, `- Ask Ari (/ari)`,
     `<!-- voice:end x -->`, `- Ask Ari (/ari)` → `lines == [4, 8]`. (It passes today too; it guards the
     new pairing.)

2. **A linked private folder** (test only; the code already refuses it).
   `private_state_through_a_linked_state_folder_is_never_an_old_folder`: in the fixture, make `<home>/real/state`,
   make `<claude>/state` a symlink to it, write a readable valid `member.json` (the same `config()` the other
   private-state tests use) at `<home>/real/state/ari/member.json`, plant `--member ~/real/state/ari/member.json`
   → `old_folder` is `None`; then plant `--member ~/old/crew/ari/member.json` with `f.old()` → `Some(old)`.
   Model it on `private_state_through_a_linked_claude_root_is_never_an_old_folder`.

Run the brief's *Done when* checks for `voice-core` only (`cargo fmt --check`,
`cargo clippy --all-targets --locked -- -D warnings`, `cargo test --locked --jobs 2 -p voice-core -- --test-threads=2`)
and print a short summary: which test was red and how it failed, then the green counts.
