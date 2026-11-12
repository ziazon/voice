# PB8.7a fix-up 1: links, Voice's own assistant, and the CLAUDE.md cap

Review of the uncommitted PB8.7a work in this worktree (brief: `plans/briefs/PB8.7a-look-before-making.md`).
Same skills and rules as that brief. Commit nothing. Red test first for each change.

1. **No new link refusals.** `existing::find` (`builder/core/src/existing.rs`) calls `refuse_symlink` on
   `claude_home`, `<home>/.claude`, `<home>/.claude/state` and `<claude_home>/skills`. The brief didn't ask for
   them, and turning on already supports a `skills` folder that is a link to a folder outside the Claude
   folder (`install_record::check_path`), so these would stop anyone with a linked Claude folder or skills
   folder from making any assistant. Remove all four. The finder only reads names; the link rules that
   matter stay as written (the state folder itself, the skill folder, `SKILL.md` and the old folder are
   never followed). Tests: a `skills` folder that is a link to a folder outside the Claude folder, holding
   `ari/`, reports the skill without an error; a `claude_home` that is a link to a real folder works.

2. **Voice's own assistant is never "older".** When `assistants::assistant_dir(home, slug)` exists (by
   `symlink_metadata`, any type), `existing::find` returns `Found { slug, ..Default::default() }` straight
   away: that name belongs to an assistant Voice already lists, and making it again is refused by the
   existing "You already have an assistant called …" sentence. Then:
   - `new_assistant_in` (`builder/api/src/assistants.rs`): drop its own `symlink_metadata(&dir)` guard and
     always call the finder (it now answers nothing for Voice's own).
   - `voice new` (`builder/cli/src/assistants.rs` `make`): drop its own crew-folder check; the existing
     `Error::Exists` mapping after `assistants::make` already gives the sentence.
   - `voice existing`: when the crew folder exists, print `You already have an assistant in
     ~/.claude/crew/<slug>/.` and exit 0, before calling the finder.
   - Tests: core, an assistant made with `assistants::make` plus a state folder with files and an unclaimed
     skill gives an empty `Found`; API, `find_existing_in` answers `found: false` for it and
     `new_assistant_in` still gives the "You already have an assistant called Ari" sentence; CLI,
     `voice existing Ari` prints the crew-folder line.

3. **Cap before reading.** `find_lines` reads the whole `CLAUDE.md`, then checks 4 MiB. Open it, read at
   most 4 MiB + 1 byte (`Read::take`), and give the same `Error::Invalid` when it is longer. Missing file:
   no lines, as now. Test: a file of 4 MiB + 1 byte is refused, one of exactly 4 MiB is read.

Run the same checks as the brief's *Done when* (2 workers), and print a short summary.
