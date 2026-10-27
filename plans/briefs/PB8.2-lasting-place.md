# PB8.2 brief: the shipped command's path is only written from a lasting place

Persona Builder is a standalone app that builds a person's AI assistant from their own Claude
history. When it makes an assistant, it writes the path of the `persona-builder` command it ships
into the assistant's `SKILL.md` (PB8.1), and Claude Code runs that path every time the assistant
looks up or records a decision. `builder/core/src/cli_command.rs` turns the path into one shell
word (`cli_word`), refusing shell characters, but it never asks *where* the path is. An app opened
straight from a disk image (`/Volumes/…`), from an external drive, or run by Gatekeeper from a
translocated copy (`…/AppTranslocation/<id>/…`) would write a path that is gone tomorrow, or one
that a different disk mounted under the same name could supply. This slice refuses that, with one
plain sentence that says to install the app first.

It also fixes one stale line of prose in `crew/_template/decisions.md`.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`.

Work test-first: write each test, run it and see it fail on its assertion, then write the code.
This is a public repo: invented names only in tests and fixtures (Ari, Robin, Wren, as the
existing tests use), no real person, no fixed paths beyond the system ones this brief names.
Commit nothing; leave the changes in the working tree.

## Sources (checked 2026-10-27)

- `builder/install/install.sh` installs into `$HOME/Applications/Persona Builder.app`, and
  `scripts/build-mac-app.sh` builds the shipped CLI with `cargo build --release`. So every copy a
  person runs is a release build, and a correctly installed one sits under the home folder.
- On macOS, `std::env::temp_dir()` is the per-user `/private/var/folders/<xx>/<id>/T`: every
  folder above it is owned by the person or root and none is writable by others. `/Applications`
  is `root:admin` mode `0775` (group-writable, not other-writable). On Linux `/tmp` is mode `1777`.
- `std::path::Path::starts_with` compares whole components (`/VolumesX` does not start with
  `/Volumes`).

## Why the rule applies to release builds only

Every integration test that makes an assistant (`builder/api/tests/*.rs`,
`builder/cli/tests/assistants.rs`) runs with a scratch home in the temp folder, while the
`persona-builder` it ships is the test build in `target/`, which is outside that scratch home.
A development build (`cargo test`, `tauri dev`) runs from its own build folder, wherever the clone
is. So the place rule applies when `cfg!(debug_assertions)` is false, and the check itself is
unit-tested directly with roots the test chooses. Do not add any environment variable or other
switch that turns the rule off.

## The change

### 1. `builder/core/Cargo.toml`

Add `libc = "=0.2.191"` under `[dependencies]` (the exact pin the other workspace crates already
use; it is already in `Cargo.lock`, so `--locked` still passes). It is for `libc::geteuid()`.

### 2. `builder/core/src/cli_command.rs`

Keep `CLI_FOLDER`, `shipped_cli()` and `cli_word()` exactly as they are. Add:

```rust
pub const CLI_PLACE: &str = "Persona Builder needs to be installed before it can make an assistant. Run install.sh from the folder you unzipped, then open Persona Builder from the Applications folder in your home folder.";

/// `shipped_cli()` as one shell word for a SKILL.md (see `cli_word`). A release build refuses
/// with `CLI_PLACE` unless the command is in a place that lasts (see `lasting`); a development
/// build runs from its own build folder, wherever that is.
pub fn shipped_cli_word(home: &Path) -> Result<String> {
    word_from(&shipped_cli()?, home, !cfg!(debug_assertions))
}

fn word_from(path: &Path, home: &Path, lasting_only: bool) -> Result<String> {
    if lasting_only {
        lasting(path, &[home, Path::new("/Applications")])?;
    }
    cli_word(path, home)
}
```

and a private `fn lasting(path: &Path, roots: &[&Path]) -> Result<()>` that does these steps, in
this order, refusing every failure with `Err(Error::Invalid(CLI_PLACE.into()))` unless the step
says otherwise:

1. `let canonical = path.canonicalize()`; an error is `Error::Io { path: path.to_path_buf(), source }`
   (a missing command is a damaged copy, not a place problem).
2. If `on_a_passing_disk(&canonical)` is true, refuse. That is a private pure function
   `fn on_a_passing_disk(canonical: &Path) -> bool`: true when `canonical.starts_with("/Volumes")`
   or any component of `canonical` is exactly `AppTranslocation`
   (`canonical.components().any(|c| c.as_os_str() == "AppTranslocation")`).
3. Canonicalize each root, skipping any root whose `canonicalize()` fails (there is no
   `/Applications` on Linux, and a missing home is caught later by `cli_word`). If `canonical`
   does not `starts_with` any canonical root, refuse.
4. Only under `#[cfg(unix)]`: for `canonical` itself and every one of its `ancestors()` (up to and
   including `/`), read `std::fs::symlink_metadata` (an error is `Error::Io` with that folder's
   path). Refuse when `metadata.uid()` is neither `unsafe { libc::geteuid() }` nor `0`, or when
   `metadata.mode() & 0o002 != 0` (writable by others). Group-writable is allowed: `/Applications`
   is `root:admin 0775`. Use `std::os::unix::fs::MetadataExt`. Read the euid once, before the loop.
   Give the `unsafe` block a `// SAFETY:` comment (geteuid takes no arguments and cannot fail).
5. `Ok(())`.

### 3. The two callers

- `builder/api/src/assistants.rs` (`new_assistant_in`): replace
  `shipped_cli().and_then(|path| cli_word(&path, home))` with `shipped_cli_word(home)`, and map
  `Error::Invalid(message)` to `failure(&message)` when `message == CLI_FOLDER || message == CLI_PLACE`;
  every other error stays `failure(MAKE)`. Fix the `use` line (drop `cli_word` and `shipped_cli`
  if nothing else in the file uses them; add `CLI_PLACE` and `shipped_cli_word`).
- `builder/cli/src/assistants.rs` (`make`): replace `cli_word(&shipped_cli()?, &home)?` with
  `shipped_cli_word(&home)?`, and fix its `use` line the same way.

Do not change the integration tests that compute the expected word with
`cli_word(&shipped_cli()…)`: in a debug build `shipped_cli_word` returns exactly that.

### 4. `crew/_template/decisions.md`

Line 18, `answers are searchable with \`crew_decisions.py similar\` and stay in …`, becomes
`answers are searchable with the skill's look-up (§2) and stay in …` (rewrap the bullet to the
file's width, about 100 columns, as the lines around it are). The file already says, a few lines
up, that it is read by the `/{{SLUG}}` skill (§2), so "the skill's look-up (§2)" is the
`SKILL.md` §2 command.

## Tests

In the `tests` module of `builder/core/src/cli_command.rs` (use `crate::test_support::Scratch`;
make the command file with `fs::create_dir_all` and `fs::write`; set modes with
`std::os::unix::fs::PermissionsExt`):

1. `a_passing_disk_is_the_volumes_folder_or_a_translocated_copy`: pure, on `on_a_passing_disk`.
   True for `/Volumes/Persona Builder/Persona Builder.app/Contents/MacOS/persona-builder`, for
   `/Volumes` itself, and for `/private/var/folders/ab/cd/T/AppTranslocation/1234-ABCD/d/Persona Builder.app/Contents/MacOS/persona-builder`.
   False for `/VolumesX/persona-builder`, for `/Applications/Persona Builder.app/Contents/MacOS/persona-builder`,
   and for `/Users/robin/AppTranslocationNotes/persona-builder`.
2. `outside_every_root_is_refused`: the command in one scratch folder, the only root another
   scratch folder: `lasting` answers `Error::Invalid(CLI_PLACE)`. (Portable: step 3 comes before
   step 4.)
3. `a_translocated_copy_under_a_root_is_refused`: root = a scratch folder; the command at
   `<root>/AppTranslocation/1234/d/Persona Builder.app/Contents/MacOS/persona-builder`: refused with
   `CLI_PLACE`.
4. `#[cfg(target_os = "macos")] a_root_folder_owned_by_the_person_is_lasting`: root = a scratch
   folder; the command at `<root>/Applications/Persona Builder.app/Contents/MacOS/persona-builder`:
   `lasting` is `Ok`, and `word_from(&command, root, true)` equals `cli_word(&command, root)`.
   Also make one folder on the way group-writable (`0o775`) and assert it is still `Ok`. Comment
   why it is macOS-only: the temp folder is per-user there, while Linux's `/tmp` is writable by
   others, which the rule refuses.
5. `#[cfg(unix)] a_folder_others_can_write_is_refused`: root = a scratch folder; set
   `<root>/open` to `0o757` and put the command under it: refused with `CLI_PLACE`. Then set
   `open` back to `0o755`, set the command file itself to `0o646`: refused with `CLI_PLACE`.
   Restore writable modes before the `Scratch` drops so its cleanup works.
6. `a_missing_command_is_an_io_error`: `lasting` on a path in a scratch root that doesn't exist
   answers `Error::Io { .. }`.
7. `the_place_rule_runs_only_when_asked`: the command in one scratch folder, home another:
   `word_from(&command, home, false)` equals `cli_word(&command, home)`, and
   `word_from(&command, home, true)` is `Error::Invalid(CLI_PLACE)`.
8. `a_development_build_skips_the_place_rule`: `#[cfg(debug_assertions)]`;
   `shipped_cli_word(scratch.path())` equals `cli_word(&shipped_cli().unwrap(), scratch.path())`
   (the test build lives in `target/`, outside the scratch home, so this fails if the rule ran).

In `builder/cli/tests/assistants.rs`, in `new_list_and_agree_use_the_claude_folders_and_final_wording`,
next to the existing `!skill.contains("crew_decisions.py")` assertion, read
`<home>/.claude/crew/wren/decisions.md` and assert it does not contain `crew_decisions.py` either
(message: `app-made decisions.md names Python`).

## Done when

- Each new test was seen failing on its assertion before the code that makes it pass.
- `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
  `cargo test --locked -p persona-builder -p persona-builder-api -p persona-builder-cli` pass,
  and then `bash scripts/ci-local.sh` passes in full.
- `git status` shows only: `builder/core/Cargo.toml`, `Cargo.lock` only if cargo changed it (it
  should not), `builder/core/src/cli_command.rs`, `builder/api/src/assistants.rs`,
  `builder/cli/src/assistants.rs`, `builder/cli/tests/assistants.rs`,
  `crew/_template/decisions.md`. Report anything else you had to touch, and why.
