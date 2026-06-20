---
name: readme-authoring
description: |
  Craft high-engagement GitHub READMEs using proven structure: hook → problem → solution → demo →
  install → usage → contributing. Based on the Daytona 4000-stars README analysis. Applies to
  b00t datums, CLI tools, and agent role supplements.
version: 1.0.0
source: https://www.daytona.io/dotfiles/how-to-write-4000-stars-github-readme-for-your-project
allowed-tools: Read, Write, Edit, Grep, Glob, Bash
---

## What This Skill Does

Guides you through authoring GitHub READMEs that earn stars. In the b00t ecosystem, READMEs
appear at three levels:

1. **Project README** — root `README.md`; faces external contributors and operators
2. **Datum hint** — the `hint` field in a `.toml`/`.tomllm` datum; ≤2 sentences, tool-readable
3. **Skill SKILL.md** — this format; guides agent activation and step-by-step use

## When It Activates

- "write a README for …"
- "improve the README"
- "what should a b00t datum hint say?"
- "draft a SKILL.md for …"
- "make the project more discoverable"

## Key Concepts

### The 7-Section README Formula (4000-star pattern)

```
1. HOOK      — one-line "what + why it matters" (≤15 words)
2. BADGES    — CI status, version, license (trust signals)
3. DEMO      — screenshot/gif/asciicast FIRST — show before explain
4. PROBLEM   — pain point the project solves (3 bullets max)
5. INSTALL   — copy-pasteable; zero prerequisites assumed
6. USAGE     — the golden path only; link to docs for the rest
7. CONTRIBUTE — one command to run tests; PR welcome line
```

### b00t Datum `hint` Field

The `hint` is the one-line README substitute read by agents and `b00t up`. Rules:
- ≤120 characters
- Lead with the noun (what it is), then the verb (what it does)
- Avoid "a tool that" — just say what it does
- Example: `"opencode ACP server — systemd-managed ch0nky coding agent; swap with pi via hive stop/start"`

### SKILL.md Format

```yaml
---
name: <kebab-case>
description: |
  <2-3 sentences: what the skill does + when to load it>
version: 1.0.0
source: <upstream URL if applicable>
allowed-tools: Read, Write, Edit, Grep, Glob, Bash
---
```

Body sections (in order):
- `## What This Skill Does` — functional summary
- `## When It Activates` — trigger phrases for progressive disclosure
- `## Key Concepts` — reference material agents need
- `## Workflow` — ordered steps for the golden path
- `## Examples` — copy-paste ready snippets
- `## Troubleshooting` — top 3 failure modes + fixes

## Workflow

### Authoring a Project README

1. **Write the hook** — complete the sentence: "X lets you _____ without _____."
   Strip "lets you" → that's your hook line.

2. **Pick a demo artifact** — terminal recording (`vhs`), screenshot, or animated gif.
   No demo = 60% fewer stars. Run `vhs readme.tape` if VHS is installed.

3. **Write install first** — then work backwards. If install is painful, fix that before writing.

4. **Trim ruthlessly** — README should be readable in 90 seconds. Cut everything past that.

### Authoring a b00t Datum Hint

```bash
# Check current hint
b00t-cli learn <name> | head -5

# Edit datum
$EDITOR _b00t_/<name>.toml  # update [b00t] hint = "..."

# Verify it reads well in b00t up output
b00t-cli . <name>
```

### Authoring a SKILL.md

1. Create directory: `mkdir -p skills/<name>/`
2. Copy this file as template: `cp skills/readme-authoring/SKILL.md skills/<name>/SKILL.md`
3. Fill frontmatter: name, description (2-3 sentences), source URL
4. Write `## When It Activates` trigger phrases — agents use these for routing
5. Add `## Workflow` with numbered steps for the golden path
6. Add `## Examples` with copy-paste snippets

## Examples

### Hook Line Pattern
```
# Before (bad):
"b00t-cli is a Rust-based command-line tool for managing software versions and installations."

# After (good hook):
"Boot any dev environment in one command — b00t detects, installs, and updates CLI tools from TOML."
```

### Datum Hint Pattern
```toml
# Bad:
hint = "This is the opencode agent for the b00t hive system."

# Good:
hint = "opencode ACP agent — ch0nky coding tier; swap with pi via: systemctl --user start b00t@opencode-agent"
```

### Minimal SKILL.md
```yaml
---
name: my-skill
description: |
  Does X when Y. Load when operator asks about Z.
version: 1.0.0
allowed-tools: Read, Write, Bash
---

## When It Activates
- "how do I X"
- "set up Y"

## Workflow
1. Step one
2. Step two
```

## Troubleshooting

**README gets no engagement**
→ Add a demo (gif/screenshot) in the first screen. It's the single highest-leverage change.

**Datum hint is truncated in `b00t up` output**
→ Keep under 120 chars; run `b00t-cli . <name>` to preview.

**SKILL.md not triggering agent activation**
→ Check `## When It Activates` phrases match operator's exact phrasing; add synonyms.

## Related Skills

- `datum-system` — creating/editing b00t TOML datums
- `b00t-interface-library` — b00t CLI patterns and conventions
- `executive-role` — README for agent role supplements
