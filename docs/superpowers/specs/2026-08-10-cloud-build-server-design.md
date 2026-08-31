# Design: Standby Cloud Build Server (GCP dstack dev-environment)

**Status**: Implemented (`dev-env/*.yaml` + `just remote-*` recipes) — not yet applied to real GCP infra; that's an explicit operator decision (cost/backend/region), not something implementing the config itself commits to.
**Motivated by**: this local machine's build times — a cold `cargo build` for `b00t-c0re-lib`
alone took 13+ minutes, and `cargo run` on `b00t-cli` appeared to trigger a full
dependency-tree rebuild on effectively every invocation (Qdrant, embed-anything, candle,
kube, OpenTelemetry, and more), turning a single integration test run into a
multi-minute-to-tens-of-minutes wait.

## Context

b00t already has a working multi-backend cloud compute layer: `DstackProvider`
(`b00t-cli/src/commands/provider.rs`, PR #857) implements the `ComputeProvider` trait —
`submit_batch_job`, `submit_training_job`, `job_status`, `cancel_job`, `list_jobs` — by
shelling out to the `dstack` CLI and generating fleet/task YAML without hardcoding a
backend. GCP was added as a second backend (`docs/superpowers/specs/2026-07-23-b00t-dstack-gcp-backend-design.md`)
using ambient Application Default Credentials already configured on this host — no
secrets to manage.

**Verified directly against the code before designing** (not assumed): `ensure_fleet`
generates a `type: fleet` config (`nodes: 0..1`, autoscaling to zero on idle) and
`dstack_task_yaml` generates a `type: task` config that runs a fixed Docker `image:` to
completion. This is entirely batch-job shaped — there is no persistent, SSH-connectable
"dev box" capability anywhere in b00t's dstack integration today. dstack itself supports
that mode natively (`type: dev-environment`), but b00t never generates that config type.
This design adds it, as static config + `just` recipes — no new Rust code.

## Goals

1. A GCP-backed dstack dev-environment that stays SSH-reachable, auto-stops after an
   idle period (no GPU — this is a CPU/disk/network build server, not an ML job), and
   resumes without losing its build cache.
2. Sync via `git push` to a scratch branch on the existing `origin` (GitHub) remote —
   the box fetches and checks it out. No rsync, no reverse-SSH into a NAT'd local
   machine.
3. Day-to-day invocation via `just` recipes wrapping raw `dstack`/`ssh`/`git` calls —
   deliberately no new `b00t-cli` subcommand (YAGNI: this is ops tooling, not a feature).
4. The actual point: a persistent `type: volume` backs `/data/b00t` (checkout +
   `target/`), so idle-stop/resume does not force a cold rebuild. Without this, the
   design would just relocate today's pain to a faster machine, not solve it.

## Non-goals

- No new `b00t-cli` command surface (explicit choice — `just` recipes only).
- Not routed through the existing `ComputeProvider`/batch-job path (`submit_batch_job`,
  `job_status` polling, PASS/FAIL contract parsing) — that machinery is for discrete,
  container-image-shaped jobs with no persistent state; this is an interactive box you
  build/test against directly over SSH. Building a `type: dev-environment` generator
  into `DstackProvider` itself was considered and explicitly declined in favor of static
  YAML + `just`, to keep this out of Rust entirely.
- No GPU, no RunPod backend (GCP chosen: ambient ADC already configured, no secrets).
- No automatic multi-user/multi-branch concurrency handling — one box, one active
  checkout, one scratch branch at a time. If that becomes a real need, it's a follow-up.

## Architecture

One dstack `type: dev-environment` (`name: b00t-build`) provisioned against a backing
`type: fleet` (`b00t-build-fleet`) on the GCP backend, no GPU. **Correction from this
design's first draft, made while implementing it**: `idle_duration` belongs on the
*fleet*, not the dev-environment — verified against dstack's real fleet/dev-environment
reference docs, not assumed. dstack also requires a fleet to exist before a
dev-environment can be provisioned against it at all, which the original draft didn't
account for; `b00t-build-fleet.yaml` (`nodes: 0..1`, `idle_duration: 30m`) is new since
that draft. The 30-minute idle window auto-stops the instance (long enough to survive a
short break mid-iteration, short enough not to burn cost overnight if forgotten). A
dstack `type: volume` (`b00t-build-cache`, GCP persistent disk) is mounted at
`/data/b00t` and holds both the git checkout and `target/`. The volume's lifecycle is
independent of the fleet's — idle-stop kills compute, not the disk, so the cargo cache
is warm again the moment the box resumes.

Sync is git-native: `just remote-push <branch>` pushes local HEAD to
`refs/heads/scratch/<branch>` on the same `origin` GitHub remote already used for
everything else. `just remote-build <branch>` / `just remote-test <branch>` SSH in
synchronously (`ssh b00t-build "cd /data/b00t && git fetch origin && git checkout
scratch/<branch> && cargo build|test"`), streaming output live to the local terminal
over the SSH session itself — no polling, no separate result-fetch step, no PASS/FAIL
contract to parse (that machinery belongs to the batch-job path this design deliberately
avoids).

## Components

- `_b00t_/dev-env/b00t-build-cache.volume.yaml` — `type: volume`, GCP, 100GB (this
  session's `target/` alone reached ~2GB partway through a single crate's build; the
  full workspace dependency tree — Qdrant, candle, kube, OpenTelemetry, etc. — is
  plausibly 10-50GB; 100GB leaves headroom at GCP persistent-disk pricing that's cheap
  relative to the time this design saves).
- `_b00t_/dev-env/b00t-build-fleet.yaml` — `type: fleet`, `nodes: 0..1`,
  `resources: gpu: 0`, `idle_duration: 30m`. No `regions:` restriction (matches the
  existing fleet/task YAML's own "don't hardcode more than necessary" convention) — GCP
  scoping comes from `b00t-build-cache`'s own `backend: gcp`, which only a GCP-scheduled
  instance can attach.
- `_b00t_/dev-env/b00t-build.dev-environment.yaml` — `type: dev-environment`,
  `name: b00t-build`, no `ide:` (SSH-only), no `resources:` override (inherits the
  fleet's), `volumes: [{name: b00t-build-cache, path: /data/b00t}]`.
- `justfile` recipes (implemented):
  - `remote-provision` — idempotent `dstack apply -y` for the fleet, then the volume,
    then the dev-environment (fleet must exist first).
  - `remote-push <branch>` — `git push origin HEAD:refs/heads/scratch/<branch>`.
  - `remote-build <branch>` / `remote-test <branch>` — SSH in, fetch, checkout, run.
  - `remote-stop` — manual `dstack stop b00t-build`, on top of the automatic
    idle-duration stop (belt and suspenders — explicit stop for "I'm done for the day"
    vs. the idle timer for "forgot to").

## Data flow

```
just remote-provision                 (one-time / idempotent)
just remote-push my-branch            → origin:refs/heads/scratch/my-branch
just remote-test my-branch            → ssh b00t-build:
                                           cd /data/b00t
                                           git fetch origin
                                           git checkout scratch/my-branch
                                           cargo test
                                         (streamed live to local terminal)
```

## Error handling

The very first `remote-build`/`remote-test` after `remote-provision` is genuinely cold —
a real full dependency compile, several minutes, expected and unavoidable exactly once.
Every subsequent call is warm (only actually-changed crates recompile) because
`/data/b00t` persists on the volume independent of VM stop/start. If `remote-build` runs
while the box is idle-stopped, `dstack apply`/`ssh`'s own resume handling brings it back
up — no bespoke wake logic needed in the recipes. If the volume is ever deleted and
recreated, the next build is cold again by construction; this is a documented, accepted
cost, not a failure mode to engineer around.

## Testing

This is ops tooling, not application code — the acceptance criterion is a manual
end-to-end smoke test, not a unit-test suite:

1. `just remote-provision` succeeds, `b00t-build` is SSH-reachable.
2. `just remote-push test-branch` then `just remote-test test-branch` succeeds and shows
   real `cargo test` output over the SSH session.
3. Let the box idle-stop (wait past `idle_duration`), then run `just remote-test
   test-branch` again. **This is the real proof**: confirm it does *not* trigger a full
   dependency rebuild — the warm-cache-across-stop/resume behavior is the entire point
   of this design, and the design is not proven until this specific step is observed to
   work.
