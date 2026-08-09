---
deepwiki=autoresearch=karpathy: pattern where agent recursively reads source material BEFORE acting. Key: a sm0l reviewer model reads retrieved doc and answers RELEVANT|SKIP:<reason>. Gate fires between Orient and Act in OODA. Without it: retrieval is keyword-match, agent acts on wrong context. In b00t: use grok ask as the sm0l reviewer. Never skip the reviewer gate just because retrieval returned a result.

---
canonical types: ufo-types::dare (DaredProposal, OodaStateMachine, OodaPhase, OodaGuards) — OODA state-change workflow types now belong to the crate, not hand-rolled shapes here.
