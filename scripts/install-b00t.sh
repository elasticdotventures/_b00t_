#!/usr/bin/env bash
# scripts/install-b00t.sh — b00t capability-aware installer
#
# Detects system capabilities, reads soul KV for past choices,
# installs minimal b00t in the best mode for this node.
#
# Usage:
#   ./scripts/install-b00t.sh              # auto-detect + prompt if needed
#   ./scripts/install-b00t.sh --mode k8s   # force mode
#   ./scripts/install-b00t.sh --dry-run    # print actions, don't execute
#   ./scripts/install-b00t.sh --reset      # clear soul install.* keys and re-detect

set -euo pipefail

# B00T_DIR = _b00t_/ datum dir (existing b00t convention — do not override)
# B00T_REPO = repo root (where Cargo.toml + justfile live)
B00T_REPO="${B00T_REPO:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
B00T_DIR="${B00T_DIR:-${B00T_REPO}/_b00t_}"
DRY_RUN=false
FORCE_MODE=""
RESET=false

for arg in "$@"; do
    case "$arg" in
        --dry-run)      DRY_RUN=true ;;
        --reset)        RESET=true ;;
        --mode=*)       FORCE_MODE="${arg#--mode=}" ;;
        --mode)         shift; FORCE_MODE="${1:-}" ;;
    esac
done

# ── Colours ───────────────────────────────────────────────────────────────────
G='\033[0;32m'; Y='\033[1;33m'; B='\033[0;34m'; R='\033[0;31m'; N='\033[0m'
ok()   { printf "${G}  ✓${N} %s\n" "$*"; }
info() { printf "${B}  ℹ${N} %s\n" "$*"; }
warn() { printf "${Y}  ⚠${N} %s\n" "$*"; }
skip() { printf "  ⏭  skipped: %s\n" "$*"; }
run()  {
    if $DRY_RUN; then
        printf "  ${Y}dry-run${N}: %s\n" "$*"
    else
        eval "$*"
    fi
}

echo "🥾 b00t install — capability-aware"
echo

# ── Soul KV helpers ───────────────────────────────────────────────────────────
_soul_get() { b00t-cli soul get "$1" 2>/dev/null || true; }
_soul_set() {
    local key="$1" val="$2"
    if ! $DRY_RUN; then
        b00t-cli soul set "$key" "$val" 2>/dev/null || true
    else
        printf "  ${Y}dry-run${N}: soul set %s=%s\n" "$key" "$val"
    fi
}

if $RESET; then
    info "Resetting soul install.* keys"
    for k in install.mode install.service install.confirmed node.fingerprint; do
        b00t-cli soul set "$k" "" 2>/dev/null || true
    done
fi

# ── Capability detection ──────────────────────────────────────────────────────
CAP_CARGO=false;    command -v cargo        >/dev/null 2>&1 && CAP_CARGO=true
CAP_KUBECTL=false;  command -v kubectl      >/dev/null 2>&1 && CAP_KUBECTL=true
CAP_HELM=false;     command -v helm         >/dev/null 2>&1 && CAP_HELM=true
CAP_PODMAN=false;   command -v podman       >/dev/null 2>&1 && CAP_PODMAN=true
CAP_DOCKER=false;   command -v docker       >/dev/null 2>&1 && CAP_DOCKER=true
CAP_NIX=false;      command -v nix          >/dev/null 2>&1 && CAP_NIX=true
CAP_SYSTEMD=false;  command -v systemctl    >/dev/null 2>&1 && CAP_SYSTEMD=true
CAP_LAUNCHD=false;  command -v launchctl    >/dev/null 2>&1 && CAP_LAUNCHD=true
CAP_UV=false;       command -v uv           >/dev/null 2>&1 && CAP_UV=true
CAP_GPU=false;      command -v nvidia-smi   >/dev/null 2>&1 && CAP_GPU=true
CAP_ROOT=false;     [[ "${EUID:-$(id -u)}" -eq 0 ]] && CAP_ROOT=true

# Quadlet = podman + systemd + quadlet drop-in dir (Podman 4.4+)
CAP_QUADLET=false
if $CAP_PODMAN && $CAP_SYSTEMD; then
    [[ -d "$HOME/.config/containers/systemd" ]] && CAP_QUADLET=true
    [[ -d /usr/share/containers/systemd ]]        && CAP_QUADLET=true
    podman info --format '{{.Host.RemoteSocket.Exists}}' 2>/dev/null | grep -q "true" && CAP_QUADLET=true || true
fi
# Trust soul KV if it already recorded quadlet on this node
[[ "$(_soul_get node.orchestration-pattern)" == *quadlet* ]] && CAP_QUADLET=true

# k8s reachable?
CAP_K8S_REACHABLE=false
if $CAP_KUBECTL; then
    kubectl cluster-info >/dev/null 2>&1 && CAP_K8S_REACHABLE=true || true
fi

# Node fingerprint
OS_TYPE="$(uname -s)"
ARCH="$(uname -m)"
MEM_GB=$(awk '/MemTotal/{printf "%.0f", $2/1024/1024}' /proc/meminfo 2>/dev/null || sysctl -n hw.memsize 2>/dev/null | awk '{printf "%.0f",$1/1073741824}' || echo "?")
GPU_NAME="$(_soul_get node.gpu)"
[[ -z "$GPU_NAME" ]] && GPU_NAME="$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -1 || echo "")"
FINGERPRINT="${ARCH}/${MEM_GB}GB$(${CAP_GPU} && echo "/gpu:${GPU_NAME}" || echo "")"

echo "  System: ${OS_TYPE} ${ARCH}, ${MEM_GB}GB RAM${GPU_NAME:+, GPU: $GPU_NAME}"
echo "  Capabilities:"
printf "    cargo=%-5s  kubectl=%-5s  helm=%-5s  k8s=%-5s\n" \
    "$CAP_CARGO" "$CAP_KUBECTL" "$CAP_HELM" "$CAP_K8S_REACHABLE"
printf "    podman=%-5s quadlet=%-5s  systemd=%-5s launchd=%-5s\n" \
    "$CAP_PODMAN" "$CAP_QUADLET" "$CAP_SYSTEMD" "$CAP_LAUNCHD"
printf "    nix=%-5s    gpu=%-5s     uv=%-5s     root=%-5s\n" \
    "$CAP_NIX" "$CAP_GPU" "$CAP_UV" "$CAP_ROOT"
echo

# ── Read previous choices from soul ──────────────────────────────────────────
PREV_MODE="$(_soul_get install.mode)"
PREV_SERVICE="$(_soul_get install.service)"
PREV_CONFIRMED="$(_soul_get install.confirmed)"

if [[ -n "$PREV_MODE" && "$PREV_CONFIRMED" == "true" && -z "$FORCE_MODE" ]]; then
    info "Soul remembers: install.mode=${PREV_MODE} install.service=${PREV_SERVICE}"
    printf "  Use previous settings? [Y/n, 5s]: "
    if read -r -t 5 REUSE_PREV 2>/dev/null; then
        [[ "${REUSE_PREV:-y}" =~ ^[Nn] ]] || { FORCE_MODE="$PREV_MODE"; info "Reusing: ${FORCE_MODE}"; }
    else
        echo "(yes)"
        FORCE_MODE="$PREV_MODE"
        info "Timeout → reusing: ${FORCE_MODE}"
    fi
fi

# ── Auto-select mode if not forced ───────────────────────────────────────────
if [[ -z "$FORCE_MODE" ]]; then
    if   $CAP_K8S_REACHABLE && $CAP_HELM;  then FORCE_MODE="k8s"          # most managed: GPU sched, health, rollout
    elif $CAP_QUADLET;                      then FORCE_MODE="quadlet"      # rootless OCI via systemd; no root
    elif $CAP_SYSTEMD;                      then FORCE_MODE="systemd-user" # portable Linux daemon; no container needed
    elif $CAP_LAUNCHD;                      then FORCE_MODE="launchd"      # macOS native lifecycle
    else                                         FORCE_MODE="binaries"     # no service; CI/dev/external orchestrator
    fi

    echo "  Detected best mode: ${Y}${FORCE_MODE}${N}"
    echo
    echo "  Available modes:"
    # k8s: highest fidelity — GPU scheduling, resource limits, rolling updates, auto-restart via pod controller
    $CAP_K8S_REACHABLE && $CAP_HELM && echo "    k8s           — helm chart → b00t namespace (k8s cluster reachable)"
    # quadlet: rootless OCI via systemd drop-ins; no root; restart + journal via systemd; Podman 4.4+ required
    $CAP_QUADLET       && echo "    quadlet       — systemd + podman container units (rootless, no root required)"
    # systemd-user: per-user scope, journald logging, portable across Linux distros; no container runtime needed
    $CAP_SYSTEMD       && echo "    systemd-user  — ~/.config/systemd/user/b00t@.service"
    # systemd-sys: same as user but system-wide; all users share the unit; requires root; use for servers/multi-user
    $CAP_SYSTEMD && $CAP_ROOT && echo "    systemd-sys   — /usr/lib/systemd/user/ (system-wide, requires root)"
    # launchd: macOS-native; auto-starts on login; plist in ~/Library/LaunchAgents; no Linux analogue
    $CAP_LAUNCHD       && echo "    launchd       — ~/Library/LaunchAgents/io.b00t.mcp.plist (macOS)"
    # binaries: no service daemon; use when b00t is driven externally (CI, another orchestrator, or dev/test)
    echo "    binaries      — b00t-cli + b00t-mcp only, no service"
    echo
    printf "  Mode [%s], 10s: " "$FORCE_MODE"
    read -r -t 10 USER_MODE 2>/dev/null || true
    echo
    FORCE_MODE="${USER_MODE:-$FORCE_MODE}"
fi

MODE="$FORCE_MODE"
info "Installing mode: ${MODE}"
echo

# ── Step 1: Build & install binaries (always) ─────────────────────────────────
_install_binaries() {
    local cargo_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
    export PATH="${cargo_bin}:${PATH}"
    mkdir -p "${cargo_bin}"

    if ! $CAP_CARGO && [[ -f "${cargo_bin}/cargo" ]]; then
        CAP_CARGO=true
        export PATH="${cargo_bin}:${PATH}"
    fi

    if $CAP_CARGO; then
        info "cargo install b00t-cli + b00t-mcp"
        run "cargo install --path '${B00T_REPO}/b00t-cli' --force --quiet"
        run "cargo install --path '${B00T_REPO}/b00t-mcp'  --force --quiet"
        run "cargo install cocogitto --locked --force --quiet"
        ok "binaries installed"
    else
        warn "cargo not found — skipping source build; install Rust from https://rustup.rs"
    fi

    # MCP servers
    if command -v b00t-cli >/dev/null 2>&1; then
        run "b00t-cli install --mcp=recommended 2>/dev/null || true"
    fi
}

# ── Step 2: Service installation (mode-specific) ──────────────────────────────
_install_systemd_user() {
    local unit_src="${B00T_REPO}/.config/systemd/user/b00t@.service"
    local unit_dst="$HOME/.config/systemd/user/b00t@.service"
    [[ -f "$unit_src" ]] || { warn "unit file not found: $unit_src"; return; }
    run "mkdir -p '$HOME/.config/systemd/user'"
    run "cp -v '$unit_src' '$unit_dst'"
    run "systemctl --user daemon-reload 2>/dev/null || true"
    ok "~/.config/systemd/user/b00t@.service installed"
    info "Enable: systemctl --user enable b00t@<profile>"
}

_install_systemd_sys() {
    local unit_src="${B00T_REPO}/.config/systemd/user/b00t@.service"
    if ! $CAP_ROOT; then warn "root required for systemd-sys — run: sudo $0 --mode systemd-sys"; return; fi
    [[ -f "$unit_src" ]] || { warn "unit file not found: $unit_src"; return; }
    run "mkdir -p /usr/lib/systemd/user"
    run "cp -v '$unit_src' /usr/lib/systemd/user/b00t@.service"
    run "systemctl daemon-reload"
    ok "/usr/lib/systemd/user/b00t@.service (system-wide)"
    info "Enable per-user: systemctl --user enable b00t@<profile>"
}

_install_quadlet() {
    local quadlet_dir="$HOME/.config/containers/systemd"
    run "mkdir -p '$quadlet_dir'"
    # Generate a minimal b00t quadlet unit
    local unit="${quadlet_dir}/b00t-mcp.container"
    if ! $DRY_RUN; then
        cat > "$unit" <<'QUADLET'
[Unit]
Description=b00t MCP server
After=network-online.target

[Container]
Image=ghcr.io/promptexecution/b00t-mcp:latest
Environment=B00T_HOME=%h/.b00t
Volume=%h/.b00t:/app/.b00t:Z
PublishPort=3000:3000

[Service]
Restart=on-failure

[Install]
WantedBy=default.target
QUADLET
        ok "Quadlet unit: $unit"
    else
        printf "  ${Y}dry-run${N}: write quadlet unit → %s\n" "$unit"
    fi
    run "systemctl --user daemon-reload 2>/dev/null || true"
    info "Start: systemctl --user start b00t-mcp"
}

_install_k8s() {
    local chart="${B00T_DIR}/k8s.🚢/b00t-mcp"
    if [[ -d "$chart" ]]; then
        run "helm upgrade --install b00t-mcp '$chart' -n b00t --create-namespace"
        ok "b00t-mcp helm chart deployed"
    else
        warn "helm chart not found at $chart — skipping k8s deploy"
    fi
}

_install_launchd() {
    local plist_dir="$HOME/Library/LaunchAgents"
    local plist="$plist_dir/io.b00t.mcp.plist"
    local b00t_cli
    b00t_cli="$(command -v b00t-cli || echo "$HOME/.cargo/bin/b00t-cli")"
    run "mkdir -p '$plist_dir'"
    if ! $DRY_RUN; then
        cat > "$plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>io.b00t.mcp</string>
  <key>ProgramArguments</key><array>
    <string>${b00t_cli}</string><string>mcp</string><string>serve</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>EnvironmentVariables</key><dict>
    <key>B00T_HOME</key><string>${HOME}/.b00t</string>
  </dict>
</dict></plist>
PLIST
        ok "LaunchAgent: $plist"
    else
        printf "  ${Y}dry-run${N}: write plist → %s\n" "$plist"
    fi
    run "launchctl load '$plist' 2>/dev/null || launchctl bootstrap gui/$(id -u) '$plist' || true"
    info "Manage: launchctl start io.b00t.mcp"
}

# ── Execute ───────────────────────────────────────────────────────────────────
_install_binaries

case "$MODE" in
    k8s)            _install_k8s ;;
    quadlet)        _install_quadlet ;;
    systemd-user)   _install_systemd_user ;;
    systemd-sys)    _install_systemd_sys ;;
    launchd)        _install_launchd ;;
    binaries)       info "binaries-only mode — no service installed" ;;
    *)              warn "unknown mode: $MODE — binaries only" ;;
esac

# ── Persist choices to soul ───────────────────────────────────────────────────
# install.mode       → skip the capability menu on next run; re-use this mode silently
# install.confirmed  → gate: only skip menu if this is "true" (prevents half-written state from skipping)
# node.fingerprint   → arch/RAM/GPU summary; used by hive to route ch0nky/sm0l workloads
# node.gpu           → GPU model; used by finetune job + sm0l deployment to pick quantization
# node.orchestration-pattern → canonical how-this-node-runs-services; read by b00t whoami + hive
_soul_set "install.mode"              "$MODE"
_soul_set "install.confirmed"         "true"
_soul_set "node.fingerprint"          "$FINGERPRINT"
[[ -n "$GPU_NAME" ]] && _soul_set "node.gpu" "$GPU_NAME"
_soul_set "node.orchestration-pattern" "$MODE"

echo
ok "b00t install complete [${MODE}]"
info "Soul updated: install.mode=${MODE}"
if command -v b00t-cli >/dev/null 2>&1; then
    echo
    b00t-cli soul status 2>/dev/null | grep -E "install\.|node\." || true
fi
