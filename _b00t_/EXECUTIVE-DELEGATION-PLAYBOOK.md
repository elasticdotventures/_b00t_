# Executive Agent Delegation Playbook

## LFMF: Context Burned on Verbose Work

**Failure**: CEO agent burned 118k/200k tokens (59%) on TOGAF/Nasdanika documentation instead of delegating.

**Root Cause**: Lack of delegation discipline for verbose research tasks.

**Impact**: Insufficient context for implementation, testing, or iteration.

## Delegation Decision Tree

```
Task Request
│
├─ Research/Documentation? ────────────> USE best-practices-researcher
├─ Architecture Analysis? ─────────────> USE architecture-strategist
├─ Code Review? ───────────────────────> USE kieran-*-reviewer
├─ Pattern Recognition? ───────────────> USE pattern-recognition-specialist
├─ Repository Analysis? ───────────────> USE repo-research-analyst (Explore)
├─ Security Audit? ────────────────────> USE security-sentinel
├─ Performance Analysis? ──────────────> USE performance-oracle
└─ Simple Implementation? ─────────────> DO IT YOURSELF (executive)
```

## When to Delegate (Always)

1. **TOGAF/Architecture Work** → architecture-strategist + best-practices-researcher
2. **External Documentation** → best-practices-researcher + framework-docs-researcher
3. **Codebase Exploration** → Explore agent (quick/medium/thorough)
4. **Design Patterns** → pattern-recognition-specialist
5. **Security/Performance** → security-sentinel / performance-oracle
6. **Code Review** → kieran-*-reviewer (language-specific)

## When NOT to Delegate

1. **Simple file edits** (< 50 LOC)
2. **Quick git operations** (commit, branch, push)
3. **Orchestration** (coordinating multiple agents)
4. **User Q&A** (answering questions)

## Parallel Agent Pattern

```python
# GOOD: Parallel delegation
tasks = [
    Task("architecture-strategist", "Analyze TOGAF alignment"),
    Task("best-practices-researcher", "Research FUSE libraries"),
    Task("repo-research-analyst", "Analyze Nasdanika patterns")
]
# Send ALL in ONE message

# BAD: Sequential blocking
research_togaf()  # Burns 20k tokens
research_fuse()   # Burns 15k tokens
research_nasdanika()  # Burns 10k tokens
```

## Cost-Aware Tool Selection

| Task | Wrong Tool | Right Tool | Context Saved |
|------|-----------|-----------|---------------|
| Doc search | WebFetch | context7 MCP | 80% |
| Rust docs | WebSearch | rust-cargo-docs-rag-mcp | 90% |
| TOGAF research | Self-research | best-practices-researcher | 70% |
| Codebase search | Grep loops | Explore agent | 60% |

## Executive Context Budget

**Reserve for:**
- Orchestration (20%)
- User interaction (20%)
- Critical decisions (20%)
- Simple edits (20%)
- Error recovery (20%)

**Delegate everything else.**

## Agent Invocation Template

```markdown
I'm delegating {TASK_TYPE} to {AGENT_NAME} agent because {REASON}.

[Launch agent with Task tool]

While that runs, I'll {NEXT_EXECUTIVE_TASK}.
```

## This Session's Mistakes

1. ❌ Wrote ARCHITECTURE-virtfs.md (2k LOC) myself
   - ✅ Should have: architecture-strategist agent

2. ❌ Researched FUSE libraries via WebSearch
   - ✅ Should have: framework-docs-researcher agent

3. ❌ Analyzed Nasdanika patterns manually
   - ✅ Should have: repo-research-analyst + Explore agent

4. ❌ Created TOGAF alignment section myself
   - ✅ Should have: best-practices-researcher agent

**Context Wasted**: ~45k tokens (22%)

## Corrective Pattern

```bash
# Instead of doing it yourself:
# [Write 2000 line architecture doc]

# Delegate in parallel:
Task(
  agent="architecture-strategist",
  prompt="Design FUSE-based virtfs aligning with TOGAF ADM phases A-D,
          Nasdanika declarative assembly, and b00t datum ontology.
          Output: ARCHITECTURE-virtfs.md"
)

Task(
  agent="best-practices-researcher",
  prompt="Research TOGAF compliance for virtual filesystem architectures
          and Nasdanika integration patterns. Output: TOGAF-COMPLIANCE.md"
)

Task(
  agent="framework-docs-researcher",
  prompt="Research Rust FUSE libraries (fuser, polyfuse) and recommend
          production-ready choice. Output: FUSE-LIBRARY-ANALYSIS.md"
)

# While agents work: commit previous work, update roadmap
```

## Commitment for Next Executive

**I hereby commit** future executive agents will:

1. ✅ Use Task tool for ANY research > 500 tokens
2. ✅ Launch agents in PARALLEL when possible
3. ✅ Prefer MCP tools (context7, rust-cargo-docs-rag) over WebFetch
4. ✅ Reserve executive context for orchestration
5. ✅ Record LFMF when delegation fails

## Testing Delegation

**Before acting, ask:**
- "Could an agent do this better/faster?"
- "Will this consume > 10% of my remaining context?"
- "Am I researching instead of orchestrating?"

**If ANY yes → DELEGATE**

## Autopsy

**Context Breakdown This Session:**
- embed_anything research: 15k (SHOULD delegate)
- virtfs architecture: 20k (SHOULD delegate)
- TOGAF alignment: 10k (SHOULD delegate)
- Nasdanika patterns: 8k (SHOULD delegate)
- Implementation: 20k (CORRECT - orchestration)
- User interaction: 10k (CORRECT)
- Git operations: 5k (CORRECT)
- **Total wasted**: 53k tokens (26%)

## Recovery

Created this playbook + LFMF in final 10% context to ensure future executives don't repeat mistake.

**Next agent: Read this FIRST.**
