# PB6.2 brief: GitHub sign-in on Windows and Linux

PB6 keeps the GitHub token in the macOS Keychain through `/usr/bin/security`, and every other OS
gets `text::MAC_ONLY`, so nobody off a Mac can sign in. This slice adds the computer's own password
store on Windows (Credential Manager) and Linux (the Secret Service), keeps the macOS path exactly
as it is, and changes the save wording from *this Mac* to *this computer*.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `builder/save/src/keychain.rs`,
`builder/save/src/text.rs`, `builder/save/src/git.rs` (`NEEDS_GIT`), `builder/cli/src/main.rs`
(the `save` help), `builder/save/tests/keychain.rs`, `builder/cli/tests/save_help.rs`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md`, `~/.codex/skills/source-driven-development/SKILL.md`.

## Sources (read 2026-10-24, from the crates' own source in the cargo registry)

- `keyring` 4.2.0 is now a facade over `keyring-core` 1.0.1 plus one crate per store.
  `keyring_core::api::CredentialStoreApi::build(service, user, modifiers)` makes an `Entry` from a
  given store, with no process-wide default store. `Entry::{set_password, get_password,
  delete_credential}`. `keyring_core::Error` is `#[non_exhaustive]`: `NoEntry` (nothing stored),
  `NoStorageAccess` (locked, prompt refused), `PlatformFailure`, `BadEncoding(Vec<u8>)` and
  `BadDataFormat(Vec<u8>, _)` (both carry the stored bytes), and others.
  `keyring_core::mock::Store` is an in-memory store; it refuses any build modifier
  (`NotSupportedByStore`), and `mock::Cred::set_error` makes the next call fail.
- `windows-native-keyring-store` 1.1.0: generic credentials through `CredWriteW`/`CredReadW`;
  `Store::new()` never fails; the password is stored as UTF-16; `ERROR_NOT_FOUND` is `NoEntry`.
  The build modifier `persistence` takes `Session`, `Local` or `Enterprise` (the default).
  `Enterprise` roams with a domain profile to the person's other computers; `Local`
  (`CRED_PERSIST_LOCAL_MACHINE`) stays on this computer. Its default feature `search` only adds
  `regex`.
- `zbus-secret-service-keyring-store` 1.0.1: pure Rust over `secret-service` 5.2.0 and `zbus`;
  exactly one of four runtime and crypto features must be on. `Store::new()` connects to the
  session bus and opens a Secret Service session (encrypted, `EncryptionType::Dh`), and fails
  (`PlatformFailure`, `secret_service::Error::Unavailable` is "no secret service provider or
  dbus session found") when none runs. Items go in the default (login) collection, labelled
  `keyring:{user}@{service}` unless the `label` modifier is given. Its docs warn about headless
  machines and WSL (no default collection).
- `ring` 0.17 (under `ureq`'s rustls) builds C code that needs the target's libc headers, so
  `cargo check --target <windows or linux> -p persona-builder-save` cannot run from a Mac with no
  cross C toolchain. That is why the stores get their own small crate.

## Decisions

1. **A new crate `builder/keystore`** (package `persona-builder-keystore`, library
   `persona_builder_keystore`), the only code that differs by OS off a Mac, so
   `cargo check -p persona-builder-keystore --target x86_64-pc-windows-msvc` and
   `--target x86_64-unknown-linux-gnu` run from a Mac. Dependencies, pinned `=`:
   `keyring-core =1.0.1` (all targets); on `cfg(windows)` `windows-native-keyring-store =1.1.0`
   with `default-features = false`; on `cfg(target_os = "linux")`
   `zbus-secret-service-keyring-store =1.0.1` with `rt-async-io-crypto-rust` (pure Rust, no
   tokio, no OpenSSL). It re-exports `keyring_core`, so the save crate pins it once.
   API: `System::open() -> Result<System, NoStore>` (Windows: Credential Manager; Linux: the
   Secret Service, `NoStore` when it does not connect; any other OS: `NoStore`),
   `System::over(Arc<CredentialStore>)` (any store, no modifiers: tests use the mock), and
   `System::entry(service, user)`, which builds with the store's modifiers: Windows
   `persistence = Local`; Linux `label = "Persona Builder: GitHub sign-in"`.
2. **The save crate's `SystemStore`** (in `keychain.rs`, compiled on every OS so it is tested on a
   Mac against the mock): `SystemStore::open(service)` (the service checked as the Keychain does;
   `NoStore` becomes `text::NO_SAFE_STORE`) and `SystemStore::over(service, System)`. Its
   `TokenStore`: `load` (`NoEntry` is `None`; `BadEncoding`/`BadDataFormat` bytes are zeroized,
   then `KEYCHAIN_FAILED`; any other error `KEYCHAIN_FAILED`), `save` (refuses a token
   `storable` refuses, then sets and reads back, as the macOS path does), `forget` (`NoEntry` is
   success). Off macOS, `Keychain`'s `TokenStore` opens a `SystemStore` per call and delegates;
   on macOS nothing changes.
3. **Never a plaintext fallback.** No store means the plain refusal, which offers *keep it on this
   computer only*. The token never goes to a file, argv or the environment.
4. **Wording.** Every *this Mac* in `builder/save/src/text.rs` and the CLI's `save` help becomes
   *this computer*. `KEYCHAIN_FAILED` says "this computer's password store". `MAC_ONLY` is
   removed. `NEEDS_GIT` comes from `const fn needs_git(macos: bool)`: the `xcode-select --install`
   hint on macOS, `https://git-scm.com/downloads` elsewhere.
5. **`scripts/ci-local.sh`** runs `cargo check --locked -p persona-builder-keystore --target <t>`
   for each of the two targets that `rustup target list --installed` lists, and prints a skipped
   line naming the `rustup target add` command for each one missing.

## Tests (red first)

- keystore: `System::open()` is `NoStore` on a Mac (`cfg(not(any(windows, target_os = "linux")))`);
  an entry over the mock store round-trips; a live round trip, `#[ignore]`, on Windows and Linux
  only, for a hand run on a desktop.
- save `SystemStore` over the mock: round trip, replace, forget twice; `NoEntry` reads as `None`;
  `NoStorageAccess`, `PlatformFailure`, `BadEncoding` on `load`, `save` and `forget` each give
  `KEYCHAIN_FAILED`; a token `storable` refuses never reaches the store; a read-back that differs
  fails; an invalid service is refused; `NoStore` gives `NO_SAFE_STORE`.
- text: no `this Mac` anywhere in `text.rs`'s strings; both `needs_git` arms.
- CLI help: the two help lines read *this computer*.

## Out of scope

The README's "You need a Mac", PB8's macOS-only installer, PB7's builds for other OSes, and the
*this Mac* lines in `builder/agent` and `builder/ingest` (left for later slices). A live sign-in on a
Windows and a Linux desktop is owed, not built here.
