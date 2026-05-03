#!/bin/sh
#
# SPDX-FileCopyrightText: 2025-present Artem Lykhvar and contributors
#
# SPDX-License-Identifier: MIT
#
set -eu

GH_REPO="vet-run/vet"
INSTALL_DIR_DEFAULT="/usr/local/bin"
SCRIPT_NAME="vet"

log_err() { echo "$*" >&2; }
log_info() { echo "==> $*"; }

find_downloader() {
    if command -v curl >/dev/null 2>&1; then
        echo "curl -fsSL"
    elif command -v wget >/dev/null 2>&1; then
        echo "wget -qO-"
    else
        log_err "ERROR: This installer requires either 'curl' or 'wget'."
        exit 1
    fi
}

main() {
    downloader=$(find_downloader)
    install_dir=${INSTALL_DIR:-$INSTALL_DIR_DEFAULT}
    tag=${VET_VERSION:-"latest"}

    if [ "$tag" = "latest" ]; then
        download_url="https://github.com/${GH_REPO}/releases/latest/download/${SCRIPT_NAME}"
    else
        download_url="https://github.com/${GH_REPO}/releases/download/${tag}/${SCRIPT_NAME}"
    fi

    log_info "Installing '${SCRIPT_NAME}' to '${install_dir}'"

    if [ ! -d "$install_dir" ]; then
        log_err "ERROR: Installation directory '${install_dir}' does not exist."
        log_err "Please create it first, or set INSTALL_DIR to a different location."
        exit 1
    fi
    if [ ! -w "$install_dir" ]; then
        log_err "ERROR: No write permissions for '${install_dir}'."
        log_err "Please run with sudo or set INSTALL_DIR to a writable location."
        log_err "Example: curl ... | sh -s -- -d /path/to/dir"
        log_err "Or: curl ... | sudo sh"
        exit 1
    fi

    install_path="${install_dir}/${SCRIPT_NAME}"

    log_info "Downloading from: ${download_url}"

    if ! $downloader "$download_url" > "$install_path"; then
        log_err "ERROR: Download failed. Check the release version and your network."
        exit 1
    fi

    chmod +x "$install_path"

    echo "----------------------------------------------------------------------"
    echo "'${SCRIPT_NAME}' installed successfully!"
    echo "Run '${SCRIPT_NAME} --help' to get started."

    needs_shellcheck=0
    needs_bat=0
    optional_tools=""

    command -v shellcheck >/dev/null 2>&1 || { needs_shellcheck=1; optional_tools="${optional_tools} shellcheck"; }
    if ! command -v bat >/dev/null 2>&1 && ! command -v batcat >/dev/null 2>&1; then
        needs_bat=1
        optional_tools="${optional_tools} bat"
    fi
    if [ -n "$optional_tools" ]; then
        echo "For the best experience, consider installing these optional tools:"
        if [ "$needs_shellcheck" -eq 1 ]; then
            printf "  • shellcheck: For automatic script analysis.\n"
        fi
        if [ "$needs_bat" -eq 1 ]; then
            printf "  • bat:        For syntax-highlighted previews.\n"
        fi

        if command -v "apt-get" >/dev/null 2>&1; then
            printf "\n  On Debian/Ubuntu: sudo apt install%s\n" "$optional_tools"
        elif command -v "brew" >/dev/null 2>&1; then
            printf "\n  On macOS/Linux (with Homebrew): brew install%s\n" "$optional_tools"
        fi
    fi
    echo "----------------------------------------------------------------------"
}

main
