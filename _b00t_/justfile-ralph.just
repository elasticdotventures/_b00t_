# ── ralph with diversity ──────────────────────────────────────────────────────
# ralph-spawn: instantiate a ralph agent with a random personality + transferable skills.
# Karpathy deepwiki OKR pattern: RESEARCH is a separate cycle from EXECUTION.
# Each spawn gets: (a) random personality archetype, (b) random 2-3 transferable skills.
# Different skills → different heuristics → better collective hive diversity.
# 🤓 Never assign the same transferable skills to every agent — entropy is a feature.
# Extracted from root justfile for module isolation.

# List of transferable skills (from _b00t_/*.skill.toml type_tags=["transferable"])
_TRANSFERABLE_SKILLS := "kaizen triz six-sigma ideo mece first-principles socratic bayesian rubber-duck pre-mortem five-whys ockham"

# Personality archetypes — injected as system bias, not hard constraints
_PERSONALITIES := "methodical-skeptic creative-synthesizer devil-advocate systems-thinker pragmatic-fixer pattern-hunter first-principles-zealot bayesian-updater"

# Spawn one ralph: random personality + N random transferable skills, run GOAL
ralph-spawn goal="" n_skills="3" tool="claude-code":
    #!/usr/bin/env bash
    set -euo pipefail

    # ── Sample random personality ─────────────────────────────────────────────
    PERSONALITIES=({{_PERSONALITIES}})
    PERSONALITY="${PERSONALITIES[$RANDOM % ${#PERSONALITIES[@]}]}"
    echo "[ralph:spawn] personality=$PERSONALITY"

    # ── Sample N random transferable skills (no repeats) ─────────────────────
    ALL_SKILLS=({{_TRANSFERABLE_SKILLS}})
    SHUFFLED=($(printf '%s\n' "${ALL_SKILLS[@]}" | shuf))
    ASSIGNED=("${SHUFFLED[@]:0:{{n_skills}}}")
    echo "[ralph:spawn] transferable skills: ${ASSIGNED[*]}"

    # ── Load blessing content for each assigned skill ─────────────────────────
    SKILL_CONTENT=""
    for SKILL in "${ASSIGNED[@]}"; do
      CONTENT=$(b00t-cli learn "$SKILL" --concise 2>/dev/null || true)
      if [ -n "$CONTENT" ]; then
        SKILL_CONTENT=$(printf '%s\n## %s\n%s\n' "$SKILL_CONTENT" "$SKILL" "$CONTENT")
      fi
    done

    # ── Karpathy OKR: RESEARCH phase (separate from execution) ───────────────
    # Research soul is pre-loaded before task starts — not inline during execution.
    # This is NOT generic RAG. It is: goal → OKR decomposition → targeted topic research.
    GOAL_TEXT="{{goal}}"
    if [ -z "$GOAL_TEXT" ]; then
      GOAL_TEXT=$(b00t-cli task next --json 2>/dev/null | jq -r '.title // empty' || true)
    fi
    if [ -z "$GOAL_TEXT" ]; then echo "[ralph] no goal"; exit 1; fi

    echo "[ralph:okr] decomposing goal: $GOAL_TEXT"
    OKR_TOPICS=$(echo "$GOAL_TEXT" | tr '[:upper:]' '[:lower:]' \
      | tr -cs 'a-z0-9-' '\n' | awk 'length>3' | sort -u | head -5)

    echo "[ralph:research] soul topics: $(echo "$OKR_TOPICS" | tr '\n' ' ')"
    while IFS= read -r TOPIC; do
      [ -z "$TOPIC" ] && continue
      SOUL=$(b00t-cli learn "$TOPIC" --concise 2>/dev/null | head -20 || true)
      [ -n "$SOUL" ] && echo "[soul:$TOPIC] loaded ($(echo "$SOUL" | wc -c)c)"
    done <<< "$OKR_TOPICS"

    # ── Compile agent context packet ────────────────────────────────────────────
    TOPICS_STR=$(echo "$OKR_TOPICS" | tr '\n' ' ')
    CTX_FILE=$(mktemp /tmp/ralph-ctx-XXXXXX.md)
    {
      echo "## Ralph Agent Instantiation"
      echo "personality: $PERSONALITY"
      echo "transferable_skills: ${ASSIGNED[*]}"
      echo "goal: $GOAL_TEXT"
      echo "research_topics: $TOPICS_STR"
      echo ""
      echo "## Transferable Skills (active this session)"
      echo "$SKILL_CONTENT"
      echo ""
      echo "## Operating Protocol"
      echo "RESEARCH phase is COMPLETE. Do NOT re-research inline during execution."
      echo "Apply personality ($PERSONALITY) as a cognitive lens, not a hard constraint."
      echo "Transferable skills are heuristics: apply when they clarify."
      echo "Report sharp corners: b00t lfmf <topic> <lesson>"
      echo "Log progress: b00t task update <id> --status done"
    } > "$CTX_FILE"
    AGENT_CTX=$(cat "$CTX_FILE")
    rm -f "$CTX_FILE"

    echo "[ralph:ready] agent context: $(echo "$AGENT_CTX" | wc -c)c"
    echo "$AGENT_CTX"

# ralph-diverse-hive: spawn N ralph agents with independent personalities/skills for same goal
ralph-diverse-hive goal="" n_agents="3" n_skills="3":
    #!/usr/bin/env bash
    set -euo pipefail
    echo "[hive:diverse] spawning {{n_agents}} ralph agents for: {{goal}}"
    for i in $(seq 1 {{n_agents}}); do
      echo "── agent $i/$(({{n_agents}})) ──"
      just ralph::ralph-spawn "{{goal}}" "{{n_skills}}" &
    done
    wait
    echo "[hive:diverse] all agents dispatched"

# compile-agent: compile a sandboxed single-file AGENTS.md for a specific role.
# Output = AGENTS.md boilerplate prefix + role supplement + random transferable skills.
# Operator provisions by running: just compile-agent --role=backend --out=/tmp/agent.md
compile-agent role="worker" n_skills="3" out="/tmp/compiled-agent.md":
    #!/usr/bin/env bash
    set -euo pipefail

    B00T_ROOT=$(git -C "$HOME/.b00t" rev-parse --show-toplevel 2>/dev/null || echo "$HOME/.b00t")
    AGENTS_BASE="$B00T_ROOT/AGENTS.md"
    ROLE_SUPPLEMENT="$B00T_ROOT/AGENTS/--role={{role}}.md"

    # ── Base boilerplate (everything before SESSION delimiter) ─────────────────
    BOILERPLATE=$(sed '/── SESSION/q' "$AGENTS_BASE" | head -n -1)

    # ── Role supplement ────────────────────────────────────────────────────────
    if [ -f "$ROLE_SUPPLEMENT" ]; then
      ROLE_CONTENT=$(cat "$ROLE_SUPPLEMENT")
    else
      echo "⚠️  Role supplement not found: $ROLE_SUPPLEMENT"
      ROLE_CONTENT="## Role: {{role}} (no supplement found — using base protocol only)"
    fi

    # ── Blessing manifest ──────────────────────────────────────────────────────
    BLESSING=$(b00t-cli blessing --manifest --role="{{role}}" 2>/dev/null \
      || echo "# blessing manifest unavailable — run: b00t blessing --manifest --role={{role}}")

    # ── Random transferable skills ─────────────────────────────────────────────
    ALL_SKILLS=(kaizen triz six-sigma ideo mece first-principles socratic bayesian rubber-duck pre-mortem five-whys ockham)
    SHUFFLED=($(printf '%s\n' "${ALL_SKILLS[@]}" | shuf))
    ASSIGNED=("${SHUFFLED[@]:0:{{n_skills}}}")
    SKILL_CONTENT=""
    for SKILL in "${ASSIGNED[@]}"; do
      CONTENT=$(b00t-cli learn "$SKILL" --concise 2>/dev/null | head -30 || true)
      [ -n "$CONTENT" ] && SKILL_CONTENT=$(printf '%s\n### %s\n%s\n' "$SKILL_CONTENT" "$SKILL" "$CONTENT")
    done

    # ── Assemble compiled AGENTS.md ────────────────────────────────────────────
    COMPILED="{{out}}"
    TS=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    {
      echo "$BOILERPLATE"
      echo ""
      echo "## Role: {{role}}"
      echo ""
      echo "$ROLE_CONTENT"
      echo ""
      echo "## Blessing Manifest"
      echo ""
      echo "$BLESSING"
      echo ""
      echo "## Transferable Skills (randomly assigned: ${ASSIGNED[*]})"
      echo "# Each instantiation gets a different random subset for hive diversity."
      echo ""
      echo "$SKILL_CONTENT"
      echo ""
      echo "<!-- SESSION compiled by operator, inject per instantiation"
      echo "Role: {{role}} | Skills: ${ASSIGNED[*]} | Compiled: $TS -->"
    } > "$COMPILED"

    echo "✅ Compiled agent: {{out}} ($(wc -l < {{out}}) lines)"
    echo "   role: {{role}}"
    echo "   skills: ${ASSIGNED[*]}"

# ── end ralph / compile-agent ──────────────────────────────────────────────────

# provision-agent: operator convenience — compile + launch agent for a role+goal in one command.
# Usage: just provision-agent worker "implement a health endpoint"
# Usage: just provision-agent executive "plan Q3 roadmap"
# Operator does NOT need to know about compile-agent or ralph-spawn internals.
provision-agent role="worker" goal="":
    #!/usr/bin/env bash
    set -euo pipefail
    AGENT_FILE="/tmp/b00t-agent-{{role}}-$(date +%s).md"
    echo "[provision] role={{role}}"
    just ralph::compile-agent "{{role}}" 3 "$AGENT_FILE"
    echo "[provision] sandbox: $AGENT_FILE"
    if [ -z "{{goal}}" ]; then
      echo "[provision] no goal specified — agent file ready, launch manually:"
      echo "  claude --agent $AGENT_FILE"
    else
      echo "[provision] launching agent with goal: {{goal}}"
      GOAL_TEXT="{{goal}}"
      echo "# Goal: $GOAL_TEXT" >> "$AGENT_FILE"
      just ralph::ralph-spawn "$GOAL_TEXT" 3 | claude --print --agent "$AGENT_FILE" 2>/dev/null \
        || echo "[provision] agent ready at: $AGENT_FILE (manual launch required if claude not in PATH)"
    fi
