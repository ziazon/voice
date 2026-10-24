#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

for t in crew_decisions crew_sources crew_labels crew_hardening crew_documents crew_rounds slack_mine slack_select crew_personas growth_loop_spec; do
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
  echo 'cargo test --locked'
  cargo test --locked
else
  echo 'ci-local: cargo not found, skipping the Rust checks (install Rust from https://rustup.rs)'
fi

echo 'ci-local: all passed'
