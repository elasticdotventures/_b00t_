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

# 🤓 No set -e: process manager must be resilient to transient failures
set -uo pipefail

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
    local svc="$1"
    local pf="$(_pid_file "$svc")"
    # Check PID file first
    [ -f "$pf" ] && kill -0 "$(cat "$pf" 2>/dev/null)" 2>/dev/null && return 0
    # Check PM2
    command -v pm2 &>/dev/null && pm2 id "b00t-${svc}" 2>/dev/null | grep -q '\[.\+\]' && return 0
    return 1
}

_start() {
    local svc="$1" cmd="$2" port="$3"
    local pf="$(_pid_file "$svc")" lf="$(_log_file "$svc")"
    if _is_running "$svc"; then
        local existing_pid
        existing_pid="$(pm2 pid "b00t-${svc}" 2>/dev/null || cat "$pf" 2>/dev/null || echo '')"
        echo "⚠️  $svc already running (PID ${existing_pid:-?})"
        [ -n "$existing_pid" ] && echo "$existing_pid" > "$pf" 2>/dev/null
        return 0
    fi
    echo "▶ Starting $svc on ${port}..."
    cd "$B00T_HOME"
    # Use PM2 if available, fallback to batch
    if command -v pm2 &>/dev/null; then
        pm2 start $cmd --name "b00t-${svc}" --cwd "$B00T_HOME" 2>/dev/null || {
            pm2 restart "b00t-${svc}" 2>/dev/null
        }
        sleep 1
        local pm2_pid
        pm2_pid=$(pm2 pid "b00t-${svc}" 2>/dev/null)
        [ -n "$pm2_pid" ] && echo "$pm2_pid" > "$pf"
    else
        # Fallback: batch detach
        local wrapper="${PID_DIR}/b00t-pm-${svc}-wrapper.sh"
        cat > "$wrapper" <<- WRAPEOF
#!/bin/bash
echo \$\$ > ${pf}
cd ${B00T_HOME}
exec ${cmd}
WRAPEOF
        chmod +x "$wrapper"
        echo "$wrapper" | batch 2>/dev/null || {
            nohup "$wrapper" > "$lf" 2>&1 & disown
        }
    fi
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
    # Try PM2 first
    if command -v pm2 &>/dev/null && pm2 id "b00t-${svc}" 2>/dev/null | grep -q '"'; then
        pm2 stop "b00t-${svc}" 2>/dev/null
        pm2 delete "b00t-${svc}" 2>/dev/null
        echo "  ✅ $svc stopped (via PM2)"
        rm -f "$pf"
        return 0
    fi
    # Fallback: kill by PID
    if [ -f "$pf" ]; then
        local pid
        pid="$(cat "$pf" 2>/dev/null || true)"
        if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null && echo "  ✅ $svc stopped (PID $pid)" || echo "  ⚠️  Could not stop $svc"
        else
            echo "  ℹ️  $svc not running"
        fi
        rm -f "$pf"
    else
        pkill -f "b00t-${svc}" 2>/dev/null && echo "  ✅ $svc stopped" || echo "  ℹ️  $svc not running"
    fi
}

_status() {
    local svc="$1" port="${2:-}"
    local lf="$(_log_file "$svc")"
    if _is_running "$svc"; then
        local pid
        pid="$(pm2 pid "b00t-${svc}" 2>/dev/null || cat "$(_pid_file "$svc")" 2>/dev/null || echo '?')"
        local mem=""
        [ "$pid" != "?" ] && mem="$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ' || true)"
        echo "  ✅ $svc — running (PID ${pid}, ${mem:+${mem}KB}${mem:-RSS}${mem:+, })"
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
            pid="$(pm2 pid "b00t-${svc}" 2>/dev/null || cat "$(_pid_file "$svc")" 2>/dev/null || echo '?')"
            echo "  $svc       ✅ :$port    (PID ${pid})"
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
