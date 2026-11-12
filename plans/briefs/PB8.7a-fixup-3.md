# PB8.7a fix-up 3: keep out of the private folder by identity, not by spelling

From the repeat Verifier on `36a631e`. Same skills and rules as `plans/briefs/PB8.7a-look-before-making.md`.
Commit nothing. Red test first for each change.

1. **The private folder is refused by device and inode** (`builder/core/src/existing.rs`, `find_old_folder`).
   The lexical `folder.starts_with(home.join(".claude/state"))` check misses two routes on macOS, where both
   probes made the finder open and parse a `member.json` inside the private folder:
   - letter case: `--member ~/.Claude/state/ari/member.json` or `~/.claude/STATE/…` (APFS ignores case,
     `Path::starts_with` doesn't);
   - a linked `~/.claude`: `~/.claude` → `~/real/claude`, with `--member <home>/real/claude/state/ari/member.json`
     (the path below home has no link in it, so the component check passes).

   Replace the lexical check with an identity check, done before any open of `member.json`:
   - `fs::metadata(home.join(".claude/state"))` (following links). If it doesn't exist, skip the check.
     Any other error: no old folder.
   - Walk the candidate folder and each of its ancestors up to `/` with `fs::metadata` (following links).
     If any has the same `(dev(), ino())` (`std::os::unix::fs::MetadataExt`) as the state folder, there is no
     old folder. An error on any ancestor: no old folder.
   - Keep the component checks that already exist.
   Tests, each with a **readable, valid** `member.json` inside the state folder as the detector (a refusal
   gives `None`; an open would give `Some`). Mode 000 can't tell an open from a refusal, so drop that part of
   `private_state_is_never_an_old_folder_even_with_unreadable_member` and rename it:
   - `--member ~/.claude/state/ari/member.json` → `None`;
   - other letter case (`~/.Claude/state/ari/member.json`): `None`. Only assert this when the scratch folder
     is case-insensitive (detect it by checking whether `home.join(".CLAUDE")` exists); otherwise skip the
     assertion with a comment;
   - `~/.claude` a link to `~/real/claude`, `--member <home>/real/claude/state/ari/member.json` → `None`;
   - a valid old folder elsewhere still found (`Some`).

2. **A Voice block is a begin and its own end.** In `find_lines`, a `<!-- voice:begin <id> -->` line starts a
   skipped block only when a later line is `<!-- voice:end <id> -->` with the same id (compare the trimmed
   marker text, as `block::find` does); the block ends at that line. A begin with no matching end hides
   nothing; an end with no begin is just skipped. Test: an unclosed `begin x`, then `- Ask Ari (/ari)`, then a
   closed `begin y` … `end y` block holding another `/ari` line → only the first `/ari` line is found.

3. **`.ai/SECURITY.md`:** in the PB8.7a sentence, change "never under `~/.claude/state`" to "never the
   private folder or inside it (checked by device and inode, so neither letter case nor a link gets round
   it)".

Run the same checks as the brief's *Done when* (2 workers) and print a short summary.
