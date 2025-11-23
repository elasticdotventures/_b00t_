#!/bin/bash
# b00t socks5 proxy service script
# 🤓 Systemd-compatible script for running koblas SOCKS5 proxy

set -euo pipefail

# Get the user who should own the service (from systemd or fallback)
SERVICE_USER="${USER:-$(whoami)}"
if [[ "$SERVICE_USER" == "root" && -n "${SUDO_USER:-}" ]]; then
    SERVICE_USER="$SUDO_USER"
fi

# Configuration
KOBLAS_CONFIG_PATH="${KOBLAS_CONFIG_PATH:-$(eval echo ~$SERVICE_USER)/.dotfiles/koblas.toml}"
KOBLAS_ADDRESS="${KOBLAS_ADDRESS:-0.0.0.0}"
RUST_LOG="${RUST_LOG:-debug}"

# Validate config file exists
if [[ ! -f "$KOBLAS_CONFIG_PATH" ]]; then
    echo "❌ Config file not found: $KOBLAS_CONFIG_PATH" >&2
    exit 1
fi

# Check if koblas binary is available, install if not found
CARGO_BIN_PATH="$(eval echo ~$SERVICE_USER)/.cargo/bin"
KOBLAS_BIN="$CARGO_BIN_PATH/koblas"

if [[ ! -x "$KOBLAS_BIN" ]] && ! command -v koblas >/dev/null 2>&1; then
    echo "⚠️ koblas binary not found, attempting to install..."
    if command -v cargo >/dev/null 2>&1; then
        echo "📦 Installing koblas via cargo for user $SERVICE_USER..."
        if [[ "$SERVICE_USER" != "$(whoami)" ]]; then
            sudo -u "$SERVICE_USER" cargo install koblas
        else
            cargo install koblas
        fi
        # Update binary path after installation
        if [[ -x "$KOBLAS_BIN" ]]; then
            echo "✅ koblas installed successfully at $KOBLAS_BIN"
        elif command -v koblas >/dev/null 2>&1; then
            KOBLAS_BIN="$(command -v koblas)"
            echo "✅ koblas installed successfully at $KOBLAS_BIN"
        else
            echo "❌ Failed to install koblas" >&2
            exit 1
        fi
    else
        echo "❌ cargo not found, cannot install koblas" >&2
        exit 1
    fi
elif command -v koblas >/dev/null 2>&1; then
    KOBLAS_BIN="$(command -v koblas)"
fi

echo "🧦 Starting koblas SOCKS5 proxy..."
echo "  Config: $KOBLAS_CONFIG_PATH"
echo "  Address: $KOBLAS_ADDRESS"
echo "  Log level: $RUST_LOG"

# Export environment variables and start koblas
export KOBLAS_ADDRESS
export RUST_LOG
export KOBLAS_CONFIG_PATH
export KOBLAS_NO_AUTHENTICATION=true
export KOBLAS_ANONYMIZE=false

exec "$KOBLAS_BIN"