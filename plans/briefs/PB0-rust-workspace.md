# PB0 brief: the Rust workspace and its checks

Persona Builder is a standalone app that builds a person's AI assistant from their own Claude
history. This slice lays its foundation in this repo: an empty Cargo workspace with a core library
and a CLI, and the Rust checks in `scripts/ci-local.sh`. No product behaviour yet beyond
`persona-builder --version`.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `CONTRIBUTING.md`. Work test-first: the failing test
before the code, and match the surrounding code.

## Rules

- Public repo: no real person's name, handle, path or data anywhere. No fixed home paths.
- Stable Rust, edition 2024. Add **no** `rust-toolchain.toml`.
- One dependency only: `clap` 4 with the `derive` feature (`clap = { version = "4.6", features =
  ["derive"] }`). Nothing else, not even dev-dependencies.
- Commit `Cargo.lock` (the checks run `--locked`). `target/` is already in `.gitignore`; leave it.
- Do not touch `crew/`, `bootstrap/`, `scripts/*.py` or `tests/*.py`.

## Files

1. **`Cargo.toml`** at the repo root, a virtual workspace:

   ```toml
   [workspace]
   members = ["builder/core", "builder/cli"]
   resolver = "3"

   [workspace.package]
   edition = "2024"
   version = "0.1.0"
   license = "MIT"
   publish = false
   ```

2. **`builder/core/Cargo.toml`**: package `persona-builder` (so the library is `persona_builder`),
   every package field from the workspace (`edition.workspace = true`, and so on), no dependencies.
   **`builder/core/src/lib.rs`**: a crate doc comment saying this is the Persona Builder core
   library, that it does no I/O beyond the local filesystem, and that the CLI and the app are thin
   layers over it. One item: `pub const VERSION: &str = env!("CARGO_PKG_VERSION");` with a doc
   comment.

3. **`builder/cli/Cargo.toml`**: package `persona-builder-cli`, workspace fields, a `[[bin]]` named
   `persona-builder` at `src/main.rs`, depending on `clap` and on `persona-builder = { path =
   "../core" }`.
   **`builder/cli/src/main.rs`**: a clap derive parser,
   `#[command(name = "persona-builder", version = persona_builder::VERSION, about = "Build your AI
   assistant from your own Claude history")]`, with no subcommands yet. `main` parses and returns.

4. **`builder/cli/tests/version.rs`** — write this **first** and watch it fail before step 3's
   `main.rs` exists or prints the version: run `env!("CARGO_BIN_EXE_persona-builder")` with
   `--version` through `std::process::Command`, assert it exits 0 and that stdout is exactly
   `format!("persona-builder {}\n", env!("CARGO_PKG_VERSION"))`. No other crates.

5. **`scripts/ci-local.sh`**: after the existing shellcheck step and before `ci-local: all passed`,
   add:

   ```bash
   if command -v cargo >/dev/null 2>&1; then
     echo 'cargo fmt --check'
     cargo fmt --check
     echo 'cargo clippy --all-targets --locked -- -D warnings'
     cargo clippy --all-targets --locked -- -D warnings
     echo 'cargo test --locked'
     cargo test --locked
   else
     echo 'ci-local: cargo not found, skipping the Rust checks (install Rust from https://rustup.rs)'
   fi
   ```

   It must stay shellcheck-clean.

6. **Docs**, plain words, one line each:
   - `.ai/PROJECT.md`: two table rows, `builder/core/` (the Persona Builder core library,
     `persona_builder`; test `cargo test`) and `builder/cli/` (the `persona-builder` command; test
     `builder/cli/tests/`). Change the "All of it runs with" line to say the Rust checks need stable
     Rust and are skipped when `cargo` is missing.
   - `README.md` Layout table: one row, `builder/` (Persona Builder, a standalone app in progress:
     the core library and the `persona-builder` command). In the `scripts/ci-local.sh` row, add that
     it runs the Rust checks when `cargo` is installed.
   - `CONTRIBUTING.md`: after "It needs Python 3.9 or newer and `shellcheck`.", add that the Rust
     checks also need stable Rust and are skipped without it.

## Done when

- `bash scripts/ci-local.sh` passes with cargo on `PATH` (all three Rust steps run).
- It also passes with cargo removed from `PATH` (it prints the skip line).
- `git status --ignored` shows `target/` ignored and no other new untracked file.

Report the red test output, the green run, and the files changed.
