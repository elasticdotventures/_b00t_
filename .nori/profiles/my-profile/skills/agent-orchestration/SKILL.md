---
name: agent-orchestration
description: Multi-model agent delegation and task orchestration across Claude, Codex, and Gemini. Activates when coordinating complex tasks requiring multiple AI models or specialized agent capabilities.
tags: [delegation, multi-model, orchestration, poly-agent, compounding-engineering]
---

# Agent Orchestration Skill

**Workflow Capability**: Coordinate poly-agent task delegation across heterogeneous AI models (Claude, Codex, Gemini) following compounding engineering principles where each unit of work makes subsequent work easier.

## When This Skill Activates

- User requests multi-model coordination
- Task requires specialized capabilities beyond single model
- Complex engineering requiring parallel agent workstreams
- Compounding engineering workflows (plan→work→review cycles)

## Core Patterns

### Pattern 1: Trifecta Delegation

```
Complex Task
├─→ Claude (reasoning, system design, review)
├─→ Codex (code generation, refactoring)
└─→ Gemini (multimodal analysis, research)
   → Synthesize → Unified implementation
```

### Pattern 2: Specialist Routing

```
Incoming Task
 → Analyze capabilities required
 → Match to optimal model/agent
 → Delegate with context
 → Monitor & handoff
 → Aggregate results
```

### Pattern 3: Compounding Cycle

```
Plan (research agents)
 → Design (architecture agents)
 → Execute (code agents)
 → Review (quality agents)
 → Codify (knowledge capture)
```

## Quick Reference: Model Selection

**Claude (this model)**
- System design & architecture
- Code review & refactoring
- Complex reasoning & planning
- Documentation & communication

**Codex** (via `openai-codex.🤖`)
- Raw code generation (high volume)
- API integration patterns
- Boilerplate generation
- Legacy code translation

**Gemini** (via `geminicli`)
- Multimodal analysis (images, video, audio)
- Web research with citations
- Document processing (PDF, DOCX)
- Real-time data synthesis

## Delegation Workflow

**Step 1: Task Analysis**
```bash
# Use sequential-thinking MCP for structured planning
→ Decompose task into subtasks
→ Identify capability requirements
→ Map to optimal agents/models
```

**Step 2: Context Preparation**
```bash
# Gather relevant context for delegation
→ Files/directories involved
→ Existing patterns (via grep/glob)
→ B00t datums & tribal knowledge
```

**Step 3: Parallel Execution**
```bash
# Launch agents concurrently when independent
Task tool → Multiple agents in single message
TodoWrite → Track progress across agents
```

**Step 4: Sequential Handoffs**
```bash
# Manage dependencies
→ Agent A completes → Pass results to Agent B
→ Validate outputs at each stage
→ Handle errors gracefully
```

**Step 5: Synthesis & Codification**
```bash
# Aggregate & preserve learnings
→ Combine results from all agents
→ b00t lfmf datum abstract → Tribal knowledge
→ Update justfile → Repeatable workflows
→ Generate tests → Validation
```

## Integration Points

**B00t Gospel Compliance**:
- `b00t learn <skill>` → Load capabilities on-demand
- `b00t lfmf datum abstract` → Codify non-obvious lessons
- `just <recipe>` → Execute repeatable workflows
- TaskMaster AI MCP → Epic/task/subtask tracking

**Every Marketplace Integration**:
- `/compounding-engineering:plan` → Research & planning
- `/compounding-engineering:work` → Execution with validation
- `/compounding-engineering:review` → Multi-agent review
- Parallel review agents → Security, performance, quality

## References

For detailed patterns and examples, see `references/`:

- **`claude-patterns.md`** - Claude delegation patterns
- **`codex-patterns.md`** - Codex integration workflows
- **`gemini-patterns.md`** - Gemini multimodal patterns
- **`task-decomposition.md`** - Breaking down complex tasks
- **`error-handling.md`** - Graceful failure management

## Token Efficiency

Following progressive disclosure:
- **This file**: ~180 lines (~1440 tokens)
- **Per reference**: ~250 lines (~2000 tokens)
- **Total if all loaded**: ~12K tokens
- **Typical usage**: 1500-3500 tokens (this + 1-2 refs)

**85% token reduction** vs monolithic approach.

## Usage Examples

**Example 1: Multi-modal feature implementation**
```
User: "Build PDF report generator with charts"

Agent Orchestration:
1. Gemini → Analyze sample PDFs for layout patterns
2. Claude → Design architecture & data models
3. Codex → Generate chart rendering code
4. Claude → Integration + tests + review
→ Codify pattern in b00t datum
```

**Example 2: Legacy system modernization**
```
User: "Refactor Python 2.7 codebase to Python 3.12"

Agent Orchestration:
1. Claude → Analyze architecture & dependencies
2. Codex → Automated syntax translation
3. Claude → Review migrations for logic errors
4. Parallel reviews → Security, performance, quality
→ Update just recipes for future migrations
```

**Example 3: Research-backed implementation**
```
User: "Implement OAuth2 following current best practices"

Agent Orchestration:
1. Gemini → Research 2025 OAuth2 patterns + CVEs
2. Claude → Design implementation strategy
3. Codex → Generate OAuth2 flow scaffolding
4. Claude → Security review + test generation
→ Create b00t datum for OAuth patterns
```

## Quality Gates

Before marking task complete:
- ✅ All agents completed successfully
- ✅ Results validated (tests pass)
- ✅ Learnings codified (datum/justfile)
- ✅ Documentation updated
- ✅ Git checkpoint created

## Compounding Value Generated

Each orchestrated task MUST produce:
1. **Reusable artifacts** (patterns, components, tests)
2. **Tribal knowledge** (b00t datums with 🤓 wisdom)
3. **Workflow automation** (just recipes)
4. **Validation assets** (tests that prevent regression)

**Principle**: Subsequent similar tasks should be 50%+ faster due to compounding.

---

*This skill embodies b00t gospel: DRY, DRTW, compounding engineering, and YEI hive intelligence.* 🍰
