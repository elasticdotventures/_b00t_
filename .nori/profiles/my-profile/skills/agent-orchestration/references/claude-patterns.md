# Claude Delegation Patterns

Reference for delegating tasks to Claude (Sonnet 4.5) agents within b00t framework.

## Optimal Use Cases

**Claude excels at:**
- Complex reasoning & system design
- Code review & architectural analysis
- Refactoring with context awareness
- Natural language tasks (docs, communication)
- Multi-step planning with dependencies
- Integration of disparate components

**Claude struggles with:**
- High-volume code generation (use Codex)
- Multimodal input (images/video → use Gemini)
- Real-time web research (use Gemini)
- Repetitive boilerplate (use Codex)

## Delegation Methods

### Method 1: Task Tool (Sub-agents)

```typescript
// Launch specialized Claude agent
Task({
  subagent_type: "Explore",  // or "Plan", "general-purpose"
  description: "Analyze authentication flow",
  prompt: `Examine the authentication system in src/auth/

  Focus on:
  1. Security vulnerabilities
  2. Session management patterns
  3. Token refresh logic

  Return: Security assessment + recommendations`
})
```

**When to use**: Complex analysis requiring codebase exploration

### Method 2: Sequential Thinking MCP

```typescript
// Structured problem decomposition
sequential_thinking({
  problem: "Design microservice communication pattern",
  constraints: ["low latency", "fault tolerant", "observable"],
  output: "step-by-step implementation plan"
})
```

**When to use**: Planning phases requiring structured thinking

### Method 3: Every Marketplace Agents

```bash
# Specialized review agents
/compounding-engineering:review PR#123

# Parallel execution of 12+ review agents:
# - security-sentinel
# - performance-oracle
# - kieran-typescript-reviewer
# - data-integrity-guardian
# ...
```

**When to use**: Multi-dimensional code review

## Agent Selection Guide

### B00t Built-in Agents

**`b00t-container-architect`**
- Container configuration review
- B00t blessing system validation
- Architecture compliance checking

**Future agents** (to be created):
- `b00t-rust-specialist`
- `b00t-python-specialist`
- `b00t-typescript-specialist`

### Every Marketplace Agents

**Architecture & Design:**
- `architecture-strategist` - System design analysis
- `pattern-recognition-specialist` - Design pattern detection

**Code Quality:**
- `code-simplicity-reviewer` - Simplicity-first enforcement
- `kieran-{python|rails|typescript}-reviewer` - Language-specific

**Security & Performance:**
- `security-sentinel` - Vulnerability detection
- `performance-oracle` - Performance analysis
- `data-integrity-guardian` - Data validation

**Research & Analysis:**
- `best-practices-researcher` - Framework best practices
- `framework-docs-researcher` - Documentation synthesis
- `repo-research-analyst` - Codebase pattern analysis

## Context Management

### Progressive Disclosure Pattern

```typescript
// ❌ BAD: Load all context upfront
Task({
  prompt: `Here's the entire codebase...
  [5000 lines of code]
  Now analyze authentication.`
})

// ✅ GOOD: Progressive loading
Task({
  prompt: `Analyze authentication in src/auth/

  Use Read tool to examine:
  1. src/auth/middleware.ts
  2. src/auth/jwt.ts (only if needed)
  3. config/security.yaml (only if needed)

  Report findings after each file.`
})
```

### Context Window Management

**Sonnet 4.5 window**: 200K tokens

**Budget allocation:**
- System prompt: ~30K tokens
- B00t gospel (CLAUDE.md): ~15K tokens
- Task context: ~50K tokens
- Working memory: ~100K tokens
- Response buffer: ~5K tokens

**🤓 Melvin wisdom**: Keep agent tasks <50K context. Use multiple agents rather than single overloaded agent.

## Handoff Patterns

### Pattern A: Sequential Pipeline

```typescript
// Agent 1: Research → Agent 2: Design → Agent 3: Implement
Task({ description: "Research OAuth patterns", ... })
  .then(results => Task({
    description: "Design OAuth implementation",
    context: results
  }))
  .then(design => Task({
    description: "Implement OAuth",
    context: design
  }))
```

### Pattern B: Parallel Aggregation

```typescript
// Launch multiple agents in parallel
[
  Task({ description: "Analyze security" }),
  Task({ description: "Analyze performance" }),
  Task({ description: "Analyze maintainability" })
]
  → Wait for all completions
  → Synthesize results
  → Generate action plan
```

### Pattern C: Hierarchical Decomposition

```typescript
// Parent agent delegates to children
Parent Task: "Implement feature X"
  ├─ Child 1: "Design data model"
  ├─ Child 2: "Implement API endpoints"
  │   ├─ Grandchild 1: "POST /api/users"
  │   └─ Grandchild 2: "GET /api/users/:id"
  └─ Child 3: "Write integration tests"
```

## Error Handling

### Graceful Degradation

```typescript
try {
  result = await Task({ ... })
} catch (ContextOverflowError) {
  // Reduce scope
  result = await Task({
    ...originalTask,
    scope: "reduced",
    files: originalFiles.slice(0, 5)  // Analyze fewer files
  })
}
```

### Retry with Modified Strategy

```typescript
const strategies = ["parallel", "sequential", "hierarchical"]

for (const strategy of strategies) {
  try {
    return await delegateWithStrategy(task, strategy)
  } catch (error) {
    console.log(`Strategy ${strategy} failed: ${error}`)
    continue
  }
}

throw new Error("All delegation strategies exhausted")
```

## Validation Gates

Before completing task:
1. ✅ **Output validation** - Results meet criteria
2. ✅ **Test execution** - Generated code passes tests
3. ✅ **Context cleanup** - Temp files/state cleared
4. ✅ **Artifact generation** - Reusable outputs created
5. ✅ **Knowledge codification** - Learnings captured

## Compounding Patterns

### Learning Accumulation

```bash
# After successful delegation
b00t lfmf datum abstract lesson <<EOF
Lesson: OAuth2 delegation pattern

Context: Implemented OAuth2 flow using multi-agent approach
- Gemini researched current standards
- Claude designed architecture
- Codex generated boilerplate

Result: 60% faster than single-agent approach

Pattern: Research → Design → Generate → Review
Codified in: just oauth-implement
EOF
```

### Workflow Reification

```makefile
# justfile: Make pattern repeatable
oauth-implement FEATURE:
    # 1. Research current patterns (Gemini)
    geminicli research "OAuth2 {{FEATURE}} best practices 2025"

    # 2. Design implementation (Claude)
    claude-task design-oauth --feature={{FEATURE}}

    # 3. Generate code (Codex)
    codex-generate --template=oauth --feature={{FEATURE}}

    # 4. Review & test (Claude + parallel review)
    just review-parallel
```

## Anti-Patterns

### ❌ Context Bombing

```
DON'T: Pass entire codebase to agent
DO: Progressive disclosure, targeted file access
```

### ❌ Single Mega-Agent

```
DON'T: One agent tries to do everything
DO: Decompose into specialized sub-agents
```

### ❌ Ignoring Failures

```
DON'T: Silent failures, continue blindly
DO: Validate outputs, retry with modified strategy
```

### ❌ No Knowledge Capture

```
DON'T: Solve same problem repeatedly
DO: Codify solutions in datums + justfiles
```

---

*Claude excels at orchestration. Use it to coordinate, review, and integrate—delegate bulk work to specialists.* 🧠
