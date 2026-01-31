# Agent Skill Wizard (Claude + Codex)

## Purpose
Define a SINGLE skill format that runs on BOTH Claude Skills and Codex Skills, with predictable selection, loading, and execution behavior.

## Canonical Skill Shape (Cross-Platform)
A skill is a folder with a required `SKILL.md` and optional support folders.

```
my-skill/
  SKILL.md        # REQUIRED: instructions + metadata
  scripts/        # OPTIONAL: executable code
  references/     # OPTIONAL: docs
  assets/         # OPTIONAL: templates/resources
```

## How Skills Load (Progressive Disclosure)
- Claude scans skill metadata first, then loads full instructions only if relevant. Metadata is small (~100 tokens), full instructions can be larger (<5k tokens).
- Codex expects a `SKILL.md` with `name` and `description` so it can select the skill.
- Codex injects only name, description, and file path into runtime context; the body loads only when a skill is invoked.
- Practical rule: keep metadata SHORT, put procedural detail in the body.

## REQUIRED Frontmatter (Works in Both)
Use YAML frontmatter at the top of `SKILL.md`:

```
---
name: skill-name
description: Trigger-oriented description to help selection
metadata:
  short-description: Optional user-facing label
---
```
Claude scans this frontmatter for a short explanation; keep it concise.

## Core Compatibility Rules (MUST/SHOULD)
1) MUST include `SKILL.md` with `name` and `description`.
2) MUST describe TRIGGERS in `description` (what user asks that should activate the skill).
3) SHOULD keep metadata <= ~100 tokens; details belong below.
4) MUST structure instructions as explicit steps with inputs/outputs.
5) SHOULD include scripts when deterministic execution is safer than freeform generation.
6) MUST document required tools, env vars, and expected file paths.
7) MUST avoid secrets in skill content; use env vars for tokens.
8) SHOULD be composable: avoid conflicting assumptions so multiple skills can stack.
9) MUST keep `name` <= 64 chars, lowercase letters/numbers/hyphens only, single line, no XML tags, and avoid reserved words (e.g., "claude", "anthropic").
10) MUST keep `description` <= 500 chars, single line, no XML tags, and include WHAT + WHEN to trigger.
11) SHOULD keep the body optional and focused; avoid giant monolithic skills.

## Example Instruction Style
```
## Inputs
- input_path: Path to CSV
- output_path: Path to report

## Steps
1) Validate input exists.
2) Run: python scripts/analyze.py --in "$input_path" --out "$output_path"
3) Verify output summary is present.

## Output
- output_path contains a markdown report.
```

## Claude-Specific Notes (Behavioral)
- Claude skills are folders containing instructions, scripts, and resources that load dynamically when relevant.
- Claude skills are COMPOSABLE, PORTABLE, and only load what is needed.
- Claude uses progressive disclosure: metadata first, full instructions on demand.
- Claude auto-invokes relevant skills based on task matching.
- Claude skills assume a code execution environment with filesystem and bash access.
- Claude Code supports direct invocation with `/skill-name` and auto-loading when relevant.

## Codex-Specific Notes (Behavioral)
- Codex skills live in well-defined locations with precedence, including repo, user, and system scopes.
- Codex supports per-skill enable/disable via `~/.codex/config.toml` (experimental).
- Codex skill creation can be bootstrapped with `$skill-creator`; optional `$create-plan` (experimental) adds a planning step.
- Codex supports explicit invocation via `$skill-name`, and can also auto-select skills based on your prompt.
- Docs disagree on symlink handling: some say symlinks are supported, others say symlinked directories are ignored. TEST in your environment.

## Codex Skill Locations (Precedence Order)
1) `REPO`  `.codex/skills/`
2) `USER`  `~/.codex/skills/`
3) `SYSTEM` Bundled with Codex

## Claude Skill Locations (Common)
1) `ENTERPRISE` Managed settings (org-wide)
2) `USER`  `~/.claude/skills/`
3) `REPO`  `.claude/skills/`
4) `PLUGIN` `<plugin>/skills/`

## Differences to Harmonize
- Discovery: Claude scans skill metadata at session start; Codex scans from skill locations. Use the same YAML frontmatter to keep both happy.
- Enablement: Claude skills may need app settings; Codex skills are file-based. Keep skills portable and installable in repo and user scopes.
- Runtime: Skills assume filesystem + command execution; design scripts to be sandbox-safe and idempotent.

## Slash Commands vs Skills (HIVE CLARITY)
- Slash commands are UI actions (explicit invocations) and are NOT the same as skills.
- Skills are capability bundles with instructions/scripts/resources that can be auto-selected by the agent.
- A slash command MAY invoke a skill, but a skill MUST NOT require a slash command to exist.

## Marketplaces and Registries (Skills + MCP Tools)
Marketplaces distribute plugins/skills (Claude) and MCP tools (b00t). Treat them as INSTALL SOURCES, not runtime dependencies.

HIVE RULES:
1) MUST track a registry of available skills and MCP tools (installed OR not).
2) MUST record source, version, license, and cost/approval requirements.
3) SHOULD prefer registry lookups before new tool creation (DRY).

Suggested registry layout:
```
_b00t_/registry/
  skills.json      # catalog of skill packages and install sources
  mcp-tools.json   # catalog of MCP tools and install sources
  marketplaces.json # Claude/Codex/MCP marketplace endpoints
```

## Roles, Views, and Lazy Install
Roles define what an agent can "see" and install:
- A role is a VIEW over b00t datums: adjacent skills + MCP tools + policies.
- An agent SHOULD request missing tools via `b00t install tool <name>` when needed.
- Tools MAY be "available but not installed" until triggered by a real task.

Approval gate (future-proofing):
1) Agent proposes install with justification: cost, benefit, risk, and alternatives.
2) Captain agent approves, denies, or reassigns to another role.
3) If denied, agent MUST pick an alternative path or delegate.

## Marketplace Notes (Claude Plugins)
- Claude marketplaces can ship skills as plugins. Treat plugin marketplaces as another registry source.
- Plugin installs are cached; skills must be self-contained within the plugin path.

## Codex Config Snippet (Disable a Skill)
```
[[skills.config]]
path = "/path/to/skill"
enabled = false
```

## b00t Learn Rendering Tips
- Write for fast skim: short sections, clear headings, explicit inputs/outputs.
- Keep the first 10 lines as metadata + brief intent.
- Use ASCII-only examples so tooling and renderers stay stable.

## Minimal Creation Checklist
1) Create `my-skill/SKILL.md` with YAML frontmatter.
2) Add scripts only if they reduce model error.
3) Add references/assets only when needed.
4) Test activation with a trigger prompt.
5) Iterate on description until selection is reliable.
