# I1 Alignment Brief — Standard Kit Compliance

## Personas, Journeys, and KPIs
- **OperatorConsole personas:** `Operator`, `Ops/Docs`, `Capability Integrator`. Each persona remains CLI-first with React Ink overlays only when dashboards unblock live debug loops. Journeys: bootstrap env via `just setup-dev`, queue workflows with `b00t run <skill>`, monitor telemetry via `just cli-smoke` and TelemetryHub dashboards, reconcile offline caches with `b00t sync`.
- **Primary KPIs:**  
  - `CLI success rate`: ≥97% of audited commands complete inside 10 s interactive SLA (per §1.0 + §3.7).  
  - `Workflow throughput`: ≥95% of batch workflows finalize under 5 min, measured via WorkflowRun `completed_at - started_at`.  
  - `Onboarding time`: ≤2 hours from repo clone to first successful `just cli-smoke`, tracked through onboarding checklist telemetry.  
- KPI telemetry MUST attach `trace_id`, `persona_id`, and `feature_flags` echo fields for every CLI invocation so OperatorConsole dashboards expose cause → effect chains.

## Standard Kit & NRtW Mandates
- Architects SHALL use the enforced stack (Rust Axum orchestrator, FastAPI adapters, PostgreSQL + Redis + LiteFS, NATS JetStream, AWS ECS/SQS/S3, Terraform, Flagsmith, GitHub Actions) without deviation (§2.0).
- NRtW: before proposing bespoke code, reference `standard-kit` crates, `just` recipes, and OSS dependencies with ≥1k stars; forks MUST upstream fixes.
- CLI/TUI remains the sole UI per §2.0 + §3.0; APIs MUST stay UI-ready for deferred browser clients.
- Contract-first schemas (OpenAPI 3.1 + protobuf + JSON sample payloads) SHALL gate every new behavior; `cargo xtask contract-test` (future) inherits these artifacts.

## Offline & Cache Expectations
- Offline parity: CLI MUST run for 8 hours offline with signed SQLite/LiteFS caches containing WorkflowRun excerpts, FeatureFlag snapshots, and Issue deltas (§1.4, §6.0).
- melvin 🤓 datum — Feature flag snapshots expire after 24 h or signature drift; CLI SHOULD trigger `b00t sync --flags` before invoking workflows when TTL <2 h (Section 1.4).
- Rehydration scripts SHALL verify ArtifactDepot signatures prior to replacing caches; conflicts bubble to OperatorConsole prompts referencing `trace_id`.

## Feature Flags & Telemetry Hooks
| Flag | Scope & Default | TTL & Runbook | Telemetry Hook |
| --- | --- | --- | --- |
| `flag.operator_console.react_ink_dashboards` | Gates React Ink overlays in OperatorConsole; default OFF production. | TTL 30d; runbook `runbooks/operator-console.md#react-ink` SHALL describe rollback. | Emit `cli.overlay.enabled` gauge + Prometheus label `flag_state`. |
| `flag.workflow.offline_queueing_v2` | Controls new offline queue reconciler + conflict detection in WorkflowOrchestrator. | TTL 21d; runbook `runbooks/workflow.md#offline` MUST link to Issue 223 for SLA context. | WorkflowRun events include `offline_queue_state`; traces send `workflow.offline.backlog` metric. |
| `flag.telemetry.dynamic_sampling` | Governs TelemetryHub adaptive sampling (X-Ray + Prometheus). | TTL 14d; runbook `runbooks/telemetry.md#sampling` references Issue 224 for retention guardrails. | TelemetryHub publishes `telemetry.sampling_ratio` metric + structured JSON logs. |
| `flag.skill_registry.signed_manifests` | Enforces signed manifest validation before CLI fetch. | TTL 28d; runbook `runbooks/skill-registry.md#signatures`. | CLI + orchestrator log `skill_registry.signature_status` with success/failure counts. |

Telemetry hooks MUST conform to `/metrics` format defined in §3.0 and propagate OpenTelemetry spans back to TelemetryHub (AWS X-Ray). Each feature flag state SHALL propagate through WorkflowRun DTO `feature_flags` arrays so audits remain deterministic.

## Governance, NRtW Controls, and Tooling
- Operators MUST invoke `just cli-smoke` before merging to prove Standard Kit parity in Docker Compose (§3.1).  
- Terraform + GitHub Actions remain the single provisioning path; manual drift is forbidden.  
- Secrets source from AWS SSM Parameter Store; `.envrc` is a rendered template used only for local dev.  
- Observability: Structured JSON logs, `/metrics` Prometheus endpoints, OpenTelemetry traces exported to AWS X-Ray with 100% sampling for failed workflows.  
- GitHub issues labeled `alignment-needed` capture assumption deltas; closing them REQUIRES doc updates plus references in CHANGELOG.

## Offline Assumptions & Operational Guides
- CLI queue buffering MUST store `x-idempotency-key` plus diff logs for replay; WorkflowOrchestrator rejects duplicates gracefully.  
- Signed snapshots older than 24 h MUST be rehydrated via ArtifactDepot; stale caches cause CLI to degrade into read-only mode until revalidated.  
- Operator laptops synchronize `.envrc` and Flagsmith deltas via `b00t sync` before entering offline windows; failure to sync is a blocking error surfaced via CLI telemetry.

## Risk Backlog (per §4.3)
1. **Operational complexity:** Mitigate with runbooks + `just` automation; track via KPIs ensuring onboarding stays ≤2 h.  
2. **Telemetry volume costs:** Controlled via `flag.telemetry.dynamic_sampling` plus Issue 224 guardrails; configure lifecycle policies at 30/90/365 day tiers.  
3. **Schema drift:** Enforce schema registry compat tests + additive-first policy; deviations require ADR with `gh` link.  
4. **Offline conflicts:** Rely on `flag.workflow.offline_queueing_v2` gating, signed manifests, and CLI prompts referencing `trace_id`.  
5. **AI latency variance:** Document fallback policies referencing Issue 223 and ensure TelemetryHub alerts fire when SLA >10 s interactive or >5 min batch.  
6. **Artifact provenance gaps:** ArtifactDepot requires cosign signatures + SBOM ingestion before release toggles turn ON.

## Open Questions (Mapped to `gh` Issues)
| Open Question | `gh` Issue | Notes |
| --- | --- | --- |
| Enumerate concrete OperatorConsole personas & CLI journeys beyond high-level roles. | [#222](https://github.com/elasticdotventures/_b00t_/issues/222) | Needs workflow catalog + TUI overlay acceptance tests. |
| Define AI adapter latency budgets + fallback behaviors for workflows that exceed Standard Kit SLIs. | [#223](https://github.com/elasticdotventures/_b00t_/issues/223) | Blocks Ops alert wiring + jitter policies. |
| Lock telemetry retention, sampling, and cost guardrails for Prometheus/OpenSearch + Glacier handoff. | [#224](https://github.com/elasticdotventures/_b00t_/issues/224) | Required before enabling `flag.telemetry.dynamic_sampling`. |

## Acceptance & Sign-off Path
- Validate via `just cli-smoke`, `cargo nextest`, and future `just contract-test` suites to ensure KPIs + feature flag hooks active.  
- OperatorConsole sign-off SHALL be recorded on [Issue #225 comment](https://github.com/elasticdotventures/_b00t_/issues/225#issuecomment-3830314054); Operator response MUST confirm alignment before toggling any feature flag ON.  
- Update `architecture-status` changelog after each iteration with summarized deltas (excerpt maintained separately under `docs/architecture/architecture-status-excerpt.md`).  
- Once OperatorConsole records sign-off, close each alignment-needed issue with links to updated docs and telemetry dashboards.
