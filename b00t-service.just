# b00t-service.just — all root-requiring b00t setup steps
#
# just is user-installed (~/.cargo/bin/just) — NOT in sudo's PATH.
# Solution: run `just <recipe>` as normal user; sudo is called PER-COMMAND inside.
#
# Usage:
#   just sudo::setup                        # via main justfile module
#   just --justfile b00t-service.just setup # standalone
#   just sudo::status                       # read-only, no sudo needed
#
# Entangled into main justfile via:  mod sudo 'b00t-service.just'
# Design: every recipe is idempotent — safe to re-run.

set shell := ["bash", "-cu"]

repo-root := env_var_or_default("JUST_REPO_ROOT", `git rev-parse --show-toplevel 2>/dev/null || echo .`)

# ─── Full system setup ────────────────────────────────────────────────────────

# Full one-shot system setup: user/group + DBus service + services
setup: setup-user-group install-dbus-policy install-dbus-service install-services
    @echo "✅  b00t system setup complete"
    @echo "  💡 restart shell or: newgrp b00t"

# ─── User/Group ───────────────────────────────────────────────────────────────

# Create _b00t system user, b00t group, add current user to group
setup-user-group:
    #!/bin/bash
    set -euo pipefail
    OPERATOR="$(whoami)"

    if ! getent group b00t >/dev/null 2>&1; then
        sudo groupadd --system b00t
        echo "  created group: b00t"
    else
        echo "  group b00t: exists"
    fi

    if ! id -u _b00t >/dev/null 2>&1; then
        sudo useradd --system --no-create-home --shell /usr/sbin/nologin -g b00t _b00t
        echo "  created user: _b00t"
    else
        echo "  user _b00t: exists"
    fi

    if ! id -nG "$OPERATOR" 2>/dev/null | grep -qw b00t; then
        sudo usermod -aG b00t "$OPERATOR"
        echo "  added $OPERATOR to b00t group"
    else
        echo "  $OPERATOR already in b00t group"
    fi

# ─── DBus ─────────────────────────────────────────────────────────────────────

# Install DBus policy for com.promptexecution.b00t1
install-dbus-policy:
    #!/bin/bash
    set -euo pipefail
    SRC="{{repo-root}}/systemd/com.promptexecution.b00t1.conf"
    DST="/etc/dbus-1/system.d/com.promptexecution.b00t1.conf"
    sudo install -m 644 "$SRC" "$DST"
    echo "  installed DBus policy: $DST"

# Build b00t-cli --features dbus (as current user) and install binary to /usr/local/bin
deploy-binary:
    #!/bin/bash
    set -euo pipefail
    B00T_DIR="{{repo-root}}"
    echo "  building b00t-cli --features dbus ..."
    cargo build --release --features dbus --manifest-path "$B00T_DIR/Cargo.toml"
    sudo install -m 755 "$B00T_DIR/target/release/b00t-cli" /usr/local/bin/b00t-cli
    echo "  installed: /usr/local/bin/b00t-cli (with dbus feature)"

# Install b00t.service unit and enable it
install-dbus:
    #!/bin/bash
    set -euo pipefail
    SRC="{{repo-root}}/systemd/b00t.service"
    DST="/etc/systemd/system/b00t.service"
    sudo install -m 644 "$SRC" "$DST"
    echo "  installed unit: $DST"
    sudo systemctl daemon-reload
    sudo systemctl enable --now b00t.service
    echo "  b00t.service: enabled + started"

# Full DBus service install: user/group + policy + deploy binary + unit
install-dbus-service: setup-user-group install-dbus-policy deploy-binary install-dbus
    @echo "✅  b00t DBus service installed"
    @echo "  verify: dbus-send --system --print-reply --dest=com.promptexecution.b00t1 /com/promptexecution/b00t1 com.promptexecution.b00t1.Ping"

# Restart b00t.service (after binary update or policy change)
restart-dbus:
    sudo systemctl restart b00t.service
    @echo "  b00t.service restarted"

# ─── SOCKS5 + Port-map services ───────────────────────────────────────────────

# Install b00t-socks5.service + b00t-port-map.service
install-services:
    sudo "{{repo-root}}/scripts/install-systemd-services.sh"

# ─── Quick redeploy ───────────────────────────────────────────────────────────

# Rebuild --features dbus and restart service (no user/policy changes)
redeploy: deploy-binary restart-dbus
    @echo "✅  redeployed"

# ─── Status (no sudo needed) ─────────────────────────────────────────────────

# Show status of all b00t system components
status:
    #!/bin/bash
    echo "🥾 b00t system status"
    echo
    echo "── Binary ──────────────────────────────────────────"
    if command -v b00t-cli >/dev/null 2>&1; then
        echo "  b00t-cli: $(b00t-cli --version 2>&1 || echo '?')"
        echo "  path:     $(which b00t-cli)"
    else
        echo "  b00t-cli: NOT FOUND"
    fi
    echo
    echo "── DBus ────────────────────────────────────────────"
    if [[ -f /etc/dbus-1/system.d/com.promptexecution.b00t1.conf ]]; then
        echo "  policy:  ✓"
    else
        echo "  policy:  MISSING"
    fi
    if systemctl is-active b00t.service >/dev/null 2>&1; then
        echo "  service: active ✓"
        systemctl status b00t.service --no-pager -l | grep -E "PID|Memory|CPU" | sed 's/^/  /'
    else
        echo "  service: NOT running"
    fi
    if dbus-send --system --print-reply --dest=com.promptexecution.b00t1 \
        /com/promptexecution/b00t1 com.promptexecution.b00t1.Ping 2>/dev/null | grep -q pong; then
        echo "  ping:    pong ✓"
    else
        echo "  ping:    FAILED"
    fi
    echo
    echo "── Users/Groups ────────────────────────────────────"
    getent group b00t >/dev/null 2>&1 && echo "  group b00t: ✓" || echo "  group b00t: MISSING"
    id _b00t >/dev/null 2>&1 && echo "  user _b00t: ✓" || echo "  user _b00t: MISSING"
    OPERATOR="$(whoami)"
    id -nG "$OPERATOR" 2>/dev/null | grep -qw b00t \
        && echo "  $OPERATOR in b00t group: ✓" \
        || echo "  $OPERATOR in b00t group: NO — run: just sudo::setup-user-group"
    echo
    echo "── Systemd Services ────────────────────────────────"
    for svc in b00t.service b00t-socks5.service b00t-port-map.service; do
        if systemctl is-active "$svc" >/dev/null 2>&1; then
            echo "  $svc: active ✓"
        elif [[ -f "/etc/systemd/system/$svc" ]]; then
            echo "  $svc: installed, not running"
        else
            echo "  $svc: not installed"
        fi
    done

# ─── Uninstall ────────────────────────────────────────────────────────────────

# Remove b00t.service and DBus policy (leaves _b00t user/group intact)
uninstall-dbus:
    #!/bin/bash
    set -euo pipefail
    sudo systemctl disable --now b00t.service 2>/dev/null || true
    sudo rm -f /etc/systemd/system/b00t.service
    sudo rm -f /etc/dbus-1/system.d/com.promptexecution.b00t1.conf
    sudo systemctl daemon-reload
    echo "  b00t.service uninstalled, DBus policy removed"
    echo "  💡 user _b00t and group b00t retained"
