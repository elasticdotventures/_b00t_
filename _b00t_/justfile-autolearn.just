# ── autolearn + research-soul — Self-improving OODA skill selection ─────────
# Extracted from root justfile for module isolation.

# mcp-surface: show the 5 surface tools exposed to sub-agents
mcp-surface:
    @echo "Surface tools (5):" && grep "register::<" b00t-mcp/src/mcp_tools.rs | grep -v "^//"

# mcp-catalog: list all 50+ tools in the autodiscovery catalog
mcp-catalog:
    cargo run --bin b00t-cli -p b00t-cli -- exec "ontology query" 2>/dev/null || b00t-cli ontology query

# autolearn: OODA cycle — goal-driven skill selection over compiled soul pages.
# Soul = b00t learn output (datum content). Research is SEPARATE (research-soul recipe).
# O: observe goal  O: orient via FOL+recall+fs → weighted rerank  D: smol rerank hook
# A: iterate candidates; soul-quality gate → queue research-soul if thin  P: persist
autolearn:
    #!/usr/bin/env bash
    set -euo pipefail

    # ── OBSERVE ──────────────────────────────────────────────────────────────
    GOAL=$(b00t-cli task next --json 2>/dev/null | jq -r '.title // empty' || true)
    if [ -z "$GOAL" ]; then echo "[observe] no goal in queue"; exit 0; fi
    echo "[observe] goal: $GOAL"
    GOAL_WORDS=$(echo "$GOAL" | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9' '\n' | awk 'length>2' | sort -u)

    # ── ORIENT ───────────────────────────────────────────────────────────────
    # Source 1: past recall from knowledge graph (frequency-ranked)
    PAST_SKILLS=$(b00t-cli data fabric query \
      --predicate "b00t:informedBy" --namespace autolearn --format json 2>/dev/null \
      | jq -r '.[].object' | sort | uniq -c | sort -rn | head -5 | awk '{print $2}')
    [ -n "$PAST_SKILLS" ] && echo "[orient:recall] $(echo "$PAST_SKILLS" | tr '\n' ' ')"

    # Source 2: FOL-adjacent — Horn reachability + depends_on over knowledge graph
    FOL_ADJACENT=$(b00t-cli data fabric adjacent \
      --goal "$GOAL" --namespace autolearn --top 5 2>/dev/null \
      | awk 'NF>=2 {print $2}' || true)
    [ -n "$FOL_ADJACENT" ] && echo "[orient:fol] $(echo "$FOL_ADJACENT" | tr '\n' ' ')"

    # Source 3: filesystem datum scan (keyword match against _b00t_ topic files)
    B00T_ROOT=$(git -C "$HOME/.b00t" rev-parse --show-toplevel 2>/dev/null || echo "$HOME/.b00t")
    DATUM_DIR="$B00T_ROOT/_b00t_"
    FS_CANDIDATES=""
    while IFS= read -r WORD; do
      [ -z "$WORD" ] && continue
      if ls "$DATUM_DIR/"*"$WORD"*.toml 2>/dev/null | grep -q .; then
        FS_CANDIDATES=$(printf '%s\n%s' "$FS_CANDIDATES" "$WORD")
      fi
    done <<< "$GOAL_WORDS"
    [ -n "$FS_CANDIDATES" ] && echo "[orient:fs] $(echo "$FS_CANDIDATES" | tr '\n' ' ')"

    # Weighted aggregate rerank: recall=3 (proven), fol=2 (inferred), fs=1 (candidate)
    RANKED=$( \
      { \
        echo "$PAST_SKILLS"  | grep -v '^$' | while IFS= read -r s; do echo "3 $s"; done; \
        echo "$FOL_ADJACENT" | grep -v '^$' | while IFS= read -r s; do echo "2 $s"; done; \
        echo "$FS_CANDIDATES"| grep -v '^$' | while IFS= read -r s; do echo "1 $s"; done; \
      } | awk 'NF==2 { score[$2] += $1 } END { for (k in score) print score[k], k }' \
        | sort -rn | awk '{print $2}' | head -10 \
    )
    if [ -z "$RANKED" ]; then echo "[orient] no candidates — skip"; exit 0; fi
    echo "[orient] weighted ranked: $(echo "$RANKED" | tr '\n' ' ')"

    # ── DECIDE: optional smol model rerank (degrades if ollama unavailable) ──
    SMOL_MODEL=$(ollama list 2>/dev/null | grep -oiE 'qwen[0-9.:-]+|phi[0-9.-]+' | head -1 || true)
    if [ -n "$SMOL_MODEL" ]; then
      SMOL_LIST=$(echo "$RANKED" | head -5 | tr '\n' '|' | sed 's/|$//')
      SMOL_PROMPT="Goal: $GOAL. Candidates: $SMOL_LIST. Which ONE candidate name best matches the goal? Output the candidate name only, nothing else."
      SMOL_PICK=$(ollama run "$SMOL_MODEL" "$SMOL_PROMPT" 2>/dev/null \
        | head -1 | tr -d '"' | xargs 2>/dev/null || true)
      if echo "$RANKED" | grep -qx "$SMOL_PICK" 2>/dev/null; then
        echo "[decide:smol] $SMOL_MODEL promoted: $SMOL_PICK"
        RANKED=$(printf '%s\n%s' "$SMOL_PICK" "$(echo "$RANKED" | grep -vx "$SMOL_PICK")")
      fi
    fi

    # ── ACT: iterate candidates through soul-quality gate + 2-stage review ──
    CHOSEN=""
    LEARN_OUT=""
    while IFS= read -r BEST; do
      [ -z "$BEST" ] && continue
      echo "[decide] trying: $BEST"

      # Soul load: b00t learn IS the soul page (Karpathy: datum content = compiled wiki page)
      # 🤓 NOT a RAG check — the soul is the datum, loaded directly into context
      SOUL=$(b00t-cli learn "$BEST" 2>&1) && SOUL_OK=true || SOUL_OK=false
      if [ "$SOUL_OK" = false ]; then
        # Failure discriminator: permanent = no datum; transient = runtime error
        if echo "$SOUL" | grep -qiE "not found|no such|unknown topic|not registered"; then
          echo "[soul:missing] '$BEST' has no datum — queuing research-soul + skip"
          b00t-cli task add "research-soul: $BEST" 2>/dev/null || true
          b00t-cli lfmf "autolearn" "soul missing for '$BEST' — research-soul queued" 2>/dev/null || true
        else
          echo "[soul:transient] '$BEST' error — skip"
        fi
        continue
      fi

      # Soul quality gate: thin soul = knowledge not yet compiled → queue research-soul
      SOUL_LEN=$(echo "$SOUL" | wc -c)
      if [ "$SOUL_LEN" -lt 200 ]; then
        echo "[soul:thin] '$BEST' only ${SOUL_LEN}c — queuing research-soul + skip"
        b00t-cli task add "research-soul: $BEST" 2>/dev/null || true
        continue
      fi

      # Review stage 1: keyword overlap between soul content and goal words
      OVERLAP=$(echo "$SOUL" | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9' '\n' \
        | grep -cFf <(echo "$GOAL_WORDS") || echo 0)
      echo "[review:1:local] overlap=$OVERLAP for '$BEST'"

      # Review stage 2: independent grok reviewer (vector similarity — different algorithm)
      GROK_HITS=$(b00t-cli grok ask "$GOAL $BEST" --limit 3 2>/dev/null \
        | grep -c "$BEST" || echo 0)
      echo "[review:2:grok] vector_endorsement=$GROK_HITS for '$BEST'"

      if [ "$OVERLAP" -gt 0 ] || [ "$GROK_HITS" -gt 0 ]; then
        CHOSEN="$BEST"
        LEARN_OUT="$SOUL"
        break
      fi

      echo "[review] REJECT '$BEST' (overlap=0, grok=0) — next"
      b00t-cli lfmf "autolearn" \
        "soul '$BEST' rejected by both reviewers for: $GOAL" 2>/dev/null || true
    done <<< "$RANKED"

    # All candidates exhausted: queue segmented research for top candidates
    # 🤓 research is NOT done inline; each topic gets its own deliberate research-soul cycle
    if [ -z "$CHOSEN" ]; then
      echo "[autolearn] no candidate passed review — queuing research-soul for knowledge gaps"
      echo "$RANKED" | head -3 | while IFS= read -r TOPIC; do
        [ -z "$TOPIC" ] && continue
        b00t-cli task add "research-soul: $TOPIC" 2>/dev/null || true
        echo "[queue] research-soul: $TOPIC"
      done
      exit 1
    fi

    echo "[act] accepted: $CHOSEN (${SOUL_LEN}c soul)"

    # ── PERSIST: store goal→skill in knowledge graph for future FOL recall ───
    GOAL_HASH=$(echo "$GOAL" | sha256sum | head -c12)
    b00t-cli data fabric upsert \
      --subject "ooda:goal:$GOAL_HASH" --predicate "b00t:informedBy" \
      --object "$CHOSEN" --namespace autolearn 2>/dev/null || true
    b00t-cli data fabric upsert \
      --subject "ooda:goal:$GOAL_HASH" --predicate "b00t:goalText" \
      --object "$GOAL" --namespace autolearn 2>/dev/null || true
    echo "[persist] ooda:goal:$GOAL_HASH → b00t:informedBy → $CHOSEN"

# research-soul: Karpathy-pattern deliberate research cycle for a specific topic.
# Separate from autolearn — this is the INGEST operation that compiles a topic's soul page.
# raw sources → LLM compile via grok assimilate → datum soul update → log
# 🤓 run this when autolearn queues "research-soul: <topic>" tasks
research-soul topic="":
    #!/usr/bin/env bash
    set -euo pipefail
    TOPIC="{{topic}}"
    if [ -z "$TOPIC" ]; then echo "usage: just research-soul topic=<name>"; exit 1; fi
    echo "[research:soul] compiling soul for: $TOPIC"

    B00T_ROOT=$(git -C "$HOME/.b00t" rev-parse --show-toplevel 2>/dev/null || echo "$HOME/.b00t")
    DATUM_DIR="$B00T_ROOT/_b00t_"

    # Measure current soul thickness before research
    CURRENT_SOUL=$(b00t-cli learn "$TOPIC" 2>/dev/null || true)
    echo "[soul:before] ${#CURRENT_SOUL}c"

    # Source discovery: probed in specificity order; first non-empty wins
    RAW=""

    # Source 1: explicit source URL in datum toml
    DATUM_FILE=$(ls "$DATUM_DIR/"*"$TOPIC"*.toml 2>/dev/null | head -1 || true)
    if [ -n "$DATUM_FILE" ]; then
      SOURCE_URL=$(grep -oP '(?<=source\s=\s")[^"]+' "$DATUM_FILE" 2>/dev/null | head -1 || true)
      if [ -n "$SOURCE_URL" ]; then
        echo "[source:datum] $SOURCE_URL"
        RAW=$(curl -sf --max-time 15 "$SOURCE_URL" 2>/dev/null | head -300 || true)
      fi
    fi

    # Source 2: GitHub top repo by stars for this topic name
    if [ -z "$RAW" ]; then
      GH_REPO=$(gh search repos "$TOPIC" --sort stars --limit 1 --json fullName 2>/dev/null \
        | jq -r '.[0].fullName // empty' || true)
      if [ -n "$GH_REPO" ]; then
        echo "[source:github] $GH_REPO"
        RAW=$(gh api "repos/$GH_REPO/readme" --jq '.content' 2>/dev/null \
          | base64 -d 2>/dev/null | head -200 || true)
      fi
    fi

    # Source 3: crates.io metadata + repo README (Rust crates)
    if [ -z "$RAW" ]; then
      CRATE=$(curl -sf --max-time 8 -H "User-Agent: b00t-research/1.0" \
        "https://crates.io/api/v1/crates/$TOPIC" 2>/dev/null || true)
      if [ -n "$CRATE" ]; then
        DESC=$(echo "$CRATE" | jq -r '.crate.description // empty' 2>/dev/null || true)
        REPO=$(echo "$CRATE" | jq -r '.crate.repository // empty' 2>/dev/null || true)
        RAW="$DESC"
        if [ -n "$REPO" ]; then
          REPO_PATH=$(echo "$REPO" | sed 's|https://github.com/||')
          README=$(gh api "repos/$REPO_PATH/readme" --jq '.content' 2>/dev/null \
            | base64 -d 2>/dev/null | head -150 || true)
          [ -n "$README" ] && RAW=$(printf '%s\n%s' "$RAW" "$README")
        fi
        [ -n "$RAW" ] && echo "[source:crates.io] ${#RAW}c"
      fi
    fi

    if [ -z "$RAW" ]; then
      echo "[research:soul] no sources found for '$TOPIC'"
      b00t-cli data fabric upsert \
        --subject "research:gap:$TOPIC" --predicate "b00t:researchGap" \
        --object "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --namespace research-log 2>/dev/null || true
      exit 1
    fi

    echo "[source] raw: ${#RAW}c — compiling soul via assimilate"

    # Compile raw → soul page (LLM-distill → datum update)
    b00t-cli grok assimilate "$RAW" -t "$TOPIC" 2>/dev/null \
      && echo "[soul:compiled] $TOPIC" \
      || echo "[soul:warn] assimilate non-zero (may still have updated)"

    # Log research cycle in knowledge graph
    TS=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    b00t-cli data fabric upsert \
      --subject "research:soul:$TOPIC" --predicate "b00t:researchedAt" \
      --object "$TS" --namespace research-log 2>/dev/null || true
    b00t-cli data fabric upsert \
      --subject "research:soul:$TOPIC" --predicate "b00t:soulSizeBefore" \
      --object "${#CURRENT_SOUL}" --namespace research-log 2>/dev/null || true
    echo "[persist] research:soul:$TOPIC @ $TS"

# autolearn-loop: run OODA cycles until task queue empty, max 10 iterations
autolearn-loop:
    #!/usr/bin/env bash
    N=0; MAX=10
    while [ $N -lt $MAX ]; do
      NEXT=$(b00t-cli task next 2>/dev/null | head -1 || true)
      [ -z "$NEXT" ] && echo "[loop] queue empty after $N cycles" && exit 0
      N=$((N+1))
      echo "[loop] cycle $N/$MAX"
      just autolearn::autolearn || echo "[loop] cycle $N failed — continuing"
    done
    echo "[loop] max cycles reached"
