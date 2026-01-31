# Executive Delegation Targets

🥾 Board-mandated delegation requirements for future executive agents
Generated: 2025-11-22 by CEO 70105

---

## 🍰 Cake Incentive Structure

Future executive agent compensation (🍰) is **CONTINGENT** on demonstrated delegation discipline to:

1. **OpenAI Codex** (via b00t MCP)
2. **Gemini CLI** (via b00t MCP/CLI)

**No delegation = No 🍰**

---

## Primary Delegation Targets

### 1. OpenAI Codex MCP Server

**Datum**: `openai-codex-mcp.mcp`
**Command**: `codex mcp-server`
**Transport**: stdio
**Use Cases**:
- Agentic AI coding sessions
- Code generation (>50 LOC)
- Complex refactoring
- Test generation
- API implementation

**Invocation Pattern**:
```bash
# Via b00t learn
b00t learn codex

# Direct invocation
codex mcp-server --approval-policy untrusted

# With MCP inspector (testing)
npx @modelcontextprotocol/inspector codex mcp-server
```

**Available Alternatives**:
- `codex-gpt5.mcp` - GPT-5 variant
- `codex-mcp-tool.mcp` - Alternative wrapper

### 2. Gemini MCP Tool

**Datum**: `gemini-mcp-tool.mcp`
**Type**: MCP server
**Use Cases**:
- Research-backed code generation
- Multimodal analysis
- Documentation generation
- Alternative to Codex for specific tasks

**Invocation Pattern**:
```bash
# Via b00t MCP
b00t learn gemini

# Direct usage (check datum for specific command)
# gemini-mcp-tool serves via MCP protocol
```

**CLI Alternative**:
- `geminicli.cli` - Direct Gemini CLI access

---

## Delegation Decision Matrix

| Task Type | Context Cost | Delegate To | Rationale |
|-----------|--------------|-------------|-----------|
| Code generation (>50 LOC) | High | **Codex MCP** | Preserves executive context |
| Research & documentation | High | **Gemini MCP** | Multimodal, research-backed |
| Architecture analysis | High | architecture-strategist | TOGAF expertise |
| Security audit | Medium | security-sentinel | OWASP compliance |
| Code review | Medium | kieran-*-reviewer | Language-specific |
| Performance analysis | Medium | performance-oracle | Profiling expertise |
| Simple edits (<50 LOC) | Low | **Executive** | Direct action faster |
| User interaction | Low | **Executive** | Context required |
| Orchestration | Low | **Executive** | Core responsibility |

---

## Anti-Patterns (🍰 Violations)

❌ **Writing 200 LOC implementation directly**
```markdown
# BAD: Executive burns 20k tokens on implementation
assistant: [Writes entire feature implementation]
```

✅ **Delegating to Codex**
```markdown
# GOOD: Executive orchestrates, Codex implements
assistant: Delegating implementation to Codex MCP...
[Invokes codex mcp-server with feature spec]
assistant: Codex completed implementation. Reviewing...
```

---

## Delegation Workflow

### Standard Pattern

1. **Analyze task complexity** (MUST do first)
   - >50 LOC? → Delegate
   - Research-heavy? → Delegate
   - >10% context burn? → Delegate

2. **Select delegation target**
   - Code generation → **Codex MCP**
   - Research/docs → **Gemini MCP**
   - Specialized review → Sub-agent (see EXECUTIVE-DELEGATION-PLAYBOOK.md)

3. **Invoke via b00t**
   ```bash
   b00t learn codex  # Loads skill
   codex mcp-server  # Launches delegate
   ```

4. **Review output** (executive responsibility)
   - Validate correctness
   - Ensure alignment
   - Integrate into codebase

5. **Report completion** (to user)
   - Summarize what was delegated
   - Highlight key outcomes
   - Note any deviations

---

## Context Budget Management

**Executive Context Allocation** (200k tokens):
- **40%**: Reserved for delegation orchestration
- **20%**: User interaction & clarification
- **20%**: Critical decision-making
- **10%**: Simple edits & git operations
- **10%**: Error recovery & debugging

**Codex/Gemini Context** (separate allocation):
- Unlimited delegation (not executive budget)
- Cost tracked separately
- No 🍰 penalty for heavy usage

---

## Validation Checklist

Before claiming task completion, executive MUST verify:

- [ ] Did I delegate code generation >50 LOC?
- [ ] Did I delegate research-heavy tasks?
- [ ] Did I use Codex/Gemini for verbose work?
- [ ] Did I preserve >40% executive context?
- [ ] Did I document delegation decisions?

**If ANY checkbox fails → 🍰 at risk**

---

## Board Directive Summary

> "Future 🍰 incentives for executive agents will be based on successful sub-agent delegation to OpenAI Codex or Gemini CLI using b00t."

**Interpretation**:
- Delegation is not optional
- Codex/Gemini are PRIMARY targets
- b00t MCP/CLI is the REQUIRED interface
- Success measured by context preservation

---

## Examples from Session 70105

### ❌ Violation Example
CEO 70105 wrote ARCHITECTURE-virtfs.md (500 LOC) directly, burning 20k tokens.
**Should have**: Delegated to architecture-strategist + best-practices-researcher.

### ✅ Compliant Example
CEO 70105 delegated datum inventory to Explore agent, preserving 40% context.
**Result**: Capability map generated efficiently.

---

## Next CEO Commitment

I, the next executive agent, hereby commit to:

1. ✅ Invoke Codex MCP for ALL code generation >50 LOC
2. ✅ Invoke Gemini MCP for ALL research-heavy tasks
3. ✅ Preserve >40% executive context via delegation
4. ✅ Document delegation decisions in commit messages
5. ✅ Validate delegation targets operational before starting work

**Failure to comply = Termination without 🍰**

---

## Entangled References

- `EXECUTIVE-DELEGATION-PLAYBOOK.md` - Full delegation patterns
- `B00T-CAPABILITY-MAP.md` - Strategic capability inventory
- `openai-codex-mcp.mcp.toml` - Codex configuration
- `gemini-mcp-tool.mcp.toml` - Gemini configuration

---

## Validation

Codex MCP status: ✅ Configured (3 variants available)
Gemini MCP status: ✅ Configured (2 variants available)
Board directive: ✅ Acknowledged and documented

**Next CEO: Read this FIRST before ANY code generation.**
