#!/bin/bash
# b00t systemd services installation script
# 🤓 Install and configure b00t SOCKS5 and port mapping services

set -euo pipefail

# Get the user who invoked sudo (or current user if not using sudo)
ACTUAL_USER="${SUDO_USER:-$(whoami)}"
ACTUAL_HOME=$(eval echo "~$ACTUAL_USER")
DOTFILES_DIR="$ACTUAL_HOME/.dotfiles"
SYSTEMD_USER_DIR="/etc/systemd/system"

# Check if running as root for system service installation
if [[ $EUID -ne 0 ]]; then
    echo "❌ This script requires root privileges to install system services" >&2
    echo "Run with: sudo $0" >&2
    exit 1
fi

echo "🛠️ Installing b00t systemd services for user $ACTUAL_USER..."
echo "   Dotfiles directory: $DOTFILES_DIR"

# Validate dotfiles directory exists
if [[ ! -d "$DOTFILES_DIR" ]]; then
    echo "❌ Dotfiles directory not found: $DOTFILES_DIR" >&2
    exit 1
fi

# Process and copy service files with variable substitution
echo "📋 Processing service files..."

# Process socks5 service
sed -e "s|\${USER}|$ACTUAL_USER|g" \
    -e "s|\${DOTFILES_DIR}|$DOTFILES_DIR|g" \
    "$DOTFILES_DIR/systemd/b00t-socks5.service" > "$SYSTEMD_USER_DIR/b00t-socks5.service"

# Process port-map service  
sed -e "s|\${DOTFILES_DIR}|$DOTFILES_DIR|g" \
    "$DOTFILES_DIR/systemd/b00t-port-map.service" > "$SYSTEMD_USER_DIR/b00t-port-map.service"

# Set correct permissions
chmod 644 "$SYSTEMD_USER_DIR/b00t-socks5.service"
chmod 644 "$SYSTEMD_USER_DIR/b00t-port-map.service"

# Reload systemd
echo "🔄 Reloading systemd daemon..."
systemctl daemon-reload

echo "✅ Services installed successfully!"
echo ""
echo "📖 Usage:"
echo "  Enable and start SOCKS5 proxy:"
echo "    sudo systemctl enable --now b00t-socks5.service"
echo ""
echo "  Enable and start port mapping:"
echo "    sudo systemctl enable --now b00t-port-map.service"
echo ""
echo "  Check service status:"
echo "    sudo systemctl status b00t-socks5.service"
echo "    sudo systemctl status b00t-port-map.service"
echo ""
echo "  View service logs:"
echo "    sudo journalctl -u b00t-socks5.service -f"
echo "    sudo journalctl -u b00t-port-map.service -f"