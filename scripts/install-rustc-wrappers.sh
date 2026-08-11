#!/bin/bash
# install-rustc-wrappers.sh — install the cargo rustc-wrapper/linker shims
#
# ~/.cargo/config.toml (per-host, NOT version-controlled) sets:
#   [build]
#   rustc-wrapper = "b00t-rustc-wrapper.sh"
#   [target.x86_64-unknown-linux-gnu]
#   linker = "b00t-cc-linker.sh"
#
# Cargo resolves both names via $PATH, so the scripts must live somewhere
# already on PATH — we use ~/.local/bin rather than shipping them in this
# repo, because:
#   - they are host/toolchain shims, not project source: every dev machine
#     needs its own copy on PATH regardless of which b00t worktree is
#     currently checked out (cargo invokes them for ANY crate build)
#   - checking them into the repo would tie a per-host PATH concern to a
#     specific worktree/branch, which breaks the moment that worktree is
#     removed
#   - keeping ~/.cargo/config.toml a stable, minimal filename reference
#     lets each host swap in a real wrapper (sccache, mold, ...) later
#     without touching repo-tracked files
#
# As shipped here, both are transparent no-op passthroughs — they exist so
# cargo's global config never fails with "program not found" on a fresh
# host. Swap in `sccache`/`mold`/etc. by editing the installed copies
# directly; this installer will not overwrite an existing file.

set -euo pipefail

BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
mkdir -p "$BIN_DIR"

install_shim() {
    local name="$1" body="$2"
    local path="$BIN_DIR/$name"
    if [ -e "$path" ]; then
        echo "skip: $path already exists" >&2
        return
    fi
    printf '%s\n' "$body" > "$path"
    chmod +x "$path"
    echo "installed: $path"
}

install_shim "b00t-rustc-wrapper.sh" '#!/usr/bin/env bash
# rustc-wrapper no-op: cargo invokes this as `<wrapper> <rustc> <args...>`.
# Passing straight through until a real wrapper (sccache, etc.) replaces this.
exec "$@"'

install_shim "b00t-cc-linker.sh" '#!/usr/bin/env bash
# linker no-op: cargo invokes this directly with the usual `cc` linker args.
# Passing straight through to the system cc until a real linker (mold, etc.) replaces this.
exec cc "$@"'

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "warning: $BIN_DIR is not on PATH — cargo will fail to find these shims" >&2 ;;
esac
