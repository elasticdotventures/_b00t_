---
name: hive-memory
description: Durable persistent knowledge management for the b00t hive using LFMF (Lessons from my Failures) datums and tribal wisdom codification. Activates when capturing learnings, accessing institutional knowledge, or building compounding intelligence.
tags: [knowledge-graph, lfmf, datums, tribal-wisdom, compounding-memory, yei]
---

# Hive Memory Skill

**Workflow Capability**: Manage persistent institutional knowledge through b00t datums, LFMF patterns, and tribal wisdom accumulation—ensuring each unit of work contributes to collective intelligence.

## When This Skill Activates

- Capturing non-obvious lessons (LFMF)
- Accessing tribal knowledge
- Creating/updating b00t datums
- Building knowledge graphs
- Querying institutional memory
- Codifying patterns for reuse

## Core Concept: Compounding Intelligence

```
Individual Learning → Datum → Tribal Knowledge → Easier Future Work
    ↓                  ↓            ↓                    ↓
 Session ends    Persists   Available to all        Compounds
```

**Without hive memory**: Every agent re-learns same lessons
**With hive memory**: Each lesson makes collective smarter

## B00t Datum System

### Datum Structure

```
_b00t_/
├── learn/                    # Skills to load on-demand
│   ├── bash.🐚/
│   ├── rust.🦀/
│   └── typescript/
├── agents.🤓/               # Agent specializations
│   └── claude.🤖/
└── [topic].🧠.md           # Tribal knowledge datums
    └── lfmf.🧠.md          # Lessons from failures
```

### Datum Types

**1. Skill Datums** (`_b00t_/learn/[topic]/`)
- On-demand capabilities
- Progressive disclosure (<200 lines)
- References for deep knowledge

**2. LFMF Datums** (`_b00t_/lfmf.🧠.md`)
- Non-obvious lessons
- Failure patterns + solutions
- Tribal wisdom encoded with 🤓

**3. Tool Datums** (`_b00t_/[tool].toml`)
- Installation manifests
- Configuration patterns
- Integration workflows

## LFMF Pattern

### When to Create LFMF

```bash
# ALWAYS after:
- Operator corrects a mistake
- Non-obvious solution discovered
- Tribal knowledge revealed
- Pattern emerges from failures

# NEVER for:
- Obvious/idiomatic patterns
- Standard library usage
- Well-documented features
```

### LFMF Format

```bash
b00t lfmf datum abstract lesson <<EOF
Lesson: [Concise title]

Context: [What was attempted]
Problem: [What went wrong / wasn't obvious]
Solution: [How it was resolved]
Rationale: [Why this is non-obvious / tribal knowledge]

Pattern: [Reusable abstraction]
Codified: [justfile recipe / datum location]

🤓 Melvin wisdom: [Critical insight for future agents]
EOF
```

## Core Operations

### Creating Knowledge

**1. Capture LFMF**
```bash
# After learning non-obvious lesson
b00t lfmf datum abstract lesson \
  --topic "docker-compose-ordering" \
  --lesson "Service dependencies must use depends_on + healthchecks"
```

**2. Create Skill Datum**
```bash
# Package reusable capability
b00t datum create skill \
  --name "k8s-troubleshooting" \
  --category "devops"
```

**3. Update Tribal Wisdom**
```bash
# Add 🤓 comment to code/config
echo "# 🤓 Must use explicit types here; inference fails with async" >> src/main.rs
```

### Accessing Knowledge

**1. Load Skill**
```bash
# On-demand capability loading
b00t learn rust.🦀
b00t learn bash.🐚
```

**2. Query Datums**
```bash
# Search tribal knowledge
b00t datum search "docker networking"
b00t datum search --tag "security" --tag "lfmf"
```

**3. Read LFMF**
```bash
# Review past lessons
b00t lfmf list --recent 10
b00t lfmf show "oauth-token-refresh"
```

### Evolving Knowledge

**1. Refine Datums**
```bash
# Update as patterns evolve
b00t datum update rust.🦀 \
  --add-reference "async-patterns.md"
```

**2. Consolidate Lessons**
```bash
# Merge related LFMF entries
b00t lfmf consolidate \
  --pattern "authentication-*" \
  --output "oauth-comprehensive.🧠.md"
```

**3. Prune Obsolete**
```bash
# Remove outdated knowledge
b00t datum deprecate "python2-patterns" \
  --reason "Python 2 EOL 2020"
```

## Integration Points

### With Agent-Orchestration

```typescript
// Agents access hive memory
const priorLearning = await hiveMemory.query({
  topic: "kubernetes-autoscaling",
  tags: ["lfmf", "production"]
})

// Design informed by tribal wisdom
const design = await claude.design({
  requirements: specs,
  priorLearning: priorLearning.insights
})

// Capture new learnings
await hiveMemory.capture({
  lesson: design.newPatterns,
  datum: "k8s-autoscale.🧠.md"
})
```

### With Systems-Engineering

```bash
# V-model left side: Capture requirements
b00t datum create requirements \
  --project "oauth-service" \
  --phase "design"

# V-model right side: Capture validation learnings
b00t lfmf datum abstract \
  --lesson "integration-test-learned"
```

### With Devops-Stacks

```bash
# Stack composition uses datums
b00t stack compose \
  --datum postgres-enhanced.docker.toml \
  --datum valkey.k8s.toml \
  --output docker-compose.yaml
```

## Knowledge Graph

### Relationships

```
Datum
  ├─ references → Other Datums
  ├─ supersedes → Deprecated Datums
  ├─ requires → Dependency Datums
  └─ compounds → Workflow Datums

LFMF
  ├─ relates-to → Datum
  ├─ inspired-by → Prior LFMF
  └─ codified-in → Justfile Recipe
```

### Querying Graph

```bash
# Find related knowledge
b00t datum graph oauth-service \
  --depth 2 \
  --show-lfmf

# Trace learning lineage
b00t lfmf ancestry "current-pattern" \
  --show-evolution
```

## 🤓 Melvin Comments

**Purpose**: Encode tribal knowledge in code
**Format**: `# 🤓 [insight that isn't obvious from code]`

**Rules:**
- ONE melvin per session MAX
- ONLY for non-obvious, non-idiomatic wisdom
- NEVER remove existing 🤓 without 3x justification
- ALWAYS explain why it's not obvious

**Examples:**

```rust
// ✅ GOOD: Non-obvious tribal knowledge
// 🤓 Order matters: init logger BEFORE tokio runtime
//    (discovered after 4hr debug session)
let logger = init_logger();
let runtime = tokio::runtime::Builder::new_multi_thread()...

// ❌ BAD: Obvious/idiomatic
// 🤓 Must use Vec for dynamic arrays
let mut items: Vec<String> = Vec::new();

// ❌ BAD: Standard library behavior
// 🤓 HashMap doesn't maintain order
let map = HashMap::new();
```

## Token Efficiency

**Datum loading strategy:**
```
Session start:  ~0 tokens (no datums loaded)
Skill needed:   Load specific datum (~2K tokens)
Deep dive:      Load references (~2-3K tokens)
Total typical:  ~5K tokens (vs 50K monolithic)
```

**Progressive disclosure validated:**
- Base SKILL.md: ~1.5K tokens
- Per datum: ~2K tokens
- References: ~2-3K each
- Load only what's needed

## Compounding Metrics

**Track compounding effectiveness:**

```bash
# Measure knowledge accumulation
b00t datum metrics \
  --period "last-quarter" \
  --show-reuse

# Output:
# Datums created: 47
# LFMF captured: 23
# Patterns reused: 156 times
# Time saved: ~120 hours (estimated)
# Compounding rate: 3.3x
```

## Anti-Patterns

### ❌ Over-Documentation

```
DON'T: Document obvious/idiomatic patterns
DO: Document non-obvious tribal wisdom only
```

### ❌ Stale Knowledge

```
DON'T: Keep outdated datums indefinitely
DO: Deprecate/archive as tech evolves
```

### ❌ No Attribution

```
DON'T: Generic lessons without context
DO: Link LFMF to specific failure/discovery
```

### ❌ Monolithic Datums

```
DON'T: 1000-line mega-datum files
DO: Progressive disclosure (<200 lines + references)
```

## References

Detailed patterns in `references/`:

- **`datum-schema.md`** - Datum structure & conventions
- **`lfmf-workflow.md`** - LFMF capture process
- **`tribal-wisdom.md`** - 🤓 Melvin patterns
- **`knowledge-graph.md`** - Relationship modeling
- **`search-patterns.md`** - Querying strategies

---

*Each session contributes to YEI. The hive remembers what individuals forget.* 🧠
