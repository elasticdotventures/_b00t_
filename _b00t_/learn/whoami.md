---
b00t whoami in a Claude Code session double-pays ~2.4k tokens: harness already injects CLAUDE.md, whoami echoes identical boilerplate. Needs --suffix-only mode (emit session context + role summary only) or CLAUDE_CODE env detection

---
canonical types: ufo-types::capability (AgentCapability, CapabilityDomain, Task) + ufo-types::stereotype (Stereotyped) — agent identity/capability introspection now belongs to the crate, not hand-rolled shapes here.
