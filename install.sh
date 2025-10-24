#!/bin/bash
# 🥾 b00t Universal Installer
# One-liner installation: curl -fsSL https://raw.githubusercontent.com/elasticdotventures/dotfiles/main/install.sh | sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REPO="elasticdotventures/dotfiles"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
CONFIG_DIR="${CONFIG_DIR:-$HOME/.config/b00t}"
USE_PKGX="${USE_PKGX:-auto}"  # auto, true, false

# Check if pkgx is available (minimal install option)
check_pkgx() {
    if [ "$USE_PKGX" = "false" ]; then
        return 1
    fi

    if command -v pkgx >/dev/null 2>&1; then
        return 0
    fi

    # If USE_PKGX=auto, offer to install pkgx
    if [ "$USE_PKGX" = "auto" ]; then
        echo "${BLUE}🔍 pkgx not found. pkgx provides minimal installation (4 MiB vs 1 GB Rust toolchain)${NC}"
        echo "${YELLOW}💡 Install pkgx for faster, cleaner b00t installation? [y/N]${NC}"
        read -r response
        if [[ "$response" =~ ^[Yy]$ ]]; then
            echo "${BLUE}📦 Installing pkgx...${NC}"
            if curl -Ssf https://pkgx.sh | sh; then
                export PATH="$HOME/.local/bin:$PATH"
                return 0
            else
                echo "${YELLOW}⚠️  pkgx installation failed, falling back to binary method${NC}"
                return 1
            fi
        fi
    fi

    return 1
}

# Install via pkgx (preferred method)
install_via_pkgx() {
    echo "${BLUE}🥾 Installing b00t via pkgx (minimal method)...${NC}"

    # Install b00t-cli via pkgx
    if pkgx +b00t-cli; then
        echo "${GREEN}✅ b00t-cli installed via pkgx${NC}"

        # Verify installation
        if pkgx b00t-cli --version >/dev/null 2>&1; then
            echo "${GREEN}✅ Installation verified${NC}"
            return 0
        fi
    fi

    echo "${RED}❌ pkgx installation failed${NC}"
    return 1
}

# Detect platform
detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)
    
    case "$arch" in
        x86_64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        armv7l) arch="armv7" ;;
        *) echo "${RED}Unsupported architecture: $arch${NC}" >&2; exit 1 ;;
    esac
    
    case "$os" in
        linux) PLATFORM="$arch-unknown-linux-gnu" ;;
        darwin) PLATFORM="$arch-apple-darwin" ;;
        *) echo "${RED}Unsupported OS: $os${NC}" >&2; exit 1 ;;
    esac
}

# Check dependencies
check_dependencies() {
    local deps=("curl" "tar")
    for dep in "${deps[@]}"; do
        if ! command -v "$dep" >/dev/null 2>&1; then
            echo "${RED}Error: $dep is required but not installed${NC}" >&2
            exit 1
        fi
    done
}

# Get latest release version
get_latest_version() {
    echo "${BLUE}🔍 Fetching latest release...${NC}"
    VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | 
              grep '"tag_name":' | 
              sed -E 's/.*"([^"]+)".*/\1/')
    
    if [ -z "$VERSION" ]; then
        echo "${RED}Failed to get latest version${NC}" >&2
        exit 1
    fi
    
    echo "${GREEN}📦 Latest version: $VERSION${NC}"
}

# Download and install binaries
install_binaries() {
    local asset_name="b00t-${PLATFORM}.tar.gz"
    local download_url="https://github.com/$REPO/releases/download/$VERSION/$asset_name"
    local temp_dir=$(mktemp -d)
    
    echo "${BLUE}⬇️  Downloading $asset_name...${NC}"
    
    if ! curl -fsSL "$download_url" -o "$temp_dir/$asset_name"; then
        echo "${RED}Failed to download release asset${NC}" >&2
        echo "${YELLOW}💡 Trying container-based installation...${NC}"
        install_from_container
        return
    fi
    
    echo "${BLUE}📂 Extracting to $INSTALL_DIR...${NC}"
    mkdir -p "$INSTALL_DIR"
    tar -xzf "$temp_dir/$asset_name" -C "$INSTALL_DIR"
    
    # Make binaries executable
    chmod +x "$INSTALL_DIR/b00t-cli" "$INSTALL_DIR/b00t-mcp" 2>/dev/null || true
    
    # Create symlink for easier access
    ln -sf "$INSTALL_DIR/b00t-cli" "$INSTALL_DIR/b00t" 2>/dev/null || true
    
    rm -rf "$temp_dir"
}

# Fallback: Container-based installation
install_from_container() {
    if ! command -v docker >/dev/null 2>&1; then
        echo "${RED}❌ Docker not found and no binary release available${NC}" >&2
        echo "${YELLOW}💡 Consider installing via cargo: cargo install b00t-cli${NC}"
        exit 1
    fi
    
    echo "${BLUE}🐳 Installing via Docker container...${NC}"
    
    # Create wrapper script
    cat > "$INSTALL_DIR/b00t" << 'EOF'
#!/bin/bash
exec docker run --rm -it \
    -v "$PWD:/workspace" \
    -v "$HOME/.config/b00t:/root/.config/b00t" \
    ghcr.io/elasticdotventures/b00t-cli:latest \
    "$@"
EOF
    
    chmod +x "$INSTALL_DIR/b00t"
    echo "${GREEN}✅ Container-based b00t installed to $INSTALL_DIR/b00t${NC}"
}

# Setup configuration
setup_config() {
    echo "${BLUE}⚙️  Setting up configuration...${NC}"
    mkdir -p "$CONFIG_DIR"
    
    # Create basic config if it doesn't exist
    if [ ! -f "$CONFIG_DIR/config.toml" ]; then
        cat > "$CONFIG_DIR/config.toml" << 'EOF'
# b00t Configuration
# Generated by install script

[user]
name = "operator"

[development]
auto_update = true

[security]
keyring_enabled = true
EOF
        echo "${GREEN}📝 Created default config at $CONFIG_DIR/config.toml${NC}"
    fi
}

# Update PATH
update_path() {
    local shell_rc=""
    
    # Detect shell config file
    if [ -n "$ZSH_VERSION" ]; then
        shell_rc="$HOME/.zshrc"
    elif [ -n "$BASH_VERSION" ]; then
        shell_rc="$HOME/.bashrc"
    else
        shell_rc="$HOME/.profile"
    fi
    
    # Check if PATH already contains install directory
    if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
        echo "${BLUE}🔧 Adding $INSTALL_DIR to PATH in $shell_rc...${NC}"
        echo "" >> "$shell_rc"
        echo "# Added by b00t installer" >> "$shell_rc"
        echo "export PATH=\"$INSTALL_DIR:\$PATH\"" >> "$shell_rc"
        export PATH="$INSTALL_DIR:$PATH"
    fi
}

# Verify installation
verify_installation() {
    echo "${BLUE}🔍 Verifying installation...${NC}"
    
    if command -v b00t >/dev/null 2>&1; then
        local version_output=$(b00t --version 2>/dev/null || echo "unknown")
        echo "${GREEN}✅ b00t installed successfully: $version_output${NC}"
    else
        echo "${YELLOW}⚠️  b00t command not found in PATH${NC}"
        echo "${BLUE}💡 Try running: export PATH=\"$INSTALL_DIR:\$PATH\"${NC}"
    fi
}

# Main installation flow
main() {
    echo "${BLUE}🥾 b00t Universal Installer${NC}"
    echo "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

    # Try pkgx first (minimal install, preferred method)
    if check_pkgx; then
        echo "${BLUE}✨ Using pkgx for minimal installation${NC}"
        if install_via_pkgx; then
            setup_config
            echo ""
            echo "${GREEN}🎉 Installation complete (via pkgx)!${NC}"
            echo "${BLUE}💡 Quick start:${NC}"
            echo "   pkgx b00t-cli --help"
            echo "   pkgx b00t-cli status"
            echo "   pkgx b00t-cli learn rust"
            echo ""
            echo "${BLUE}💡 Or install permanently:${NC}"
            echo "   pkgx +b00t-cli  # Adds to ~/.local/bin"
            echo ""
            echo "${BLUE}📚 Documentation: https://github.com/$REPO${NC}"
            return 0
        fi
        echo "${YELLOW}⚠️  pkgx method failed, falling back to binary installation${NC}"
    fi

    # Fallback: Binary installation from GitHub releases
    detect_platform
    check_dependencies
    get_latest_version
    install_binaries
    setup_config
    update_path
    verify_installation

    echo ""
    echo "${GREEN}🎉 Installation complete!${NC}"
    echo "${BLUE}💡 Quick start:${NC}"
    echo "   b00t --help"
    echo "   b00t status"
    echo "   b00t learn rust"
    echo ""
    echo "${BLUE}📚 Documentation: https://github.com/$REPO${NC}"
}

# Handle script arguments
case "${1:-}" in
    --help|-h)
        echo "b00t Universal Installer"
        echo ""
        echo "Usage: $0 [options]"
        echo ""
        echo "Options:"
        echo "  --help, -h     Show this help message"
        echo "  --version, -v  Show installer version"
        echo ""
        echo "Environment variables:"
        echo "  INSTALL_DIR    Installation directory (default: \$HOME/.local/bin)"
        echo "  CONFIG_DIR     Configuration directory (default: \$HOME/.config/b00t)"
        echo "  USE_PKGX       Use pkgx for installation: auto (default), true, false"
        echo ""
        echo "Examples:"
        echo "  # Default: auto-detect pkgx"
        echo "  curl -fsSL https://raw.githubusercontent.com/$REPO/main/install.sh | sh"
        echo ""
        echo "  # Force pkgx method"
        echo "  curl -fsSL https://raw.githubusercontent.com/$REPO/main/install.sh | USE_PKGX=true sh"
        echo ""
        echo "  # Skip pkgx, use binary method"
        echo "  curl -fsSL https://raw.githubusercontent.com/$REPO/main/install.sh | USE_PKGX=false sh"
        exit 0
        ;;
    --version|-v)
        echo "b00t-installer 1.0.0"
        exit 0
        ;;
    *)
        main "$@"
        ;;
esac