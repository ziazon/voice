#!/usr/bin/env bash
set -euo pipefail

if [ "$(uname -s)" != Darwin ] || ! command -v cc >/dev/null 2>&1 || ! command -v codesign >/dev/null 2>&1; then
  echo 'mac_install_test: skipped (needs macOS with cc and codesign)'
  exit 0
fi

here="$(cd "$(dirname "$0")/.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
check() { grep -Fq -- "$2" "$1" || { cat "$1" >&2; fail "$3"; }; }

if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then
  arch=arm64
  other_arch=x86_64
else
  arch=x86_64
  other_arch=arm64
fi

fake_app() {  # path architecture
  mkdir -p "$1/Contents/MacOS"
  cat > "$1/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>persona-builder-app</string>
<key>CFBundleIdentifier</key><string>dev.personabuilder.test</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>
PLIST
  printf 'int main(void){return 0;}\n' | cc -arch "$2" -x c - -o "$1/Contents/MacOS/persona-builder-app"
  codesign -s - --force --deep "$1" >/dev/null 2>&1
}

kit() {  # case architecture (empty for no app)
  kit_dir="$scratch/kit folder/$1"
  home="$scratch/home/$1"
  installed="$home/Applications/Persona Builder.app"
  mkdir -p "$kit_dir" "$home"
  cp "$here/builder/install/install.sh" "$kit_dir/install.sh"
  if [ -n "$2" ]; then
    fake_app "$kit_dir/Persona Builder.app" "$2"
  fi
}

run() {
  status=0
  HOME="$home" bash "$kit_dir/install.sh" "$@" > "$scratch/out" 2> "$scratch/err" || status=$?
}

not_installed() {
  [ ! -e "$installed" ] || fail 'a refused copy was installed'
}

# Installs, leaving the downloaded original alone and cleaning its staging folder.
kit installs "$arch"
source_app="$kit_dir/Persona Builder.app"
xattr -w com.apple.quarantine '0081;00000000;Test;' "$source_app"
xattr -w com.apple.quarantine '0081;00000000;Test;' "$source_app/Contents/MacOS/persona-builder-app"
run --no-open
[ "$status" = 0 ] || { cat "$scratch/err" >&2; fail 'install failed'; }
[ -d "$installed" ] || fail 'installed app is missing'
attributes=$(xattr -lr "$installed")
[[ "$attributes" != *com.apple.quarantine* ]] || fail 'installed copy is quarantined'
codesign --verify --deep --strict "$installed" || fail 'installed signature is invalid'
xattr -p com.apple.quarantine "$source_app" >/dev/null || fail 'source quarantine was removed'
xattr -p com.apple.quarantine "$source_app/Contents/MacOS/persona-builder-app" >/dev/null || fail 'source executable quarantine was removed'
check "$scratch/out" "Persona Builder is installed in your Applications folder: $installed" 'installed path not printed'
[ -z "$(find "$home/Applications" -name '.persona-builder-install.*' -print)" ] || fail 'staging folder remains'
echo 'mac_install_test: installs passed'

# Replaces an old app, including files that the new app no longer contains.
kit replaces "$arch"
run --no-open
[ "$status" = 0 ] || fail 'first install before replacement failed'
printf 'old\n' > "$installed/old-marker"
run --no-open
[ "$status" = 0 ] || fail 'replacement failed'
[ ! -e "$installed/old-marker" ] || fail 'old copy was not replaced'
[ -x "$installed/Contents/MacOS/persona-builder-app" ] || fail 'new executable is missing'
echo 'mac_install_test: replaces passed'

kit wrong-chip "$other_arch"
run --no-open
[ "$status" = 1 ] || fail 'wrong chip was not refused'
check "$scratch/err" 'Ask for a copy made for this Mac' 'wrong-chip explanation missing'
not_installed
echo 'mac_install_test: wrong chip passed'

# The two signature checks and the quarantine re-check are belt and braces, so breaking only one of them does not fail this test.
kit damaged "$arch"
printf x >> "$kit_dir/Persona Builder.app/Contents/MacOS/persona-builder-app"
run --no-open
[ "$status" = 1 ] || fail 'damaged app was not refused'
check "$scratch/err" 'looks damaged' 'damage explanation missing'
not_installed
mkdir -p "$installed"
printf 'keep\n' > "$installed/old-marker"
run --no-open
[ "$status" = 1 ] || fail 'damaged app replaced an earlier app'
check "$scratch/err" 'looks damaged' 'damage explanation missing on replacement'
[ "$(cat "$installed/old-marker")" = keep ] || fail 'earlier app was changed'
[ ! -e "$installed/Contents" ] || fail 'damaged app was copied into earlier app'
echo 'mac_install_test: damaged passed'

kit no-app ''
run --no-open
[ "$status" = 1 ] || fail 'missing app was not refused'
check "$scratch/err" "isn't next to this installer" 'missing-app explanation missing'
not_installed
echo 'mac_install_test: no app passed'

kit symlinked ''
fake_app "$scratch/real app/Persona Builder.app" "$arch"
ln -s "$scratch/real app/Persona Builder.app" "$kit_dir/Persona Builder.app"
run --no-open
[ "$status" = 1 ] || fail 'symlinked app was not refused'
check "$scratch/err" "isn't next to this installer" 'symlink explanation missing'
not_installed
echo 'mac_install_test: symlinked app passed'

kit bad-argument ''
run --open-now
[ "$status" = 2 ] || fail 'bad argument did not exit 2'
check "$scratch/err" 'Usage:' 'usage explanation missing'
not_installed
echo 'mac_install_test: bad argument passed'
