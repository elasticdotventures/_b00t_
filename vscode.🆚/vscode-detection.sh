#!/bin/bash
# VS Code detection functions - lightweight alternative to sourcing full _b00t_.bashrc

# Check if running in VS Code integrated terminal
# Returns 0 (success) if in VS Code, 1 (failure) otherwise
# Usage: if is_vscode_shell; then echo "In VS Code terminal"; fi
unset -f is_vscode_shell
function is_vscode_shell() {
    # Check multiple VS Code environment variables for better detection
    [[ -n "${VSCODE_GIT_IPC_HANDLE:-}" ]] || \
    [[ "${TERM_PROGRAM:-}" == "vscode" ]] || \
    [[ -n "${VSCODE_IPC_HOOK_CLI:-}" ]] || \
    [[ -n "${VSCODE_INJECTION:-}" ]]
}

export -f is_vscode_shell


