# LFMF Workflow: Lessons From My Failures

Systematic knowledge capture from mistakes, discoveries, and tribal wisdom.

## Core Principle

**LFMF = Non-obvious lessons that prevent future failures**

Not documentation. Not obvious patterns. **Tribal knowledge** that compounds.

## When to LFMF

### ✅ ALWAYS Capture

- **Operator correction**: Human fixed your mistake
- **Non-obvious solution**: Took >30min to discover
- **Tribal wisdom revealed**: "Oh that's why!" moment
- **Pattern emergence**: Same issue 3+ times
- **Hidden gotcha**: Framework/tool quirk
- **Cross-cutting concern**: Affects multiple areas

### ❌ NEVER Capture

- Standard library usage
- Framework basics (in official docs)
- Idiomatic patterns (language-specific norms)
- Obvious syntax
- Self-evident logic

## LFMF Template

```markdown
# LFMF: [Concise Title]

## Context
What was being attempted? What was the goal?

## Problem
What went wrong? What wasn't obvious? Why did this cause friction?

## Discovery Process
How was the solution found? (research, trial-error, operator guidance)

## Solution
Concrete fix. Code examples. Configuration changes.

## Rationale
WHY is this non-obvious? Why isn't it documented clearly elsewhere?

## Reusable Pattern
Abstract the specific into general. How does this apply broadly?

## Codification
Where is this captured for reuse?
- Justfile recipe: `just [command]`
- Datum location: `_b00t_/[file]`
- Code comment: `src/[file]:[line]`

## 🤓 Melvin Wisdom
One-liner tribal insight for future agents.

## Tags
#lfmf #[technology] #[category] #[severity]

## Date & Agent
- Captured: 2025-11-07
- Agent: Claude Sonnet 4.5
- Session: [PID/identifier]

## Related LFMFs
- [Link to related lessons]
```

## Capture Workflow

### Step 1: Recognize LFMF Moment

```bash
# Operator corrects you
operator: "No, Docker services need depends_on AND healthcheck"

# Recognition: This is non-obvious tribal knowledge
→ Trigger LFMF capture
```

### Step 2: Use b00t lfmf Tool

```bash
b00t lfmf datum abstract lesson <<EOF
Lesson: Docker Compose service dependencies require healthchecks

Context: Setting up multi-service Docker stack
Problem: Services started before dependencies ready → crashes
Discovery: Operator revealed depends_on alone insufficient

Solution:
services:
  app:
    depends_on:
      db:
        condition: service_healthy
  db:
    healthcheck:
      test: ["CMD", "pg_isready"]
      interval: 5s

Rationale: Docker Compose docs don't emphasize this clearly.
          Common gotcha in production deployments.

Pattern: All service dependencies need:
         1. depends_on with condition
         2. Explicit healthcheck definition

Codified: _b00t_/docker.🐳/compose-patterns.🧠.md

🤓 depends_on alone = race conditions. Always add healthchecks.

Tags: #lfmf #docker #production #dependencies
EOF
```

### Step 3: Link to Codebase

```bash
# Add 🤓 comment where pattern is used
cat >> docker-compose.yaml <<EOF
services:
  app:
    # 🤓 depends_on alone insufficient; must include healthcheck condition
    depends_on:
      db:
        condition: service_healthy
EOF
```

### Step 4: Update Justfile

```makefile
# Codify pattern for reuse
docker-service-with-deps SERVICE DEPENDENCY:
    #!/usr/bin/env bash
    # Properly configured service dependencies (LFMF: docker-healthchecks)
    cat >> docker-compose.yaml <<EOF
  {{SERVICE}}:
    depends_on:
      {{DEPENDENCY}}:
        condition: service_healthy
EOF
```

### Step 5: Validate & Commit

```bash
# Test pattern works
docker-compose up -d

# Commit with LFMF reference
git commit -m "feat: add service with proper healthcheck deps

See LFMF: _b00t_/docker.🐳/compose-patterns.🧠.md#healthchecks"
```

## LFMF Categories

### 1. Configuration Gotchas

**Example**: "CORS must be configured BEFORE routes in Express"

```bash
Tags: #lfmf #configuration #gotcha #ordering
```

### 2. Performance Lessons

**Example**: "N+1 query discovered after load testing"

```bash
Tags: #lfmf #performance #database #optimization
```

### 3. Security Vulnerabilities

**Example**: "JWT exp claim must be validated server-side"

```bash
Tags: #lfmf #security #authentication #vulnerability
```

### 4. Integration Patterns

**Example**: "gRPC requires HTTP/2 end-to-end, not just client"

```bash
Tags: #lfmf #integration #grpc #protocol
```

### 5. Debugging Techniques

**Example**: "tokio-console requires explicit subscriber setup"

```bash
Tags: #lfmf #debugging #tooling #tokio
```

## Searching LFMF

### By Topic

```bash
b00t lfmf search "docker"
b00t lfmf search "authentication"
```

### By Tag

```bash
b00t lfmf search --tag security
b00t lfmf search --tag performance --tag database
```

### By Date Range

```bash
b00t lfmf list --since 2025-10-01
b00t lfmf list --recent 20
```

### By Pattern

```bash
# Find related lessons
b00t lfmf related "oauth-token-refresh"
```

## Compounding Through LFMF

### Iteration 1: Discovery

```
Problem encountered → 4 hours debugging → Solution found
Capture: Initial LFMF
```

### Iteration 2: Refinement

```
Same problem class → Check LFMF → 30 min solution
Update: Add edge case to LFMF
```

### Iteration 3: Automation

```
Pattern clear → Create justfile recipe
Codify: Turn LFMF into automated workflow
```

### Iteration 4: Prevention

```
Pre-commit hook → Validates pattern
Compound: Problem class eliminated entirely
```

**Time saved compounds:**
- Iteration 1: -4 hours (learning cost)
- Iteration 2: +3.5 hours (saved)
- Iteration 3: +3.8 hours (automation)
- Iteration 4: +4 hours (prevention)
- **Net: +7.3 hours after initial investment**

## Quality Standards

### Good LFMF

```markdown
✅ Specific problem + concrete solution
✅ Non-obvious rationale clearly explained
✅ Reusable pattern extracted
✅ Codified for future use
✅ Tagged appropriately
✅ 🤓 wisdom distilled to one-liner
```

### Bad LFMF

```markdown
❌ Obvious/documented behavior
❌ No reusable pattern
❌ Vague problem description
❌ Missing rationale
❌ Not codified anywhere
❌ No tribal insight
```

## Maintenance

### Review Cycle

```bash
# Quarterly LFMF review
b00t lfmf review --quarter Q1-2025

# Check for:
- Outdated technology (deprecate)
- Superseded patterns (archive)
- Frequently accessed (promote to datum)
- Consolidation opportunities (merge)
```

### Promotion Path

```
LFMF (single lesson)
  → Referenced 5+ times
  → Promote to Datum
  → Add to _b00t_/learn/

Datum (capability)
  → Used 20+ times
  → Integrate to Core
  → Part of b00t gospel
```

## Anti-Patterns

### ❌ Documentation Dump

```markdown
# BAD: Just copying docs
LFMF: How to use Array.map()

Context: Needed to transform array
Solution: array.map(x => x * 2)
```

**Problem**: This is basic language feature, not tribal knowledge.

### ❌ No Codification

```markdown
# BAD: Lesson captured but not reused
LFMF: Complex OAuth flow discovered

[Detailed explanation]

Codified: ❌ Nowhere
```

**Problem**: Can't reuse. Will be rediscovered.

### ❌ Overly Specific

```markdown
# BAD: Too narrow
LFMF: Bug in UserController line 47

Solution: Changed `<=` to `<`
```

**Problem**: No reusable pattern. Too implementation-specific.

### ❌ Missing Rationale

```markdown
# BAD: Solution without "why"
LFMF: Use Buffer.from() not new Buffer()

Solution: Buffer.from(data)
```

**Problem**: Why? Security? Deprecation? Performance? Need rationale.

## Integration with Agents

### Agent reads LFMF

```typescript
// Before implementing feature
const priorLFMF = await b00t.lfmf.search({
  topic: "oauth-implementation",
  tags: ["security"]
})

// Design informed by past failures
design = await designWithKnowledge(priorLFMF)
```

### Agent creates LFMF

```typescript
// After operator correction
await b00t.lfmf.capture({
  title: "JWT refresh token rotation",
  context: operatorFeedback.context,
  problem: myAttempt.mistake,
  solution: operatorFeedback.correction,
  rationale: "Not mentioned in JWT RFC, tribal knowledge"
})
```

---

*LFMF is how the hive learns from failure. Each mistake captured makes YEI smarter.* 🧠
