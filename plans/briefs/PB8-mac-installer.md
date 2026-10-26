# PB8 brief: the unsigned Mac app and its installer

Written by Claude (lane V1), 2026-10-25. Spec: lane V's `PB8` entry, from PB8 of the Persona Builder
plan (§ Phases PB8, PB-D3: signing waits until the public release; until then copies go only to
people the maintainer knows). Read the repo's `AGENTS.md` and `.ai/PROJECT.md` first, then
`builder/app/tauri.conf.json`, `builder/app/tests/lockdown.rs`, `scripts/ci-local.sh` and
`tests/crew_install_test.sh` (the shape of a shell test here). Work test-first
(`~/.codex/skills/tdd-skill/SKILL.md`); security rules: `~/.codex/skills/security-and-hardening/SKILL.md`;
every sentence a person sees: `~/.codex/skills/plain-language/SKILL.md`; review bar:
`~/.codex/skills/code-quality/SKILL.md`.

Run cargo with `CARGO_NET_OFFLINE=true`. You do **not** run the app build itself (`tauri build`
needs the network and takes minutes); Claude runs it after you, as the live proof. Do not commit;
Claude reviews and commits.

## What this slice does

It makes a copy of Persona Builder that someone can install on their Mac without Homebrew, without
a password and without an Apple Developer account: a build script that makes the app and zips it
with an installer and a plain how-to, and the installer itself. Nothing in the app's code changes.

## Facts checked (do not re-derive)

- Tauri 2's docs (`https://v2.tauri.app/distribute/sign/macos/`, read 2026-10-25): setting
  `bundle > macOS > signingIdentity` to `"-"` gives an *ad-hoc* signature, "useful on ARM (Apple
  Silicon) devices, where code-signing is required for all apps from the Internet", and it "does not
  prevent MacOS from requiring users to whitelist the installation". So the installer clears the
  download quarantine on its own copy (below), which is what lets the app open.
- `https://v2.tauri.app/distribute/macos-application-bundle/`: build only the `.app` with
  `tauri build --bundles app`; macOS 10.13 and up by default (leave it).
- `builder/ui/package.json`'s `tauri` script is `cd ../app && tauri`, so
  `pnpm -C builder/ui tauri build --bundles app` builds the app from the repo root.
- The bundle's folder is `<cargo target dir>/release/bundle/macos/Persona Builder.app`, or
  `<cargo target dir>/universal-apple-darwin/release/bundle/macos/…` with
  `--target universal-apple-darwin`. Read the target dir from
  `cargo metadata --format-version 1 --no-deps` (`target_directory`), never assume `target/`.
- Mimi has `cc`, `lipo`, `codesign`, `ditto`, `xattr` and `/usr/libexec/PlistBuddy` in `/usr/bin`
  (and `/usr/libexec`), and only the `aarch64-apple-darwin` Rust target for macOS.

## Decisions (made, do not reopen)

### 1. `builder/app/tauri.conf.json`

Add `"macOS": { "signingIdentity": "-" }` inside `bundle` (after `icon`). Nothing else changes.
In `builder/app/tests/lockdown.rs` add a test `bundle_is_an_ad_hoc_signed_app_only` (red first: it
fails before the config change) asserting `bundle.targets == ["app"]` and
`bundle.macOS.signingIdentity == "-"`.

### 2. The installer: `builder/install/install.sh`

Bash, `set -euo pipefail`, shellcheck-clean, no network, no `sudo`, no Homebrew, never writes
outside `$HOME/Applications`. It lives in the zip next to `Persona Builder.app` and installs that
copy. Usage: `bash install.sh [--no-open]`. Steps, in this order, each failure printing its message
to stderr and exiting 1 (unless said otherwise), with nothing installed:

1. Arguments: none, or `--no-open`. Anything else: print
   `Usage: bash install.sh [--no-open]` to stderr, exit 2.
2. `uname -s` is not `Darwin`: `Persona Builder's installer is for Macs only.`
3. `$EUID` is 0: `Please run this without sudo. Persona Builder installs into your own Applications folder and needs no password.`
4. The source is `<the script's own folder>/Persona Builder.app` (find the folder from `$0`, as
   `cd "$(dirname "$0")" && pwd`). If it is not a directory, or is a symlink:
   `Persona Builder.app isn't next to this installer. Unzip the file you were sent again and run install.sh from the folder it makes.`
5. `codesign --verify --deep --strict` on the source fails (silence its output):
   `This copy of Persona Builder looks damaged. Ask for a new copy.`
6. The chip. The executable is `Contents/MacOS/<CFBundleExecutable>` read with
   `/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' <Info.plist>`; its architectures come
   from `lipo -archs`. This Mac is `arm64` when `sysctl -n hw.optional.arm64` prints `1` (true even
   in a Terminal running under Rosetta), else `x86_64`. Accept when any architecture equals this
   Mac's, or this Mac is `arm64` and an architecture is `arm64e`. Otherwise:
   `This copy is made for <made-for> Macs, and this Mac has <this-mac>. Ask for a copy made for this Mac.`
   with `arm64`/`arm64e` → `Apple chip`, `x86_64` → `Intel chip`, anything else as printed; for
   `<made-for>`, join two names with ` and ` (e.g. `Intel chip`), and for `<this-mac>` write
   `an Apple chip` or `an Intel chip`.
7. Copy: `mkdir -p "$HOME/Applications"`; make a staging folder
   `mktemp -d "$HOME/Applications/.persona-builder-install.XXXXXX"`, removed by an `EXIT` trap on
   every path; `ditto` the source into `<staging>/Persona Builder.app`.
8. Clear the quarantine on the copy only: `xattr -dr com.apple.quarantine` on it, tolerating "no
   such attribute"; then check no file in the copy still carries `com.apple.quarantine`
   (`xattr -lr`), else `Persona Builder couldn't be made ready to open. Nothing was installed.`
   The source copy keeps its own attributes (never touch it).
9. `codesign --verify --deep --strict` on the copy; failure gives step 5's message.
10. Replace: `rm -rf "$HOME/Applications/Persona Builder.app"` then `mv` the staged copy there.
11. Print `Persona Builder is installed in your Applications folder: <full path>` to stdout. Unless
    `--no-open`, run `open` on the installed app.

Top-of-file comment (three lines at most): what it installs, where, and why it clears the
quarantine (the copy is ad-hoc signed, not by Apple yet; PB-D3).

### 3. The build script: `scripts/build-mac-app.sh`

Bash, `set -euo pipefail`, shellcheck-clean, finds the repo from its own path
(`cd "$(dirname "$0")/.."`). Usage `bash scripts/build-mac-app.sh [--universal]`; any other argument:
usage to stderr, exit 2.

1. Refuse off macOS (`build-mac-app: macOS only`), and when `cargo`, `pnpm` or `python3` is missing
   (name it, with the install link `scripts/ci-local.sh` already uses for that tool). Require pnpm 10
   or newer, read as `ci-local.sh` reads it.
2. Label and target: no flag → label from `uname -m` (`arm64` or `x86_64`), no `--target`.
   `--universal` → label `universal`, `--target universal-apple-darwin`, and first check
   `rustup target list --installed` lists both `aarch64-apple-darwin` and `x86_64-apple-darwin`,
   else exit 1 with `build-mac-app: --universal needs both Mac targets: rustup target add aarch64-apple-darwin x86_64-apple-darwin`.
3. Version: `python3 -I -c` reading `version` from `builder/app/tauri.conf.json`.
4. `pnpm -C builder/ui install --frozen-lockfile`, then
   `pnpm -C builder/ui tauri build --bundles app` (plus the target).
5. Find the app under the cargo target dir (Facts above); missing → exit 1 naming the path.
   `codesign --verify --deep --strict` it, else exit 1.
6. Assemble in a fresh `dist/persona-builder-<version>-mac-<label>/` staging folder (removed at the
   end and on failure by a trap): a folder `Persona Builder` holding `Persona Builder.app`
   (`ditto`), `install.sh` (mode 755) and `How to install.txt` (both from `builder/install/`).
   Zip it with `ditto -c -k --keepParent` to `dist/persona-builder-<version>-mac-<label>.zip`
   (replace an old one), then write `<zip>.sha256` as `shasum -a 256` prints it, with the zip's
   base name only (run it from `dist/`).
7. Print the zip's path and the sha256 line.

Add `/dist/` to `.gitignore` (next to the other build outputs).

### 4. `builder/install/How to install.txt`

Exactly this text (plain words for someone who has never used Terminal; do not reword):

```
How to install Persona Builder on your Mac

Persona Builder works with Claude Code, which you sign in to yourself. If you don't have it yet,
the app tells you how to get it.

1. Double-click the zip file you were sent. A folder called "Persona Builder" appears next to it.
2. Open Terminal: press Command and Space, type Terminal, then press Return.
3. Type bash and a space. Drag the file install.sh from the "Persona Builder" folder into the
   Terminal window, then press Return.
4. Persona Builder opens. Next time, you'll find it in the Applications folder inside your home
   folder (in the Finder, choose Go, then Home).

This copy isn't signed by Apple yet, so the installer tells your Mac it is safe to open. It asks
for no password and changes nothing else on your Mac.

To remove the app, drag it from that Applications folder to the Trash. Your assistant stays. To
delete what the app made, use Forget in the app first.
```

### 5. The test: `tests/mac_install_test.sh`

Bash, shellcheck-clean, the shape of `tests/crew_install_test.sh` (`fail`, a scratch folder removed
by a trap). Off macOS, or without `cc` or `codesign`, print
`mac_install_test: skipped (needs macOS with cc and codesign)` and exit 0. Put a space in the
scratch path (`$scratch/kit folder/…`) so quoting bugs fail. Run the installer as
`HOME="$scratch/home" bash "<kit>/install.sh" --no-open`, capturing stdout, stderr and the exit code.

A helper makes a fake app at a path for an architecture: `Contents/Info.plist` with
`CFBundleExecutable` `persona-builder-app`, `CFBundleIdentifier` `dev.personabuilder.test`,
`CFBundlePackageType` `APPL`; `Contents/MacOS/persona-builder-app` compiled with
`cc -arch <arch>` from `int main(void){return 0;}`; then `codesign -s - --force --deep`.
A kit folder holds that app plus a copy of `builder/install/install.sh`. This Mac's arch and "the
other one" come from `sysctl -n hw.optional.arm64` as in the installer. Cases, one assertion group each:

1. **Installs:** quarantine set on the kit's app and its executable
   (`xattr -w com.apple.quarantine '0081;00000000;Test;'`); exit 0; the installed app exists;
   `xattr -lr` shows no `com.apple.quarantine` in it; `codesign --verify --deep --strict` passes on
   it; the kit's own app still has its quarantine; stdout names the installed path; no
   `.persona-builder-install.*` left in `$HOME/Applications`.
2. **Replaces an old copy:** a previous `Persona Builder.app` with a marker file is gone after a
   second install, and the new copy is in place.
3. **Wrong chip:** an app built only for the other architecture → exit 1, stderr has
   `Ask for a copy made for this Mac`, nothing installed.
4. **Damaged:** append one byte to the signed executable → exit 1, stderr has `looks damaged`,
   nothing installed (and an earlier installed copy, if one is there, is left as it was).
5. **No app:** an empty kit → exit 1, stderr has `isn't next to this installer`.
6. **Symlinked app:** the kit's `Persona Builder.app` is a symlink to a real fake app → the step 5
   message, nothing installed.
7. **Bad argument:** `--open-now` → exit 2, stderr has `Usage:`.

Each case uses its own `HOME` under the scratch folder. The `sudo` refusal is not tested (the
test can't run as root); keep it as the third step so a review sees it.

### 6. Wiring and docs

- `scripts/ci-local.sh`: run `bash tests/mac_install_test.sh` right after `tests/crew_install_test.sh`,
  and add `scripts/build-mac-app.sh builder/install/install.sh tests/mac_install_test.sh` to the
  shellcheck line (both the `echo` and the command).
- `builder/app/README.md`: a short section *Making a copy for a Mac*: run
  `bash scripts/build-mac-app.sh` (`--universal` for a copy that runs on both chips, which needs both
  Mac Rust targets); it writes `dist/persona-builder-<version>-mac-<label>.zip` and its `.sha256`;
  the copy is ad-hoc signed, not by Apple, until the public release (PB-D3), so share it only with
  people you know; they follow `How to install.txt` inside it; compare the zip's SHA-256 on their Mac
  (`shasum -a 256 <zip>`) with the `.sha256` you sent separately.
- `.ai/SECURITY.md`: one row in the threats table, in the existing style:
  *A copy of the app is swapped or damaged on its way to someone* | The copy is ad-hoc signed and
  the installer checks the signature (`codesign --verify --deep --strict`) before and after copying;
  it clears the download quarantine only on its own copy in `~/Applications` and nothing else, asks
  for no password, and refuses a copy for the other chip; the build writes the zip's SHA-256 to send
  separately. Known limit: an ad-hoc signature shows a copy is whole, not who made it, so copies go
  only to people the maintainer knows until it is signed (PB-D3) | `tests/mac_install_test.sh`

## Done means

- `bash tests/mac_install_test.sh` passes, and each of its seven cases fails when you break the
  installer step it covers (try it, then restore with `cp` from a backup; never `git checkout`).
- `cargo test -p persona-builder-app` passes with the new test, red before the config change.
- `shellcheck` is clean on the three new scripts; `bash scripts/ci-local.sh` passes up to whatever
  your sandbox can't run (say which steps).
- No real person, no fixed path, no audio anywhere. Report the files you changed and the commands you ran.
