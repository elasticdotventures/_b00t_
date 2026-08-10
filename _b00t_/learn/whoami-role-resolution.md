---
b00t whoami --role=reviewer reports role: reviewer in --json output but renders AGENTS/--role=worker.md content in the markdown text output instead of AGENTS/--role=reviewer.md (confirmed correct on disk, 147 lines, header 'Reviewer Role Supplement'). Role name resolves correctly for JSON metadata but the AGENTS/ file loader picks the wrong file. Repro: diff <(b00t whoami --role=reviewer | tail -100) AGENTS/--role=reviewer.md

---
canonical types: ufo-types::capability (AgentCapability, CapabilityDomain, Task) + ufo-types::stereotype (Stereotyped) — agent identity/capability introspection now belongs to the crate, not hand-rolled shapes here.
