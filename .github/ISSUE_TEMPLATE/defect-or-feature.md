---
name: Defect or feature
about: Report a defect, propose a change, or spec new work — carries a service_contract so "done" is a command, not a memory.
title: ""
labels: []
assignees: ""
---

<!--
House style (see issues #924, #927, #929 for lived examples): the backlog is
dogfooding CLAUDE.md's trace-or-filler law — "a command that actually ran with
an observable PASS/FAIL evidence line is a trace row worth fifty rows of
narrative." This template exists to put that contract at spec time, not
close time. Fill in each section; delete these HTML comments before
submitting.
-->

## The problem / finding

<!--
What is true today. Prefer a reproducible command + its actual output over
prose description — e.g.:

    $ <command>
    <observed output>
-->

## Why it matters

<!-- The consequence — not a restatement of the problem above. What breaks,
what it blocks, what it costs to leave as-is. -->

## What this issue requires

<!-- The acceptance surface: what has to change for this to be considered
done. Bullet list is fine. -->

```toml
[[service_contract]]
capability  = "<short-kebab-case-name>"
handler     = "<command that proves this is done>"
evidence    = "PASS: <expected output shape>"
tier        = "sm0l|ch0nky|frontier"
```

<!--
service_contract field guide:
  capability — short kebab-case name for the thing being proven
  handler    — a runnable command (just recipe, test, script) whose exit/output
               is the definition of done for this issue
  evidence   — the PASS/FAIL evidence line shape the handler is expected to
               produce; this is what gets pasted verbatim to close the issue
  tier       — cognitive-tier routing hint (sm0l/ch0nky/frontier, see
               CLAUDE.md's tier table); should agree with whichever
               `tier/*` label ends up applied below
-->

---

**No runnable handler yet?** Some design/open-ended issues legitimately
cannot declare a `[[service_contract]]` at spec time. If that's this issue,
leave the block as-is (or delete it) and apply the `needs/evidence` label
instead of blocking on inventing one — see the label-taxonomy issue (#929).

**Labels (submitter or triager applies):**
- One `tier/sm0l` / `tier/ch0nky` / `tier/frontier` — must agree with `tier`
  above, if declared.
- `needs/evidence` if no `[[service_contract]]` handler is declared.
- Priority (`P0`–`P5`), `type/*`, `area/*` as applicable.
