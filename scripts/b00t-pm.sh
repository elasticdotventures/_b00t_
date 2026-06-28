#!/usr/bin/env bash
# 🥾 b00t-pm — lightweight process manager for b00t background services
# Usage: b00t pm <start|stop|restart|status|logs|list> <service>
#
# Services:
#   admin     — b00t-admin dashboard (:31337)
#   tauri     — ledgerr-tauri desktop host (:15115)
#   mcp       — b00t-mcp HTTP mode (:3000)
#
# PID files: /tmp/b00t-pm-<service>.pid
# Log files: /tmp/b00t-pm-<service>.log

set -euo pipefail

CMD="${1:-help}"
SERVICE="${2:-}"

B00T_HOME="${B00T_HOME:-$HOME/.dotfiles}"
PID_DIR="/tmp"

usage() {
    echo "Usage: b00t pm <command> [service]"
    echo ""
    echo "Commands:"
    echo "  start   <service>   Start a background service"
    echo "  stop    <service>   Stop a service"
    echo "  restart <service>   Restart a service"
    echo "  status  <service>   Show service status"
    echo "  logs    <service>   Tail service logs"
    echo "  list                List all managed services"
    echo ""
    echo "Services:"
    echo "  admin    — b00t-admin dashboard     (:31337)"
    echo "  tauri    — ledgerr-tauri desktop    (:15115)"
    echo "  mcp      — b00t-mcp HTTP            (:3000)"
    exit 0
}

_pid_file() { echo "${PID_DIR}/b00t-pm-${1}.pid"; }
_log_file() { echo "${PID_DIR}/b00t-pm-${1}.log"; }

_is_running() {
    local pf="$(_pid_file "$1")"
    [ -f "$pf" ] && kill -0 "$(cat "$pf" 2>/dev/null)" 2>/dev/null
}

_start() {
    local svc="$1" cmd="$2" port="$3"
    local pf="$(_pid_file "$svc")" lf="$(_log_file "$svc")"
    if _is_running "$svc"; then
        echo "⚠️  $svc already running (PID $(cat "$pf"))"
        return 0
    fi
    echo "▶ Starting $svc on ${port}..."
    cd "$B00T_HOME"
    # Write a wrapper script that records its own PID, then exec the command
    local wrapper="${PID_DIR}/b00t-pm-${svc}-wrapper.sh"
    # Unquoted heredoc: expand ${pf}, ${B00T_HOME}, ${cmd} now; escape $$ for runtime
    cat > "$wrapper" <<- WRAPEOF
#!/bin/bash
echo \$\$ > ${pf}
cd ${B00T_HOME}
exec ${cmd}
WRAPEOF
    chmod +x "$wrapper"
    # Submit via batch — fully detached from this shell
    echo "$wrapper" | batch 2>/dev/null || {
        # Fallback: direct background
        nohup "$wrapper" > "$lf" 2>&1 & disown
    }
    sleep 2
    if _is_running "$svc"; then
        echo "  ✅ $svc started (PID $(cat "$pf"))"
        echo "  📋 Logs: $lf"
        [ -n "$port" ] && echo "  🌐 http://localhost:${port}/"
    else
        echo "  ❌ $svc failed to start — check logs: $lf"
        cat "$lf" | tail -5 2>/dev/null || true
        return 1
    fi
}

_stop() {
    local svc="$1"
    local pf="$(_pid_file "$svc")"
    if [ ! -f "$pf" ]; then
        echo "⚠️  $svc not managed (no PID file)"
        # Try pkill anyway
        pkill -f "b00t-admin" 2>/dev/null && echo "  ✅ $svc stopped" || echo "  ℹ️  $svc not running"
        return 0
    fi
    local pid
    pid="$(cat "$pf" 2>/dev/null || true)"
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null && echo "  ✅ $svc stopped (PID $pid)" || echo "  ⚠️  Could not stop $svc"
    else
        echo "  ℹ️  $svc not running"
    fi
    rm -f "$pf"
}

_status() {
    local svc="$1" port="${2:-}"
    local pf="$(_pid_file "$svc")"
    local lf="$(_log_file "$svc")"
    if _is_running "$svc"; then
        local pid
        pid="$(cat "$pf")"
        local elapsed=$(( $(date +%s) - $(stat -c %Y "$pf" 2>/dev/null || echo $(date +%s)) ))
        local mem
        mem="$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ')"
        echo "  ✅ $svc — running (PID $pid, ${elapsed}s up, ${mem:-?}KB RSS)"
        [ -n "$port" ] && echo "     http://localhost:${port}/"
    else
        echo "  ⬜ $svc — stopped"
    fi
}

_logs() {
    local svc="$1"
    local lf="$(_log_file "$svc")"
    if [ -f "$lf" ]; then
        tail -20 "$lf"
    else
        echo "  ℹ️  No logs for $svc"
    fi
}

list_services() {
    echo "  Service    Status      Port"
    echo "  ───────    ──────      ────"
    for svc in admin tauri mcp; do
        local port=""
        case "$svc" in
            admin) port="31337" ;;
            tauri) port="15115" ;;
            mcp)   port="3000"  ;;
        esac
        if _is_running "$svc"; then
            local pid
            pid="$(cat "$(_pid_file "$svc")" 2>/dev/null)"
            echo "  $svc       ✅ :$port    (PID $pid)"
        else
            echo "  $svc       ⬜ :$port"
        fi
    done
}

case "$CMD" in
    start)
        [ -z "$SERVICE" ] && { echo "Usage: b00t pm start <service>"; exit 1; }
        case "$SERVICE" in
            admin) _start "admin"   "target/debug/b00t-admin" "31337" ;;
            tauri) _start "tauri"   "ledgerr-tauri"           "15115" ;;
            mcp)   _start "mcp"     "b00t-mcp --http --port 3000" "3000" ;;
            *)     echo "Unknown service: $SERVICE"; exit 1 ;;
        esac
        ;;
    stop)
        [ -z "$SERVICE" ] && { echo "Usage: b00t pm stop <service>"; exit 1; }
        _stop "$SERVICE"
        ;;
    restart)
        [ -z "$SERVICE" ] && { echo "Usage: b00t pm restart <service>"; exit 1; }
        _stop "$SERVICE"
        sleep 1
        exec "$0" start "$SERVICE"
        ;;
    status)
        if [ -n "$SERVICE" ]; then
            case "$SERVICE" in
                admin) _status "admin" "31337" ;;
                tauri) _status "tauri" "15115" ;;
                mcp)   _status "mcp"   "3000"  ;;
                *)     echo "Unknown service: $SERVICE"; exit 1 ;;
            esac
        else
            list_services
        fi
        ;;
    logs)
        [ -z "$SERVICE" ] && { echo "Usage: b00t pm logs <service>"; exit 1; }
        _logs "$SERVICE"
        ;;
    list)
        list_services
        ;;
    *)
        usage
        ;;
esac
