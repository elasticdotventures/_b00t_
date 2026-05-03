#!/bin/bash
# b00t port mapping service script
# 🤓 Systemd-compatible script for mapping privileged ports to unprivileged ports

set -euo pipefail

# Check if running as root
if [[ $EUID -ne 0 ]]; then
    echo "❌ Port mapping requires root privileges" >&2
    echo "This script must be run as root or via systemd" >&2
    exit 1
fi

# Check if socat is available, install if not found
if ! command -v socat >/dev/null 2>&1; then
    echo "⚠️ socat binary not found, attempting to install..."
    if command -v apt >/dev/null 2>&1; then
        echo "📦 Installing socat via apt..."
        apt update && apt install -y socat
        if ! command -v socat >/dev/null 2>&1; then
            echo "❌ Failed to install socat" >&2
            exit 1
        fi
        echo "✅ socat installed successfully"
    elif command -v yum >/dev/null 2>&1; then
        echo "📦 Installing socat via yum..."
        yum install -y socat
    elif command -v dnf >/dev/null 2>&1; then
        echo "📦 Installing socat via dnf..."
        dnf install -y socat
    elif command -v pacman >/dev/null 2>&1; then
        echo "📦 Installing socat via pacman..."
        pacman -S --noconfirm socat
    else
        echo "❌ No supported package manager found, cannot install socat" >&2
        echo "Please install socat manually" >&2
        exit 1
    fi
fi

# Function to cleanup on exit
cleanup() {
    echo "🧹 Cleaning up port mapping processes..."
    pkill -f "socat.*:80.*:8080" || true
    pkill -f "socat.*:443.*:8443" || true
}

# Set up signal handlers
trap cleanup EXIT TERM INT

echo "🔌 Starting port mapping service..."
echo "  80 -> 8080 (HTTP)"
echo "  443 -> 8443 (HTTPS)"

# Kill any existing socat processes for these port mappings
cleanup

# Start port forwarding on all interfaces (0.0.0.0)
# TCP-LISTEN with reuseaddr allows immediate restart
socat TCP-LISTEN:80,bind=0.0.0.0,fork,reuseaddr TCP:127.0.0.1:8080 &
HTTP_PID=$!

socat TCP-LISTEN:443,bind=0.0.0.0,fork,reuseaddr TCP:127.0.0.1:8443 &
HTTPS_PID=$!

echo "✅ Port mapping active"
echo "  HTTP proxy PID: $HTTP_PID"
echo "  HTTPS proxy PID: $HTTPS_PID"

# Wait for both processes (systemd will manage this)
wait $HTTP_PID $HTTPS_PID