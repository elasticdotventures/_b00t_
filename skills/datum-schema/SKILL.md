---
name: datum-schema
type: skill
hint: Datum type taxonomy and field schema reference — the canonical types in _b00t_ ecosystem
version: 1.0.0
tags: [datum, schema, type-taxonomy, reference]
tier: ch0nky
complexity: 5
description: |-
  Datum types (skill, cli, mcp, agent, role, gate, schema, hardware) with [b00t] required fields, skill-specific fields, tail-map format, and forbidden fields.
---

## What

The datum-schema skill defines the canonical type taxonomy and field schema for the `_b00t_` ecosystem. Supported datum types include: skill (skill definitions), cli (CLI tool wrappers), mcp (MCP server definitions), agent (sub-agent definitions), role (role definitions with skills/compliance/entanglements), gate (governance gates), schema (data protocol schemas), and hardware (hardware capability descriptions).

Required `[b00t]` fields for all datums: `name` (unique kebab-case identifier), `type` (one of the datum types above), and `hint` (human-readable description, ≤120 chars). Skill-specific fields under `[b00t.skill]` include `depends_on` (prerequisite skills), `unlocks` (capabilities unlocked when loaded), `description`, `tags`, and `metadata`. The tail-map format (`# b00t:map v1`) requires: summary (≤120 chars), tags, tier (sm0l/ch0nky/frontier), cmds, and complexity (1-10).

## When to Use

Load this skill when authoring new datums, validating existing ones, or understanding the b00t type system. It is a prerequisite for `datum-authoring`.

## How

1. Identify the datum type that matches your resource (skill, cli, mcp, agent, etc.).
2. Fill required `[b00t]` fields: name, type, hint.
3. If creating a skill, use `[b00t.skill]` with depends_on, unlocks, description, tags.
4. Append the `# b00t:map v1` tail-map with summary, tags, tier, cmds, complexity.
5. Verify: no `[b00t.version]` (use `[b00t.schema]` instead), no `[b00t.type_tags]` (use tail-map tags).

<!-- b00t:map v1
summary: Datum type taxonomy — all supported datum types, required fields, forbidden patterns
tags: datum, schema, type-taxonomy, reference
tier: ch0nky
cmds: b00t learn datum-schema
complexity: 5
-->
