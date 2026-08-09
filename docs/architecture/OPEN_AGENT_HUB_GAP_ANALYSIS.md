# ADR: open-agent-hub gap analysis — 11 capabilities evaluated for b00t adoption

**Status**: Accepted (analysis only — no code changes)
**Related**: _b00t_ issue #790

## Context

Issue #790 asked for a gap analysis between
[`guanyang/open-agent-hub`](https://github.com/guanyang/open-agent-hub) and b00t,
listing 11 candidate capabilities to evaluate for adoption. This document verifies
open-agent-hub's actual contents against the GitHub API and skill index (not
assumed from the issue body), cross-references each of the 11 items against
what exists in this repo today, and gives an honest verdict per item plus a
prioritized recommendation.

### Verifying open-agent-hub is real

Confirmed via `gh api repos/guanyang/open-agent-hub`: 947 stars, 163 forks,
TypeScript, pushed 2026-08-08 (active). Description: "A lightweight,
zero-dependency CLI tool to manage and activate capabilities for AI coding
assistants (Claude Code, Cursor, Trae, etc.)." `skills_index.json` lists 83
skills; fetched and enumerated directly rather than trusting the issue body's
paraphrase.

### The load-bearing finding this analysis surfaces

A material fraction of open-agent-hub's `skills/` directory — `brainstorming`,
`dispatching-parallel-agents`, `executing-plans`,
`finishing-a-development-branch`, `receiving-code-review`,
`requesting-code-review`, `subagent-driven-development`,
`systematic-debugging`, `test-driven-development`, `using-git-worktrees`,
`using-superpowers`, `verification-before-completion`, `writing-plans`,
`writing-skills`, `skill-creator` — are **the `obra/superpowers` skill pack**,
already installed as a Claude Code plugin in this operator's environment
(`~/.claude/plugins/data/superpowers-claude-plugins-official`) and visible in
every session's skill listing (`superpowers:*`). This is not a b00t gap and
not an open-agent-hub original — both projects are redistributing the same
third-party pack. Any recommendation to "adopt" these skills into b00t would
be adopting something the harness already provides at the session level,
independent of b00t's own datum/CLI layer.

## Verdict table

| # | Capability | Verdict | Evidence |
|---|---|---|---|
| 1 | **Context Engineering** (context-compression, context-degradation, context-optimization, memory-systems, long-horizon-prompting, latent-briefing) | **Genuine gap, partial** | `_b00t_/learn/agent-orchestration.md` has one relevant fragment ("agent prompt compression: reserve ~10% of context... token optimization budgets") but there is no structured context-lifecycle datum set (compression triggers, degradation diagnosis, memory-system taxonomy). `context-degradation`, `context-compression`, `memory-systems` etc. do not exist as `_b00t_/learn/*.md` or `.skill.toml` files. Grepping the datum store for `context.compress\|context.degrad\|context.budget` found no matches. |
| 2 | **Content Creation Pipeline** (image gen, comic, infographic, slide-deck, translate, docx/pdf/pptx/xlsx gen, remotion video) | **Not worth adopting** | Zero hits for `image.gen\|slide.deck\|infographic` anywhere in the repo. b00t is an agent-tooling/hive CLI, not a content-authoring product; this is genuinely out of scope, not merely low-value. |
| 3 | **Design/UI Skills** (canvas-design, frontend-design, ui-ux-pro-max, theme-factory, brand-guidelines) | **Not worth adopting** | No design-system datums found. b00t has no UI surface of its own to design for (CLI + MCP + hive daemon); adopting this would be speculative scope, not a filled gap. |
| 4 | **Social Media Posting** (post-to-wechat/weibo/x) | **Not worth adopting** | No related capability or use case anywhere in the codebase. Confirmed absent, and there is no plausible b00t workflow this serves. |
| 5 | **Obsidian Skills** (obsidian-bases, obsidian-cli, obsidian-markdown) | **Partial — b00t already has the substantive half** | b00t has a working MCP transport: `_b00t_/obsidian-mcp.mcp.toml`, `_b00t_/obsidian-mcp.docker.toml`, `scripts/obsidian-mcp.sh`/`.js`, `containers/obsidian-mcp-bridge`, plus TLS certs. What's missing is skill-level *usage pattern* documentation (a `_b00t_/learn/obsidian.md`), not the integration itself. Low-effort close if ever needed — not urgent. |
| 6 | **Merge Request Pipeline** (`commit.md`/`review.md`/`test-tdd.md` slash commands) | **Genuine gap, low priority** | `justfile` has `commit-hook`/`commit-hook2` recipes (pre-commit machinery, not an agent-facing commit workflow) and `b00t-c0re-lib/src/gate_result.rs` (a reviewer/gate primitive), but no `.claude/commands/*.md` slash-command surface exists in this repo (confirmed: no `.claude/commands` directory). This repo's operator already carries the `superpowers` plugin's `requesting-code-review`/`receiving-code-review` skills at the session level (see finding above), which covers much of the same ground informally. |
| 7 | **Skill Creation Framework** (skill-creator, evaluation, verification-before-completion) | **Already covered, different layer** | b00t has `_b00t_/learn/agent-skill-wizard.md`, a real, populated cross-platform (Claude Skills + Codex Skills) skill-authoring spec (canonical `SKILL.md` shape, progressive-disclosure loading rules). `evaluation`/`verification-before-completion` as *generic* skills are also already present via the superpowers plugin at the session level. No gap. |
| 8 | **Agent Handoff Patterns** (multi-agent-patterns, subagent-driven-development, dispatching-parallel-agents, hosted-agents, self-improvement-loops) | **Mostly already covered, two real threads** | b00t has substantial native infrastructure: `b00t-c0re-lib/src/agent_coordination.rs` (Redis pub/sub agent discovery, team-captain delegation, voting, blocking receive) plus `_b00t_/learn/agent-orchestration.md` (parallel dispatch guidance: "Independent concerns... should always be parallel," `delegation.max_concurrent_children`). `dispatching-parallel-agents`/`subagent-driven-development` as generic skills are also present via superpowers at the session level. `hosted-agents` and `self-improvement-loops` as structured b00t-native patterns are not documented — the only defensible sliver of gap here. |
| 9 | **Context Degradation** (recognizing wasted context-window budget) | **Genuine gap** | Same root cause as #1 — no `_b00t_/learn/context-degradation.md` or equivalent. This is real, useful, and currently undocumented in b00t's own datum store (the session-level superpowers pack does not cover this specific skill name). |
| 10 | **mcp-builder** (skill for building MCP servers) | **Genuine gap, moderate value** | b00t has `mcp` CLI commands and `b00t-c0re-lib/src/mcp_registry.rs` for *registering/managing* MCP servers, and `_b00t_/learn/learn-mcp.md` exists but is about *consuming* `b00t learn` via MCP, not a guide for *authoring* a new MCP server. b00t builds and vendors several MCP servers (`b00t-mcp`, `codebase-memory-mcp`, others) without a single documented authoring pattern — worth closing. |
| 11 | **Receiving/Requesting Code Review** | **Already covered, different layer** | `_b00t_/learn/b00t-review-skills.md` exists and points reviewers at `b00t whoami --role=operator`, `b00t learn rust`, `b00t learn agent-orchestration`, `b00t learn systematic-debugging` — a b00t-flavored review-context loader. `receiving-code-review`/`requesting-code-review` as generic skills are additionally present via the superpowers plugin at the session level (confirmed in this very session's skill listing). No gap. |

## Recommendation (prioritized)

1. **#1 + #9 (context engineering / context degradation)** — highest-value real
   gap. Neither is covered by the session-level superpowers pack under those
   exact names in a b00t-flavored way, and b00t's own `agent-orchestration.md`
   only has a stray fragment. Close as one or two new `_b00t_/learn/*.md`
   datums (`context-engineering.md`, `context-degradation.md`) with concrete
   b00t-specific triggers (when to `lfmf`, when a hive session should
   checkpoint/compact). Cheap, LFMF-shaped, no new dependency.
2. **#10 (mcp-builder)** — moderate value. b00t maintains multiple MCP
   servers and has no single authoring reference; a `_b00t_/learn/mcp-builder.md`
   would consolidate scattered knowledge. Second priority.
3. **#8's uncovered sliver (hosted-agents, self-improvement-loops)** — worth a
   short datum addendum to `agent-orchestration.md`, not a new subsystem;
   b00t's `agent_coordination.rs` already does the structural work.
4. **#6 (MR pipeline slash commands)** — low priority. The operator already
   has `superpowers`'s review skills at the session level; a b00t-native
   `.claude/commands/{commit,review,test-tdd}.md` surface is a nice-to-have,
   not urgent.
5. **#5 (Obsidian usage-pattern doc)** — low priority, low effort if picked up
   incidentally; the hard part (MCP transport) is already done.
6. **#2, #3, #4 (content creation, design/UI, social posting)** — do not
   adopt. Out of scope for an agent-tooling/hive CLI; no plausible b00t
   workflow consumes them.
7. **#7, #11 (skill-creator, code-review skills)** — no action. Already
   covered, either natively (`agent-skill-wizard.md`, `b00t-review-skills.md`)
   or via the session-level superpowers plugin.

## What this ADR does not resolve

This is an evaluation only — no `_b00t_/learn/*.md` datums were created as
part of this change. Items 1, 9, and 10 above are recommended follow-up
issues, not implemented here.
