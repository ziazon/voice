# PB8.4 brief: the lasting place is the two Applications folders only

Persona Builder writes the path of the `persona-builder` command it ships into each assistant's
`SKILL.md`, and Claude Code runs that path every time the assistant looks up or records a
decision. PB8.2 made a release build refuse a path that will not last (`lasting` in
`builder/core/src/cli_command.rs`), but one of its two roots is the whole home folder. So the
unzipped app in `~/Downloads/Persona Builder/` passes: someone who skips `install.sh`, makes an
assistant from there and then deletes the download leaves a `SKILL.md` pointing at nothing. This
slice narrows that root from the home folder to the `Applications` folder inside it, keeping
`/Applications`. The owner chose this over writing the case down as a known limit.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`.

Work test-first: write the new test, run it and see it fail on its assertion, then change the
code. This is a public repo: invented names only in tests and fixtures, no real person, no fixed
paths beyond the system ones this brief names. Commit nothing; leave the changes in the working
tree.

## Sources (checked 2026-10-27)

- `builder/install/install.sh` installs only into `$HOME/Applications/Persona Builder.app` and
  refuses to run under `sudo`, so every correct install sits under `~/Applications`. A copy
  dragged into `/Applications` by hand still passes, by the second root.
- `lasting` canonicalizes each root and skips any root whose `canonicalize()` fails, so a person
  with no `~/Applications` folder simply has no home root (and is refused, which is right: the
  app is not installed).
- `std::path::Path::starts_with` compares whole components, so `~/ApplicationsOld/…` does not
  start with `~/Applications`.

## The change

### 1. `builder/core/src/cli_command.rs`

In `word_from`, the roots become `<home>/Applications` and `/Applications`:

```rust
fn word_from(path: &Path, home: &Path, lasting_only: bool) -> Result<String> {
    if lasting_only {
        lasting(path, &[&home.join("Applications"), Path::new("/Applications")])?;
    }
    cli_word(path, home)
}
```

Change nothing else in the file's code: `CLI_PLACE`, `CLI_FOLDER`, `shipped_cli`, `cli_word`,
`shipped_cli_word`, `lasting` and `on_a_passing_disk` stay exactly as they are. (`CLI_PLACE`
already tells the person to install with `install.sh` and open the app from the Applications
folder in their home folder.)

### 2. `.ai/SECURITY.md`

In the paragraph that starts `` `persona-builder record` writes only into an existing state
folder``, replace `the canonical path must be under the home folder or \`/Applications\`` with
`the canonical path must be under the Applications folder in the home folder (where \`install.sh\`
puts it) or \`/Applications\``. Rewrap that paragraph to the width its lines already have (about
100 columns). Change nothing else in the file.

## Tests

In the `tests` module of `builder/core/src/cli_command.rs`, using the existing `command_at` helper
and `crate::test_support::Scratch`:

1. New, `#[cfg(target_os = "macos")]` (same reason as `a_root_folder_owned_by_the_person_is_lasting`:
   Linux's `/tmp` is writable by others, so every scratch path is refused there for another
   reason), named `only_the_applications_folder_in_home_is_lasting`. With `home` a scratch
   folder:
   - make `<home>/Applications/Persona Builder.app/Contents/MacOS/persona-builder`: 
     `word_from(&command, home.path(), true)` equals `cli_word(&command, home.path())`;
   - make `<home>/Downloads/Persona Builder/Persona Builder.app/Contents/MacOS/persona-builder`:
     `word_from(&that, home.path(), true)` is `Err(Error::Invalid(message))` with
     `message == CLI_PLACE`;
   - make `<home>/ApplicationsOld/Persona Builder.app/Contents/MacOS/persona-builder`: refused
     the same way.
   Run it before changing `word_from` and see the Downloads assertion fail; that is the red.
2. Keep every existing test as it is. `a_root_folder_owned_by_the_person_is_lasting` already puts
   its command under `<root>/Applications`, so it still passes with `root` as the home; check
   that it does.

## Done when

- The new test was seen failing on its Downloads assertion before the change to `word_from`.
- `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings` and
  `cargo test --locked -p persona-builder` pass, and then `bash scripts/ci-local.sh` passes in
  full.
- `git status` shows only `builder/core/src/cli_command.rs` and `.ai/SECURITY.md` changed (this
  brief is already there, untracked). Report anything else you had to touch, and why.
