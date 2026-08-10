---
role datums need depends_on BEFORE testing manifest — empty required block is the symptom; fix: add depends_on to *.role.toml then re-run b00t blessing --manifest

---
canonical types: ufo-types::capability (AgentCapability, CapabilityDomain, Task) + ufo-types::stereotype (Stereotyped) — agent identity/capability introspection now belongs to the crate, not hand-rolled shapes here.
