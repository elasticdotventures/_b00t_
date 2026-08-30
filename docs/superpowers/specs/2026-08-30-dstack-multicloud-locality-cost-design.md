# dstack Multi-Cloud Backends: Data-Locality Placement + Startup-Time Cost Accounting

**Date:** 2026-08-30
**Status:** Proposed
**Motivated by:** investigating a stuck CI job on `PromptExecution/rust-docs-mcp-b00t#1` (queued against a
`runners-promptexecution-org` ARC label with zero registered runners — confirmed no persistent ARC
deployment ever existed) surfaced a broader ask: b00t should have registered, on-demand compute patterns
across AWS/Azure/GCP (minimal listener spawns a job), with placement that respects data locality
(datasets can be petabyte-scale and effectively cloud-bound) and cost accounting that charges
cold-start/idle time, not just runtime. RunPod is deliberately excluded from this design's scope (see
`feedback_runpod_antipattern_and_local_build_toil` memory: no Pod TTL is a confirmed budget-spike
incident, not a gap worth engineering around — this applies with equal force to ephemeral ARC-style
runner-like usage).

**Supersedes an earlier draft plan** that proposed new `AwsProvider`/`AzureProvider`/`GcpProvider`
`ComputeProvider` implementations. That was the wrong layer: `DstackProvider` (PR #857, 2026-07-22) is
already b00t's generic multi-cloud `ComputeProvider` — it shells out to the `dstack` CLI, which natively
supports RunPod/AWS/GCP/Azure/Lambda/Kubernetes/bare-metal as backends configured together in one
`~/.dstack/server/config.yml`. Per the already-approved `2026-07-23-b00t-dstack-gcp-backend-design.md`,
adding a cloud backend is a **configuration/credential-generation change, not new provider code** — no
new `PROVIDER-*` datum, no new `ComputeProvider` impl. Verified against the current recipe
(`_b00t_/justfile-dstack-sdd.just`): only the `runpod` backend block is actually generated today; the
GCP design was approved but never wired in. So this design's job/provider-generalization piece reduces
to: wire GCP (finishing the approved-but-unimplemented design) + add AWS + add Azure, all via the same
recipe extension pattern — plus the genuinely new pieces this session's ask requires that don't yet
exist anywhere in this system: per-job backend/region selection driven by data locality, spot/idle
knobs, and startup-time-inclusive cost accounting.

**Out of scope, deferred as independent follow-ons** (per this session's decomposition):
GH-Actions-runner-specific integration (a future `CodeRunnerActionProcess` provider consuming this
system's tier-A path); the `ufo-types` DAG construct (needs its own earnest search for an existing
"b00t bus" pipeline and a state-machine component, tied to the in-progress `scxml`/statechart epic
`elasticdotventures/_b00t_#1177` P5a-P8 — not started here); Nydus/lazy-pull cold-start image
acceleration.

---

## Scope

**In scope:**

1. Finish wiring the already-approved GCP dstack backend into `justfile-dstack-sdd.just`'s
   `dstack-server-config` recipe (it was designed and approved 2026-07-23 but never actually landed in
   the recipe — verified by grep: only `type: runpod` exists there today).
2. Add AWS and Azure dstack backend blocks to the same recipe, following the identical pattern GCP's
   design established (ambient-credential-first: AWS via an existing configured CLI profile/SSO,
   Azure via `az` CLI ADC-equivalent — exact credential shape to confirm at implementation time per
   dstack's `core/backends/{aws,azure}/models.py`, same verification method the GCP design used).
3. Update `_b00t_/PROVIDER-DSTACK.provider.toml` to document all three new/completed backends
   (mirroring its existing `[resource.secrets.runpod_api_key]` block shape).
4. Extend `BatchJobSpec` (`b00t-cli/src/commands/provider.rs:94`) with:
   - `dependencies: Vec<String>` — dataset/resource URIs this job needs (see "Dependencies" naming
     note below, and the forward-compatibility note under "Open Questions"). Sibling to the existing
     `depends_on: Vec<String>` on `JobStep`
     (`b00t-cli/src/datum_job.rs:117`, which drives real topological step-ordering) — kept as a
     separate field rather than overloading `depends_on`, since mixing dataset URIs into that field
     would corrupt its topo-sort semantics (a dataset isn't a step whose completion is awaited).
   - `interruptible: bool` (default `false`) — hint for spot/preemptible eligibility.
   - `#[serde(default)] pub backend_hint: Option<String>` and `pub region_hint: Option<String>` —
     resolved by the new placement step below and passed through to `dstack_task_yaml`/
     `dstack_fleet_yaml` as explicit `-b`/`--region` equivalents. (The GCP design deferred exactly this
     as YAGNI in 2026-07-23 — real, concrete justification now exists: without it, a job with a
     petabyte-scale dataset dependency has no way to avoid being scheduled against the wrong cloud and
     incurring real cross-cloud egress cost.)
5. Extend `dstack_fleet_yaml`/`dstack_task_yaml` (`provider.rs:716`/`743`, currently minimal — just
   `nodes: 0..1` + GPU count, no spot/idle/region/backend fields at all) to emit `backends:`/`regions:`
   when a hint is resolved, and a `spot_policy`/`inactivity_duration` block driven by `interruptible`
   (dev-environments already require `inactivity_duration` per `PROVIDER-DSTACK.provider.toml`'s
   existing `[resource.cost_control]` note — this design generalizes that same knob, doesn't invent a
   new one).
6. New `b00t-cli/src/placement.rs`: resolves a job's `dependencies` against each configured backend's
   declared data-residency (new `[resource.data_residency.<backend>]` tables in
   `PROVIDER-DSTACK.provider.toml`, e.g. `datasets = ["s3://app4dog-ml-training/*"]`) into a
   `backend_hint`/`region_hint` pair, following the TRIZ "no free lunch" rule: locality match beats
   cheaper-cross-cloud, always.
7. `JobUtilizationEvent` emission from `job_executor.rs` (job_id, backend, cold_start_duration,
   run_duration, estimated_cost) into a small outbox ledgrrr's `ledger-core` ingests as a new
   ledger-entry kind — keeps the existing split (b00t-cli = execution mechanism, ledgrrr = ledger of
   record) rather than a second cost-accounting system.

**Out of scope (this design):**
- Per-job backend selection beyond the locality-driven hint above (dstack's own ad hoc `-b`/`-r` CLI
  flags remain available for manual overrides; no new CLI surface for that here).
- Warm-pool "keep a model resident in RAM" scheduling — dstack's existing **persistent-volume**
  mechanism (`type: volume`, already documented in `PROVIDER-DSTACK.provider.toml`'s TRIZ note: "stage
  image/deps/datasets once, dispatch many jobs against the same warm environment") already covers the
  bulk of this need at the environment-reuse level; this design does not add a second, competing
  warm-pool registry. A dedicated model-residency scheduler (deciding *which* models stay resident
  *where* and for how long, per this session's ask) is real additional work, deferred as a follow-on
  once basic multi-cloud dispatch + locality hinting exist to schedule *against*.
- RunPod as an ephemeral-runner-style backend (excluded per standing guidance; RunPod's existing
  training/inference use via dstack is unaffected).
- GH Actions runner integration, `ufo-types` DAG, Nydus (all noted above).

---

## Architecture

### Component placement

```
_b00t_/justfile-dstack-sdd.just
  dstack-server-config            ← extended: gcp/aws/azure backend blocks alongside existing runpod

_b00t_/PROVIDER-DSTACK.provider.toml
  [resource.secrets.gcp_credentials]    ← new (per approved 2026-07-23 design, never landed)
  [resource.secrets.aws_credentials]    ← new
  [resource.secrets.azure_credentials]  ← new
  [resource.data_residency.<backend>]   ← new: dataset URI globs "local" to each backend/region

b00t-cli/src/commands/provider.rs
  BatchJobSpec                     ← + dependencies, interruptible, backend_hint, region_hint
  dstack_fleet_yaml / dstack_task_yaml
                                    ← + backends:/regions:/spot_policy/inactivity_duration emission

b00t-cli/src/placement.rs          ← new: dependencies -> (backend_hint, region_hint) resolution

b00t-cli/src/job_executor.rs
  JobUtilizationEvent              ← new: emitted per completed job (cold_start_duration separate
                                      from run_duration)

ledgrrr/crates/ledger-core/       ← new ledger-entry kind ingesting JobUtilizationEvent
```

### Data flow

1. A `.job.toml` step declares `dependencies = ["s3://app4dog-ml-training/dataset-x/*"]` and
   `interruptible = true` on its `[b00t.job.steps.batch]` block.
2. `placement.rs` matches `dependencies` against every configured backend's
   `[resource.data_residency.<backend>]` globs, ranks candidates (locality match desc, then cost),
   and resolves a `backend_hint`/`region_hint` pair — or returns an explicit error naming the
   unmatched dependency if no backend declares residency for it (never a silent cross-cloud fallback).
3. `dstack_task_yaml`/`dstack_fleet_yaml` emit that hint as `backends:`/`regions:`, plus
   `spot_policy: auto` + `inactivity_duration` when `interruptible = true`.
4. `job_executor.rs` records wall-clock from submission to `provisioning`→`running` transition as
   `cold_start_duration`, and `running`→terminal as `run_duration`, emitting both in a
   `JobUtilizationEvent` regardless of success/failure.
5. ledgrrr ingests the event as a new ledger-entry kind, attributing cost to both phases distinctly —
   a job that's cheap to run but slow to cold-start is visible as such, not hidden inside a single
   "runtime" number.

---

## Error Handling

- **Zero placement candidates** (no backend declares residency matching a job's `dependencies`) →
  explicit error naming the unmatched URI(s). Never silently dispatch cross-cloud — a wrong-region
  fallback risks a real egress-cost surprise against a petabyte-scale dataset.
- **Spot/interruptible preemption** → dstack's own `terminated`/`failed` run states already exist in
  `PROVIDER-DSTACK.provider.toml`'s `[resource.run_states]`; this design does not add a new status
  bucket, but `job_executor.rs`'s retry/checkpoint path (already used for `.job.toml` steps) should
  requeue an interruptible job that hits `terminated` rather than treating it as a hard failure —
  confirm at implementation time whether dstack's status alone can distinguish "preempted" from
  "operator cancelled" (open question below).
- **Cold start exceeding a configurable budget** → still emits the utilization event (startup time is
  real cost even on a miss); does not fail the job by itself.

---

## Testing Strategy

- `dstack_fleet_yaml`/`dstack_task_yaml`: extend the existing pure-function YAML-builder tests
  (`dstack_fleet_yaml_is_autoscaling_zero_to_one_with_gpu`, etc. — same file) with cases asserting
  `backends:`/`spot_policy`/`inactivity_duration` appear only when a hint/interruptible flag is set,
  and are absent otherwise (preserving today's "match whatever the operator configured" default when
  no hint resolves).
- `placement.rs`: pure-function unit tests — locality match beats no match, zero-candidate case errors
  with the unmatched URI named, tie-break behavior when two backends both declare residency.
- `dstack-server-config` recipe: verify generated YAML by hand through dstack's own `ServerConfig`
  pydantic models for each new backend block, same method the GCP design used — no Rust unit test,
  this recipe lives in shell/YAML.
- **No live cloud spend** as part of this work — generating config and a read-only `dstack offer -b
  <backend>` query is in scope; actually submitting a real job against AWS/Azure is a separate,
  explicitly-gated step per the existing dstack-provider-design precedent.
- ledgrrr: a round-trip test asserting `cold_start_duration`/`run_duration` survive as distinct fields
  through the new ledger-entry kind, not collapsed into one.

---

## Open Questions (resolve during planning/implementation)

1. Exact AWS/Azure dstack backend credential model (`core/backends/{aws,azure}/models.py`) — verify
   against the installed dstack version directly, same method the GCP design used, before assuming
   ambient-credential support parallels GCP's `GCPDefaultCreds`.
2. Can dstack's own run states distinguish spot preemption from other termination causes, or does
   `job_executor.rs` need to infer it from timing/exit-code heuristics?
3. Exact shape of `[resource.data_residency.<backend>]` — one table per backend keyed by name, vs. a
   list of `{backend, region, datasets}` entries if a backend spans multiple regions with different
   local datasets. Lean toward the list form given AWS/Azure/GCP all support multi-region deployment.

**Forward-compatibility note (does not block this design):** `dependencies: Vec<String>` above is a
deliberately simple, concrete placeholder. The user's stated intent is for `Dependency` to eventually
be a generic, cheaply-derivable type — analogous to how `Serialize`/`Deserialize`/`Eq` attach via
`#[derive(...)]` — parameterized over a `Constraint` it can be matched/satisfied against (reusing the
`Satisfies<T>`/`PolyConnector<T>` pattern already documented in `WRKFLW.tomllmd`'s ontology section,
which produces evidence nodes for an audit trail). That generic `Dependency<'a, C: Constraint>` type
belongs in `ufo-types` alongside `Stereotyped`, as an edge type on the canonical abstract `Node`/`Edge`
DAG construct — **the `ufo-types` DAG construct (sub-project 3, deferred) must be built on this
generic `Dependency`/`Constraint` pattern, not a separate ad hoc graph/edge type.** This design's
`Vec<String>` is intentionally migration-shaped (a list of URI strings is a trivial special case of
"a list of things satisfying a dataset-locality constraint") so that migrating `BatchJobSpec.dependencies`
onto the real `ufo-types` type later is a mechanical follow-up, not a redesign.

---

## Dependencies to Add

None beyond what `dstack-provider-design.md`/`dstack-gcp-backend-design.md` already require (dstack
CLI, already installed per this session's prior work). No new Rust crates.

---

# b00t:map v1
# summary: dstack multi-cloud backend completion (GCP/AWS/Azure) + data-locality-aware placement hints + startup-time-inclusive cost accounting via ledgrrr
# tags: provider, dstack, orchestration, multi-cloud, aws, azure, gcp, placement, data-locality, cost-accounting, ledgrrr
# tier: frontier
# cmds: just dstack-server-config, b00t-cli provider job submit-batch --provider dstack
# complexity: 6
