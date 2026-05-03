#!/bin/bash

# Alternative approach - define VS Code detection inline in .bash_profile
# Replace the vscode-detection.sh sourcing with this:

# Check if running in VS Code integrated terminal
function is_vscode_shell() {
    [[ -n "${VSCODE_GIT_IPC_HANDLE:-}" ]] || \
    [[ "${TERM_PROGRAM:-}" == "vscode" ]] || \
    [[ -n "${VSCODE_IPC_HOOK_CLI:-}" ]] || \
    [[ -n "${VSCODE_INJECTION:-}" ]]
}

# Simple logging function
function log_📢_记录() {
    echo "$@"
}

if is_vscode_shell; then
    log_📢_记录 "🥾💻 hi VS Code! running b00t-cli"
    # Only load full _b00t_ if we're in VS Code
    if [ -f ~/.dotfiles/_b00t_/_b00t_.bashrc ] ; then
        echo "🥾 _b00t_ (in vscode)"
        . ~/.dotfiles/_b00t_/_b00t_.bashrc
        echo "/🥾"
    fi
    b00t-cli vscode
else
    log_📢_记录 "Not VSCODE"
fi
