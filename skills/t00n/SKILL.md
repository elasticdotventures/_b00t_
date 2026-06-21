---
name: t00n
type: skill
hint: TOON (Token-Oriented Object Notation) format serializer for FOCUS records
version: 1.0.0
tags: [toon, t00n, serialization, focus, reqif, validation, contract]
tier: ch0nky
complexity: 5
depends_on: [ledgrrr]
unlocks: [t00n-serialization, focus-processing]
applies_to:
  - focus record serialization
  - t00n format conversion
  - contract validation
output_types: [.t00n, .tomllmd]
metadata:
  spec: https://github.com/toon-format/spec
  spec_version: "3.0"
  canonical_fields:
    - BillingAccountId
    - ServiceName
    - BilledCost
    - EffectiveCost
    - ChargeCategory
    - x_ExperimentId
    - x_Variant
---

## What

t00n implements the TOON (Token-Oriented Object Notation) spec v3.0 for serializing FOCUS cost/usage records into a token-efficient format optimized for LLM consumption.

## When to Use

Use t00n when serializing FOCUS records for LLM agents, validation pipelines, or contract verification. The format reduces token overhead vs JSON while preserving semantic structure.

## How

t00n encodes FOCUS records using compact field identifiers aligned with the `focus.schema.tomllmd` definition. It is entangled with `ledgrrr.cli` for CLI access and `ledgerr-mcp.mcp` for MCP-based tool invocation.

## VFS Future

The skill defines a stub `SkillStorage` trait for future virtual filesystem backends (FUSE, io_uring, memfs) that would mount t00n-serialized records as an in-memory file tree accessible via POSIX I/O — no MCP channel needed.

<!-- b00t:map v1
summary: TOON format serializer for FOCUS records — token-efficient LLM serialization
tags: toon, t00n, serialization, focus, reqif, validation, contract
tier: ch0nky
cmds: ledgrrr.cli, ledgerr-mcp.mcp
complexity: 5
-->
