# Persona Builder app

The Tauri shell opens the screens package and exposes only the commands listed in its capability.

`dev.personabuilder.desktop` is a placeholder until signing. The app keeps no data under this
identifier, so changing it later costs nothing.

## Making a copy for a Mac

From the repo root, run `bash scripts/build-mac-app.sh`. Add `--universal` for a copy that runs
on both chips; this needs both Mac Rust targets (`rustup target add aarch64-apple-darwin
x86_64-apple-darwin`). It writes `dist/persona-builder-<version>-mac-<label>.zip` and its `.sha256`.

The copy is ad-hoc signed, not by Apple, until the public release (PB-D3), so share it only with
people you know. The build runs only from a git checkout and refuses uncommitted changes under
`builder/`, `crew/` and the Cargo files. They follow `How to install.txt` inside the zip. Send the `.sha256` line in a separate message that says to check it before running anything
from the zip (the how-to travels in the zip, so it can't vouch for itself); the recipient compares
the zip's SHA-256 as step 3 of `How to install.txt` says.
