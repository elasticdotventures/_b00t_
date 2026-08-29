#!/bin/bash
# 🥾 b00t Universal Installer
# One-liner installation: curl -fsSL https://raw.githubusercontent.com/elasticdotventures/_b00t_/main/install.sh | sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REPO="elasticdotventures/_b00t_"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
CONFIG_DIR="${CONFIG_DIR:-$HOME/.config/b00t}"
B00T_HOME="${B00T_HOME:-$HOME/.b00t}"  # b00t installation directory (includes datums)

# Detect platform
detect_platform() {
    local os
    local arch
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    case "$arch" in
        x86_64)         arch="x86_64" ;;
        aarch64|arm64)  arch="aarch64" ;;
        armv7l)         arch="armv7" ;;
        *) printf '%s\n' "Unsupported architecture: $arch" >&2; exit 1 ;;
    esac

    case "$os" in
        linux)  PLATFORM="$arch-unknown-linux-gnu" ;;
        darwin) PLATFORM="$arch-apple-darwin" ;;
        *) printf '%s\n' "Unsupported OS: $os" >&2; exit 1 ;;
    esac
}

# Check dependencies
check_dependencies() {
    local deps=("curl" "tar")
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" >/dev/null 2>&1; then
            printf '%s\n' "Error: $dep is required but not installed" >&2
            exit 1
        fi
    done
}

# Get latest release version. Sets VERSION to empty on failure so the caller
# can fall back to a source build instead of hard-exiting (GitHub API can be
# unreachable/rate-limited on a clean box with no prior curl usage).
get_latest_version() {
    printf '%b\n' "${BLUE}🔍 Fetching latest release...${NC}"
    VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" |
              grep '"tag_name":' |
              sed -E 's/.*"([^"]+)".*/\1/')

    if [ -z "$VERSION" ]; then
        printf '%b\n' "${YELLOW}⚠️  Could not resolve latest release from GitHub API${NC}" >&2
        return 1
    fi

    printf '%b\n' "${GREEN}📦 Latest version: $VERSION${NC}"
}

# _b00t_/AGENT.md ships as a repo-relative symlink (../AGENTS.md) that only
# resolves if AGENTS.md lands as a sibling of _b00t_/ inside $B00T_HOME.
# Release tarballs built before this installer was patched don't include it,
# so `b00t whoami` dead-ends on a dangling symlink post-install. Backfill it
# from the same ref so both old and new release assets end up working.
repair_datum_symlinks() {
    if [ ! -f "$B00T_HOME/AGENTS.md" ]; then
        printf '%b\n' "${BLUE}🩹 Backfilling AGENTS.md (older release asset predates this fix)...${NC}"
        curl -fsSL "https://raw.githubusercontent.com/$REPO/main/AGENTS.md" -o "$B00T_HOME/AGENTS.md" || \
            printf '%b\n' "${YELLOW}⚠️  Could not backfill AGENTS.md — 'b00t whoami' may fail${NC}"
    fi
}

# Build from source when no release binary is available for this platform.
# This is the chicken-and-egg fallback: a clean box has curl+tar but no compiler,
# so we bootstrap rustup ourselves before building. Populates $B00T_HOME the same
# way install_binaries() does, so callers don't need to know which path ran.
install_from_source() {
    printf '%b\n' "${YELLOW}⚠️  Falling back to source build (rustup + cargo)...${NC}"

    if ! command -v cargo >/dev/null 2>&1; then
        printf '%b\n' "${BLUE}🦀 Installing Rust toolchain via rustup...${NC}"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
        # shellcheck disable=SC1091
        . "$HOME/.cargo/env"
    fi

    if ! command -v git >/dev/null 2>&1; then
        printf '%b\n' "${RED}❌ git is required for the source-build fallback but was not found${NC}" >&2
        exit 1
    fi

    local src_dir
    src_dir=$(mktemp -d)

    printf '%b\n' "${BLUE}📦 Cloning $REPO (HTTPS, shallow)...${NC}"
    if ! git clone --depth 1 "https://github.com/$REPO.git" "$src_dir/src"; then
        printf '%b\n' "${RED}❌ Clone failed — cannot fall back to source build${NC}" >&2
        rm -rf "$src_dir"
        exit 1
    fi

    # b00t-cli's only submodule path-dependency; other path deps are in-tree workspace members.
    (cd "$src_dir/src" && git submodule update --init --depth 1 vendor/runpod-sdk 2>/dev/null || true)

    printf '%b\n' "${BLUE}🔨 Building b00t-cli + b00t-mcp (this can take several minutes)...${NC}"
    if ! (cd "$src_dir/src" && cargo install --path b00t-cli --root "$src_dir/out" --force --quiet); then
        printf '%b\n' "${RED}❌ Source build of b00t-cli failed${NC}" >&2
        rm -rf "$src_dir"
        exit 1
    fi
    (cd "$src_dir/src" && cargo install --path b00t-mcp --root "$src_dir/out" --force --quiet) || \
        printf '%b\n' "${YELLOW}⚠️  b00t-mcp build failed — continuing with b00t-cli only${NC}"

    mkdir -p "$B00T_HOME"
    rm -rf "${B00T_HOME:?}"/*
    cp "$src_dir/out/bin/b00t-cli" "$B00T_HOME/b00t-cli"
    [ -f "$src_dir/out/bin/b00t-mcp" ] && cp "$src_dir/out/bin/b00t-mcp" "$B00T_HOME/b00t-mcp"
    cp -r "$src_dir/src/_b00t_" "$B00T_HOME/_b00t_"

    mkdir -p "$INSTALL_DIR"
    cp "$B00T_HOME/b00t-cli" "$INSTALL_DIR/b00t-cli"
    [ -f "$B00T_HOME/b00t-mcp" ] && cp "$B00T_HOME/b00t-mcp" "$INSTALL_DIR/b00t-mcp"
    ln -sf "b00t-cli" "$INSTALL_DIR/b00t"

    rm -rf "$src_dir"
    repair_datum_symlinks
    printf '%b\n' "${GREEN}✅ Installed from source to $B00T_HOME${NC}"
}

# Download and install binaries + datums
install_binaries() {
    local asset_name="b00t-${PLATFORM}.tar.gz"
    local sha_name="${asset_name}.sha256"
    local download_url="https://github.com/$REPO/releases/download/$VERSION/$asset_name"
    local sha_url="https://github.com/$REPO/releases/download/$VERSION/$sha_name"
    local temp_dir
    temp_dir=$(mktemp -d)

    printf '%b\n' "${BLUE}⬇️  Downloading $asset_name...${NC}"

    if ! curl -fsSL "$download_url" -o "$temp_dir/$asset_name"; then
        printf '%b\n' "${YELLOW}⚠️  No release binary for $PLATFORM (or download failed)${NC}" >&2
        rm -rf "$temp_dir"
        install_from_source
        return
    fi

    # Verify SHA256
    printf '%b\n' "${BLUE}🔐 Verifying SHA256...${NC}"
    if curl -fsSL "$sha_url" -o "$temp_dir/$sha_name" 2>/dev/null; then
        # Sidecar records only a checksum + a filename column; normalize that
        # filename to the bare asset name regardless of what path the CI build
        # embedded (older releases wrote "dist/<asset>", not "<asset>").
        local expected_sum
        expected_sum=$(awk '{print $1; exit}' "$temp_dir/$sha_name")
        printf '%s  %s\n' "$expected_sum" "$asset_name" > "$temp_dir/$sha_name"

        # sha256sum on Linux, shasum -a 256 on macOS
        local verified_ok=0
        if command -v sha256sum >/dev/null 2>&1; then
            (cd "$temp_dir" && sha256sum -c "$sha_name") || {
                printf '%b\n' "${RED}SHA256 mismatch — aborting installation${NC}" >&2
                rm -rf "$temp_dir"
                exit 1
            }
            verified_ok=1
        elif command -v shasum >/dev/null 2>&1; then
            (cd "$temp_dir" && shasum -a 256 -c "$sha_name") || {
                printf '%b\n' "${RED}SHA256 mismatch — aborting installation${NC}" >&2
                rm -rf "$temp_dir"
                exit 1
            }
            verified_ok=1
        else
            printf '%b\n' "${YELLOW}⚠️  No SHA256 verification tool (sha256sum/shasum) found, skipping verification${NC}"
        fi
        if [ "$verified_ok" -eq 1 ]; then
            printf '%b\n' "${GREEN}✅ SHA256 verified${NC}"
        fi
    else
        printf '%b\n' "${YELLOW}⚠️  No SHA256 sidecar found, skipping verification${NC}"
    fi

    printf '%b\n' "${BLUE}📂 Extracting to $B00T_HOME...${NC}"
    mkdir -p "$B00T_HOME"
    tar -xzf "$temp_dir/$asset_name" -C "$temp_dir"

    # Move extracted b00t/ contents to B00T_HOME
    if [ -d "$temp_dir/b00t" ]; then
        rm -rf "$B00T_HOME"/*
        cp -r "$temp_dir/b00t"/* "$B00T_HOME/"

        # Make binaries executable
        chmod +x "$B00T_HOME/b00t-cli" "$B00T_HOME/b00t-mcp" 2>/dev/null || true

        # Install binaries to INSTALL_DIR (copy, not symlink — more robust)
        mkdir -p "$INSTALL_DIR"
        cp "$B00T_HOME/b00t-cli" "$INSTALL_DIR/b00t-cli"
        cp "$B00T_HOME/b00t-mcp" "$INSTALL_DIR/b00t-mcp" 2>/dev/null || true
        ln -sf "b00t-cli" "$INSTALL_DIR/b00t"

        repair_datum_symlinks

        printf '%b\n' "${GREEN}✅ Installed binaries and datums to $B00T_HOME${NC}"
        printf '%b\n' "${GREEN}✅ Installed binaries to $INSTALL_DIR${NC}"
    else
        printf '%b\n' "${RED}❌ Unexpected tarball structure (expected $temp_dir/b00t/)${NC}" >&2
        rm -rf "$temp_dir"
        exit 1
    fi

    rm -rf "$temp_dir"
}

# Setup configuration
setup_config() {
    printf '%b\n' "${BLUE}⚙️  Setting up configuration...${NC}"
    mkdir -p "$CONFIG_DIR"

    # Create basic config only if it doesn't exist — never overwrite
    if [ ! -f "$CONFIG_DIR/config.toml" ]; then
        local current_user
        current_user=$(whoami)
        cat > "$CONFIG_DIR/config.toml" << EOF
# b00t Configuration
# Generated by install script

[user]
name = "$current_user"

[development]
auto_update = true

[security]
keyring_enabled = true
EOF
        printf '%b\n' "${GREEN}📝 Created default config at $CONFIG_DIR/config.toml${NC}"
    fi
}

# Update PATH and set _B00T_Path — detect shell from $SHELL, not $BASH_VERSION
update_path() {
    local shell_rc=""

    case "${SHELL:-}" in
        */zsh)  shell_rc="$HOME/.zshrc" ;;
        */bash) shell_rc="$HOME/.bashrc" ;;
        */fish) shell_rc="$HOME/.config/fish/config.fish" ;;
        *)      shell_rc="$HOME/.profile" ;;
    esac

    if ! grep -q "# Added by b00t installer" "$shell_rc" 2>/dev/null; then
        printf '%b\n' "${BLUE}🔧 Configuring shell environment in $shell_rc...${NC}"
        # Ensure parent directory exists (e.g. ~/.config/fish/)
        mkdir -p "$(dirname "$shell_rc")"
        # Use the RC file path to determine syntax, not $SHELL, to be robust
        case "$shell_rc" in
            */.config/fish/*)
                # Fish does not support `export`; use set -gx with list syntax
                cat >> "$shell_rc" << EOF

# Added by b00t installer
set -gx PATH "$INSTALL_DIR" \$PATH
set -gx _B00T_Path "$B00T_HOME/_b00t_"
EOF
                ;;
            *)
                cat >> "$shell_rc" << EOF

# Added by b00t installer
export PATH="$INSTALL_DIR:\$PATH"
export _B00T_Path="$B00T_HOME/_b00t_"
EOF
                ;;
        esac
        printf '%b\n' "${GREEN}✅ Shell configuration updated ($shell_rc)${NC}"
    else
        printf '%b\n' "${BLUE}💡 Shell already configured for b00t${NC}"
    fi

    # Set for current session
    export PATH="$INSTALL_DIR:$PATH"
    export _B00T_Path="$B00T_HOME/_b00t_"
}

# Verify installation — execute the binary, not just check the symlink.
# Uses this script's own updated PATH/_B00T_Path, since `curl | sh` runs in a
# subshell: the exports in update_path() never reach the caller's interactive
# shell, only this process. That's why the final message below tells the user
# to source their rc file rather than claiming the "b00t" command is ready.
verify_installation() {
    printf '%b\n' "${BLUE}🔍 Verifying installation...${NC}"

    if ! command -v b00t >/dev/null 2>&1; then
        printf '%b\n' "${RED}❌ b00t not found even with updated PATH — installation failed${NC}" >&2
        return 1
    fi

    local version_output
    if version_output=$(b00t --version 2>&1); then
        printf '%b\n' "${GREEN}✅ b00t installed: $version_output${NC}"
    else
        printf '%b\n' "${RED}❌ b00t found in PATH but --version failed${NC}" >&2
        return 1
    fi

    if ! B00T_WHOAMI_OUT=$(b00t whoami 2>&1); then
        printf '%b\n' "${YELLOW}⚠️  b00t whoami failed — datums may not be readable at \$_B00T_Path${NC}"
        printf '%s\n' "$B00T_WHOAMI_OUT"
        return 1
    fi
    printf '%b\n' "${GREEN}✅ datums readable (_B00T_Path=$B00T_HOME/_b00t_)${NC}"
}

# Main installation flow
main() {
    printf '%b\n' "${BLUE}🥾 b00t Universal Installer${NC}"
    printf '%b\n' "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    check_dependencies
    detect_platform
    if get_latest_version; then
        install_binaries
    else
        install_from_source
    fi
    setup_config
    update_path
    verify_installation

    printf '\n'
    printf '%b\n' "${GREEN}🎉 Installation complete!${NC}"
    printf '%b\n' "${YELLOW}⚠️  This ran in a subshell — your current terminal doesn't have PATH/_B00T_Path yet.${NC}"
    printf '%b\n' "${BLUE}💡 Run this now, or open a new terminal:${NC}"
    printf '%s\n' "   source ~/.bashrc   # or ~/.zshrc, per your shell"
    printf '\n'
    printf '%b\n' "${BLUE}💡 Quick start:${NC}"
    printf '%s\n' "   b00t --help"
    printf '%s\n' "   b00t status"
    printf '%s\n' "   b00t learn rust"
    printf '\n'
    printf '%b\n' "${BLUE}📚 Documentation: https://github.com/$REPO${NC}"
}

# Handle script arguments
case "${1:-}" in
    --help|-h)
        printf '%s\n' "b00t Universal Installer"
        printf '\n'
        printf '%s\n' "Usage: $0 [options]"
        printf '\n'
        printf '%s\n' "Options:"
        printf '%s\n' "  --help, -h     Show this help message"
        printf '%s\n' "  --version, -v  Show installer version"
        printf '\n'
        printf '%s\n' "Environment variables:"
        printf '%s\n' "  INSTALL_DIR    Binary directory (default: \$HOME/.local/bin)"
        printf '%s\n' "  B00T_HOME      b00t home directory (default: \$HOME/.b00t)"
        printf '%s\n' "  CONFIG_DIR     Config directory (default: \$HOME/.config/b00t)"
        printf '\n'
        printf '%s\n' "Examples:"
        printf '%s\n' "  curl -fsSL https://raw.githubusercontent.com/$REPO/main/install.sh | sh"
        exit 0
        ;;
    --version|-v)
        printf '%s\n' "b00t-installer 1.0.0"
        exit 0
        ;;
    *)
        main "$@"
        ;;
esac
