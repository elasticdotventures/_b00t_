---
name: hive-process-documentation
description: "Visual process documentation — SysMLv2/KerML flow charts, state machines, and health metrics from hive nodes"
version: 1.0.0
platforms: [linux, macos]
metadata:
  hermes:
    tags: [visualization, sysml, kerml, pipeline, process, health, documentation]
    related_skills: [document-evidence-pipeline, executive-operator, architecture-diagram]
---

# Hive Process Documentation Skill

Generate visual SysMLv2/KerML-compatible process documentation from hive pipeline nodes.

## When to use

When the user asks to visualize hive processes, document pipeline flows, generate state machine diagrams, check system health, or export process models for SysMLv2/KerML tooling.

## NRA Principle

NEVER REINVENT ANYTHING. This skill leverages existing infrastructure:
- `b00t-admin` server → `/api/admin/health`, `/api/admin/processes`
- `pipeline_nodes` crate → `NodeGraph`, `StateMachine`, `PipelineNode`
- `b00t-cli` → `hive status`, `doctor_cmd::health_json()`

## Quick Reference

| Action | Command |
|--------|---------|
| Health metrics | `curl http://localhost:31337/api/admin/health` |
| Process graph (JSON) | `curl http://localhost:31337/api/admin/processes` |
| Process graph (Mermaid) | `curl -s http://localhost:31337/api/admin/processes \| jq -r '.mermaid'` |
| State machines | `curl http://localhost:31337/api/admin/processes \| jq '.nodes[].state_machine'` |
| Hive status | `b00t hive status` |
| Container health | `systemctl --user status b00t-admin` |

## Related Skills

- `document-evidence-pipeline` — the underlying document evidence pipeline
- `executive-operator` — delegation contract, bouncer pattern
- `architecture-diagram` — dark theme SVG diagrams

See `references/sysml-kerml-mapping.md` for the complete SysMLv2/KerML type mapping.
