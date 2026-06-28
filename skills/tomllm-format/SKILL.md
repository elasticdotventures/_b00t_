---
name: tomllm-format
type: skill
hint: TOML+LLM format conventions — valid TOML with enriched # comment conventions for LLM consumption
version: 1.0.0
tags: [tomllm, format, tail-map, convention, llm]
tier: sm0l
complexity: 3
description: |-
  .tomllm / .tomllmd = valid TOML + enriched # comment conventions for LLM context.
---

## What

The TOML+LLM format defines conventions for enriching valid TOML with comment annotations optimized for LLM consumption. `.tomllm` (machine-oriented) and `.tomllmd` (documentation-heavy) files extend plain `.toml` with three comment conventions: `# @tribal:` / `# 🤓` for non-obvious knowledge, design rationale, and gotchas; `# @example:` for usage examples inline with the datum; and `# b00t:map v1` as a structured tail-map footer (last ≤10 lines of the file).

The tail-map contains: `summary` (one-line description, ≤120 chars), `tags` (comma-separated keywords), `tier` (sm0l/ch0nky/frontier), `cmds` (comma-separated relevant commands), and `complexity` (1-10 integer). The critical rule: the tail-map MUST be the last ≤10 lines of the file — nothing follows it.

## When to Use

Use `.tomllmd` for documentation-heavy datums like PRDs and schemas. Use `.tomllm` for machine-oriented configuration datums. Use plain `.toml` as the fallback. Load this skill when authoring any new b00t datum.

## How

1. Write valid TOML as normal.
2. Add `# 🤓` / `# @tribal:` comments for non-obvious design rationale.
3. Add `# @example:` comments for usage examples.
4. Append `# b00t:map v1` as the footer with summary, tags, tier, cmds, complexity.
5. Ensure nothing follows the tail-map — it is a footer, not a middle section.

<!-- b00t:map v1
summary: TOML+LLM format — .tomllm/.tomllmd conventions, tail-map, comment annotations
tags: tomllm, format, tail-map, convention, llm
tier: sm0l
cmds: b00t learn tomllm-format
complexity: 3
-->
