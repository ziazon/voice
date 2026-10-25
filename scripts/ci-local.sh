#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

for t in crew_decisions crew_sources crew_extra_sources crew_labels crew_hardening crew_documents crew_rounds slack_mine slack_select crew_personas growth_loop_spec; do
  echo "python3 tests/${t}_test.py"
  python3 "tests/${t}_test.py"
done
echo 'bash tests/crew_install_test.sh'
bash tests/crew_install_test.sh
echo 'shellcheck scripts/ci-local.sh bootstrap/install.sh tests/crew_install_test.sh'
shellcheck scripts/ci-local.sh bootstrap/install.sh tests/crew_install_test.sh

if command -v cargo >/dev/null 2>&1; then
  echo 'cargo fmt --check'
  cargo fmt --check
  echo 'cargo clippy --all-targets --locked -- -D warnings'
  cargo clippy --all-targets --locked -- -D warnings
  echo 'PERSONA_BUILDER_REQUIRE_PARITY=1 cargo test --locked'
  PERSONA_BUILDER_REQUIRE_PARITY=1 cargo test --locked
  echo 'git diff --exit-code -- builder/ui/src/types/generated'
  git diff --exit-code -- builder/ui/src/types/generated
  if [ -n "$(git status --porcelain -- builder/ui/src/types/generated)" ]; then
    echo 'ci-local: builder/ui/src/types/generated has files cargo test wrote that are not committed'
    exit 1
  fi
  # The Windows and Linux password stores are pure Rust, so any computer can check them.
  for target in x86_64-pc-windows-msvc x86_64-unknown-linux-gnu; do
    if rustup target list --installed 2>/dev/null | grep -qx "$target"; then
      echo "cargo check --locked -p persona-builder-keystore --all-targets --target $target"
      cargo check --locked -p persona-builder-keystore --all-targets --target "$target"
    else
      echo "ci-local: the $target target is missing, skipping its check (rustup target add $target)"
    fi
  done
else
  echo 'ci-local: cargo not found, skipping the Rust checks (install Rust from https://rustup.rs)'
fi

if command -v pnpm >/dev/null 2>&1; then
  pnpm_version=$(pnpm --version)
  pnpm_major=${pnpm_version%%.*}
  if [[ "$pnpm_major" == *[!0-9]* || -z "$pnpm_major" ]]; then
    echo "ci-local: couldn't read pnpm's version, skipping the app screens (update pnpm: https://pnpm.io/installation)"
  elif [ "$pnpm_major" -lt 10 ]; then
    echo "ci-local: pnpm $pnpm_version is older than 10, skipping the app screens (update pnpm: https://pnpm.io/installation)"
  else
    for step in 'install --frozen-lockfile' typecheck test build; do
      echo "pnpm -C builder/ui $step"
      # shellcheck disable=SC2086
      pnpm -C builder/ui $step
    done
  fi
else
  echo 'ci-local: pnpm not found, skipping the app screens (install Node and pnpm: https://pnpm.io/installation)'
fi

echo 'ci-local: all passed'
