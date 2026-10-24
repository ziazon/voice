# Persona Builder: slice records

One record per slice, newest first. Paraphrase only; no real person.

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
