#!/bin/bash
# 🤓 bootstrap-vendor-ledgrrr — populate vendor/ledgrrr on a fresh clone
#
# vendor/ledgrrr stopped being a git submodule in commit 99417975
# ("convert ledgrrr from submodule to shared worktree"), which dropped it
# from .gitmodules and .git's gitlink so `git submodule update --init`
# no longer touches it. On machines that already had the old submodule
# checkout on disk this was invisible, but the root Cargo.toml still has a
# hard path dependency on vendor/ledgrrr/crates/b00t-reflect-types, so any
# genuinely fresh clone (a new dev machine, a CI runner, `git clone` into
# /tmp) fails to resolve the workspace manifest at all. Confirmed live on
# fung1 2026-09-13: `git submodule update --init vendor/ledgrrr` errors
# with "pathspec 'vendor/ledgrrr' did not match any file(s) known to git".
#
# This script is the missing bootstrap step: clone the canonical
# promptexecution/ledgrrr repo into vendor/ledgrrr if it isn't already a
# working checkout. Safe to run repeatedly — a no-op once populated.
set -euo pipefail

REPO_ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
LEDGRRR_DIR="${REPO_ROOT}/vendor/ledgrrr"
LEDGRRR_URL="https://github.com/PromptExecution/ledgrrr.git"

if [ -f "${LEDGRRR_DIR}/Cargo.toml" ]; then
  echo "vendor/ledgrrr already populated, nothing to do"
  exit 0
fi

echo "vendor/ledgrrr missing or empty — cloning ${LEDGRRR_URL}"
rm -rf "${LEDGRRR_DIR}"
git clone --depth 1 --branch main "${LEDGRRR_URL}" "${LEDGRRR_DIR}"
