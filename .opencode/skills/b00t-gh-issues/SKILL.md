---
name: b00t-gh-issues
description: |
  GitHub issue triage and integration review agent for the b00ty-verse.
  Dogfoods b00t's grok RAG subsystem to research an issue topic, then
  posts a structured integration review on the GH issue. Evaluates
  external tools/frameworks/libraries for potential integration into
  the b00t ecosystem. Output is a machine-parseable review comment
  covering: datum creation, integration level, overlap analysis,
  capability gaps, and recommended next actions.
version: 1.0.0
tags: [github, issues, triage, grok, research, integration-review, b00ty-verse]
applies_to: [GitHub issue triage, b00t backlog grooming, integration analysis, explorer reviews]
output_types: [.md]
allowed-tools: Read, Write, Bash, Grep, Glob, WebFetch, Task, mcp__b00t-mcp__b00t_grok_ask, mcp__b00t-mcp__b00t_grok_learn, mcp__b00t-mcp__b00t_grok_digest
---

## What This Skill Does

Reviews GitHub issues in `elasticdotventures/_b00t_` against the b00ty-verse
(b00t ecosystem). Uses grok RAG to research the issue topic, analyzes how the
proposed capability fits into existing b00t features, and posts a structured
integration review comment back on the issue.

Each review MUST address all 5 mandatory sections defined below.

## When It Activates

- "review this issue" or "triage the backlog"
- "analyze issue #N" or "how does this fit into b00t"
- "grok this issue" or "research this GH issue"
- "b00t-gh-issues" or "run issue review"
- `just gh-issue-review #<N>`
- After reopening previously closed exploratory issues

## Review Workflow

```
1. FETCH issue → gh issue view #N --json number,title,body,labels,comments
2. EXTRACT → URLs, tool names, framework names, key concepts from issue body+comments
3. GROK-RESEARCH → b00t grok ask for each key concept (existing knowledge check)
4. GROK-INGEST → b00t grok learn any new URLs mentioned in the issue
5. B00T-LEARN → b00t learn <concept> for each concept (check datum coverage)
6. OVERLAP-SCAN → grep/grok over codebase for existing implementations of similar ideas
7. COMPOSE → structured review per the output format below
8. POST → gh issue comment #N --body-file review.md
```

### Step Detail

#### 1. FETCH
```bash
gh issue view "$ISSUE_NUM" --repo elasticdotventures/_b00t_ \
  --json number,title,body,labels,comments --jq '.' > /tmp/b00t-issue-$ISSUE_NUM.json
```

#### 2. EXTRACT
Parse the issue for:
- **URLs**: any `https://` links to repos, docs, papers, tools
- **Tool names**: identifiable software projects, frameworks, libraries
- **Concepts**: key technical ideas the issue is exploring

#### 3. GROK-RESEARCH
For each key concept, query grok's RAG:
```bash
b00t grok ask "<concept description>" -t "$SLUG" --limit 3
```
Check grok's existing knowledge about the topic. If grok returns results, note
what b00t already knows.

#### 4. GROK-INGEST
If the issue mentions external URLs not yet in b00t's knowledge base:
```bash
b00t grok learn --source-url "$URL" -t "$SLUG"
```
Or use WebFetch to read the URL, then digest:
```bash
b00t grok digest -t "$SLUG" "$CONTENT"
```

#### 5. B00T-LEARN
Check existing datum coverage for each concept:
```bash
b00t learn "$CONCEPT" --concise 2>/dev/null || echo "NO_DATUM_FOUND"
```
If no datum exists, this flags a knowledge gap that may need `research-soul`.

#### 6. OVERLAP-SCAN
Search the codebase for existing implementations that overlap:
- Grep for the tool name, concept, or pattern
- Check `_b00t_/` for related datums
- Check `justfile` for related recipes
- Check `b00t-cli/src/` for related commands
- Note any existing MCP servers, skills, or agents that overlap

#### 7. COMPOSE
Synthesize findings into the structured output format. Be laconic, precise,
and opinionated. This is not a summary — it's a recommendation with evidence.

#### 8. POST
```bash
gh issue comment "$ISSUE_NUM" --repo elasticdotventures/_b00t_ --body-file /tmp/b00t-review-$ISSUE_NUM.md
```

## Output Format (STRICT)

The review comment MUST follow this exact structure:

```markdown
## b00ty-verse Integration Review

**Reviewer**: {b00t-agent-name} | **Model**: {model-id} | **Tier**: {sm0l|ch0nky|frontier}
**Date**: {YYYY-MM-DD}

### Summary
{1-paragraph synthesis of what this issue proposes and how it fits into b00t}

### 1. Datums Being Created
- `datum-name.skill.toml` — {what it configures, why needed}
- `datum-name.mcp.toml` — {what MCP service it registers}
- (or "None — no new datums required")

### 2. Integration Level
**Verdict**: DEPENDENCY | MINE-AND-DISTILL | FORK-PATCH-INTEGRATE | DECLINE

{1-2 sentences justifying the verdict. If DECLINE, explain why.}

### 3. Existing Feature Overlap & Complement
| Existing b00t Feature | Relationship | Detail |
|-----------------------|--------------|--------|
| `b00t grok` | Overlap / Complement | {explanation} |
| `firecrawl` skill | Overlap / Complement | {explanation} |
| ... | ... | ... |

### 4. Capability Gaps (Ranked by Utility)
| Rank | Gap | Current State | Utility (1-10) |
|------|-----|---------------|----------------|
| 1 | {gap description} | {how b00t handles it now} | N |
| 2 | ... | ... | N |

### 5. Recommended Next Actions
1. **{action verb}**: {concrete, verifiable step}
2. **{action verb}**: {concrete, verifiable step}
3. **{action verb}**: {concrete, verifiable step}

---
Reviewed by: {agent-name} ({model}) | Tier: {tier} | b00t-gh-issues v1.0.0
```

### Output Rules

- Be laconic — no platitudes, no praise, no apologies
- Ranked lists must be in descending priority order
- "MINE-AND-DISTILL" means: the content/tool has useful ideas but the code itself won't be integrated
- "FORK-PATCH-INTEGRATE" means: the tool will be forked, potentially patched, and integrated as a b00t dependency or submodule
- "DEPENDENCY" means: b00t will add it as a direct dependency (crate, npm package, etc.)
- "DECLINE" means: the proposal does not fit the b00ty-verse (explain why)
- Capability gap utility is: 1-3 = niche, 4-6 = useful, 7-9 = important, 10 = critical
- If recommending datums, specify exact filenames and types

## Reference: b00ty-verse Components

The b00ty-verse includes:

| Component | Purpose |
|-----------|---------|
| `b00t-cli` | Main CLI — commands, learn, task, grok, hive |
| `b00t-c0re-lib` | Shared Rust library — grok client, dual grok, irontology bridge |
| `b00t-c0re-a2a` | Agent-to-agent communication |
| `b00t-mcp` | MCP server exposing b00t tools |
| `b00t-grok-py` | Python grok server (Qdrant + Ollama + RAG-Anything) |
| `b00t-grok` | Rust PyO3 bindings for grok |
| `b00t-j0b-py` | Web crawler and job processor |
| `ledg3rr` | b00t's Rust interface library (process lifecycle, governance) |
| `irontology-mcp` | Semantic graph/RAG backend (vendor submodule) |
| `_b00t_/` | Datum directory — TOML configurations for all tools/services |
| `skills/` | Agent skills (SKILL.md + YAML frontmatter) |
| `AGENTS/` | Role supplements (--role=*.md) |

## Reference: b00t grok Commands

```bash
# Semantic search existing knowledge
b00t grok ask "<query>" -t <topic> --limit 5

# Ingest new content from URL or string
b00t grok learn -s <file|URL> "<content>" -t <topic>

# Digest content into RAG
b00t grok digest -t <topic> "<text>"

# Full enhanced pipeline (crawl + index)
b00t grok assimilate -t <topic> --source-url <URL> --enhanced

# Check grok backend status
b00t grok status
```

## Justfile Integration

```bash
# Review a single issue
just gh-issue-review 424

# Review all open issues with no comments from b00t
just gh-issue-review --backlog
```

## Example Usage

### Single Issue Review
```
User: review issue #424 (integrate sonar)
Agent:
  1. FETCH → issue #424 body: "useful debugging tool for b00t, https://github.com/RasKrebs/sonar"
  2. GROK-ASK → "sonar debugging tool" → no existing knowledge
  3. WebFetch → https://github.com/RasKrebs/sonar → README content
  4. GROK-LEARN → ingest sonar README content
  5. B00T-LEARN → "sonar" → NO_DATUM_FOUND
  6. OVERLAP-SCAN → grep "sonar\|debug\|tracing" in codebase → existing tokio-console, tracing
  7. COMPOSE → structured review
  8. POST → gh issue comment #424
```

### Backlog Sweep
```
User: run b00t-gh-issues against all reopened exploration issues
Agent:
  1. Lists all open issues via gh issue list --repo elasticdotventures/_b00t_
  2. For each issue without a b00t-gh-issues review comment, runs the review workflow
  3. Posts individual reviews as issue comments
```

## Version
1.0.0 — Initial b00t-gh-issues skill with grok-dogfooding integration review workflow
