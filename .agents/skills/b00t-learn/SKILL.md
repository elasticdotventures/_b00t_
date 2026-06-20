---
name: b00t-learn
description: Load a b00t datum soul page for a topic (tool, skill, library, concept). Use when asked to learn about a specific technology, check what b00t knows about a topic, or load tribal knowledge before implementing. Activates on "b00t learn X", "load soul for X", or "what does b00t know about X".
license: MIT
---

Load a datum soul page to get b00t's compiled knowledge for a topic:

```bash
b00t learn <topic>              # load soul: description, hints, usage examples
b00t learn <topic> --concise    # compact view for context-constrained agents
b00t learn <topic> --raw        # raw TOML datum for introspection
```

When a soul is missing, b00t queues research automatically:
```bash
b00t learn rust                 # found: returns compiled soul page
b00t learn unknown-lib          # miss: queues research-soul task
```

The soul pipeline:
1. `b00t learn` — loads existing datum or returns miss
2. Stage 1 review: keyword-overlap discriminator (flags graph-adjacent noise)
3. Stage 2 review: grok vector endorsement (semantic similarity check)
4. Both reject → `review-soul: <topic>` task queued for pi/opencode review
5. `just research-soul topic=<X>` — ingest raw sources → grok assimilate → datum update

Always `b00t learn` a topic before implementing it — the soul contains non-obvious
constraints, tribal knowledge (🤓 comments), and usage patterns that prevent mistakes.
