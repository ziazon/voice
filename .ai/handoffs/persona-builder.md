# Persona Builder: slice records

One record per slice, newest first. Paraphrase only; no real person.

## PB6: saving, to an existing repo, a private GitHub repo, or this Mac only (2026-10-24, lane: V1)

- **What:** a new crate, `builder/save` (`persona_builder_save`), because the core library keeps
  to the filesystem. It holds every sentence a person sees while saving (`text`), a `Secret` type
  that hides a token from `Debug` and zeroes it on drop, GitHub's device-flow sign-in with the
  granted-scope check, creating a repo private and reading it back as private, GitHub's address
  rules, the token store in the macOS Keychain, git plumbing that commits an assistant's folder
  without touching anyone's work tree, the saved choice in `<state>/save.json`, and the flow
  (`detect`, `options`, `choose_*`, `save_now`). The CLI gains `persona-builder save options |
  sign-in | sign-out | choose (local | here | github) | push`. Brief: `plans/briefs/PB6-saving.md`.
- **Who wrote it:** Codex, in separate test and implementation passes for each half; Claude wrote
  the brief, the person-facing texts and the docs, and reviewed every test and line.
- **Decisions made at pickup:** the network, git and Keychain code is a crate of its own, so core's
  "filesystem only" rule stays true. Sign-in is an OAuth app's device flow with the `repo` scope,
  the only scope that can make a private repo; the client ID is unset until V-Q1 is answered, and
  sign-in says it is not set up. A save to a person's own repo goes to a `persona/<slug>` branch
  of its own (the guided build's step 14 also saves to a branch, `crew/<slug>`), built from a
  scratch index, so their index and work tree never change; if that branch is checked out
  anywhere, the save is refused rather than moving it. The GitHub backup is a bare repo in the state folder, pushed with the
  token as an environment header scoped to `https://github.com/`, the person's git config off.
  Every push address must read back private from GitHub's API before every push; any other host
  is refused, since nothing else can be confirmed. The privacy check is a seam: the CLI's `push`
  refuses with "can't run the privacy check yet" until PB2 plugs the Rust check in.
- **Dependency intake:** `ureq` 3.4.2 (rustls, no default features; MIT or Apache-2.0; the same
  pin Flightdeck uses), and `base64` 0.23.1, `libc` 0.2.191 and `zeroize` 1.9.1, which were
  already in ureq's tree. New in the lock: ureq's tree (rustls, ring, webpki-roots,
  rustls-pki-types, ureq-proto, http, httparse, percent-encoding, untrusted, subtle, log,
  getrandom, cc, utf8-zero). `ring` builds C and assembly through `cc` at build time. `cargo
  audit` is not installed, so no advisory scan was run.
- **Sources:** GitHub's docs on the device flow, OAuth scopes, the repos REST API and API
  versions, and actions/checkout's git auth header, all read 2026-10-24 and cited in the brief
  and the code.
- **Verified:** every test file went red before its code existed (part 1: 65 tests on stubs;
  part 2: 51 tests on stubs plus the review fixes' new files red on their assertions), then green:
  135 Rust tests in the two packages, and `bash scripts/ci-local.sh` passed. Mutation checks, each
  anchored, restored from a backup and confirmed with `cmp`, and each failing a named test, not a
  build: 26 in all (bearer only on API calls; both private fields; the exact host; the slow-down
  step; the exact `repo` scope; no redirects; the expiry stop; the device page's host; the
  Keychain alphabet; the read-back after create; the git config off for the backup repo, both
  variables; the credential helper off; the cleared environment; `private.json` skipped; links
  refused; the credential hidden in errors; `--no-gpg-sign`; the privacy check run; the remote
  check run; the backup's exact origin; every push URL on GitHub; a repo `detect` did not offer;
  the remote-name rule; the 0700 folder; a local choice sending nothing). Two missed at first; two
  tests were added, and both now fail when their line is removed. The live Keychain test (a throwaway
  `persona-builder.test` item) passed and left nothing behind. The CLI was driven by hand in a
  scratch home: the options with and without a repo around `~/.claude`, every refusal, and a
  `save.json` carrying `--upload-pack=…` as its remote refused before git ran. A build with an
  invented client ID reached github.com over rustls and turned GitHub's real `404 {"error":"Not
  Found"}` into the plain "sign-in didn't work" line.
- **Test harness, fixed on review:** on this Mac `python3` is a pyenv shim, which macOS will not
  run as a `#!` interpreter, so the git wrapper now resolves the real interpreter; APFS refuses
  non-UTF-8 file names, so that case is skipped where the filesystem refuses it; one test deleted
  the folder holding its own record. Codex stopped on that one rather than bend the code.
- **Gates:** `/security-review` (run as a sub-agent): no high-confidence findings; two low notes
  kept below. The Verifier (Fable) failed the first pass on one must-fix, that the token alphabet had
  no `_` and so would refuse real `gho_…` tokens. That was a misread: the alphabet's literal is
  `._~:+/=-`, whose second character is `_`. Realistic `gho_` fixtures now pin it. Her
  should-fixes, all fixed test-first:
  a save moved the person's `persona/<slug>` branch when it was checked out (now refused); a poll
  reply with status 400, as RFC 8628 sends, was treated as unexpected; a relative `state_dir` was
  accepted.
- **Known limits:** an existing state folder keeps its mode (only one Persona Builder creates gets
  0700), and `backup.git` takes the umask's modes inside it. The folder is committed whole except
  `private.json` and `.DS_Store`: `.gitignore` is not read, so PB2's privacy check must cover any
  other file kept there. Copies of the token outside `Secret` (form fields, the git header, the
  child's environment, visible to the same user while git runs) are not zeroed. The Keychain item
  can be read by any process of the same user through `/usr/bin/security`. A lost `backup.git` on
  a second Mac makes the next push a non-fast-forward, refused, with no recovery yet; a git failure
  after GitHub made the repo leaves the repo, so a retry under the same name is refused. A file swapped for a link between listing and hashing would be read through the link
  (same-user only). The person's own `pre-push` hook runs on a save to their repo, with Persona
  Builder's cleared environment. A non-GitHub remote cannot be used at all.
- **Not verified:** a real sign-in, repo creation and push (they need V-Q1's client ID and PB2's
  privacy check: PB6.1, card `51dkJ0YE`, an attended walk); Linux (the Keychain is macOS only).

## PB1: the member format in Rust, the neutral base, one field pack (2026-10-24, lane: V1)

- **What:** `builder/core` reads and writes an assistant's files the way the Python tooling does:
  `member.json` with `private.json` laid over (`member`), JSON written byte for byte as Python's
  `json.dumps(indent=2)` (`json`), a new `crew/<slug>/` folder filled from the template as
  `install.sh` fills it, without a kit, and removed again if filling it fails (`crew`), the dial in `autonomy.md` (`autonomy`) and field
  packs (`fields`). The CLI gains `persona-builder member show <member.json>`. The template's
  `IDENTITY.md` and `autonomy.md` are rewritten in plain words, keeping every dial key and its
  state; `crew/_fields/general.md` is the first field pack. Brief:
  `plans/briefs/PB1-member-format.md`.
- **Who wrote it:** Codex, from the brief; Claude reviewed it and wrote the user-facing text.
- **Dependency intake:** `serde` 1 (derive) and `serde_json` 1 (`preserve_order`), both MIT or
  Apache-2.0 and the standard Rust JSON pair. They add `serde_core`, `serde_derive`, `itoa`,
  `memchr`, `zmij` (serde_json's float formatter, same author), and `indexmap` with `hashbrown`,
  `equivalent` and `foldhash`. None of them touches the network, files or processes. No
  `cargo audit` scan was run because it is not installed.
- **Verified:** every test went red on its assertion first. The parity tests run the real Python:
  Rust's `read_member` matches `crew_decisions.py`'s byte for byte on three invented members and
  gives the same error on two invalid ones. `new_member` matches `install.sh --no-history` file for
  file and byte for byte on four invented cases, one of them non-ASCII. Five mutations each turned
  their test red: the ASCII escaping, the private-filter join, the placeholder order, the skipped
  `README.md` and the cleanup on failure. `bash scripts/ci-local.sh` passed with all Rust steps; it
  sets `PERSONA_BUILDER_REQUIRE_PARITY=1`, so a missing `python3`, `git` or `bash` fails the run
  instead of skipping the parity tests.
- **Gates:** `/security-review` found three low issues; the two cheap ones are fixed (the target
  folder is created with `create_dir`, and entry types come from `file_type()`). The Verifier
  (Fable) found that parity holds for configs in the template's shape but not beyond it. Fixed: a
  nested `member.json` is now copied unfilled, as `install.sh` does, and a non-string filter in
  `member.json` is refused instead of being replaced. Two `IDENTITY.md` sentences overclaimed: one
  said a change is kept only once approved (that step comes in PB4), and one promised no backup.
  Both now say only what is true today.
- **A miss caught by the gate:** the first `autonomy.md` text said "passwords", which the
  template's own secret filter flags, so the privacy-check tests failed. It now says "logins".
- **Known limits (documented in the code):** the writer puts keys in the struct's order with
  unknown keys last, drops `null` optional fields and empty codebook lists, and reads numbers as
  `f64` or `i64`. Python keeps all of these as they were. `slugify` matches `install.sh` under
  `LC_ALL=C`; in a UTF-8 locale `tr` may lowercase some non-ASCII letters differently. Rust keeps
  CRLF line endings and does not copy a file's executable bit, while `install.sh` normalizes the
  first and keeps the second. The template has none of these cases.
- **Not verified:** a kit (`install.sh --kit`), which `new_member` does not cover yet.

## PB0: the Rust workspace and its checks (2026-10-23, lane: V2)

- **What:** a virtual Cargo workspace at the root (edition 2024, stable, no toolchain file) with
  `builder/core` (package `persona-builder`, library `persona_builder`, one `VERSION` constant) and
  `builder/cli` (binary `persona-builder`, clap derive, `--version` and `--help` only).
  `scripts/ci-local.sh` runs `cargo fmt --check`, `cargo clippy --all-targets --locked -- -D
  warnings` and `cargo test --locked` after the existing steps, and prints a skip line when `cargo`
  is missing. `.ai/PROJECT.md`, `README.md` and `CONTRIBUTING.md` name the new folders and the Rust
  requirement. `target/` was already ignored. Brief: `plans/briefs/PB0-rust-workspace.md`.
- **Who wrote it:** Claude, directly. Codex was at a hard usage limit (until 2026-10-29), so the
  brief was implemented by hand at the same bar.
- **Dependency intake:** `clap` 4.6 with `derive` (MIT or Apache-2.0, the standard Rust argument
  parser); the lock adds only clap's own tree (anstream, anstyle, clap_lex, heck, strsim, syn,
  proc-macro2, quote, windows-sys on Windows). No network, file or process access of its own.
  `cargo audit` is not installed here, so no advisory scan was run.
- **Verified:** red first (`builder/cli/tests/version.rs` failed on its assertion against an empty
  `main`), then green; a mutation (core `VERSION` set to `9.9.9`) turned it red and was restored with
  `cmp`. `bash scripts/ci-local.sh` passed with cargo on `PATH` (all three Rust steps ran) and with
  `~/.cargo/bin` removed from `PATH` (it printed the skip line and passed).
- **Not verified:** a Linux or Windows build; a contributor machine with an older stable Rust than
  1.98 (edition 2024 needs 1.85 or newer).
- **Landed by V1 (2026-10-24):** rebased onto `main` (one README table conflict, both rows kept),
  re-authored to the project's noreply identity, and pushed to `main` as a fast-forward so the
  committer stays noreply too. `bash scripts/ci-local.sh` passed again with and without `cargo` on
  `PATH`.

## PB4.0: the growth loop spec (2026-10-23)

- **What landed:** `docs/spec/growth-loop.md`, five JSON Schemas and five invented examples in
  `docs/spec/growth-loop/`, a standard-library validator for exactly the keywords the schemas use
  (`tests/schema_subset.py`, which refuses any other keyword anywhere in a schema), and
  `tests/growth_loop_spec_test.py`, wired into `scripts/ci-local.sh`. Brief:
  `plans/briefs/PB4.0-growth-loop-spec.md`.
- **Who built it:** implemented directly from the brief; the usual implementer was at a usage limit.
- **Decisions made at pickup:** no new dependency (a small validator instead of `jsonschema`, so the
  kit still needs only Python and shellcheck); `additionalProperties: false` everywhere, so any shape
  change is a new version; the file-path pattern avoids lookaround so a Rust implementation can compile
  it; a case's status changes by appending the whole case again (last line wins).
- **Review:** an independent review passed it with should-fixes, all applied: rule 3 now requires the
  results to cover every case the proposal names (an omitted new case can no longer pass), versioning
  matches the closed schemas, rejected proposals get no measurement, a stale `base` is never applied,
  and the doc lists the checks the schemas cannot make. Security review: no findings.
- **Verified:** red first (validator assertions failed, spec files missing), then green; seven mutation
  checks each failed the test and were restored (`cmp`); the reference `jsonschema` 4.x library, in a
  throwaway environment, agreed on all five schemas and examples and on the path probes;
  `bash scripts/ci-local.sh` green.
- **Not verified:** the patterns under a Rust or JavaScript regex engine (they use only portable
  syntax; PB4 will run them in Rust).
