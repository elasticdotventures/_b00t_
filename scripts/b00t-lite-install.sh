#!/bin/bash
# b00t-lite-install.sh — Self-installing, self-detecting b00t bootstrap
set -euo pipefail

B00T_HOME="${B00T_HOME:-$HOME/.b00t}"
B00T_REPO="${B00T_REPO:-git@github.com:elasticdotventures/_b00t_.git}"
B00T_BRANCH="${B00T_BRANCH:-main}"

# ─── Step 1: Detect variant ───────────────────────────────────────────
detect_variant() {
    local variant_file="$B00T_HOME/_b00t_/schema/variant.toml"
    local insp_file="$B00T_HOME/r3src_资源/inspiration.yaml"

    # Check variant.toml first (authoritative)
    if [ -f "$variant_file" ]; then
        local name=$(grep '^name' "$variant_file" | head -1 | sed 's/.*"\(.*\)".*/\1/')
        if [ -n "$name" ]; then
            echo "$name"
            return
        fi
    fi

    # Fallback: detect from inspiration content
    if [ -f "$insp_file" ]; then
        if grep -q '"anal"\|"buttstuff"\|"penetrate"' "$insp_file" 2>/dev/null; then
            echo "nsfw0r1d"
            return
        fi
    fi

    # Default
    echo "core"
}

# ─── Step 2: Set gates based on variant ──────────────────────────────
configure_gates() {
    local variant="$1"
    local gates_dir="$B00T_HOME/_b00t_/schema"
    mkdir -p "$gates_dir"

    case "$variant" in
        nsfw0r1d)
            cat > "$gates_dir/variant.toml" << 'GATEEOF'
[b00t.variant]
name = "nsfw0r1d"
description = "Personal variant — full inspiration, crypto features enabled"
features = ["nsfw-inspiration", "crypto-enabled"]
GATEEOF
            echo "🔄 Gates set: nsfw-inspiration, crypto-enabled"
            ;;
        *)
            cat > "$gates_dir/variant.toml" << 'GATEEOF'
[b00t.variant]
name = "core"
description = "SFW public variant — inspiration stripped, no crypto"
features = ["sfw", "no-crypto"]
GATEEOF
            echo "🔄 Gates set: sfw, no-crypto"
            ;;
    esac
}

# ─── Step 3: Detect OS ────────────────────────────────────────────────
detect_os() {
    case "$(uname -s)" in
        Linux)
            if grep -qi microsoft /proc/version 2>/dev/null; then
                echo "wsl"
            else
                echo "linux"
            fi
            ;;
        Darwin) echo "macos" ;;
        *) echo "unknown" ;;
    esac
}

# ─── Step 4: Install dependencies ────────────────────────────────────
ensure_rust() {
    if ! command -v rustc &>/dev/null; then
        echo "🦀 Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        . "$HOME/.cargo/env"
    else
        echo "✅ Rust already installed: $(rustc --version)"
    fi
}

ensure_just() {
    if ! command -v just &>/dev/null; then
        echo "🥷 Installing just via cargo..."
        cargo install just
    else
        echo "✅ Just already installed: $(just --version 2>/dev/null | head -1)"
    fi
}

ensure_repo() {
    if [ ! -d "$B00T_HOME" ]; then
        echo "📦 Cloning b00t repository..."
        git clone --branch "$B00T_BRANCH" "$B00T_REPO" "$B00T_HOME"
    else
        echo "✅ Repository already present at $B00T_HOME"
        # Update if it's a git repo
        if [ -d "$B00T_HOME/.git" ]; then
            cd "$B00T_HOME" && git pull origin "$B00T_BRANCH" 2>/dev/null || true
        fi
    fi
}

build_b00t() {
    if [ -f "$B00T_HOME/b00t-cli/Cargo.toml" ]; then
        echo "🔨 Building b00t-cli..."
        cd "$B00T_HOME" && cargo build --bin b00t-cli --release 2>&1 | tail -5
        # Symlink to ~/.local/bin
        mkdir -p "$HOME/.local/bin"
        ln -sf "$B00T_HOME/target/release/b00t-cli" "$HOME/.local/bin/b00t"
        ln -sf "$B00T_HOME/target/release/b00t-cli" "$HOME/.local/bin/b00t-cli"
        echo "✅ b00t-cli installed to ~/.local/bin/"
    else
        echo "⚠️  Cargo.toml not found at \$B00T_HOME/b00t-cli/Cargo.toml — skipping build"
    fi
}

# ─── Main ─────────────────────────────────────────────────────────────
main() {
    echo "🥾 b00t-lite-install.sh — Self-installing bootstrap"
    echo "═══════════════════════════════════════════════════"

    local variant="${B00T_VARIANT:-$(detect_variant)}"
    local os=$(detect_os)

    echo "📋 Detected: OS=$os Variant=$variant"
    echo ""

    configure_gates "$variant"
    ensure_rust
    ensure_just
    ensure_repo
    build_b00t

    echo ""
    echo "✅ b00t installation complete!"
    echo "   Variant: $variant"
    echo "   Binary:  $(command -v b00t 2>/dev/null || echo 'add ~/.local/bin to PATH')"
    echo ""
    echo "   Run: b00t whoami"
}

main "$@"
