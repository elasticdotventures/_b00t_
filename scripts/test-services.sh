#!/bin/bash
# Test script to verify services work before enabling systemd

echo "🧪 Testing b00t services..."

echo "1. Testing SOCKS5 script..."
timeout 5 /home/brianh/.dotfiles/scripts/socks5.sh &
SOCKS5_PID=$!
sleep 2
if kill -0 $SOCKS5_PID 2>/dev/null; then
    echo "✅ SOCKS5 script started successfully"
    kill $SOCKS5_PID
else
    echo "❌ SOCKS5 script failed to start"
fi

echo ""
echo "2. Testing port mapping script (requires root)..."
if [[ $EUID -eq 0 ]]; then
    timeout 5 /home/brianh/.dotfiles/scripts/port-map.sh &
    PORTMAP_PID=$!
    sleep 2
    if kill -0 $PORTMAP_PID 2>/dev/null; then
        echo "✅ Port mapping script started successfully"
        kill $PORTMAP_PID
    else
        echo "❌ Port mapping script failed to start"
    fi
else
    echo "⚠️ Skipping port mapping test (not running as root)"
    echo "   Run: sudo /home/brianh/.dotfiles/scripts/test-services.sh"
fi

echo ""
echo "3. Service files status:"
echo "SOCKS5 service: $(systemctl is-enabled b00t-socks5.service 2>/dev/null || echo 'not enabled')"
echo "Port map service: $(systemctl is-enabled b00t-port-map.service 2>/dev/null || echo 'not enabled')"