# _b00t_/cake.just — PRD-011 X1: verified execution → artifact → ledgrrr → 🍰
# MOCK ledgrrr interface: same record shape as experiment.rs FOCUS emitters
# (ledgrrr focus append --earned/--consumed), file-backed until ledgrrr-mcp wiring lands.
# Doctrine: cake is allocated to ARTIFACTS (proof of verified execution), never to prose.
# The command runs through `b00t exec` so it also lands in ~/.b00t/exec-log.jsonl (guards+audit).

set shell := ["bash", "-euo", "pipefail", "-c"]

artifacts := home_directory() / ".b00t/.b00t/verification-artifacts.jsonl"
ledger := home_directory() / ".b00t/.b00t/ledgrrr-mock.jsonl"

# run <cmd> audited, mint a verification artifact, submit to (mock) ledgrrr for 🍰
verify task_id +cmd:
    #!/usr/bin/env bash
    set -uo pipefail
    agent="${B00T_AGENT:-$(id -un)}"
    ts=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    git_sha=$(git -C "$HOME/.b00t" rev-parse --short HEAD 2>/dev/null || echo none)
    out=$(b00t-cli exec -- {{cmd}} 2>&1); rc=$?
    evidence=$(grep -oE '^(PASS|FAIL)[^"]*' <<<"$out" | tail -1)
    [[ -n "$evidence" ]] || evidence="exit=$rc"
    artifact=$(jq -nc --arg task "{{task_id}}" --arg agent "$agent" --arg cmd "{{cmd}}" \
        --arg ev "$evidence" --arg sha "$git_sha" --arg ts "$ts" --argjson rc "$rc" \
        '{task:$task,agent:$agent,cmd:$cmd,exit:$rc,evidence:$ev,git_sha:$sha,ts:$ts}')
    aid=$(sha256sum <<<"$artifact$out" | cut -c1-12)
    mkdir -p "$(dirname {{artifacts}})"
    jq -nc --argjson a "$artifact" --arg id "$aid" '$a + {artifact_id:$id}' >> {{artifacts}}
    if [[ $rc -eq 0 && "$evidence" != FAIL* ]]; then earned="1.0"; else earned="0.0"; fi
    jq -nc --arg id "$aid" --arg agent "$agent" --arg task "{{task_id}}" --arg ts "$ts" \
        --argjson earned "$earned" \
        '{ledger:"mock-ledgrrr",category:"verification",artifact:$id,agent:$agent,task:$task,earned:$earned,consumed:0,ts:$ts}' >> {{ledger}}
    echo "$out"
    if [[ "$earned" == "1.0" ]]; then
        echo "🍰 +1.0 cake → $agent | artifact $aid | task #{{task_id}} | evidence: $evidence"
    else
        echo "🧂 no cake — verification FAILED | artifact $aid recorded (failure is also evidence)"
    fi
    exit $rc

# cake balance per agent (mock ledger)
balance:
    @jq -rs 'group_by(.agent) | map({agent:.[0].agent, cake:(map(.earned)|add), verifications:length}) | .[] | "🍰 \(.cake)  \(.agent)  (\(.verifications) verifications)"' {{ledger}} 2>/dev/null || echo "ledger empty — earn cake: just -f _b00t_/cake.just verify <task> <cmd>"

# recent ledger entries
ledger n="10":
    @tail -n {{n}} {{ledger}} 2>/dev/null | jq -r '"\(.ts)  \(.artifact)  task#\(.task)  +\(.earned)🍰  \(.agent)"' || echo "ledger empty"

# b00t:map v1
# summary: verified execution to cake — b00t exec audited run mints artifact, mock ledgrrr allocates 🍰 to artifacts never prose
# tags: cake, ledgrrr, verification, artifact, exec, audit, focus, mock
# tier: sm0l
# cmds: just -f _b00t_/cake.just verify <task> <cmd>, just -f _b00t_/cake.just balance
# complexity: 3
