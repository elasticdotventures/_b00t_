# 🤖 `b00t whoami --next` Output Specification

## Command Purpose
Shows the next logical action, next recommended agent, and continuation context for the current session.

## Example Output

### Scenario: After Agent A completes parser fixes (85% progress)

```bash
$ b00t whoami --next
```

**Output:**
```
🎯 NEXT ACTION RECOMMENDATION
═══════════════════════════════════════════════════════════

📍 Current Position
  Agent: first-mate (Agent A)
  Progress: 85% complete
  Session: 349284-active
  Branch: fix/b00t-py-mcp-list-api
  Latest milestone: "Fixed k0mmand3r parser tests" (75% → 85%)

🎯 Next Immediate Action
  → Run remaining test suite: cargo test --workspace
  → Fix 3 remaining k0mmand3r parser test failures
  → Target: 100% completion, ship-ready state

🤖 Recommended Next Agent
  → Specialist: code-critic (for final code review)
  → Alternative: qa-agent (for comprehensive testing)
  → Or: continue with first-mate (if you prefer)

📊 Success Metrics
  Tests passing: 536/537 (99.8%)
  Time invested: 75 minutes
  Estimated remaining: 15-30 minutes
  Ship-readiness: 95% → 100%

🔄 Handoff Options
  → b00t delegate code-critic --session=349284
  → b00t milestone add --ready-for-review
  → git commit -m "fix(k0mmand3r): resolve parser test failures"
  → b00t whoami --handoff --agent=code-critic

📦 Artifacts Ready for Next Agent
  → Fixed: Rhai macro reference resolution (hive.rs:779-813)
  → Fixed: Handshake agent prefix parsing (mod.rs:337-343)
  → Fixed: Loop spec "with" keyword handling (mod.rs:247-270)
  → Fixed: Vote command "on" keyword parsing (mod.rs:330-370)
  → Test results: 536/537 passing (only crew e2e test unrelated)

🎯 Branch Strategy
  → Current: fix/b00t-py-mcp-list-api
  → Ready for merge: YES (after final tests pass)
  → Next branch: main (merge target)
  → PR creation: b00t pr create --target=main

⏱️  Timeline
  Started: 2025-06-14 09:30 UTC
  Now: 2025-06-14 10:45 UTC
  Estimated completion: 2025-06-14 11:15 UTC (30 minutes)

💡 Recommendation Summary
  Continue with current agent for final 15% → Ship to main → 
  Then delegate to code-critic for final review before production merge.
```

---

## Command Options

```bash
b00t whoami --next                    # Show next action recommendation
b00t whoami --next --agent=critic     # Show next actions from critic's perspective
b00t whoami --next --json             # Output as JSON for automation
b00t whoami --next --verbose          # Show detailed reasoning
```

---

## Data Structure Behind --next

Based on the WHOAMI-HANDOFF spec:

```rust
struct NextActionRecommendation {
    current_position: CurrentPosition,
    immediate_action: RecommendedAction,
    next_agent: AgentRecommendation,
    success_metrics: ProgressMetrics,
    handoff_options: Vec<HandoffOption>,
    artifacts_ready: Vec<Artifact>,
    branch_strategy: BranchStrategy,
    timeline: TimelineEstimate,
    reasoning: RecommendationSummary
}

struct RecommendedAction {
    action: String,
    priority: Priority,
    estimated_time_minutes: u16,
    dependencies: Vec<Dependency>,
    expected_outcome: String
}

enum AgentRecommendation {
    ContinueCurrent,
    DelegateTo { agent_id: AgentId, reason: String },
    SpecialistRequired { domain: Domain, reason: String }
}
```

---

## Use Cases

### 1. **During Active Work**
```bash
$ b00t whoami --next
# Shows: "Continue debugging test failures" 
#        "Next milestone: Fix remaining parser tests"
```

### 2. **Before Handoff**
```bash
$ b00t whoami --next
# Shows: "Ready for code-critic delegation"
#        "All tests passing, ready for review"
```

### 3. **After Completion**
```bash
$ b00t whoami --next
# Shows: "Task complete, ready for merge to main"
#        "Create PR: b00t pr create --target=main"
```

### 4. **When Blocked**
```bash
$ b00t whoami --next
# Shows: "Blocked: Missing dependency, require human intervention"
#        "Recommended: Escalate to operator for decision"
```

---

## Integration with Handoff System

The `--next` flag reads from:
1. **Current session state** (`.b00t/handoff.jsonl`)
2. **Latest milestone** (what was just completed)
3. **Remaining tasks** (what's left to do)
4. **Agent capabilities** (who can do what next)
5. **Git context** (branch state, commit history)

**Algorithm:**
```rust
fn generate_next_recommendation(handoff: &HandoffLog) -> NextActionRecommendation {
    let current_progress = handoff.current_progress();
    let remaining_tasks = handoff.remaining_tasks();
    let agent_capabilities = get_agent_capabilities();
    
    match current_progress.status {
        SessionState::Active => {
            if remaining_tasks.is_empty() {
                NextActionRecommendation::complete_and_delegate()
            } else if current_progress.percent_complete > 80 {
                NextActionRecommendation::final_push_or_review()
            } else {
                NextActionRecommendation::continue_current_task()
            }
        },
        SessionState::Blocked => {
            NextActionRecommendation::escalate_or_unblock()
        },
        SessionState::Completed => {
            NextActionRecommendation::merge_and_deploy()
        }
    }
}
```

---

## Configuration

```toml
# .b00t/whoami-config.toml
[next_recommendations]
enabled = true
show_estimated_time = true
suggest_agent_delegation = true
show_artifacts = true
include_git_strategy = true
verbose_reasoning = false
max_suggestions = 5
```

---

## JSON Output Format

```bash
$ b00t whoami --next --json
```

```json
{
  "current_position": {
    "agent": "first-mate",
    "progress": 85,
    "session": "349284-active",
    "branch": "fix/b00t-py-mcp-list-api"
  },
  "immediate_action": {
    "action": "Run remaining test suite",
    "command": "cargo test --workspace",
    "priority": "high",
    "estimated_minutes": 15
  },
  "next_agent": {
    "recommendation": "continue",
    "alternatives": ["code-critic", "qa-agent"]
  },
  "success_metrics": {
    "tests_passing": "536/537",
    "completion": "85% → 100%",
    "ship_readiness": "95%"
  },
  "handoff_options": [
    {
      "action": "delegate",
      "to": "code-critic",
      "command": "b00t delegate code-critic --session=349284"
    },
    {
      "action": "continue",
      "command": "cargo test --workspace"
    }
  ],
  "artifacts_ready": [
    "hive.rs:779-813",
    "mod.rs:337-370"
  ],
  "timeline": {
    "started": "2025-06-14T09:30:00Z",
    "now": "2025-06-14T10:45:00Z",
    "estimated_completion": "2025-06-14T11:15:00Z"
  }
}
```

---

## Why This Matters

### 1. **Continuous Flow**
- Agents don't get stuck wondering "what's next?"
- Clear progression path with actionable recommendations
- Reduces decision fatigue during handoffs

### 2. **Optimized Delegation**
- Suggests right agent for next task
- Shows capability matches
- Provides delegation commands

### 3. **Progress Visibility**
- Always shows how far along you are
- Estimates remaining work
- Tracks success metrics

### 4. **Context Preservation**
- Shows what artifacts are ready for next agent
- Includes git state for reproducibility
- Maintains session continuity

### 5. **Timeline Management**
- Shows elapsed vs estimated time
- Helps with planning and resource allocation
- Flags when tasks are taking longer than expected

---

## Implementation Notes

The `--next` flag would be implemented as:

1. **Read current state** from handoff.jsonl
2. **Analyze remaining tasks** and current progress
3. **Match agent capabilities** to remaining work
4. **Generate recommendations** using priority scoring
5. **Format output** based on flags (human/JSON)

**Key insight:** This turns the whoami command from a "what am I" tool into a "what should I do next" tool, making it more actionable and valuable for continuous agent workflows.