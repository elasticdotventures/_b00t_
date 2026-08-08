#!/usr/bin/env bash
# Shared defaults for worktree placement, build-cache reuse, and package-manager detection.

b00t_default_hive_worktree_root() {
    printf '%s\n' "${B00T_WORKTREE_ROOT:-$HOME/.cache/b00t-worktrees}"
}

b00t_default_agent_worktree_root() {
    local project_root="${1:?project_root required}"
    printf '%s\n' "${B00T_AGENT_WORKTREE_ROOT:-${project_root}/.claude/worktrees}"
}

b00t_shared_cargo_target_dir() {
    if [ -n "${CARGO_TARGET_DIR:-}" ]; then
        printf '%s\n' "${CARGO_TARGET_DIR}"
        return 0
    fi
    printf '%s\n' "${B00T_SHARED_CARGO_TARGET_DIR:-$HOME/.cache/b00t-cargo-target}"
}

b00t_detect_package_manager() {
    local project_root="${1:-$PWD}"
    local detect_script="${project_root}/scripts/setup-package-manager.js"
    local detected=""

    if command -v node >/dev/null 2>&1 && [ -f "${detect_script}" ]; then
        detected="$(
            (
                cd "${project_root}" && \
                node "${detect_script}" --detect
            ) 2>/dev/null \
                | awk -F': *' '/Selected:/ { print $2; exit }'
        )"
    fi

    if [ -n "${detected}" ]; then
        printf '%s\n' "${detected}"
        return 0
    fi

    if [ -f "${project_root}/pnpm-lock.yaml" ]; then
        printf '%s\n' "pnpm"
    elif [ -f "${project_root}/bun.lockb" ]; then
        printf '%s\n' "bun"
    elif [ -f "${project_root}/yarn.lock" ]; then
        printf '%s\n' "yarn"
    else
        printf '%s\n' "npm"
    fi
}

b00t_init_required_submodules() {
    local worktree_dir="${1:?worktree_dir required}"
    git -C "${worktree_dir}" submodule update --init -- \
        vendor/ledgrrr \
        vendor/runpod-sdk \
        vendor/embed-anything-b00t
}
