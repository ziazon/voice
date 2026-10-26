#!/usr/bin/env bash
# Installs Persona Builder into your own ~/Applications folder.
# Clears quarantine on the installed copy because it is ad-hoc signed, not by Apple yet (PB-D3).
set -euo pipefail

no_open=false
case "$#:${1-}" in
  0:) ;;
  1:--no-open) no_open=true ;;
  *) echo 'Usage: bash install.sh [--no-open]' >&2; exit 2 ;;
esac

fail() { echo "$*" >&2; exit 1; }
[ "$(uname -s)" = Darwin ] || fail "Persona Builder's installer is for Macs only."
[ "$EUID" != 0 ] || fail 'Please run this without sudo. Persona Builder installs into your own Applications folder and needs no password.'

here="$(cd "$(dirname "$0")" && pwd)"
source_app="$here/Persona Builder.app"
if [ ! -d "$source_app" ] || [ -L "$source_app" ]; then
  fail "Persona Builder.app isn't next to this installer. Unzip the file you were sent again and run install.sh from the folder it makes."
fi
damaged='This copy of Persona Builder looks damaged. Ask for a new copy.'
codesign --verify --deep --strict "$source_app" >/dev/null 2>&1 || fail "$damaged"

executable=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$source_app/Contents/Info.plist" 2>/dev/null) || fail "$damaged"
architectures=$(lipo -archs "$source_app/Contents/MacOS/$executable" 2>/dev/null) || fail "$damaged"
if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then
  mac_arch=arm64
  this_mac='an Apple chip'
else
  mac_arch=x86_64
  this_mac='an Intel chip'
fi
read -r -a arch_list <<< "$architectures"
compatible=false
made_for=''
for arch in "${arch_list[@]}"; do
  if [ "$arch" = "$mac_arch" ] || { [ "$mac_arch" = arm64 ] && [ "$arch" = arm64e ]; }; then
    compatible=true
  fi
  case "$arch" in
    arm64|arm64e) chip='Apple chip' ;;
    x86_64) chip='Intel chip' ;;
    *) chip="$arch" ;;
  esac
  made_for="${made_for:+$made_for and }$chip"
done
"$compatible" || fail "This copy is made for $made_for Macs, and this Mac has $this_mac. Ask for a copy made for this Mac."

mkdir -p "$HOME/Applications"
staging=$(mktemp -d "$HOME/Applications/.persona-builder-install.XXXXXX")
trap 'rm -rf "$staging"' EXIT
copy="$staging/Persona Builder.app"
ditto "$source_app" "$copy"
not_ready="Persona Builder couldn't be made ready to open. Nothing was installed."
if ! quarantine_error=$(xattr -dr com.apple.quarantine "$copy" 2>&1); then
  case "$quarantine_error" in
    *'No such xattr'*|*'No such attribute'*) ;;
    *) fail "$not_ready" ;;
  esac
fi
attributes=$(xattr -lr "$copy" 2>/dev/null) || fail "$not_ready"
[[ "$attributes" != *com.apple.quarantine* ]] || fail "$not_ready"
codesign --verify --deep --strict "$copy" >/dev/null 2>&1 || fail "$damaged"

installed="$HOME/Applications/Persona Builder.app"
rm -rf "$installed"
mv "$copy" "$installed"
echo "Persona Builder is installed in your Applications folder: $installed"
if ! "$no_open"; then
  open "$installed"
fi
