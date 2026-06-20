#!/usr/bin/env bash
# 🥾 Zellij User Interaction Protocol
# Persistent Zellij context store — local KVCache file
# Path: ~/.b00t/kv-store.json (compatible with b00t-c0re-lib KvStore)

set -euo pipefail

KV_FILE="${B00T_KV_FILE:-$HOME/.b00t/kv-store.json}"
KV_DIR="$(dirname "$KV_FILE")"

# Ensure directory exists
mkdir -p "$KV_DIR"

# 🎯 Read a key from KVCache
kv_get() {
    local key="$1"
    if [ ! -f "$KV_FILE" ]; then
        echo ""
        return 1
    fi
    python3 -c "
import json, sys
try:
    with open('$KV_FILE') as f:
        data = json.load(f)
    val = data.get('$key', '')
    if val:
        print(val)
        sys.exit(0)
    else:
        sys.exit(1)
except:
    sys.exit(1)
" 2>/dev/null || echo ""
}

# 🎯 Write a key to KVCache
kv_set() {
    local key="$1"
    local value="$2"
    python3 -c "
import json, os
filepath = '$KV_FILE'
data = {}
if os.path.exists(filepath):
    try:
        with open(filepath) as f:
            data = json.load(f)
    except:
        pass
data['$key'] = '$value'
os.makedirs(os.path.dirname(filepath), exist_ok=True)
with open(filepath, 'w') as f:
    json.dump(data, f, indent=2)
"
}

# 🎯 Delete a key from KVCache
kv_del() {
    local key="$1"
    python3 -c "
import json, os
filepath = '$KV_FILE'
if os.path.exists(filepath):
    try:
        with open(filepath) as f:
            data = json.load(f)
        data.pop('$key', None)
        with open(filepath, 'w') as f:
            json.dump(data, f, indent=2)
    except:
        pass
"
}

# 🎯 List all keys in KVCache
kv_list() {
    if [ ! -f "$KV_FILE" ]; then
        echo "{}"
        return
    fi
    python3 -c "
import json, sys
try:
    with open('$KV_FILE') as f:
        data = json.load(f)
    print(json.dumps(data, indent=2))
except:
    print('{}')
"
}

# 🚀 CLI entry
case "${1:-}" in
    get) kv_get "${2:-}" ;;
    set) kv_set "${2:-}" "${3:-}" ;;
    del) kv_del "${2:-}" ;;
    list) kv_list ;;
    *)
        echo "🥾 Zellij KVCache — Local agent persistent store"
        echo ""
        echo "Usage: kv-cache get|set|del|list"
        echo ""
        echo "  kv-cache get <key>      Read value"
        echo "  kv-cache set <key> <val> Write value"
        echo "  kv-cache del <key>      Delete key"
        echo "  kv-cache list           Show all"
        echo ""
        echo "File: $KV_FILE"
        echo "Backend: $(kv_get "zellij.backend" 2>/dev/null || echo 'not initialized')"
        echo "Session: $(kv_get "zellij.session" 2>/dev/null || echo 'not initialized')"
        ;;
esac