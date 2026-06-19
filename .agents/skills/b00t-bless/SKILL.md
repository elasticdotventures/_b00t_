---
name: b00t-bless
description: Load a b00t blessing — a prerequisite graph of skills, tools, and authorizations for a specific role. Use when starting a specialized agent session, checking what tools a role needs, or compiling a sandboxed agent manifest. Activates on "load blessing", "what does role X need", "compile agent", "blessing manifest".
license: MIT
---

Load blessings for role-based agent authorization:

```bash
b00t whoami                          # orient: load AGENTS.md + current role
b00t whoami --role=<role>            # load role supplement (≤120 lines)
b00t blessing --manifest --role=<R>  # walk depends_on graph → tool auth manifest
b00t compile-agent --role=X --random-transferable=3  # compiled sandbox AGENTS.md
```

Fresh agent startup protocol (MUST follow in order):
1. `b00t whoami` — orient + load session context
2. `b00t blessing --manifest` — walk prerequisite graph
3. `b00t learn <skill>` for each unlocked skill — load soul pages
4. Execute task — tools are authorized after learning

Role supplement files: `AGENTS/<role>.md` — concise (≤120 lines), tail-map required.

Available roles: executive, worker, tester, reviewer, researcher, architect.

No learning = no authorization. The blessing system enforces separation of concern:
agents only get tools their role requires, preventing privilege escalation.
