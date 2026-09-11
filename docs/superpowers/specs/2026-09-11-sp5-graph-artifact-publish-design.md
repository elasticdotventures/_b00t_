<!-- Approved implementation plan (2026-09-11, session_01198jdJbpYhj8AFqM7LCk5a).
     Continues SP5 (SysML-v2 coherence substrate). Un-defers one item the
     original SP5 spec explicitly punted to "SP6 or later" ("S3 / content-hash
     of the asserted graph") per operator direction. Phase 1 only — per-artifact
     tagging (Phase 2) and cross-repo aggregation (Phase 3) are out of scope,
     tracked separately (see "Deferred" section). -->

# SP5 continuation: signed graph-artifact publish, S3/ZeroFS storage, best-effort NATS reindex hint

## Problem

SP5's core substrate (Oxigraph store, SHACL, KerML view, `b00t-graph` CLI —
SP5-01 through SP5-08) is implemented and CI-green on branch
`spec/sp5-sysml-coherence`, not yet merged to main. It produces a KerML view
+ `iso_ir` JSON on demand, but nothing publishes that output anywhere durable,
and nothing tells other consumers (kr0ki, other hive nodes) when a new
version exists. The original SP5 spec's own "Deferred" section named this
exact gap ("S3 / content-hash of the asserted graph") and pushed it to SP6.
This spec un-defers it, scoped narrowly to Phase 1.

## Non-goals

- Per-artifact/per-model tag granularity (Phase 2) — separate spec, own gh
  issue, needs its own brainstorming session before any code.
- Cross-repo aggregation — ledgrrr/ufo-types/kr0ki each publishing their own
  slice (Phase 3) — same.
- A central registry service (CF Worker + D1, mirroring `b00t-identity`) —
  deliberately deferred; see the separate registry gh issue. Phase 1 treats
  the S3/ZeroFS bucket's own key layout as the source of truth, no indexing
  service in front of it.
- Anything routed through `b00t-comms`/`NatsMeshNode`/the `chat` CLI — that
  subsystem is a confirmed, separate, currently-unbuilt gap (gh
  `elasticdotventures/_b00t_#1299`, task #210). This spec's NATS reindex
  event is a plain, direct `async_nats` publish from the CI job and a plain
  subscribe in the consumer agent — it does **not** depend on #1299 landing
  first. Keeping these decoupled is deliberate: SP5 shouldn't block on an
  unrelated, unfinished subsystem, and vice versa.

## Architecture

```
git push (tag v*)
        │
        ▼
  CI: .github/workflows/graph-artifact-publish.yml
        │  (gated on the tag's commit already being CI-green)
        ▼
  build: b00t-graph CLI (SP5-07/08, already built) loads _b00t_/ as of
         that tag → emits KerML view text + iso_ir JSON
        │
        ▼
  sign: generalized F4 primitive (canonicalize() + DatumSignature{RS256,...},
        b00t-cli/src/datum_agent_profile.rs — already implemented for
        AgentProfileSpec, generalize the call site, not the primitive) over
        a manifest covering both payloads
        │
        ▼
  publish (strict order, via StorageKind::{S3,Zerofs} — reused from
           b00t-cli/src/commands/finetune_job.rs, not reinvented):
    1. <bucket>/graph/tags/<tag>/{kerml-view.ttl, iso_ir.json, manifest.json}
    2. only after (1) succeeds: overwrite <bucket>/graph/latest/manifest.json
        │
        ▼
  reindex hint: one direct async_nats publish, subject
                `b00t.graph.reindex` (see "Subject naming" below),
                payload {tag, commit_sha, content_hash, s3_prefix}
        │
        ├──────────────► per-node systemd proxy agents (sm3lly, fung1, ...)
        │                 pull s3_prefix, verify signature, rebuild local
        │                 OxigraphStore. New binary, modeled directly on
        │                 b00t-historian.service's unit shape.
        │
        └──────────────► kr0ki-server (already shipped) — same pull+verify,
                          feeds its render step. Small addition to an
                          existing service, not a new one.
```

## Components

**1. CI trigger.** New `.github/workflows/graph-artifact-publish.yml` on
`push: tags: ['v*']` — reuses the existing tag convention (23 tags already
drive `b00t-mcp-npm-release.yml`/`b00t-npm-release.yml`/`browser-ext-release.yml`/
`build-tauri-windows.yml`; no new scheme). Gated on the tagged commit's own
CI already being green (`needs:` the relevant check, or re-run
`cargo check (workspace)` as a guard job).

**2. Build.** `cargo run -p b00t-c0re-lib --example b00t-graph -- kerml --view
<...>` per the SP5-08 runbook (`docs/runbooks/graph-substrate.md`) — no new
export logic, this already exists and is CI-green on the SP5 branch.

**3. Sign.** The one real new code: generalize
`b00t-cli/src/datum_agent_profile.rs`'s `canonicalize()` /
`DatumSignature{alg:"RS256",...}` (currently hardcoded to `AgentProfileSpec`)
into a reusable helper taking any `serde_json::Value`, then apply it to a new
`GraphArtifactManifest { tag, commit_sha, content_hash, signed_fields_hash,
kerml_digest, iso_ir_digest }`. Same key: `$B00T_DATUM_SIGNING_KEY_PEM`, same
kid namespace as SP1's JWT key (per the existing code comment). New
subcommand: `b00t graph sign` (or fold into `b00t graph publish`).

**4. Storage.** Reuse `finetune_job.rs`'s `enum StorageKind { S3, Zerofs }`
and its established credential story verbatim:
- S3: `spire-agent` short-lived credentials (`profile = "spire-agent"`), not
  a static/root key — per `infrastructure#189`'s already-landed pattern.
- ZeroFS: mount via the container from `infrastructure#192` (**merged** —
  `containers/zerofs/`, `Barre/ZeroFS` v2.3.2, non-root uid 1001), write
  through its POSIX interface; ZeroFS's own `[aws]` config block credentials
  come from `secretEnvy`/`globalEnvy`, never hand-typed.
- Key layout (either backend): `graph/tags/<tag>/{kerml-view.ttl,iso_ir.json,
  manifest.json}`, `graph/latest/manifest.json`.
- Publish order is strict: per-tag objects first, `latest` pointer last and
  only on success — `latest` never references a half-published tag.

**5. Reindex hint — subject naming.** `b00t.graph.reindex` follows the
`b00t.hive.mesh.*` dot-delimited convention already established for
cross-node hive traffic (`b00t-comms`'s designated subject family — see
`_b00t_/nats.skill.toml`), rather than the colon-delimited `b00t:learn:*`
query-bus convention (`query_bus.rs`) or the competing unbuilt `c0re.*`
proposal (task #140). Publishing is a **direct `async_nats::Client::publish`
call from the CI job/signing step** — not routed through `NatsMeshNode` or
any `b00t-comms` binary, since neither is required for a one-shot fire-and-
forget publish and #1299 (the binary) doesn't exist yet. `b00t-historian`
already proves durable NATS consumption works in this environment
(`Requires=b00t-nats.service` pattern) if a future iteration wants this
subject durably archived too — not required for v1.

**6. Per-node consumers.** New small binary (name TBD at implementation —
e.g. `b00t-graph-reindex-agent`), systemd unit modeled directly on
`nats/pyinfra/files/b00t-historian.service`'s shape (`After=/Requires=
b00t-nats.service`, `Restart=always`, `EnvironmentFile=/etc/<name>.env`).
Deployed via the existing `nats/pyinfra/` tooling — **note:**
`nats/pyinfra/inventory.py` currently only lists one host (`b00t_node`,
the Vultr VPS); sm3lly and fung1 are not in it. Extending that inventory
with sm3lly/fung1 entries is in scope for this work (small, additive) rather
than assumed to already work.

**7. kr0ki-server.** Same pull+verify logic as the per-node agent (share the
code — a small library function, not two implementations), added to the
already-shipped `kr0ki-server` binary. Stays inside kr0ki's existing
"KerML/`iso_ir` in, never raw datums" boundary (PRD-KR0KI-001 §1.1) — it's
just gaining an actual source to read from.

## Error handling

Strict sequential steps (build → sign → per-tag upload → `latest` update →
NATS publish); any failure aborts before the next step, so nothing partially-
visible becomes reachable as "latest." NATS publish failure after a
successful S3/ZeroFS write is non-fatal and only logged — the object is
already durably stored, the event is a best-effort low-latency hint, and
`latest`/per-tag S3 objects stay pollable as the fallback recovery path (same
posture `b00t-historian`'s own design notes call out: "an agent that isn't
subscribed at that exact instant is simply lost" for core NATS pub/sub —
design around it, don't fight it).

## Testing

- CI dry-run against a test bucket/prefix on PRs touching this workflow;
  real publish only on an actual tag push.
- Unit tests for the generalized sign/verify helper (extend the existing
  `datum_agent_profile.rs` test patterns to the new payload type).
- Local round-trip integration test: build → sign → temp-dir (not real S3) →
  verify — no network dependency for CI correctness of the sign/verify logic
  itself.
- Per-node agent: a test that feeds it a known-good signed manifest (fixture)
  and asserts the local OxigraphStore rebuild succeeds; a second test with a
  tampered signature asserts it's rejected.

## Deferred (tracked separately, not designed here)

- Central registry service (CF Worker + D1) — own gh issue, own library
  crate requirement, needs a dedicated brainstorming session before any
  sub-agent touches it. File as part of this same work session.
- Phase 2 (per-artifact tags), Phase 3 (cross-repo aggregation) — same issue,
  explicitly staged after Phase 1 ships and is validated.
- MCP exposure of the per-node agent / kr0ki fetch step — CLI-first for v1.

<!-- b00t:map v1
summary: SP5 continuation — CI-driven signed graph-artifact publish to S3/ZeroFS, best-effort NATS reindex hint, per-node systemd consumers
tags: sp5, sysml, graph, s3, zerofs, nats, reindex, kr0ki, signing
tier: frontier
cmds: b00t task list, cargo run -p b00t-c0re-lib --example b00t-graph
complexity: 7
-->
