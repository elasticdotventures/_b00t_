#!/usr/bin/env bash
# Verify actual gate evaluation and CLI dispositions, using the shared worktree cache.
set -euo pipefail
system_normal_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${system_normal_repo_root}/scripts/lib/worktree-env.sh"
export CARGO_TARGET_DIR="$(b00t_shared_cargo_target_dir)"
cd "${system_normal_repo_root}"
cargo test -p b00t-cli --lib gates:: --no-default-features
cargo test -p b00t-cli --test checklist_probe_test --no-default-features
cargo build -p b00t-cli --bin b00t --no-default-features
uv run --no-project --python python3 python b00t-cli/tests/system_normal_git_test.py
B00T_TEST_BINARY="${CARGO_TARGET_DIR}/debug/b00t" \
    uv run --no-project --python python3 python b00t-cli/tests/checklist_probe_cli_test.py
printf '%s\n' 'PASS: system-normal bounded probes and three-valued CLI'
