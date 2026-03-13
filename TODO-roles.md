# TODO: Roles Design Notes

## Goal

b00t needs role-based composition where a role includes one or more skills, MCP tools, and apps.

Example flow:
- `b00t install app-server`
- `b00t mcp install app-server-mcp`
- `b00t whoami --role app-server-wizard`

Constraints:
- Datums remain source of truth.
- DRY.
- NTRW (Never Reinvent The Wheel).
- MECE modeling.

## Core Model

Role should be composition-only:
- `role` datum references existing datums.
- No duplicated install logic in role datum.

Role composition dimensions:
- skills
- mcp
- apps

## Source Of Truth Rules

1. Install metadata lives in component datums (`app`, `cli`, `mcp`, `skill`), not in role.
2. Role datum only expresses intent/composition.
3. Dependency resolution runs over the datum graph.
4. `whoami --role` reads resolved graph and reports readiness.

## MECE Datum Boundaries

- `skill`: agent behavior/instructions and optional templates/examples.
- `mcp`: MCP server/client integration.
- `app`: app/runtime/service install surface.
- `cli`: command/tool install surface.
- `role`: composition of the above.
- `stack`: broader environment aggregate (optional; distinct from role).

## CLI Contract (Target)

1. `b00t install role <name>`
- Resolve role -> transitive datums -> dependency DAG -> deterministic install order.

2. `b00t install role <name> --only mcp|skills|apps`
- Scoped install by composition category.

3. `b00t mcp install role <name>`
- Convenience alias for MCP-scoped role install.

4. `b00t whoami --role <name>`
- Render role summary:
- composed skills
- composed MCP tools
- composed apps
- missing deps
- readiness/capability checks

## Minimal Role Datum Example

```toml
[b00t]
name = "app-server-wizard"
type = "role"
hint = "Build and operate app server workflows"

[b00t.role]
skills = ["app-server.skill", "debugging.skill"]
mcp = ["app-server.mcp", "github.mcp"]
apps = ["app-server.app", "docker.cli"]
```

## Implementation Approach

Reuse first (DRY/NTRW):
- existing datum loader/parser
- existing dependency resolver
- existing install executor
- existing whoami capability checks

Add only:
- `RoleResolver` (expands composition refs into datum set)
- install scope filter (`all|mcp|skills|apps`)
- role-aware CLI handlers

## Structural Upgrade (Inspired by `review-prompts`)

Improve the roles plan by introducing **role packs** with a consistent on-disk layout and installer:

```text
roles/
  app-server-wizard/
    role.toml                 # composition datum
    skills/                   # role-specific skill files or mappings
    commands/                 # slash/command templates for common workflows
    patterns/                 # failure patterns, anti-patterns, checklists
    scripts/
      setup.sh                # install/link role pack into target client(s)
    README.md                 # role-specific usage
```

Key benefits:
- predictable packaging and discovery
- easier install/update/remove semantics
- role-local verification artifacts (`patterns/`) for lower false positives
- client setup automation from one script

## Command Taxonomy Per Role

Adopt a consistent triad for each role (similar to review/debug/verify):

- `run` (execute role workflow)
- `debug` (investigate failures)
- `verify` (validate output / readiness)

CLI examples:
- `b00t role run app-server-wizard`
- `b00t role debug app-server-wizard`
- `b00t role verify app-server-wizard`

These commands should resolve to role-pack command templates, not hardcoded logic.

## Setup Script Pattern

Each role pack should ship `scripts/setup.sh` that:

1. Resolves absolute role-pack path.
2. Installs/symlinks role assets into client-specific locations.
3. Substitutes path placeholders in command/skill templates.
4. Prints available role commands after install.

This mirrors the proven install ergonomics from `review-prompts` (`claude-setup.sh` pattern).

## Conditional Context Loading

Role context should load conditionally:

1. Lightweight role metadata first.
2. Load role `patterns/` and deep docs only on `debug`/`verify`.
3. Load subsystem-specific prompts only when relevant to current mission/files.

This reduces token burn and keeps role execution focused.

## Verification Assets

For each role pack, require:

- `patterns/false-positives.md`
- `patterns/checklist.md`
- optional subsystem notes

`b00t role verify <role>` should use these files as deterministic review criteria.

## Phase Plan

Phase 1:
- finalize role schema (`[b00t.role]`)
- implement resolver
- expand `whoami --role` output

Phase 2:
- `b00t install role <name>`
- deterministic DAG install for resolved datums

Phase 3:
- scoped install (`--only`)
- `b00t mcp install role <name>` shortcut

Phase 4:
- tests:
- resolver unit tests
- install ordering tests
- whoami readiness tests
- docs/examples

Phase 5:
- add policy hooks with `prek`:
- validate role datum schema before commit
- validate role references resolve to existing datums
- enforce no inline install commands in `type="role"` datums
- run role smoke checks (`b00t whoami --role ...`) for changed role files

Phase 6:
- introduce role-pack filesystem layout (`roles/<name>/...`)
- add setup script generator/template for role packs
- add role command triad (`run|debug|verify`) routed from role-pack commands
- add conditional prompt/pattern loading for token efficiency

## Hook Plan (`prek`)

Goal:
- Prevent broken role compositions from entering main.
- Enforce DRY/NTRW: role datums MUST compose, not duplicate install logic.

Hook checks to add:
- `role-schema-check`
- validate `type = "role"` and required `[b00t.role]` shape.
- `role-reference-check`
- ensure `skills`, `mcp`, `apps` entries resolve to existing datums.
- `role-no-install-check`
- fail if role datum declares install/version shell commands directly.
- `role-whoami-smoke`
- run `b00t whoami --role <role>` for changed role datums.

Suggested wiring:
- keep existing `.git/hooks/pre-commit` as entrypoint.
- update `just commit-hook` to execute `prek run --hook pre-commit`.
- add `prek` config file (`prek.toml` or `.prek.toml`) and register checks there.

Initial rollout strategy:
1. Start with advisory mode for `role-reference-check`.
2. Switch to blocking mode once role catalog is clean.
3. Add pre-push `prek` checks for broader graph validation.

## Open Questions

1. Should roles allow inline dependency overrides, or strictly inherit from referenced datums?
2. Should `apps` be a dedicated datum type or normalized to `cli` + `docker` + `service` datums?
3. Should `stack` be allowed to depend on `role`, or should dependency direction be one-way?
4. How strict should resolver validation be for missing refs (fail-fast vs warn)?
