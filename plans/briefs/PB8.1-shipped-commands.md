# PB8.1 brief: the installed app ships the look-up and record commands

Persona Builder is a standalone app that builds a person's AI assistant from their own Claude
history. When the app turns an assistant on, it copies the assistant's `SKILL.md` into
`~/.claude/skills/<slug>/`. That `SKILL.md` comes from `crew/_template/SKILL.md` and tells Claude
Code to run two commands:

- `python3 -I {{REPO}}/scripts/crew_decisions.py --member {{REPO}}/crew/{{SLUG}}/member.json similar --query …`
  (find the person's past answers to questions like this one), and
- `python3 -I … record --work … --kind … --category … --tier … --question … --{{SLUG}}-pick …`
  (keep a decision the assistant made, or the person's answer to one it handed back).

For an assistant the app makes, `{{REPO}}` is `~/.claude`, and there is no
`~/.claude/scripts/crew_decisions.py`: the app ships no Python (decision PB-D2). So both commands
fail in the person's Claude Code. This slice fixes that:

1. **Part A:** `persona-builder record` (the Rust CLI gains the command `similar` already has a
   twin of), and the template names its commands through two new placeholders, `{{SIMILAR}}` and
   `{{RECORD}}`, which the guided build fills with the Python commands (unchanged behaviour) and
   the app fills with the `persona-builder` command it ships.
2. **Part B:** the Mac build puts that `persona-builder` command inside the app bundle, at
   `Persona Builder.app/Contents/MacOS/persona-builder`, signed inside-out; the installer refuses
   a copy without it.

You will be dispatched once per part. Do only the part your dispatch names, then stop.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`.

Work test-first: write each test, run it and see it fail on its assertion, then write the code.
This is a public repo: invented names only in tests and fixtures (Ari, Robin, Mo Lee, Sam O'Neil,
as the existing tests use), no real person, no fixed paths.

## Sources (checked 2026-10-26)

- Apple TN2206, *macOS Code Signing In Depth*
  (<https://developer.apple.com/library/archive/technotes/tn2206/_index.html>), Table 3: helper
  apps and tools go in `Contents/MacOS`; "when a code signature is created, all nested code must
  already be signed correctly"; sign "inside out … signing the outermost bundle last"; "While the
  `--deep` option can be applied to a signing operation, this is not recommended."
- Tauri's `bundle > externalBin` (<https://v2.tauri.app/develop/sidecar/>) is **not** used: its
  build script (`tauri-build` 2.7.0 `src/lib.rs` `copy_binaries`) copies the sidecar into the
  cargo target folder under its bare name, which is the CLI's own output name
  (`target/release/persona-builder`), and it needs a `-<target triple>` copy that a universal
  build doesn't make. The build script copies and signs the CLI itself instead.
- Python's `json.dumps(..., ensure_ascii=False)` is what `scripts/crew_decisions.py record` writes;
  `builder/ingest/src/mine.rs` `to_jsonl` already matches it byte for byte
  (`builder/ingest/tests/python_compat.rs`).

## Part A: `persona-builder record`, and the template's two command placeholders

### A1. The template

In `crew/_template/SKILL.md`, replace the three commands:

- §2 step 2: the two-line `python3 -I {{REPO}}/scripts/crew_decisions.py --member … \` /
  `similar --query "<the question and its options>"` block becomes the one line
  `{{SIMILAR}} --query "<the question and its options>"` (same indentation, three spaces).
- §3, both `record` blocks: the first line `python3 -I {{REPO}}/scripts/crew_decisions.py --member
  {{REPO}}/crew/{{SLUG}}/member.json record \` becomes `{{RECORD}} \`. The flag lines under it stay
  exactly as they are (`--{{SLUG}}-pick`, `--{{PRINCIPAL_SLUG}}-answer` included).

Change nothing else in the template.

### A2. Filling the placeholders: the guided build (`bootstrap/install.sh`)

In the embedded Python, `values` gains two keys **first**, before `NAME`, in this order:

```python
values = {"SIMILAR": "python3 -I {{REPO}}/scripts/crew_decisions.py --member {{REPO}}/crew/{{SLUG}}/member.json similar",
          "RECORD": "python3 -I {{REPO}}/scripts/crew_decisions.py --member {{REPO}}/crew/{{SLUG}}/member.json record",
          "NAME": name, ...}
```

Because they are filled first, the `{{REPO}}` and `{{SLUG}}` inside them are filled by the later
keys, so a member made by the guided build runs exactly the commands it ran before (now on one line).

### A3. Filling the placeholders: Rust (`builder/core/src/crew.rs`)

- `NewMember` gains `pub decisions: Decisions<'a>`, a new public enum in `crew.rs`:

  ```rust
  /// The commands an assistant's SKILL.md runs to look up and record decisions.
  pub enum Decisions<'a> {
      /// The guided build's `scripts/crew_decisions.py`, inside the repo.
      Python,
      /// A `persona-builder` command, already written as one shell word (see `cli_word`).
      Cli(&'a str),
  }
  ```

- `create_member` puts `("SIMILAR", …)` and `("RECORD", …)` at the front of `values`, before
  `NAME`, exactly as A2 does: for `Python`, the two strings in A2; for `Cli(word)`,
  `format!("{word} similar {{{{REPO}}}}/crew/{{{{SLUG}}}}")` and the same with `record` (that is,
  the text `<word> similar {{REPO}}/crew/{{SLUG}}`).
- Every existing caller that builds a `NewMember` (tests included: `grep -rn "NewMember {" builder`)
  passes `decisions: Decisions::Python`, except `assistants::make` (A4).
- `builder/core/tests/install_parity.rs` keeps passing unchanged in meaning: it compares
  `new_member` and `new_member_embedded` (both with `Decisions::Python`) with `bootstrap/install.sh`,
  byte for byte. Add one parity case whose principal is `{{SIMILAR}}` (like the existing
  `nested_principal_placeholder_matches_installer_order` case), so the fill order is pinned.

### A4. The app and the CLI name the shipped command

- New module `builder/core/src/cli_command.rs` (add it to `lib.rs`), two public functions:

  ```rust
  /// The `persona-builder` command that ships with this program: this program itself when it is
  /// that command, else the `persona-builder` beside it (the app's `Contents/MacOS`).
  pub fn shipped_cli() -> Result<PathBuf>
  /// `path` as one shell word for a SKILL.md: `"$HOME/<rest>"` when under `home`, else `"<path>"`.
  pub fn cli_word(path: &Path, home: &Path) -> Result<String>
  ```

  - `shipped_cli`: `std::env::current_exe()`, then `canonicalize`; if its file name is
    `persona-builder` return it; else return `parent.join("persona-builder")`. It does **not**
    check that the file exists (a `tauri dev` build has no CLI beside it; that is fine).
  - `cli_word`: canonicalize `home` (an error is `Error::Io`); `path` is used as given (the caller
    canonicalized it). Refuse with `Error::Invalid(CLI_FOLDER)` when the path is not valid UTF-8,
    or any character of it (the whole path, not only the part after home) is a control character
    or one of `"` `$` `` ` `` `\` `!` `{` `}`. `CLI_FOLDER` is a public const:
    `"Persona Builder can't be used from a folder whose name has \" $ ` \\ ! { or } in it. Move Persona Builder to the Applications folder in your home folder, then try again."`
    Under home: `format!("\"$HOME/{rest}\"")`; elsewhere `format!("\"{path}\"")`. A space stays as
    it is (double quotes keep it one word).
  - Unit tests in the module: under home with a space (`"$HOME/Applications/Persona Builder.app/Contents/MacOS/persona-builder"`),
    outside home, each refused character, and a non-UTF-8 path (`OsStr::from_bytes`).
- `builder/core/src/assistants.rs` `make` gains a last parameter `cli: &str` (the shell word) and
  passes `Decisions::Cli(cli)`.
- `builder/api/src/assistants.rs` (the app's make): before calling `assistants::make`, compute
  `cli_word(&shipped_cli()?, home)`; a `CLI_FOLDER` refusal reaches the screen as that sentence
  (not the generic `MAKE` failure); any other error there is the generic `MAKE` failure.
- `builder/cli/src/assistants.rs` `make` does the same with its own `home` and prints `CLI_FOLDER`
  as the error when refused.
- Tests: an assistant made through the api (`builder/api/tests/assistants.rs`) and through the CLI
  (`builder/cli/tests/assistants.rs`) has a `SKILL.md` with no `{{` left, no `crew_decisions.py`,
  and the lines `<word> similar ~/.claude/crew/<slug> --query` and `<word> record ~/.claude/crew/<slug> \`,
  where `<word>` is `cli_word(shipped_cli(), home)` computed in the test the same way.
  Then **run the command the SKILL.md names**: in the CLI test, take the `similar` line from the
  made `SKILL.md`, replace the `<word>` with the built binary (`env!("CARGO_BIN_EXE_persona-builder")`),
  and run it against an invented `decisions.jsonl` in the test home's state folder: it prints the
  invented precedent. Do the same with a `record` line: it writes the line in A5.

### A5. `persona-builder record`

Same meaning as `scripts/crew_decisions.py record` (read `cmd_record` and its argparse lines),
for one assistant's private folder only.

- Usage: `persona-builder record <MEMBER_DIR> --work W --kind decided|escalated --category C
  --tier high|medium|low --question Q --<slug>-pick P [--rule R] [--<principal_slug>-answer A]`.
  `<slug>` and `<principal_slug>` are the member's `slug` and `principal_slug`, as in Python, so
  the flag lines the template already writes work unchanged.
- In the derive `Command` enum: `Record { member_dir: PathBuf, #[arg(trailing_var_arg = true,
  allow_hyphen_values = true)] args: Vec<String> }` with a doc comment ("Keep one decision the
  assistant made, or your answer to one it handed back."). In a new `builder/cli/src/record.rs`,
  read the member with `member_and_home` (so a `state_dir` other than `~/.claude/state/<slug>` is
  refused as elsewhere), then build a `clap::Command` at run time with the member's two flag
  names and `try_get_matches_from` on `args`; `--kind` and `--tier` use
  `value_parser([...])` with exactly Python's choices. A clap error exits as clap's does
  (`error.exit()`), as the other commands' usage errors do. No `--log`: the line goes only to
  `live.jsonl` in the state folder.
- The line, a JSON object in this key order, written with `persona_builder_ingest::mine::to_jsonl`:
  `timestamp` (`persona_builder_grow::store::utc_now()`, `YYYY-MM-DDTHH:MM:SSZ`), `work`, `kind`,
  `category`, `tier`, `question`, `<slug>_pick` (`member.pick_field()`), `rule` (null when not
  given), `<principal_slug>_answer` (`member.answer_field()`, null when not given).
- Refuse a line over 64 KiB, before writing:
  `"That is too long to keep (over 64 KiB). Shorten it, then record it again."`
- Writing: new `pub fn append_private(home, slug, name, line: &[u8]) -> Result<PathBuf>` in
  `builder/ingest/src/private.rs`, beside `write_private_at`:
  - opens the state folder through `place_directory(home, slug, None, false)` (never creates it);
  - `validate_private_name(name)`;
  - `openat(dir, name, O_WRONLY | O_APPEND | O_CREAT | O_NOFOLLOW | O_CLOEXEC, 0o600)`;
  - refuses anything but a regular file (`fstat`), and a file others can open (`only_mine` with
    `0o077`, as the inner folders use);
  - takes `flock(LOCK_EX)` on that file (blocking: two Claude Code sessions recording at once
    each get a whole line), writes the line in one `write_all`, `sync_all`.
  - It does **not** take the state folder's `lock_state` lock: that lock is held for a whole
    build, and recording during a build must still work.
- When the state folder isn't there (never made, or removed by Forget), nothing is created:
  `"<Name> has nowhere to keep this yet: ~/.claude/state/<slug> isn't there. Open Persona Builder and give <Name> permission, then try again."`
  (`<Name>` the member's name, `<slug>` its slug.)
- On success print exactly what Python prints:
  `recorded a <kind> <category> decision (<tier>) for <work>`.
- Tests (`builder/cli/tests/record.rs`; follow `builder/cli/tests/keep.rs` for the test home and
  member setup):
  - the line's bytes equal Python's for the same arguments, timestamp excepted: run
    `scripts/crew_decisions.py --member <member.json> record --log <tmp> …` as the parity tests in
    `builder/cli/tests/steps_parity.rs` run Python (skip when Python is missing unless
    `PERSONA_BUILDER_REQUIRE_PARITY=1`), with non-ASCII text and a quote in `--question`, once with
    `--rule` and the answer flag and once without (both nulls);
  - two records append two lines; an existing `live.jsonl` keeps its lines;
  - a missing state folder: the sentence above, exit 1, and no `~/.claude/state/<slug>` afterwards;
  - `live.jsonl` as a symlink, as a folder, and as a 0644 file: refused, the target unchanged;
  - a wrong `--kind` or `--tier`, a missing `--<slug>-pick`, and another member's pick flag
    (`--robin-pick` on Ari): usage errors (exit 2), nothing written;
  - a 64 KiB + 1 line: the sentence above, nothing written;
  - recording while another process holds `lock_state` on the folder still works.
  - `append_private` unit tests in `private.rs` for the regular-file, mode and no-create rules.
- `builder/api/src/parity.rs` (the CLI-parity table) and `builder/cli/tests/app_parity.rs`: if they
  list CLI-only commands, add `record` there the way `similar` is.

### A6. Records

- `.ai/PROJECT.md`: the `builder/cli/` row's test column is unchanged; add to the `builder/core/`
  row's description "the command an assistant's SKILL.md runs (`cli_command.rs`)".
- `.ai/SECURITY.md`: one short paragraph: `record` writes only into an existing state folder
  (Forget's removal is never undone by it), appends to a regular owner-only file under `flock`,
  and has no `--log`; the SKILL.md's command is one double-quoted path the app checks for shell
  characters before writing it.
- `.ai/DECISIONS.md`: one entry (match the file's format): "An app-made assistant's SKILL.md runs
  the `persona-builder` shipped inside the app (`Contents/MacOS/persona-builder`), named as one
  quoted path when the assistant is made; the guided build keeps `scripts/crew_decisions.py`. The
  template names both through `{{SIMILAR}}` and `{{RECORD}}`. An assistant made before this keeps
  its old SKILL.md; make it again to get the new one." Date it 2026-10-26, from PB8.1.

### Part A done means

`cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`PERSONA_BUILDER_REQUIRE_PARITY=1 cargo test --locked`, and `bash tests/crew_install_test.sh` all
pass. Commit nothing; leave the changes in the worktree.

## Part B: the Mac build ships the command; the installer checks it

### B1. `scripts/build-mac-app.sh`

After `pnpm -C builder/ui tauri "${build_args[@]}"` and the existing `codesign --verify` of `$app`:

- Build the CLI: without `--universal`,
  `CARGO_NET_OFFLINE=true cargo build --release --locked -p persona-builder-cli`, binary at
  `$target_dir/release/persona-builder`. With `--universal`, build it for
  `aarch64-apple-darwin` and `x86_64-apple-darwin` (`--target …`, binaries at
  `$target_dir/<triple>/release/persona-builder`) and join them with
  `lipo -create -output "$staging_cli" <both>` into a file in the staging folder (removed by the
  existing `trap`). Fail plainly (`fail 'the persona-builder command was not built'`) when the
  binary is missing.
- After `ditto "$app" "$package/Persona Builder.app"`, on the staged copy only (never the bundle in
  the target folder):
  1. `cp` the CLI to `"$package/Persona Builder.app/Contents/MacOS/persona-builder"`, `chmod 755`;
  2. sign it: `codesign --force --sign - "$package/Persona Builder.app/Contents/MacOS/persona-builder"`;
  3. re-sign the app, outermost last and without `--deep`:
     `codesign --force --sign - --preserve-metadata=identifier,entitlements,requirements,flags,runtime "$package/Persona Builder.app"`;
  4. `codesign --verify --deep --strict` the staged app, else `fail "signature check failed: …"`.
- `lipo -archs` of the copied CLI must include every architecture of the app's main executable
  (`CFBundleExecutable`, read as `builder/install/install.sh` reads it), else
  `fail 'the persona-builder command was built for other Macs than the app'`.

### B2. `builder/install/install.sh`

Right after the `CFBundleExecutable` / `lipo` check of the main executable, check the shipped
command: `"$source_app/Contents/MacOS/persona-builder"` must be a regular file (not a symlink) and
executable, and `lipo -archs` of it must succeed and include `$mac_arch` (arm64e counts for arm64,
as for the main executable); otherwise `fail "$damaged"`. Nothing else in the installer changes.

### B3. `tests/mac_install_test.sh`

`fake_app` also compiles a tiny `persona-builder` for the given architecture into
`Contents/MacOS/` **before** it signs the bundle (inside-out, no `--deep` on the inner sign; the
existing outer `codesign -s - --force --deep` may stay for the fake). New cases: a copy without
`Contents/MacOS/persona-builder` and a copy whose `persona-builder` is a symlink are both refused
with the damaged sentence and install nothing. Every existing case still passes.

### B4. `How to install.txt`

Read it: it must still be true. It is (nothing it says changes); do not edit it unless a sentence
became false.

### Part B done means

`bash tests/mac_install_test.sh`, and `shellcheck scripts/ci-local.sh bootstrap/install.sh
tests/crew_install_test.sh scripts/build-mac-app.sh builder/install/install.sh
tests/mac_install_test.sh` pass. Do not run `scripts/build-mac-app.sh` itself (it needs a clean
checkout; the runner runs it after committing). Commit nothing.
