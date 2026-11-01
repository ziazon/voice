# VO2 brief: rename Persona Builder to Voice

The app in `builder/` is called Persona Builder today. It is renamed **Voice**: the product, the
command, the crates, the app, the UI package and the docs. The app is not released (it is unsigned,
and no assistant exists outside invented test ones), so **nothing is migrated**: every name on disk
moves to its new name, and a file an older build left under an old name is not read. This brief
is decision-complete; do not invent other names.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/code-quality/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md` (every sentence a person reads),
`~/.codex/skills/project-config/SKILL.md`.

This is a public repo: invented names only, no real person, no fixed paths. Add no dependency.
Commit nothing; leave every change in the working tree. The work comes in three dispatches, one
section each (§ 2, § 3, § 4); do only the section your dispatch names.

## 1. The name map (every section uses it)

Apply the longer forms first, so `persona_builder_agent` never becomes `voice_core_agent`.

| Old | New | Where |
|---|---|---|
| `persona_builder_<x>` (a crate path, a test name) | `voice_<x>` | `persona_builder_api` → `voice_api`, `persona_builder_ingest` → `voice_ingest`, and so on |
| `persona_builder` (the core library alone) | `voice_core` | `use persona_builder::…`, `persona_builder::VERSION` |
| `persona-builder-<x>` (a package, a file or folder prefix) | `voice-<x>` | `persona-builder-agent` → `voice-agent`, `persona-builder-app` → `voice-app`, `persona-builder-cli` → `voice-cli`, `persona-builder-keystore` → `voice-keystore`, `.persona-builder-` (the staging prefix in `builder/build/src/draft.rs`) → `.voice-`, `.CLAUDE.md.persona-builder-` → `.CLAUDE.md.voice-`, `.persona-builder-install.XXXXXX` → `.voice-install.XXXXXX`, `dist/persona-builder-` → `dist/voice-` |
| the package `persona-builder` in `builder/core/Cargo.toml`, and every `persona-builder = { path = … }` dependency on it | `voice-core` | Cargo files only; its `[lib]` name becomes `voice_core` |
| the command `persona-builder` (the `[[bin]]` in `builder/cli/Cargo.toml`, clap's `name`, `bin_name`, every `persona-builder <subcommand>` in a message, the `persona-builder: ` error prefix, `Contents/MacOS/persona-builder`, `CARGO_BIN_EXE_persona-builder`, `cli_command.rs`'s file name) | `voice` | `voice consent agree`, `voice: …`, `CARGO_BIN_EXE_voice` |
| `Persona Builder` (prose, the window title, `productName`) | `Voice` | `Persona Builder's` → `Voice's`. Read each sentence after the swap; fix an article or verb that no longer reads well ("a Persona Builder line" → "a Voice line") |
| `Persona Builder.app` | `Voice.app` | the bundle, the installer, `scripts/`, `tests/` |
| `Library/Application Support/Persona Builder/claude-code` | `Library/Application Support/Voice/claude-code` | `builder/agent/src/claude.rs` and its tests |
| `.local/share/persona-builder/claude-code` | `.local/share/voice/claude-code` | the same |
| `PersonaBuilder` (the root Vue component and its export) | `VoiceApp` | `builder/ui/src/PersonaBuilder.vue` → `VoiceApp.vue`, `PersonaBuilder.test.ts` → `VoiceApp.test.ts` (`git mv`), `export { default as VoiceApp }`, `main.ts` |
| `@persona-builder/screens` | `@voice/screens` | `builder/ui/package.json` and every import or doc line |
| `PERSONA_BUILDER_<X>` (environment variables) | `VOICE_<X>` | `VOICE_GITHUB_CLIENT_ID`, `VOICE_REQUIRE_PARITY`, `VOICE_REQUIRE_SANDBOX`, `scripts/ci-local.sh` included |
| `dev.personabuilder.desktop` | `dev.voice.desktop` | `builder/app/tauri.conf.json`; still a placeholder until signing |
| `dev.personabuilder.test` | `dev.voice.test` | tests |
| `persona-builder.github` (the password-store service) | `voice.github` | `builder/save/src/keychain.rs` |
| `persona-builder` (the password-store account, `ACCOUNT`) | `voice` | the same file |
| `persona-builder.test` (the test service in the keystore and keychain tests) | `voice.test` | tests |
| `<!-- persona-builder:` (the CLAUDE.md block markers, `builder/core/src/block.rs`) | `<!-- voice:` | `<!-- voice:begin ari -->`, `<!-- voice:end ari -->`; the message that names them says "its voice markers" |
| `persona-builder/report/1` (the report schema) | `voice/report/1` | `builder/build/src/report.rs` |
| `persona-builder/<version>` (the self-hosted user agent) | `voice/<version>` | `builder/agent/src/self_hosted.rs` |
| `persona-builder-public-literal-` | `voice-public-literal-` | the same file's test |
| `generated_by: persona-builder build …` | `generated_by: voice build …` | `builder/build/src/draft.rs` |
| the `cli` argument `"persona-builder"` passed to `assistants::make` in tests | `"voice"` | `builder/core/src/assistants.rs` tests |
| `~/.claude/persona-builder/…` | see § 3 | **not** a straight swap |

**What keeps its old name:** `.ai/handoffs/persona-builder.md` (its file name and every line in it:
it is the dated record of what was built, and other tools name the file); any `PB…` slice id and
`PB-D…` decision id; the folder names under `builder/` (none holds the old name). The existing
`voice-panels` and `voice-panel-runner` crates and `~/.claude/voice/panels/` are already named.

## 2. Dispatch A: the code

Everything outside the docs listed in § 4 and outside § 3's two files: `Cargo.toml` files,
`Cargo.lock`, `builder/**` sources and tests (Rust, TypeScript, Vue, JSON, TOML, the Tauri
`capabilities/`, `permissions/` and `gen/` files), `scripts/**`, `tests/**`, and any other file
`git grep` finds. Apply § 1, except every `~/.claude/persona-builder` path (in
`builder/core/src/install.rs`, `install_record.rs`, and the tests that name it: `use_it.rs` and
others): leave those exactly as they are for § 3.

1. Rename with the map. `git mv` the two Vue files.
2. Rewrite `Cargo.lock` for the renamed members only: run
   `cargo check --workspace --all-targets --offline` once **without** `--locked`. Then
   `git diff Cargo.lock` must show only package names changing: no version, source or checksum
   line may change. If one does, stop and say so.
3. Check: `git grep -n -I -i -E 'persona.?builder|personabuilder' -- ':!*.md' ':!plans/**'`
   prints only the `~/.claude/persona-builder` lines § 3 owns. Say how many and where.
4. Run every step of `scripts/ci-local.sh` (after your rename) that works offline, one by one,
   and report each result: the Python tests, the two install tests, shellcheck, `cargo fmt
   --check`, clippy, `VOICE_REQUIRE_PARITY=1 cargo test --locked` (the sandbox tests in
   `builder/agent/tests/sandbox_profile.rs` cannot run inside your own sandbox; say so), and the
   `builder/ui/src/types/generated` diff check (a generated file that changes with the rename is
   kept). The `pnpm` steps need the network for `install`: run `typecheck`, `test` and `build`
   only if `builder/ui/node_modules` exists, and say whether it did.
5. No behaviour changes in this dispatch: a test that fails after the rename means a name was
   missed or mistyped. Fix the name, never the test's assertion.

## 3. Dispatch B: where an assistant that is turned on keeps its rules

Today turning an assistant on (`builder/core/src/install.rs`) writes
`~/.claude/persona-builder/<slug>/RULES.md` and the record
`~/.claude/persona-builder/installed.json` (`install_record.rs`), and records each folder it
creates so turning off removes it. A straight rename to `~/.claude/voice/` breaks two things,
because Panels already keep their files in `~/.claude/voice/panels/<id>/`
(`builder/panel-runner/src/room.rs`, which refuses a `~/.claude/voice` that is not the person's own
0700 folder):

- an assistant named "Panels" would get `~/.claude/voice/panels/RULES.md`, inside the panels' folder;
- if turning on created `~/.claude/voice`, it records that folder as its own, so turning off (and
  turning on again, which turns off first and refuses anything left: `install.rs` "was left in
  place") reports `~/.claude/voice` as "it still holds other files" once a panel exists.

The new layout:

| What | Path |
|---|---|
| The shared root | `~/.claude/voice/` (Panels and turning on both use it) |
| The record | `~/.claude/voice/installed.json` |
| An assistant's rules | `~/.claude/voice/assistants/<slug>/RULES.md` |

Rules:

1. **The shared root is never the install's.** Turning on creates `~/.claude/voice` when it is
   missing, mode 0700 (the journal's `directory` already creates 0700), refusing a symlink there
   (the same `refuse_symlink` and `install_record::check_path` checks the rules folder gets), and
   **never** adds it to the record's `added` list, so turning off never removes it and never
   reports it as left. Undo on a failed turn-on may remove it only if this same turn-on created it
   (the journal's undo already does that; keep it).
2. `~/.claude/voice/assistants/` is created and recorded like `~/.claude/persona-builder/` was
   (0700, in `added`, removed by turning off when empty). `<slug>/` under it as before.
3. `install_record::read` looks at `~/.claude/voice/installed.json`: a missing `~/.claude/voice`
   or a missing record is "nothing is on", as today; a `~/.claude/voice` that is a symlink or not a
   folder is refused with today's messages.
4. Every message that names the old folder names the new one. Messages already use the § 1 map
   after dispatch A.

Test first (`builder/core` tests beside the existing install tests, and `builder/api/tests/use_it.rs`
for the record path), each red before you change `install.rs`:

- turning on with no `~/.claude/voice` creates it 0700, writes
  `voice/assistants/<slug>/RULES.md` and `voice/installed.json`, and the record's `added` does not
  hold `~/.claude/voice`;
- turning off after that removes `assistants/<slug>/`, `assistants/` and the record, keeps
  `~/.claude/voice`, and reports nothing left;
- with `~/.claude/voice/panels/dl-x/` present (create it in the test), turning on, turning on
  again (the refresh), and turning off all succeed, report nothing left, and leave `panels/dl-x/`
  untouched;
- an assistant named "Panels" (slug `panels`) writes `voice/assistants/panels/RULES.md` and does
  not touch `voice/panels/`;
- a symlink at `~/.claude/voice` is refused and nothing is written;
- turning on creates a `~/.claude/voice` the panel runner then accepts: in
  `builder/panel-runner/tests/`, after a turn-on into a temporary home, `Room::open(home, id, true)`
  succeeds (use whatever public entry the existing room tests use).

Also in this dispatch, a fix-up to § 2: the consent wording changed (it names Voice now), and
`builder/core/src/consent.rs` says "When changing the wording, bump VERSION and update this". § 2
updated `WORDING_SHA1` but not `VERSION`. Bump `VERSION` to 2, so a consent given to the old
wording asks again (`builder/api/src/assistants.rs` accepts only `consent::VERSION`). A test that
writes a literal version-1 consent meaning "the current one" uses `consent::VERSION` instead; a
test that means "an old consent" keeps 1 and should now see the consent asked again. Add one test
in `builder/api/tests/assistants.rs` (beside the existing consent tests): a version-1 record
reads as no permission given.

Run `cargo test -p voice-core -p voice-api -p voice-cli -p voice-panel-runner --locked` and
clippy on them; report each (sockets your sandbox refuses: say which tests). Keep each change
minimal and in the style of the file.

## 4. Dispatch C: the docs

Apply § 1 to every Markdown file except `.ai/handoffs/**` and this brief: `README.md`, `CONTRIBUTING.md`,
`.ai/PROJECT.md`, `.ai/SECURITY.md`, `.ai/DECISIONS.md` (its entries name code, commands and paths;
rename them all), `builder/**/README.md`, `docs/**`, `bootstrap/**`, `crew/**`, and
`plans/briefs/*.md` (the briefs are kept as instructions that still describe the code; rename code
names, commands, paths and the product name, and leave their slice ids). Use § 3's layout wherever
a doc names `~/.claude/persona-builder`. "Persona Builder (soon Voice)" becomes "Voice". Where a
doc says the app "is called" or "will be renamed", say it is Voice now. Do not add a history of the
rename to any doc (the records hold it).

Check: `git grep -n -I -i -E 'persona.?builder|personabuilder' -- ':!.ai/handoffs/**'
':!plans/briefs/VO2-rename-to-voice.md'` prints nothing at all. Read `README.md` and `.ai/PROJECT.md` through once after the swap: the repo is
"voice, a kit for building AI assistants" and the app inside it is now also Voice; where a
sentence would read as if the kit and the app were the same thing, say "the Voice app".
