#!/bin/bash
# Ralph Wiggum + b00t - Enhanced AI agent with NRtW/DRY principles
# Enhanced version integrating b00t g0spell, role-based capabilities, and self-learning
# Fork of ralph-plus-_b00t_ with b00t g0spell integration

set -euo pipefail

# ===== B00T GOSPEL INTEGRATION =====
export B00T_GOSPEL_PATH="${B00T_GOSPEL_PATH:-$HOME/.b00t}"
export B00T_WORKSPACE_PATH="${B00T_WORKSPACE_PATH:-$HOME/_b00t_}"
export B00T_RALPH_CONFIG="$B00T_GOSPEL_PATH/ralphs/ralph-b00t-config.toml"

# Load b00t core functions if available
if [[ -f "$B00T_GOSPEL_PATH/_b00t_.bashrc" ]]; then
    source "$B00T_GOSPEL_PATH/_b00t_.bashrc"
fi

# ===== RALPH ENHANCED PARAMETERS =====
TOOL="${RALPH_TOOL:-amp}"  # Default tool with env override
MAX_ITERATIONS="${RALPH_MAX_ITERATIONS:-10}"
AGENT_ROLE="${RALPH_AGENT_ROLE:-general}"  # New: role-based capabilities
B00T_INTEGRATION="${RALPH_B00T_INTEGRATION:-true}"
NRtW_MODE="${RALPH_NRtW_MODE:-true}"  # Never Reinvent the Wheel
DRY_ENFORCEMENT="${RALPH_DRY_ENFORCEMENT:-true}"  # Don't Repeat Yourself

# ===== AGENT ROLE SYSTEM =====
declare -A AGENT_ROLES
AGENT_ROLES["architect"]="system-design,container-orchestration,compliance-review"
AGENT_ROLES["developer"]="code-implementation,testing,debugging"
AGENT_ROLES["researcher"]="documentation,analysis,library-discovery"
AGENT_ROLES["devops"]="deployment,infrastructure,monitoring"
AGENT_ROLES["general"]="all-capabilities"

# ===== NRtW LIBRARY DISCOVERY =====
declare -A NRtW_LIBRARIES
NRtW_LIBRARIES["python"]="requests,click,rich,typer,fastapi"
NRtW_LIBRARIES["javascript"]="lodash,axios,express,react,vue"
NRtW_LIBRARIES["rust"]="tokio,serde,clap,reqwest,actix"
NRtW_LIBRARIES["go"]="gin,cobra,viper,grpc,gorm"

# ===== CORE FUNCTIONS =====

ralph_whoami() {
    echo "🥾 Ralph Wiggum + b00t Enhanced Agent"
    echo "====================================="
    echo "Tool: $TOOL"
    echo "Role: $AGENT_ROLE"
    echo "Capabilities: ${AGENT_ROLES[$AGENT_ROLE]:-unknown}"
    echo "NRtW Mode: $NRtW_MODE"
    echo "DRY Enforcement: $DRY_ENFORCEMENT"
    echo "b00t Integration: $B00T_INTEGRATION"
    echo "Max Iterations: $MAX_ITERATIONS"
    echo ""

    if [[ "$B00T_INTEGRATION" == "true" ]]; then
        echo "B00t G0spell Status:"
        if [[ -d "$B00T_GOSPEL_PATH" ]]; then
            echo "  ✓ G0spell path: $B00T_GOSPEL_PATH"
        else
            echo "  ✗ G0spell path not found: $B00T_GOSPEL_PATH"
        fi

        if [[ -d "$B00T_WORKSPACE_PATH" ]]; then
            echo "  ✓ Workspace path: $B00T_WORKSPACE_PATH"
        else
            echo "  ✗ Workspace path not found: $B00T_WORKSPACE_PATH"
        fi

        # Check b00t-cli availability
        if command -v b00t-cli &> /dev/null; then
            echo "  ✓ b00t-cli available"
            echo "  Version: $(b00t-cli --version 2>/dev/null || echo 'unknown')"
        else
            echo "  ✗ b00t-cli not available"
        fi
    fi

    echo ""
    echo "Agent Memory:"
    if [[ -f "$PROGRESS_FILE" ]]; then
        echo "  ✓ Progress file: $PROGRESS_FILE"
        echo "  Entries: $(wc -l < "$PROGRESS_FILE")"
    else
        echo "  ✗ No progress file found"
    fi
}

ralph_help() {
    cat << EOF
🥾 Ralph Wiggum + b00t Enhanced Agent

USAGE:
    ralph-b00t-enhanced.sh [OPTIONS] [MAX_ITERATIONS]

OPTIONS:
    --tool TOOL                 Select AI tool (amp, claude, codex) [default: amp]
    --role ROLE                 Set agent role (architect, developer, researcher, devops, general)
    --max-iterations N          Maximum iterations [default: 10]
    --whoami                    Show agent status and capabilities
    --learn TOPIC               Enter learning mode for specific topic
    --discover LANGUAGE         Discover recommended libraries for language
    --no-nrtw                   Disable Never Reinvent the Wheel mode
    --no-dry                    Disable DRY enforcement
    --no-b00t-integration       Disable b00t g0spell integration
    --help, -h                  Show this help message

ROLES:
    architect       System design, container orchestration, compliance review
    developer       Code implementation, testing, debugging
    researcher      Documentation, analysis, library discovery
    devops          Deployment, infrastructure, monitoring
    general         All capabilities (default)

EXAMPLES:
    # Run with default settings
    ./ralph-b00t-enhanced.sh

    # Run as architect with Claude
    ./ralph-b00t-enhanced.sh --tool claude --role architect

    # Discover Python libraries using NRtW principles
    ./ralph-b00t-enhanced.sh --discover python

    # Learn about Docker patterns
    ./ralph-b00t-enhanced.sh --learn docker

    # Show agent status
    ./ralph-b00t-enhanced.sh --whoami

B00T INTEGRATION:
    The enhanced Ralph integrates with b00t g0spell for:
    - Capability loading based on agent roles
    - Library discovery using NRtW principles
    - DRY pattern enforcement
    - Self-learning and memoization
    - Sub-agent coordination

EOF
}

# ===== COMMAND LINE PARSING =====
while [[ $# -gt 0 ]]; do
    case $1 in
        --tool)
            TOOL="$2"
            shift 2
            ;;
        --tool=*)
            TOOL="${1#*=}"
            shift
            ;;
        --role)
            AGENT_ROLE="$2"
            shift 2
            ;;
        --role=*)
            AGENT_ROLE="${1#*=}"
            shift
            ;;
        --max-iterations)
            MAX_ITERATIONS="$2"
            shift 2
            ;;
        --max-iterations=*)
            MAX_ITERATIONS="${1#*=}"
            shift
            ;;
        --no-nrtw)
            NRtW_MODE="false"
            shift
            ;;
        --no-dry)
            DRY_ENFORCEMENT="false"
            shift
            ;;
        --b00t-integration)
            B00T_INTEGRATION="true"
            shift
            ;;
        --no-b00t-integration)
            B00T_INTEGRATION="false"
            shift
            ;;
        --whoami)
            ralph_whoami
            exit 0
            ;;
        --learn)
            ralph_learn_mode "$2"
            shift 2
            ;;
        --discover)
            ralph_discover_libraries "$2"
            shift 2
            ;;
        --help|-h)
            ralph_help
            exit 0
            ;;
        *)
            if [[ "$1" =~ ^[0-9]+$ ]]; then
                MAX_ITERATIONS="$1"
            fi
            shift
            ;;
    esac
done

# ===== CORE FUNCTIONS =====

ralph_whoami() {
    echo "🥾 Ralph Wiggum + b00t Enhanced Agent"
    echo "====================================="
    echo "Tool: $TOOL"
    echo "Role: $AGENT_ROLE"
    echo "Capabilities: ${AGENT_ROLES[$AGENT_ROLE]:-unknown}"
    echo "NRtW Mode: $NRtW_MODE"
    echo "DRY Enforcement: $DRY_ENFORCEMENT"
    echo "b00t Integration: $B00T_INTEGRATION"
    echo "Max Iterations: $MAX_ITERATIONS"
    echo ""

    if [[ "$B00T_INTEGRATION" == "true" ]]; then
        echo "B00t G0spell Status:"
        if [[ -d "$B00T_GOSPEL_PATH" ]]; then
            echo "  ✓ G0spell path: $B00T_GOSPEL_PATH"
        else
            echo "  ✗ G0spell path not found: $B00T_GOSPEL_PATH"
        fi

        if [[ -d "$B00T_WORKSPACE_PATH" ]]; then
            echo "  ✓ Workspace path: $B00T_WORKSPACE_PATH"
        else
            echo "  ✗ Workspace path not found: $B00T_WORKSPACE_PATH"
        fi

        # Check b00t-cli availability
        if command -v b00t-cli &> /dev/null; then
            echo "  ✓ b00t-cli available"
            echo "  Version: $(b00t-cli --version 2>/dev/null || echo 'unknown')"
        else
            echo "  ✗ b00t-cli not available"
        fi
    fi

    echo ""
    echo "Agent Memory:"
    if [[ -f "$PROGRESS_FILE" ]]; then
        echo "  ✓ Progress file: $PROGRESS_FILE"
        echo "  Entries: $(wc -l < "$PROGRESS_FILE")"
    else
        echo "  ✗ No progress file found"
    fi
}

ralph_help() {
    cat << EOF
🥾 Ralph Wiggum + b00t Enhanced Agent

USAGE:
    ralph-b00t-enhanced.sh [OPTIONS] [MAX_ITERATIONS]

OPTIONS:
    --tool TOOL                 Select AI tool (amp, claude, codex) [default: amp]
    --role ROLE                 Set agent role (architect, developer, researcher, devops, general)
    --max-iterations N          Maximum iterations [default: 10]
    --whoami                    Show agent status and capabilities
    --learn TOPIC               Enter learning mode for specific topic
    --discover LANGUAGE         Discover recommended libraries for language
    --no-nrtw                   Disable Never Reinvent the Wheel mode
    --no-dry                    Disable DRY enforcement
    --no-b00t-integration       Disable b00t g0spell integration
    --help, -h                  Show this help message

ROLES:
    architect       System design, container orchestration, compliance review
    developer       Code implementation, testing, debugging
    researcher      Documentation, analysis, library discovery
    devops          Deployment, infrastructure, monitoring
    general         All capabilities (default)

EXAMPLES:
    # Run with default settings
    ./ralph-b00t-enhanced.sh

    # Run as architect with Claude
    ./ralph-b00t-enhanced.sh --tool claude --role architect

    # Discover Python libraries using NRtW principles
    ./ralph-b00t-enhanced.sh --discover python

    # Learn about Docker patterns
    ./ralph-b00t-enhanced.sh --learn docker

    # Show agent status
    ./ralph-b00t-enhanced.sh --whoami

B00T INTEGRATION:
    The enhanced Ralph integrates with b00t g0spell for:
    - Capability loading based on agent roles
    - Library discovery using NRtW principles
    - DRY pattern enforcement
    - Self-learning and memoization
    - Sub-agent coordination

EOF
}

ralph_discover_libraries() {
    local language="$1"
    echo "🔍 NRtW Library Discovery for $language"
    echo "======================================"

    if [[ -n "${NRtW_LIBRARIES[$language]:-}" ]]; then
        echo "✅ Recommended libraries (NRtW approved):"
        IFS=',' read -ra libs <<< "${NRtW_LIBRARIES[$language]}"
        for lib in "${libs[@]}"; do
            echo "  📦 $lib"
        done
        echo ""
        echo "💡 These libraries are widely adopted, well-maintained, and follow b00t principles."
        echo "🚫 Avoid building custom solutions when these exist."
    else
        echo "❓ No curated library list for $language"
        echo "🤓 Consider researching popular, well-maintained libraries that solve your problem."
    fi
}

ralph_learn_mode() {
    local topic="$1"
    echo "🧠 Ralph Learning Mode: $topic"
    echo "=============================="

    # Create learning directory if it doesn't exist
    local learn_dir="$SCRIPT_DIR/learning"
    mkdir -p "$learn_dir"

    local learn_file="$learn_dir/${topic}-$(date +%Y%m%d).md"

    echo "Creating learning record: $learn_file"
    cat > "$learn_file" << EOF
# Ralph Learning: $topic
Date: $(date)

## What I Learned

## Patterns Discovered

## Gotchas Encountered

## Recommendations for Future

## B00t G0spell Integration
- Relevant b00t skills:
- Recommended tools:
- NRtW applications:

EOF

    echo "✅ Learning record created. Fill it with insights!"
}

ralph_check_nrtw_violation() {
    local file_path="$1"
    local language="$2"

    if [[ "$NRtW_MODE" != "true" ]]; then
        return 0
    fi

    echo "🔍 NRtW Check: $file_path"

    # Common anti-patterns that violate NRtW
    local violations=()

    case "$language" in
        python)
            # Check for common reinventions
            grep -q "import urllib" "$file_path" && violations+=("Consider using 'requests' instead of urllib")
            grep -q "class.*HTTP" "$file_path" && violations+=("Consider using existing HTTP libraries")
            grep -q "def.*parse.*json" "$file_path" && violations+=("Use built-in json module or pydantic")
            ;;
        javascript)
            grep -q "function.*debounce" "$file_path" && violations+=("Use lodash.debounce instead")
            grep -q "function.*throttle" "$file_path" && violations+=("Use lodash.throttle instead")
            ;;
        rust)
            grep -q "impl.*HttpClient" "$file_path" && violations+=("Use reqwest instead of custom HTTP client")
            ;;
    esac

    if [[ ${#violations[@]} -gt 0 ]]; then
        echo "⚠️  NRtW Violations detected:"
        for violation in "${violations[@]}"; do
            echo "   - $violation"
        done
        return 1
    fi

    echo "✅ No NRtW violations detected"
    return 0
}

ralph_enforce_dry() {
    local directory="$1"

    if [[ "$DRY_ENFORCEMENT" != "true" ]]; then
        return 0
    fi

    echo "🔍 DRY Enforcement: $directory"

    # Look for duplicate code patterns
    local duplicate_files=()

    # Simple heuristic: find files with similar content
    while IFS= read -r file; do
        local basename=$(basename "$file")
        local dirname=$(dirname "$file")

        # Look for similar files in other directories
        find "$directory" -name "*$basename*" -not -path "$file" | while read -r similar_file; do
            if diff -q "$file" "$similar_file" >/dev/null 2>&1; then
                echo "⚠️  Identical files detected:"
                echo "   $file"
                echo "   $similar_file"
                duplicate_files+=("$file:$similar_file")
            fi
        done
    done < <(find "$directory" -type f -name "*.py" -o -name "*.js" -o -name "*.ts" -o -name "*.rs")

    if [[ ${#duplicate_files[@]} -gt 0 ]]; then
        echo "💡 Consider extracting common functionality into shared modules"
        return 1
    fi

    echo "✅ No obvious DRY violations detected"
    return 0
}

ralph_b00t_integration() {
    if [[ "$B00T_INTEGRATION" != "true" ]]; then
        return 0
    fi

    echo "🔗 Integrating with b00t g0spell..."

    # Check if b00t-cli is available
    if ! command -v b00t-cli &> /dev/null; then
        echo "⚠️  b00t-cli not available, skipping integration"
        return 1
    fi

    # Load agent capabilities based on role
    echo "Loading capabilities for role: $AGENT_ROLE"

    # Use b00t to discover relevant skills
    if [[ -n "${AGENT_ROLES[$AGENT_ROLE]:-}" ]]; then
        IFS=',' read -ra capabilities <<< "${AGENT_ROLES[$AGENT_ROLE]}"
        for capability in "${capabilities[@]}"; do
            echo "  📋 Loading capability: $capability"
            # TODO: Integrate with b00t skill system
            # b00t learn "$capability" 2>/dev/null || echo "    ⚠️  Skill not available: $capability"
        done
    fi

    return 0
}

# ===== MAIN EXECUTION =====

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PRD_FILE="$SCRIPT_DIR/prd.json"
PROGRESS_FILE="$SCRIPT_DIR/progress.txt"
ARCHIVE_DIR="$SCRIPT_DIR/archive"
LAST_BRANCH_FILE="$SCRIPT_DIR/.last-branch"

# Validate tool choice
if [[ "$TOOL" != "amp" && "$TOOL" != "claude" && "$TOOL" != "codex" ]]; then
    echo "Error: Invalid tool '$TOOL'. Must be 'amp', 'claude', or 'codex'."
    exit 1
fi

echo "🥾 Starting Ralph Wiggum + b00t Enhanced Agent"
echo "=============================================="
echo "Tool: $TOOL"
echo "Role: $AGENT_ROLE"
echo "Max iterations: $MAX_ITERATIONS"
echo "NRtW Mode: $NRtW_MODE"
echo "DRY Enforcement: $DRY_ENFORCEMENT"
echo "b00t Integration: $B00T_INTEGRATION"
echo ""

# Initialize b00t integration
ralph_b00t_integration

# Archive previous run if branch changed (existing logic)
if [[ -f "$PRD_FILE" ]] && [[ -f "$LAST_BRANCH_FILE" ]]; then
    CURRENT_BRANCH=$(jq -r '.branchName // empty' "$PRD_FILE" 2>/dev/null || echo "")
    LAST_BRANCH=$(cat "$LAST_BRANCH_FILE" 2>/dev/null || echo "")

    if [[ -n "$CURRENT_BRANCH" ]] && [[ -n "$LAST_BRANCH" ]] && [[ "$CURRENT_BRANCH" != "$LAST_BRANCH" ]]; then
        DATE=$(date +%Y-%m-%d)
        FOLDER_NAME=$(echo "$LAST_BRANCH" | sed 's|^ralph/||')
        ARCHIVE_FOLDER="$ARCHIVE_DIR/$DATE-$FOLDER_NAME"

        echo "📦 Archiving previous run: $LAST_BRANCH"
        mkdir -p "$ARCHIVE_FOLDER"
        [[ -f "$PRD_FILE" ]] && cp "$PRD_FILE" "$ARCHIVE_FOLDER/"
        [[ -f "$PROGRESS_FILE" ]] && cp "$PROGRESS_FILE" "$ARCHIVE_FOLDER/"
        echo "   Archived to: $ARCHIVE_FOLDER"

        echo "# Ralph Progress Log" > "$PROGRESS_FILE"
        echo "Started: $(date)" >> "$PROGRESS_FILE"
        echo "---" >> "$PROGRESS_FILE"
    fi
fi

# Track current branch
if [[ -f "$PRD_FILE" ]]; then
    CURRENT_BRANCH=$(jq -r '.branchName // empty' "$PRD_FILE" 2>/dev/null || echo "")
    if [[ -n "$CURRENT_BRANCH" ]]; then
        echo "$CURRENT_BRANCH" > "$LAST_BRANCH_FILE"
    fi
fi

# Initialize progress file
if [[ ! -f "$PROGRESS_FILE" ]]; then
    echo "# Ralph Progress Log" > "$PROGRESS_FILE"
    echo "Started: $(date)" >> "$PROGRESS_FILE"
    echo "---" >> "$PROGRESS_FILE"
fi

# Add b00t integration notes to progress
if [[ "$B00T_INTEGRATION" == "true" ]]; then
    echo "## B00t Integration Active" >> "$PROGRESS_FILE"
    echo "- Role: $AGENT_ROLE" >> "$PROGRESS_FILE"
    echo "- NRtW Mode: $NRtW_MODE" >> "$PROGRESS_FILE"
    echo "- DRY Enforcement: $DRY_ENFORCEMENT" >> "$PROGRESS_FILE"
    echo "---" >> "$PROGRESS_FILE"
fi

echo "🚀 Starting main execution loop..."

# Main execution loop (enhanced version of original)
for i in $(seq 1 "$MAX_ITERATIONS"); do
    echo ""
    echo "==============================================================="
    echo "  Ralph Iteration $i of $MAX_ITERATIONS ($TOOL) [$AGENT_ROLE]"
    echo "==============================================================="

    # Pre-execution checks
    if [[ "$NRtW_MODE" == "true" ]]; then
        echo "🔍 Running NRtW pre-checks..."
        # TODO: Implement NRtW checks on current codebase
    fi

    # Run the selected tool with enhanced prompt
    if [[ "$TOOL" == "amp" ]]; then
        OUTPUT=$(cat "$SCRIPT_DIR/prompt.md" | amp --dangerously-allow-all 2>&1 | tee /dev/stderr) || true
    elif [[ "$TOOL" == "codex" ]]; then
        if [[ ! -f "$CODEX_PROMPT_FILE" ]]; then
            echo "Error: CODEX_PROMPT_FILE not found: $CODEX_PROMPT_FILE"
            exit 1
        fi

        # Enhanced Codex prompt with b00t integration
        CODEX_PROMPT_CONTENT="$(cat "$CODEX_PROMPT_FILE")"

        # Add b00t-specific instructions if integration is enabled
        if [[ "$B00T_INTEGRATION" == "true" ]]; then
            CODEX_PROMPT_CONTENT+="

## B00t G0spell Integration
You are running with b00t integration enabled. Follow these principles:
- Use 'b00t learn' to load skills when needed
- Apply NRtW (Never Reinvent the Wheel) - prefer existing libraries
- Enforce DRY (Don't Repeat Yourself) - avoid duplicate code
- Document patterns in CLAUDE.md files for future agents
- Use b00t-cli for project management when appropriate
"
        fi

        CODEX_ARGS=(exec -m "$CODEX_MODEL" --config "model_reasoning_effort=\"$CODEX_REASONING_EFFORT\"" --sandbox "$CODEX_SANDBOX")
        if [[ "$CODEX_FULL_AUTO" == "true" ]]; then
            CODEX_ARGS+=(--full-auto)
        fi
        if [[ -n "$CODEX_EXTRA_ARGS" ]]; then
            CODEX_ARGS+=($CODEX_EXTRA_ARGS)
        fi

        OUTPUT=$(codex "${CODEX_ARGS[@]}" "$CODEX_PROMPT_CONTENT" 2>&1 | tee /dev/stderr) || true
    else
        # Claude Code with b00t integration
        ENHANCED_CLAUDE_PROMPT="$SCRIPT_DIR/CLAUDE.md"
        if [[ "$B00T_INTEGRATION" == "true" ]]; then
            # Create enhanced prompt with b00t instructions
            ENHANCED_CLAUDE_PROMPT="$SCRIPT_DIR/CLAUDE-b00t-enhanced.md"
            cat "$SCRIPT_DIR/CLAUDE.md" > "$ENHANCED_CLAUDE_PROMPT"
            cat >> "$ENHANCED_CLAUDE_PROMPT" << EOF

## B00t G0spell Compliance ($AGENT_ROLE Role)

### NRtW (Never Reinvent the Wheel)
- Before implementing anything, search for existing solutions
- Use b00t-cli to discover available tools and libraries
- Prefer mature, well-maintained libraries over custom code
- Document library choices in progress.txt

### DRY (Don't Repeat Yourself)
- Extract common patterns into reusable functions/modules
- Use b00t's skill system to avoid duplicating knowledge
- Create CLAUDE.md files with reusable patterns
- Reference existing solutions instead of copying code

### B00t Integration
- Use 'b00t learn' to load relevant skills
- Follow b00t g0spell principles and patterns
- Document b00t-specific learnings in progress.txt
- Use b00t-cli for project management tasks

EOF
        fi

        OUTPUT=$(claude --model sonnet --dangerously-skip-permissions --print < "$ENHANCED_CLAUDE_PROMPT" 2>&1 | tee /dev/stderr) || true
    fi

    # Post-execution analysis
    if [[ "$NRtW_MODE" == "true" ]] || [[ "$DRY_ENFORCEMENT" == "true" ]]; then
        echo "🔍 Running post-execution analysis..."

        # Find recently modified files
        local modified_files=()
        while IFS= read -r file; do
            modified_files+=("$file")
        done < <(find . -type f -name "*.py" -o -name "*.js" -o -name "*.ts" -o -name "*.rs" -mtime -1)

        for file in "${modified_files[@]}"; do
            if [[ -f "$file" ]]; then
                local language=""
                case "$file" in
                    *.py) language="python" ;;
                    *.js) language="javascript" ;;
                    *.ts) language="typescript" ;;
                    *.rs) language="rust" ;;
                esac

                if [[ -n "$language" ]]; then
                    if [[ "$NRtW_MODE" == "true" ]]; then
                        ralph_check_nrtw_violation "$file" "$language" || true
                    fi
                fi
            fi
        done

        if [[ "$DRY_ENFORCEMENT" == "true" ]]; then
            ralph_enforce_dry "." || true
        fi
    fi

    # Check for completion signal
    if echo "$OUTPUT" | grep -q "<promise>COMPLETE</promise>"; then
        echo ""
        echo "🎉 Ralph completed all tasks!"
        echo "Completed at iteration $i of $MAX_ITERATIONS"

        if [[ "$B00T_INTEGRATION" == "true" ]]; then
            echo ""
            echo "📋 Final b00t integration summary:"
            echo "  - Role: $AGENT_ROLE"
            echo "  - NRtW violations checked: $NRtW_MODE"
            echo "  - DRY enforcement applied: $DRY_ENFORCEMENT"
            echo "  - Check progress.txt for detailed learnings"
        fi

        exit 0
    fi

    echo "Iteration $i complete. Continuing..."
    sleep 2
done

echo ""
echo "⚠️  Ralph reached max iterations ($MAX_ITERATIONS) without completing all tasks."
echo "Check $PROGRESS_FILE for status and learnings."
exit 1