#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

for t in crew_decisions crew_sources crew_labels crew_hardening slack_mine slack_select crew_personas growth_loop_spec; do
  echo "python3 tests/${t}_test.py"
  python3 "tests/${t}_test.py"
done
echo 'bash tests/crew_install_test.sh'
bash tests/crew_install_test.sh
echo 'shellcheck scripts/ci-local.sh bootstrap/install.sh tests/crew_install_test.sh'
shellcheck scripts/ci-local.sh bootstrap/install.sh tests/crew_install_test.sh

echo 'ci-local: all passed'
