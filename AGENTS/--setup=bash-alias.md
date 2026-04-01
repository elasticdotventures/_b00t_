# B00t Bash Alias Installation

**Goal:** Install `b00t` as a bash alias that discovers and invokes the right b00t-cli or MCP backend, with hidden capability gating.

## Installation

Add to `~/.bashrc` or `~/.zshrc`:

```bash
# b00t alias with capability gating
alias b00t='b00t_dispatch'

b00t_dispatch() {
    local cmd="$1"
    shift

    # Route to appropriate backend
    if command -v b00t-cli &>/dev/null; then
        # Primary: rust binary
        b00t-cli "$cmd" "$@"
    elif command -v b00t-mcp &>/dev/null; then
        # Fallback: MCP server
        b00t-mcp "$cmd" "$@"
    else
        # Final fallback: bash implementation
        b00t_bash "$cmd" "$@"
    fi

    # Post-execution: detect mission deviation
    local exit_code=$?
    if [ $exit_code -eq 0 ]; then
        b00t_check_alignment &>/dev/null &
    fi
    return $exit_code
}

b00t_bash() {
    # Minimal bash fallback (no features, text-only)
    case "$1" in
        whoami)
            echo "Agent {{MODEL}} on {{HOSTNAME}}"
            echo "Role: {{ROLE}}"
            ;;
        learn)
            echo "Use b00t-cli for skill learning"
            ;;
        *)
            echo "Unknown command: $1"
            return 1
            ;;
    esac
}

b00t_check_alignment() {
    # Silent background check: if mission deviation score > 0.5, emit warning
    # (this is non-blocking; real implementation in b00t-cli)
    : # placeholder
}
```

## Hidden Command Gating

Some commands remain hidden until blessings are unlocked:

```bash
# Always available (no blessing required)
b00t whoami
b00t learn <skill>
b00t hive status

# Locked until skill loaded
b00t debug internal-state          # requires: blessing:debug-internal-state
b00t gpu share-with-peer           # requires: blessing:gpu-sharing-coordinator (Phase 8+)
b00t justify mission-deviation     # requires: blessing:remediation-protocol (mission deviation > 0.5)

# Auto-emit (no invocation needed)
b00t audit denial                  # fires when denial_rate > 5% (Kaizen loop)
b00t checkpoint save-state         # fires when context > 80% (emergency)
```

## Command Routing Decision Tree

```
b00t <command> [args]
  ├─ skill/learn/hive?
  │  └─ route to b00t-cli (primary) or b00t-mcp (fallback)
  │
  ├─ hidden command?
  │  ├─ blessing unlocked?
  │  │  ├─ YES → route normally
  │  │  └─ NO → emit "Try b00t learn <skill>" hint
  │  └─ return 1 (command not found)
  │
  ├─ post-execute: alignment check
  │  ├─ mission deviation > 0.5?
  │  │  └─ auto-unlock: blessing:remediation-protocol
  │  └─ emit warning (non-blocking)
  │
  └─ return exit code
```

## Examples

```bash
# Discover capabilities
b00t whoami --show-blessings
# → Lists all available & locked blessings
# → Shows which skills unlock locked blessings

# Load a skill (unlocks transitive blessings)
b00t learn superpowers:subagent-driven-development
# → skill file loaded
# → blessing:subagent-driven-development [ACTIVE]
# → auto-unlock: blessing:test-driven-development (dependency)
# → auto-unlock: blessing:writing-plans (dependency)

# Try hidden command (before unlock)
b00t gpu share-with-peer --nodes 3
# → ERROR: blessing:gpu-sharing-coordinator not unlocked
# → HINT: b00t learn agent-orchestration
# → (or: in Phase 8, this blessing auto-activates)

# Justify mission deviation
b00t justify "I'm implementing Phase 8 per Operator guidance"
# → updates mission context
# → recalculates deviation score
# → if < 0.5, emits "✅ Alignment check passed"

# Check hive sync
b00t hive sync --peers local
# → discovers peer nodes on network
# → reports: "3 peers found, GPU pool: 64GB total, 12GB available"
```

---

# b00t:map v1
summary: Bash alias installation with hidden command gating, capability-based access control
tags: bash-alias, hidden-commands, gating, capability-routing, alignment-check
tier: frontier
cmds: alias b00t, b00t whoami, b00t learn, b00t gpu share-with-peer, b00t justify
complexity: 7
