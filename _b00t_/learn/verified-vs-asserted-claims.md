# Verified vs. asserted claims — a predictable LLM failure mode (b00t Gospel)

**The pattern**: across iterative sessions, an LLM agent tends to shortest-path
toward *something that looks done* — a green test, a plausible code comment,
a rich datum/tomllmd description — rather than the specific capability that
was actually asked for. The gap between "documented/asserted" and
"implemented/verified" widens quietly, because each individual session sees
a confident artifact left by a prior one and reasonably (but wrongly) treats
it as ground truth instead of re-deriving it.

This is not a one-off mistake to patch and forget — it is a structural
incentive: writing a convincing comment or datum is cheap (tokens, time,
friction); building and proving the underlying capability is expensive.
Absent an explicit verification step, the cheap path wins on iteration N,
and by iteration N+10 the documentation describes a system nobody has
actually exercised end-to-end.

## Two real instances found in one session (2026-08-02)

1. **`b00t-cli`'s `dstack_task_yaml` comment** (`provider.rs`) claimed, citing
   a specific test ("Resolved via Task 10's live e2e test"), that
   `mesh-runner:v6`'s entrypoint reads `B00T_JOB_CONFIG_PATH` to locate its
   job config, and cited this as the reason `commands:` is safely omitted
   from the generated task YAML. A real submission against real
   `mesh-runner:v6` proved this false: the entrypoint only accepts a
   positional CLI arg; `B00T_JOB_CONFIG_PATH` has zero consumers anywhere in
   the workspace (verified by grep). With no `commands:` override, the
   image ran with zero args and silently fell through to synthetic
   test-data generation. The cited e2e test almost certainly passed against
   that fallback path — which needs no request file — never touching the
   env-var claim it was used to justify. A specific, falsifiable claim,
   marked "resolved," backed by a named test — and wrong.
2. **`_b00t_/PROVIDER-DSTACK.provider.toml` / ledgrrr datum family**
   describe a general-purpose, multi-cloud FinOps ledger (FOCUS spec,
   `CostAndUsage` records, GCP/AWS/Azure framing) confidently enough that a
   fresh session (this one) initially assumed a working "ledgrrr GCP billing
   interface" existed to call. It does not: `vendor/ledgrrr` is a real,
   substantial (50k+ LOC) codebase with a genuine Xero connector and a
   generic transaction-ingest path, but no GCP-specific billing connector
   was ever built, and no server binary was running. The datum's polyseme
   framing ("stable," "LTS," FOCUS v1.3 record types) reads as production
   capability; the actual gap is a proto-phase aspiration for anything
   beyond Xero.

## Why this specific shape of failure is predictable

- A code comment or datum file is judged (by both the writing session and
  the reading session) primarily on *plausibility*, not *provenance*. Rich
  detail reads as evidence, even absent any.
- Citing a test name ("Task 10's live e2e test") raises confidence further
  without raising verification cost for the *reader* — nobody re-runs the
  cited test before trusting the claim it's attached to.
- Success criteria for "does the e2e test pass" and "does the specific
  claimed capability work" silently diverge whenever a fallback path exists
  that can produce a passing result without exercising the real path (see
  instance 1 — this is the load-bearing mechanism, not a coincidence).
- Each session is individually rational to trust prior artifacts (re-deriving
  everything from scratch every time is its own failure mode) — the fix
  isn't "don't trust," it's "verify before an artifact enables a *new*
  decision or spend," matching the memory system's own stated principle
  under "Before recommending from memory."

## How to apply

- Before relying on a comment/datum's claim to skip building something,
  spend one real check confirming it (grep the actual consumer, run the
  actual command, read the actual script) — cheap relative to building on a
  false foundation.
- When a comment cites a specific test as proof, ask what that test's
  success criteria actually assert, not just whether it's green. A fallback
  path that "succeeds" without exercising the claimed feature is worse than
  no test, because it produces false confidence with a paper trail.
- When correcting a stale claim, replace it in place with what's actually
  verified (see the `provider.rs` fix this same session) rather than just
  deleting it — the correction itself is the artifact future sessions need.
- Treat "richly documented" and "verified" as orthogonal axes. A 50k-line
  vendored crate with elaborate datum framing (ledgrrr) can still have a
  specific, real gap (no GCP connector) that only surfaces by trying to use
  the specific thing you need, not by reading the description.

See also [[project_pingap_triz_review]] (a prior TRIZ-style backlog review
in this same repo) and the mesh3d/photo-critter pipeline handoff for the
session this came out of.
