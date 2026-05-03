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

# Get latest release version
get_latest_version() {
    printf '%b\n' "${BLUE}🔍 Fetching latest release...${NC}"
    VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" |
              grep '"tag_name":' |
              sed -E 's/.*"([^"]+)".*/\1/')

    if [ -z "$VERSION" ]; then
        printf '%b\n' "${RED}Failed to get latest version from https://api.github.com/repos/$REPO/releases/latest${NC}" >&2
        exit 1
    fi

    printf '%b\n' "${GREEN}📦 Latest version: $VERSION${NC}"
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
        printf '%b\n' "${RED}Failed to download $download_url${NC}" >&2
        printf '%b\n' "${YELLOW}💡 Try: cargo install b00t-cli (requires Rust toolchain)${NC}" >&2
        rm -rf "$temp_dir"
        exit 1
    fi

    # Verify SHA256
    printf '%b\n' "${BLUE}🔐 Verifying SHA256...${NC}"
    if curl -fsSL "$sha_url" -o "$temp_dir/$sha_name" 2>/dev/null; then
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

# Verify installation — execute the binary, not just check the symlink
verify_installation() {
    printf '%b\n' "${BLUE}🔍 Verifying installation...${NC}"

    if ! command -v b00t >/dev/null 2>&1; then
        printf '%b\n' "${YELLOW}⚠️  b00t not found in PATH${NC}"
        printf '%b\n' "${BLUE}💡 Run: export PATH=\"$INSTALL_DIR:\$PATH\"${NC}"
        return 1
    fi

    local version_output
    if version_output=$(b00t --version 2>&1); then
        printf '%b\n' "${GREEN}✅ b00t installed: $version_output${NC}"
    else
        printf '%b\n' "${RED}❌ b00t found in PATH but --version failed${NC}" >&2
        return 1
    fi
}

# Main installation flow
main() {
    printf '%b\n' "${BLUE}🥾 b00t Universal Installer${NC}"
    printf '%b\n' "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    check_dependencies
    detect_platform
    get_latest_version
    install_binaries
    setup_config
    update_path
    verify_installation

    printf '\n'
    printf '%b\n' "${GREEN}🎉 Installation complete!${NC}"
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
