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
    cd "$HOME/.b00t"
    b00t-cli soul table-show lifecycle >/dev/null 2>&1 || \
      b00t-cli soul table-create lifecycle file:text label:text method:text reason:text ts:timestamp

# refresh node.* soul keys from live observation — powers the whoami node preamble
# (compose_node_summary anchors on node.board; see b00t-cli/src/memory_provider.rs)
soul-node-refresh:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "$HOME/.b00t"
    b00t-cli soul set node.board "$(hostname)"
    b00t-cli soul set node.soc "$(cat /sys/class/dmi/id/product_name 2>/dev/null || echo unknown)"
    b00t-cli soul set node.arch "$(uname -m)"
    b00t-cli soul set node.ram_gb "$(free -g | awk '/^Mem:/{print $2}')"
    b00t-cli soul set node.gpu "$(nvidia-smi --query-gpu=name,memory.total --format=csv,noheader 2>/dev/null | head -1 || echo none)"
    b00t-cli soul set node.os "$(. /etc/os-release && echo "$PRETTY_NAME")"
    b00t-cli soul set node.serving "$(podman ps --noheading 2>/dev/null | awk '$NF!=""{print $NF}' | paste -sd' ' - || echo none)"
    echo "node soul refreshed — b00t whoami preamble now populated"

# classify unlabeled datum files via ch0nky (thinking ON); restartable via cursor scan
classify limit="0":
    #!/usr/bin/env bash
    set -euo pipefail
    DATUMS="{{justfile_directory()}}"
    HIVE="$DATUMS/inference-qwen36-35b-a3b-llamacpp.hive.toml"
    EP=$(grep -m1 -oP 'endpoint\s*=\s*"\K[^"]+' "$HIVE")
    KEY=$(grep -m1 -oP 'api_key\s*=\s*"\K[^"]+' "$HIVE")
    cd "$HOME/.b00t"
    donef=$(mktemp); trap 'rm -f "$donef"' EXIT
    b00t-cli soul cursor-reset lifedone >/dev/null 2>&1 || b00t-cli soul cursor-create lifedone lifecycle >/dev/null
    while row=$(b00t-cli soul cursor-next lifedone 2>/dev/null); do
        grep -oP 'file = Text\("\K[^"]+' <<<"$row" || true
    done > "$donef"
    n=0
    for f in "$DATUMS"/*; do
        [[ -f "$f" ]] || continue
        name=$(basename "$f")
        if grep -qxF "$name" "$donef"; then continue; fi
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
        b00t-cli soul frame-insert lifecycle file="$name" label="$label" method="$method" reason="${reason:-}" ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)" >/dev/null
        n=$((n+1))
        echo "[$n] $name -> $label"
        if [[ "{{limit}}" != "0" && "$n" -ge "{{limit}}" ]]; then break; fi
    done
    echo "classified $n files this run"

# deterministic gate — sm0l output contract: PASS/FAIL, no LLM
gate:
    #!/usr/bin/env bash
    set -euo pipefail
    DATUMS="{{justfile_directory()}}"
    cd "$HOME/.b00t"
    inv=$(find "$DATUMS" -maxdepth 1 -type f | wc -l)
    b00t-cli soul cursor-reset lifegate >/dev/null 2>&1 || b00t-cli soul cursor-create lifegate lifecycle >/dev/null
    total=0 ; unres=0
    while row=$(b00t-cli soul cursor-next lifegate 2>/dev/null); do
        total=$((total+1))
        if grep -q 'label = Text("unresolved")' <<<"$row"; then unres=$((unres+1)); fi
    done
    if [[ "$total" -eq "$inv" ]]; then
        echo "PASS: $((total-unres))/$inv labeled, $unres unresolved"
    else
        echo "FAIL: $total rows for $inv files"
        exit 1
    fi

# label distribution — read side of the flashtable
status:
    #!/usr/bin/env bash
    set -euo pipefail
    cd "$HOME/.b00t"
    b00t-cli soul cursor-reset lifestat >/dev/null 2>&1 || b00t-cli soul cursor-create lifestat lifecycle >/dev/null
    while row=$(b00t-cli soul cursor-next lifestat 2>/dev/null); do
        grep -oP 'label = Text\("\K[^"]+' <<<"$row" || true
    done | sort | uniq -c | sort -rn
