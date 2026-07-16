# _b00t_/lifecycle.just — PRD-011 G5: datum lifecycle classification → first flashtable
# Flashtable v0 = soul DataFramerr table `lifecycle` (typed columns, cursors, alarms).
# ch0nky endpoint+key are discovered from the hive datum — never hardcoded.
# Thinking stays ON (quality over speed); grunt work runs on local silicon.
# Soul commands run from ~/.b00t so the active soul is deterministic (global ~/._b00t_).

set shell := ["bash", "-euo", "pipefail", "-c"]

# create the lifecycle flashtable (idempotent)
table-init:
    #!/usr/bin/env bash
    set -euo pipefail
    B00T_CLI="${B00T_CLI:-b00t-cli}"
    cd "$HOME/.b00t"
    "$B00T_CLI" soul table-show lifecycle >/dev/null 2>&1 || \
      "$B00T_CLI" soul table-create lifecycle file:text label:text method:text reason:text ts:timestamp

# refresh node.* soul keys from live observation — powers the whoami node preamble
# (compose_node_summary anchors on node.board; see b00t-cli/src/memory_provider.rs)
soul-node-refresh:
    #!/usr/bin/env bash
    set -euo pipefail
    B00T_CLI="${B00T_CLI:-b00t-cli}"
    cd "$HOME/.b00t"
    "$B00T_CLI" soul set node.board "$(hostname)"
    "$B00T_CLI" soul set node.soc "$(cat /sys/class/dmi/id/product_name 2>/dev/null || echo unknown)"
    "$B00T_CLI" soul set node.arch "$(uname -m)"
    "$B00T_CLI" soul set node.ram_gb "$(free -g | awk '/^Mem:/{print $2}')"
    "$B00T_CLI" soul set node.gpu "$(nvidia-smi --query-gpu=name,memory.total --format=csv,noheader 2>/dev/null | head -1 || echo none)"
    "$B00T_CLI" soul set node.os "$(. /etc/os-release && echo "$PRETTY_NAME")"
    "$B00T_CLI" soul set node.serving "$(podman ps --noheading 2>/dev/null | awk '$NF!=""{print $NF}' | paste -sd' ' - || echo none)"
    echo "node soul refreshed — b00t whoami preamble now populated"

# classify unlabeled datum files via ch0nky (thinking ON); restartable via typed missing-file audit
classify limit="0":
    #!/usr/bin/env bash
    set -euo pipefail
    B00T_CLI="${B00T_CLI:-b00t-cli}"
    DATUMS="{{justfile_directory()}}"
    HIVE="$DATUMS/inference-qwen36-35b-a3b-llamacpp.hive.toml"
    EP=$(grep -m1 -oP 'endpoint\s*=\s*"\K[^"]+' "$HIVE")
    KEY=$(grep -m1 -oP 'api_key\s*=\s*"\K[^"]+' "$HIVE")
    cd "$HOME/.b00t"
    pendingf=$(mktemp); trap 'rm -f "$pendingf"' EXIT
    "$B00T_CLI" lifecycle missing --datums "$DATUMS" > "$pendingf"
    n=0
    while IFS= read -r name; do
        [[ -n "$name" ]] || continue
        f="$DATUMS/$name"
        [[ -f "$f" ]] || continue
        label="" ; method="" ; reason=""
        case "$name" in
          *~|*.bak|*.orig|*.swp) label=archive ; method=rule ; reason="backup/editor artifact" ;;
          DEPRECATED*)           label=deprecated ; method=rule ; reason="filename marker" ;;
        esac
        if [[ -z "$label" ]]; then
            body=$({ head -c 6000 "$f"; printf '\n...tail:\n'; tail -c 1200 "$f"; } 2>/dev/null | tr -d '\0')
            prompt=$(printf 'Classify the lifecycle of this file from the b00t datum store (agent config/knowledge corpus). Labels: active (operational, in use) | partial (in use, flagged gaps) | aspirational (plan/PRD/POC, not implemented) | deprecated (superseded, broken, sunset) | archive (historical, backup, one-off log). After reasoning reply with ONLY a JSON object {"label":"<label>","reason":"<max 12 words>"}\n\nfilename: %s\ncontent:\n%s' "$name" "$body")
            resp=$(jq -n --arg p "$prompt" '{model:"ch0nky",temperature:0,max_tokens:3000,messages:[{role:"user",content:$p}]}' \
                | curl -s -m 300 "$EP/chat/completions" -H 'Content-Type: application/json' -H "Authorization: Bearer $KEY" --data-binary @-)
            content=$(jq -r '.choices[0].message.content // empty' <<<"$resp" 2>/dev/null || true)
            obj=$(grep -oP '\{[^{}]*\}' <<<"$content" | tail -1 || true)
            label=$(jq -r '.label // empty' <<<"$obj" 2>/dev/null || true)
            reason=$(jq -r '.reason // empty' <<<"$obj" 2>/dev/null | head -c 110 || true)
            method=ch0nky
            case "$label" in active|partial|aspirational|deprecated|archive) : ;; *) label=unresolved ; reason="unparseable response" ;; esac
        fi
        "$B00T_CLI" soul frame-insert lifecycle file="$name" label="$label" method="$method" reason="${reason:-}" ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)" >/dev/null
        n=$((n+1))
        echo "[$n] $name -> $label"
        if [[ "{{limit}}" != "0" && "$n" -ge "{{limit}}" ]]; then break; fi
    done < "$pendingf"
    echo "classified $n files this run"

# deterministic gate — Rust-owned typed DataFramerr audit
gate:
    #!/usr/bin/env bash
    set -euo pipefail
    B00T_CLI="${B00T_CLI:-b00t-cli}"
    cd "$HOME/.b00t"
    "$B00T_CLI" lifecycle gate --datums "{{justfile_directory()}}"

# label distribution — read side of the flashtable
status:
    #!/usr/bin/env bash
    set -euo pipefail
    B00T_CLI="${B00T_CLI:-b00t-cli}"
    cd "$HOME/.b00t"
    "$B00T_CLI" lifecycle status --datums "{{justfile_directory()}}"
