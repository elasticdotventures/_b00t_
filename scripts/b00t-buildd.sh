#!/bin/bash
# b00t-buildd — background build daemon
# Watches git for changed files, pre-builds affected workspace crates.
# Start: b00t buildd start
# Stop:  b00t buildd stop
# Status: b00t buildd status
set -euo pipefail

B00T_ROOT="${HOME}/.dotfiles"
STATUS_DIR="${HOME}/.b00t/buildd"
PID_FILE="${STATUS_DIR}/pid"
STATUS_FILE="${STATUS_DIR}/status.json"
LOG_FILE="${STATUS_DIR}/buildd.log"
CONCURRENCY=1

cmd_start() {
    if [ -f "$PID_FILE" ] && kill -0 $(cat "$PID_FILE") 2>/dev/null; then
        echo "buildd already running (PID $(cat $PID_FILE))"
        return 1
    fi
    mkdir -p "$STATUS_DIR"
    nohup bash "$0" _daemon_loop >> "$LOG_FILE" 2>&1 &
    echo $! > "$PID_FILE"
    echo "buildd started (PID $!)"
}

cmd_stop() {
    if [ -f "$PID_FILE" ]; then
        kill $(cat "$PID_FILE") 2>/dev/null && echo "buildd stopped" || echo "buildd not running"
        rm -f "$PID_FILE"
    else
        echo "buildd not running"
    fi
}

cmd_status() {
    if [ -f "$PID_FILE" ] && kill -0 $(cat "$PID_FILE") 2>/dev/null; then
        echo "buildd running (PID $(cat $PID_FILE))"
        [ -f "$STATUS_FILE" ] && python3 -c "
import json, os, time
try:
    with open('$STATUS_FILE') as f:
        d = json.load(f)
    for crate, state in d.items():
        age = int(time.time() - state.get('started', 0))
        print(f'  {crate}: {state[\"status\"]} ({age}s ago)')
except: pass
" 2>/dev/null
    else
        echo "buildd not running"
    fi
}

cmd_log() {
    tail -f "$LOG_FILE" 2>/dev/null || echo "no log yet"
}

# Map changed files to workspace crates
files_to_crates() {
    local files="$1"
    local crates=""
    for f in $files; do
        # Extract crate name from path: b00t-cli/src/... -> b00t-cli
        local crate=$(echo "$f" | grep -oP '^[^/]+' | head -1)
        if [ -f "$B00T_ROOT/$crate/Cargo.toml" ]; then
            crates="$crates $crate"
        elif echo "$f" | grep -q "Cargo.toml\|Cargo.lock"; then
            # Workspace-level changes affect everything
            crates="$crates b00t-cli b00t-admin b00t-mcp"
        fi
    done
    echo "$crates" | tr ' ' '\n' | sort -u | tr '\n' ' '
}

_daemon_loop() {
    cd "$B00T_ROOT"
    local last_head=""

    while true; do
        sleep 5
        local head=$(git rev-parse HEAD 2>/dev/null) || continue
        [ "$head" = "$last_head" ] && continue
        last_head="$head"

        local changed=$(git diff --name-only HEAD 2>/dev/null | grep -E '\.rs$|Cargo\.(toml|lock)$' || true)
        [ -z "$changed" ] && continue

        local crates=$(files_to_crates "$changed")
        [ -z "$crates" ] && continue

        for crate in $crates; do
            local state_file="${STATUS_DIR}/${crate}.state"
            # Skip if already building
            [ -f "$state_file" ] && continue

            touch "$state_file"
            echo "[$(date -Iseconds)] building $crate" >> "$LOG_FILE"

            # Update status
            python3 -c "
import json, time, os
s = {}
if os.path.exists('$STATUS_FILE'):
    with open('$STATUS_FILE') as f:
        try: s = json.load(f)
        except: pass
s['$crate'] = {'status': 'building', 'started': time.time()}
with open('$STATUS_FILE', 'w') as f:
    json.dump(s, f)
" 2>/dev/null

            cargo build -p "$crate" 2>&1 | while IFS= read -r line; do
                echo "[$crate] $line" >> "$LOG_FILE"
            done

            # Auto-restart b00t-admin after build
            if [ "$crate" = "b00t-admin" ]; then
                local old_pid=$(pgrep -f "target/debug/b00t-admin" 2>/dev/null || true)
                if [ -n "$old_pid" ]; then
                    kill "$old_pid" 2>/dev/null
                    sleep 1
                fi
                nohup "$B00T_ROOT/target/debug/b00t-admin" >> "$LOG_FILE" 2>&1 &
                echo "[$(date -Iseconds)] b00t-admin restarted (new PID $!)" >> "$LOG_FILE"
            fi

            # Update status to done
            python3 -c "
import json, time, os
s = {}
if os.path.exists('$STATUS_FILE'):
    with open('$STATUS_FILE') as f:
        try: s = json.load(f)
        except: pass
s['$crate'] = {'status': 'done', 'started': time.time()}
with open('$STATUS_FILE', 'w') as f:
    json.dump(s, f)
" 2>/dev/null

            rm -f "$state_file"
            echo "[$(date -Iseconds)] $crate done" >> "$LOG_FILE"
        done
    done
}

case "${1:-}" in
    start)   cmd_start ;;
    stop)    cmd_stop ;;
    status)  cmd_status ;;
    log)     cmd_log ;;
    _daemon_loop) _daemon_loop ;;
    *)       echo "Usage: b00t buildd {start|stop|status|log}" ;;
esac
