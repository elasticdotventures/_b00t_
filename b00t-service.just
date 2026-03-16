# b00t-service.just — all root-requiring b00t setup steps
#
# Standalone:  sudo just --justfile b00t-service.just <recipe>
# Via module:  sudo just sudo::<recipe>
#
# Examples:
#   sudo just --justfile b00t-service.just setup
#   sudo just --justfile b00t-service.just status
#   sudo just sudo::install-dbus-service
#
# Entangled into main justfile via:  mod sudo 'b00t-service.just'
# Design: every recipe is idempotent — safe to re-run.

set shell := ["bash", "-cu"]

# Resolve the repo root regardless of cwd
repo-root := env_var_or_default("JUST_REPO_ROOT", `git rev-parse --show-toplevel 2>/dev/null || echo .`)

# The operator username (user who owns ~/.b00t — not root)
OPERATOR := env_var_or_default("SUDO_USER", `whoami`)
OPERATOR_HOME := `eval echo "~${SUDO_USER:-$(whoami)}"`

# ─── Guard: ensure running as root ───────────────────────────────────────────
_check-root:
    #!/bin/bash
    if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
        echo "❌  requires root — run: sudo just sudo::${JUST_RECIPE_NAME:-<recipe>}" >&2
        exit 1
    fi

# ─── Full system setup (calls all sub-recipes in order) ──────────────────────

# Full one-shot system setup: user/group + DBus service + services
setup: _check-root setup-user-group install-dbus-policy install-dbus-service install-services
    @echo "✅  b00t system setup complete"
    @echo "  💡 restart shell or run: newgrp b00t"

# ─── User/Group ───────────────────────────────────────────────────────────────

# Create _b00t system user, b00t group, add operator to group
setup-user-group: _check-root
    #!/bin/bash
    set -euo pipefail
    OPERATOR="${SUDO_USER:-$(whoami)}"

    if ! getent group b00t >/dev/null 2>&1; then
        groupadd --system b00t
        echo "  created group: b00t"
    else
        echo "  group b00t: exists"
    fi

    if ! id -u _b00t >/dev/null 2>&1; then
        useradd --system --no-create-home --shell /usr/sbin/nologin -g b00t _b00t
        echo "  created user: _b00t"
    else
        echo "  user _b00t: exists"
    fi

    if ! id -nG "$OPERATOR" 2>/dev/null | grep -qw b00t; then
        usermod -aG b00t "$OPERATOR"
        echo "  added $OPERATOR to b00t group"
    else
        echo "  $OPERATOR already in b00t group"
    fi

# ─── DBus ─────────────────────────────────────────────────────────────────────

# Install DBus policy for com.promptexecution.b00t1
install-dbus-policy: _check-root
    #!/bin/bash
    set -euo pipefail
    SRC="{{repo-root}}/systemd/com.promptexecution.b00t1.conf"
    DST="/etc/dbus-1/system.d/com.promptexecution.b00t1.conf"
    install -m 644 "$SRC" "$DST"
    echo "  installed DBus policy: $DST"

# Build b00t-cli --features dbus and install to /usr/local/bin
deploy-binary: _check-root
    #!/bin/bash
    set -euo pipefail
    OPERATOR="${SUDO_USER:-$(whoami)}"
    OPERATOR_HOME=$(eval echo "~$OPERATOR")
    CARGO_HOME="${OPERATOR_HOME}/.cargo"
    B00T_DIR="{{repo-root}}"

    echo "  building b00t-cli --features dbus (as $OPERATOR) ..."
    sudo -u "$OPERATOR" bash -c "cd '$B00T_DIR' && '$CARGO_HOME/bin/cargo' build --release --features dbus"
    install -m 755 "$B00T_DIR/target/release/b00t-cli" /usr/local/bin/b00t-cli
    echo "  installed: /usr/local/bin/b00t-cli (with dbus feature)"

# Install b00t.service unit + DBus policy and enable it
install-dbus: _check-root setup-user-group install-dbus-policy
    #!/bin/bash
    set -euo pipefail
    SRC="{{repo-root}}/systemd/b00t.service"
    DST="/etc/systemd/system/b00t.service"
    install -m 644 "$SRC" "$DST"
    echo "  installed unit: $DST"
    systemctl daemon-reload
    systemctl enable --now b00t.service
    echo "  b00t.service: enabled + started"

# Full DBus service install: user/group + policy + deploy binary + unit
install-dbus-service: _check-root setup-user-group install-dbus-policy deploy-binary install-dbus
    @echo "✅  b00t DBus service installed"
    @echo "  verify: dbus-send --system --print-reply --dest=com.promptexecution.b00t1 /com/promptexecution/b00t1 com.promptexecution.b00t1.Ping"

# Restart b00t.service (after binary update, policy change)
restart-dbus: _check-root
    systemctl restart b00t.service
    @echo "  b00t.service restarted"

# ─── SOCKS5 + Port-map services ───────────────────────────────────────────────

# Install b00t-socks5.service + b00t-port-map.service
install-services: _check-root
    #!/bin/bash
    set -euo pipefail
    "{{repo-root}}/scripts/install-systemd-services.sh"

# ─── Deploy binary only (no user/service changes) ────────────────────────────

# Quick redeploy: rebuild --features dbus and restart service
redeploy: _check-root deploy-binary restart-dbus
    @echo "✅  redeployed"

# ─── Status ───────────────────────────────────────────────────────────────────

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
        echo "  policy:  /etc/dbus-1/system.d/com.promptexecution.b00t1.conf ✓"
    else
        echo "  policy:  MISSING"
    fi
    if systemctl is-active b00t.service >/dev/null 2>&1; then
        echo "  service: active (running) ✓"
        systemctl status b00t.service --no-pager -l | grep -E "PID|Memory|CPU" | sed 's/^/  /'
    else
        echo "  service: NOT running"
    fi
    if dbus-send --system --print-reply --dest=com.promptexecution.b00t1 \
        /com/promptexecution/b00t1 com.promptexecution.b00t1.Ping 2>/dev/null | grep -q pong; then
        echo "  ping:    pong ✓"
    else
        echo "  ping:    FAILED (service not responding)"
    fi
    echo
    echo "── Users/Groups ────────────────────────────────────"
    if getent group b00t >/dev/null 2>&1; then
        echo "  group b00t: $(getent group b00t)"
    else
        echo "  group b00t: MISSING"
    fi
    if id _b00t >/dev/null 2>&1; then
        echo "  user _b00t: exists"
    else
        echo "  user _b00t: MISSING"
    fi
    OPERATOR="${SUDO_USER:-$(whoami)}"
    if id -nG "$OPERATOR" 2>/dev/null | grep -qw b00t; then
        echo "  $OPERATOR in b00t group: ✓"
    else
        echo "  $OPERATOR in b00t group: NO (run: sudo just sudo::setup-user-group)"
    fi
    echo
    echo "── Systemd Services ────────────────────────────────"
    for svc in b00t.service b00t-socks5.service b00t-port-map.service; do
        if systemctl is-active "$svc" >/dev/null 2>&1; then
            echo "  $svc: active ✓"
        elif [[ -f "/etc/systemd/system/$svc" ]]; then
            echo "  $svc: installed but not running"
        else
            echo "  $svc: not installed"
        fi
    done

# ─── Uninstall ────────────────────────────────────────────────────────────────

# Remove b00t.service and DBus policy (leaves _b00t user/group intact)
uninstall-dbus: _check-root
    #!/bin/bash
    set -euo pipefail
    systemctl disable --now b00t.service 2>/dev/null || true
    rm -f /etc/systemd/system/b00t.service
    rm -f /etc/dbus-1/system.d/com.promptexecution.b00t1.conf
    systemctl daemon-reload
    echo "  b00t.service uninstalled"
    echo "  DBus policy removed"
    echo "  💡 user _b00t and group b00t retained (remove manually if desired)"
