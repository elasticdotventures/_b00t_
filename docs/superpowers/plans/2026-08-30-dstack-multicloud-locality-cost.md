# dstack Multi-Cloud Locality + Cost Accounting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Finish wiring GCP into dstack's multi-cloud backend config, add AWS and Azure alongside it, and add data-locality-aware job placement plus startup-time-inclusive cost accounting on top of the existing `DstackProvider`/`.job.toml` job system.

**Architecture:** `DstackProvider` (`b00t-cli/src/commands/provider.rs`) already shells out to the `dstack` CLI, which natively supports RunPod/GCP/AWS/Azure as backends in one `~/.dstack/server/config.yml`. This plan extends the existing config-generation recipe (no new Rust provider), extends `BatchJobSpec`/the YAML builders with locality/interruptibility hints, adds a new pure-function placement module, and instruments `job_executor.rs` to emit cost/timing events that `ledgrrr`'s existing `OperationKind::RecordCost` mechanism ingests.

**Tech Stack:** Rust (`b00t-cli`, `ledgrrr/crates/ledger-core`), `dstack` CLI, `just` recipes, TOML datums.

**Spec:** `docs/superpowers/specs/2026-08-30-dstack-multicloud-locality-cost-design.md` (PR elasticdotventures/_b00t_#1207)

## Global Constraints

- No new `PROVIDER-*` datum and no new `ComputeProvider` implementation — GCP/AWS/Azure are dstack backends, not separate b00t providers (spec "Supersedes" section).
- RunPod is excluded from any ephemeral-runner-style use in this work (standing no-Pod-TTL guidance) — untouched by this plan.
- `dependencies: Vec<String>` is an intentionally simple placeholder — do not generalize it into a typed `Dependency<'a, C: Constraint>` here; that belongs in the deferred `ufo-types` DAG follow-on.
- No live cloud spend in any task below — config generation and read-only `dstack offer` queries only; a real job submission against AWS/Azure is a separate, explicitly-gated step outside this plan.
- Each epoch below is one stacked PR: dedicated worktree off the previous epoch's merged branch (or `main` for Epoch 1) → implement its tasks → verify locally → commit per task → push → PR referencing #1207 and the previous epoch's PR → monitor CI → merge → clean up worktree, before starting the next epoch.

---

## Epoch 1: Wire GCP + add AWS/Azure dstack backend config (docs/config only, no Rust)

**Files:**
- Modify: `_b00t_/justfile-dstack-sdd.just` (the `dstack-server-config` recipe, currently only emits a `runpod` backend block)
- Modify: `_b00t_/PROVIDER-DSTACK.provider.toml` (documentation of new backends + their credential model)

**Interfaces:**
- Produces: a `~/.dstack/server/config.yml` with `backends:` entries for `runpod`, `gcp`, `aws`, `azure` — consumed as-is by the existing, unmodified `DstackProvider` (no Rust changes in this epoch).

### Task 1: Add the GCP backend block (already approved 2026-07-23, never wired in)

- [ ] **Step 1: Write a shell assertion test for the recipe's current (runpod-only) output**

Create `_b00t_/tests/dstack-server-config.bats` (bats-core; check `which bats` first — if absent, install via `npm install -g bats` or the project's existing bats convention, confirm at implementation time which this repo already uses):

```bash
#!/usr/bin/env bats

setup() {
  export HOME="$(mktemp -d)"
  export RUNPOD_API_KEY="test-runpod-key"
  export GCP_PROJECT_ID="test-gcp-project"
  cd "$BATS_TEST_DIRNAME/.."
}

@test "dstack-server-config writes a runpod backend block" {
  echo "RUNPOD_API_KEY=$RUNPOD_API_KEY" > .env
  just dstack-server-config
  grep -q "type: runpod" ~/.dstack/server/config.yml
}

@test "dstack-server-config writes a gcp backend block using ambient ADC" {
  echo "RUNPOD_API_KEY=$RUNPOD_API_KEY" > .env
  echo "GCP_PROJECT_ID=$GCP_PROJECT_ID" >> .env
  just dstack-server-config
  grep -q "type: gcp" ~/.dstack/server/config.yml
  grep -q "project_id: \"$GCP_PROJECT_ID\"" ~/.dstack/server/config.yml
  grep -q "type: default" ~/.dstack/server/config.yml
}
```

- [ ] **Step 2: Run it to verify the GCP test fails**

Run: `cd _b00t_ && bats tests/dstack-server-config.bats`
Expected: the `runpod` test PASSes (recipe already does this), the `gcp` test FAILs (no `type: gcp` in output yet).

- [ ] **Step 3: Extend the recipe to add the GCP block**

In `_b00t_/justfile-dstack-sdd.just`, modify the `dstack-server-config` recipe body (replace the single `printf` call with one that appends a second backend entry):

```just
dstack-server-config:
    #!/bin/bash
    set -euo pipefail
    root="$(git rev-parse --show-toplevel)"
    set -a
    source "$root/.env"
    set +a
    : "${RUNPOD_API_KEY:?RUNPOD_API_KEY not set in $root/.env — see _b00t_/learn/managing-secrets.md}"
    GCP_PROJECT_ID="${GCP_PROJECT_ID:-app4dog}"
    mkdir -p ~/.dstack/server
    umask 077
    printf '%s\n' \
        'projects:' \
        '  - name: b00t' \
        '    backends:' \
        '      - type: runpod' \
        '        creds:' \
        '          type: api_key' \
        "          api_key: \"$RUNPOD_API_KEY\"" \
        '      - type: gcp' \
        "        project_id: \"$GCP_PROJECT_ID\"" \
        '        creds:' \
        '          type: default' \
        > ~/.dstack/server/config.yml
    chmod 600 ~/.dstack/server/config.yml
    echo "Wrote ~/.dstack/server/config.yml (mode 600, not git-tracked, key sourced from .env)"
```

- [ ] **Step 4: Run the bats tests again to verify both pass**

Run: `cd _b00t_ && bats tests/dstack-server-config.bats`
Expected: both tests PASS.

- [ ] **Step 5: Verify the generated YAML round-trips through dstack's own pydantic models**

Run (per the GCP design's own verification method):
```bash
just dstack-server-config
python3 -c "
import yaml
from dstack._internal.server.services.config import ServerConfig
with open('$HOME/.dstack/server/config.yml') as f:
    raw = yaml.safe_load(f)
cfg = ServerConfig(**raw)
print('OK:', [b.type for p in cfg.projects for b in p.backends])
"
```
Expected: prints `OK: ['runpod', 'gcp']` with no validation error. If `dstack` isn't installed in this environment, note this as a manual verification step to run once before merging the epoch's PR — do not skip it silently.

- [ ] **Step 6: Commit**

```bash
git add _b00t_/justfile-dstack-sdd.just _b00t_/tests/dstack-server-config.bats
git commit -m "feat(dstack): wire the approved GCP backend into dstack-server-config"
```

### Task 2: Add the AWS backend block

- [ ] **Step 1: Verify AWS's actual dstack credential model before writing code**

Run against the installed dstack version (same method Task 1 / the GCP design used):
```bash
python3 -c "
import dstack._internal.core.backends.aws.models as m
import inspect
print(inspect.getsource(m))
" | head -80
```
Read the output. Confirm whether an ambient-credential type exists (e.g. a `AWSDefaultCreds`/profile-based type analogous to GCP's `GCPDefaultCreds`) or whether AWS requires explicit `access_key`/`secret_key` fields. **This resolves Open Question 1 from the spec — do not guess.** Write down the actual field names found; Step 3 below uses them.

- [ ] **Step 2: Write a failing bats test for the AWS block**

Add to `_b00t_/tests/dstack-server-config.bats`:

```bash
@test "dstack-server-config writes an aws backend block" {
  echo "RUNPOD_API_KEY=$RUNPOD_API_KEY" > .env
  just dstack-server-config
  grep -q "type: aws" ~/.dstack/server/config.yml
}
```

Run: `bats tests/dstack-server-config.bats` — expect this new test to FAIL.

- [ ] **Step 3: Extend the recipe with the AWS block**

Add the AWS backend entry to the same `printf` block in `dstack-server-config`, using the credential shape confirmed in Step 1. If AWS supports ambient credentials (e.g. from `~/.aws/credentials` / `AWS_PROFILE`), mirror GCP's `creds: { type: default }` pattern; if it requires explicit keys, source them from `.env` the same way `RUNPOD_API_KEY` is sourced (never hand-authored, never committed) — follow whichever the Step 1 findings actually show, do not assume.

- [ ] **Step 4: Run bats + the pydantic round-trip verification again**

Run: `bats tests/dstack-server-config.bats` (all tests pass) and the Step 5-equivalent Python round-trip from Task 1, expecting `['runpod', 'gcp', 'aws']`.

- [ ] **Step 5: Commit**

```bash
git add _b00t_/justfile-dstack-sdd.just _b00t_/tests/dstack-server-config.bats
git commit -m "feat(dstack): add AWS backend to dstack-server-config"
```

### Task 3: Add the Azure backend block

- [ ] **Step 1: Verify Azure's actual dstack credential model**

Same method as Task 2 Step 1, against `dstack._internal.core.backends.azure.models`. Write down the actual field names.

- [ ] **Step 2: Write a failing bats test, extend the recipe, verify, commit**

Repeat Task 2's Steps 2-5 for Azure (test name `"dstack-server-config writes an azure backend block"`, grep for `"type: azure"`, final round-trip expecting `['runpod', 'gcp', 'aws', 'azure']`).

```bash
git add _b00t_/justfile-dstack-sdd.just _b00t_/tests/dstack-server-config.bats
git commit -m "feat(dstack): add Azure backend to dstack-server-config"
```

### Task 4: Document all three new/completed backends in `PROVIDER-DSTACK.provider.toml`

- [ ] **Step 1: Update the datum's header comment and `[resource.secrets.*]` tables**

Edit `_b00t_/PROVIDER-DSTACK.provider.toml`:
- Update the file's top comment (currently says "b00t uses it as the provider-agnostic backend so RunPod... is optional") to mention GCP/AWS/Azure are now configured, not just RunPod.
- Add `[resource.secrets.gcp_credentials]`, mirroring the existing `[resource.secrets.runpod_api_key]` shape but with `required = false`, `source_provider = "gcloud-adc"`, `delivery_type = "ambient"` (per the already-approved GCP design's own text — copy it in, it was written and approved but never landed in this file either).
- Add `[resource.secrets.aws_credentials]` and `[resource.secrets.azure_credentials]` tables using the actual credential shape found in Task 2/3 Step 1 (ambient vs `.env`-sourced — whichever applies).

- [ ] **Step 2: Validate the TOML parses**

Run: `python3 -c "import tomllib; tomllib.load(open('_b00t_/PROVIDER-DSTACK.provider.toml', 'rb'))"`
Expected: no exception.

- [ ] **Step 3: Commit**

```bash
git add _b00t_/PROVIDER-DSTACK.provider.toml
git commit -m "docs(dstack): document gcp/aws/azure backend credentials in PROVIDER-DSTACK datum"
```

### Epoch 1 wrap-up

- [ ] Push the branch, open a PR titled `feat(dstack): wire GCP/AWS/Azure backends into dstack-server-config`, body referencing `#1207` (the spec PR) as "Epoch 1 of 4".
- [ ] Wait for CI, merge, delete the worktree.

---

## Epoch 2: `BatchJobSpec` locality/interruptibility fields + YAML emission

**Depends on:** Epoch 1 merged (branch off its resulting `main`).

**Files:**
- Modify: `b00t-cli/src/commands/provider.rs:94-112` (`BatchJobSpec` struct)
- Modify: `b00t-cli/src/commands/provider.rs:716-770` (`dstack_fleet_yaml`, `dstack_task_yaml`)
- Test: same file's existing `#[cfg(test)]` module (`dstack_fleet_yaml_is_autoscaling_zero_to_one_with_gpu`, `dstack_task_yaml_includes_image_env_and_command`, etc. — extend, don't replace)

**Interfaces:**
- Consumes: nothing new from Epoch 1 (config-only, no Rust interface).
- Produces: `BatchJobSpec.dependencies: Vec<String>`, `.interruptible: bool`, `.backend_hint: Option<String>`, `.region_hint: Option<String>` — consumed by Epoch 3's `placement.rs` (which sets `backend_hint`/`region_hint` before submission) and by `dstack_fleet_yaml`/`dstack_task_yaml` (which read all four).

### Task 1: Add the four new fields to `BatchJobSpec`, all defaulted

- [ ] **Step 1: Write a failing test asserting the new fields exist and default correctly**

Add to the existing test module in `b00t-cli/src/commands/provider.rs`:

```rust
#[test]
fn batch_job_spec_new_fields_default_to_empty_and_false() {
    let spec = BatchJobSpec {
        image: "test:latest".into(),
        config_path: "/tmp/config.json".into(),
        env: Default::default(),
        flavor: "cpu".into(),
        timeout_hours: 1.0,
        gpu_count: 1,
        volumes: vec![],
        inputs: vec![],
        dependencies: vec![],
        interruptible: false,
        backend_hint: None,
        region_hint: None,
    };
    assert!(spec.dependencies.is_empty());
    assert!(!spec.interruptible);
    assert_eq!(spec.backend_hint, None);
    assert_eq!(spec.region_hint, None);
}
```

- [ ] **Step 2: Run it to verify it fails to compile** (fields don't exist yet)

Run: `CARGO_TARGET_DIR=~/cargo-target-scratch cargo test -p b00t-cli batch_job_spec_new_fields_default_to_empty_and_false`
Expected: compile error, "no field `dependencies` on type `BatchJobSpec`".

- [ ] **Step 3: Add the fields**

In `b00t-cli/src/commands/provider.rs`, modify the struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchJobSpec {
    pub image: String,
    pub config_path: String,
    pub env: std::collections::HashMap<String, String>,
    pub flavor: String,
    pub timeout_hours: f32,
    #[serde(default = "default_gpu_count")]
    pub gpu_count: u32,
    #[serde(default)]
    pub volumes: Vec<VolumeMount>,
    #[serde(default)]
    pub inputs: Vec<String>,
    /// Dataset/resource URIs this job needs — consumed by placement.rs to pick
    /// a backend_hint/region_hint with matching data residency. Deliberately a
    /// plain string list (see the design doc's forward-compatibility note —
    /// not a generic typed Dependency<C: Constraint>; that belongs in the
    /// deferred ufo-types DAG work).
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Hint that this job tolerates spot/preemptible compute.
    #[serde(default)]
    pub interruptible: bool,
    /// Resolved by placement.rs; None means "let dstack pick from whatever
    /// backends the operator's config.yml has configured" (today's behavior).
    #[serde(default)]
    pub backend_hint: Option<String>,
    #[serde(default)]
    pub region_hint: Option<String>,
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `CARGO_TARGET_DIR=~/cargo-target-scratch cargo test -p b00t-cli batch_job_spec_new_fields_default_to_empty_and_false`
Expected: PASS.

- [ ] **Step 5: Run the full existing test suite to confirm no regression**

Run: `CARGO_TARGET_DIR=~/cargo-target-scratch cargo test -p b00t-cli commands::provider`
Expected: all existing tests still PASS (the four new fields are all `#[serde(default)]`, so every existing `BatchJobSpec` literal in the test module needs updating to include them — do that now for each one that fails to compile, following the same pattern as Step 3's test).

- [ ] **Step 6: Commit**

```bash
git add b00t-cli/src/commands/provider.rs
git commit -m "feat(provider): add dependencies/interruptible/backend_hint/region_hint to BatchJobSpec"
```

### Task 2: Emit `backends:`/`regions:`/`spot_policy`/`inactivity_duration` in the YAML builders

- [ ] **Step 1: Write failing tests for the new YAML output**

Add to the existing test module:

```rust
#[test]
fn dstack_fleet_yaml_omits_backends_and_spot_policy_when_no_hint() {
    let yaml = dstack_fleet_yaml("test-fleet", 1);
    assert!(!yaml.contains("backends:"));
    assert!(!yaml.contains("spot_policy"));
}

#[test]
fn dstack_fleet_yaml_includes_backend_and_region_when_hinted() {
    let yaml = dstack_fleet_yaml_hinted("test-fleet", 1, Some("aws"), Some("us-east-1"), false);
    assert!(yaml.contains("backends: [aws]"));
    assert!(yaml.contains("regions: [us-east-1]"));
    assert!(!yaml.contains("spot_policy"));
}

#[test]
fn dstack_fleet_yaml_sets_spot_policy_and_inactivity_duration_when_interruptible() {
    let yaml = dstack_fleet_yaml_hinted("test-fleet", 1, None, None, true);
    assert!(yaml.contains("spot_policy: auto"));
    assert!(yaml.contains("idle_duration:"));
}
```

- [ ] **Step 2: Run to verify failure**

Run: `CARGO_TARGET_DIR=~/cargo-target-scratch cargo test -p b00t-cli dstack_fleet_yaml`
Expected: FAIL — `dstack_fleet_yaml_hinted` doesn't exist yet.

- [ ] **Step 3: Implement `dstack_fleet_yaml_hinted` and route the existing call site through it**

```rust
/// Extends `dstack_fleet_yaml` with an optional backend/region hint (from
/// placement.rs) and a spot-policy/idle-duration block when the job tolerates
/// interruption. `dstack_fleet_yaml` becomes a thin wrapper calling this with
/// all-None/false, preserving today's "match whatever's configured" default.
fn dstack_fleet_yaml_hinted(
    name: &str,
    gpu_count: u32,
    backend_hint: Option<&str>,
    region_hint: Option<&str>,
    interruptible: bool,
) -> String {
    let mut out = format!("type: fleet\nname: {name}\nnodes: 0..1\nresources:\n  gpu: {gpu_count}\n");
    if let Some(b) = backend_hint {
        out.push_str(&format!("backends: [{b}]\n"));
    }
    if let Some(r) = region_hint {
        out.push_str(&format!("regions: [{r}]\n"));
    }
    if interruptible {
        out.push_str("spot_policy: auto\nidle_duration: 5m\n");
    }
    out
}

fn dstack_fleet_yaml(name: &str, gpu_count: u32) -> String {
    dstack_fleet_yaml_hinted(name, gpu_count, None, None, false)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `CARGO_TARGET_DIR=~/cargo-target-scratch cargo test -p b00t-cli dstack_fleet_yaml`
Expected: all PASS, including the pre-existing `dstack_fleet_yaml_is_autoscaling_zero_to_one_with_gpu` (unchanged behavior for the no-hint case).

- [ ] **Step 5: Repeat Steps 1-4 for `dstack_task_yaml`**, adding a `dstack_task_yaml_hinted` that emits the same `backends:`/`regions:` lines (task-level, not just fleet-level — dstack tasks can also carry backend/region constraints; verify this against the installed dstack's `TaskConfiguration` model before assuming task-level support mirrors fleet-level, same verification discipline the existing code comments already establish for this function). Wire `submit_batch_job`/`submit_training_job` to pass `spec.backend_hint`/`spec.region_hint`/`spec.interruptible` through to whichever `_hinted` function each calls.

- [ ] **Step 6: Commit**

```bash
git add b00t-cli/src/commands/provider.rs
git commit -m "feat(provider): emit backend/region hints and spot policy in dstack YAML builders"
```

### Epoch 2 wrap-up

- [ ] Push, open PR `feat(provider): BatchJobSpec locality/interruptibility fields + dstack YAML emission`, referencing #1207 and Epoch 1's merged PR, as "Epoch 2 of 4".
- [ ] Wait for CI, merge, delete the worktree.

---

## Epoch 3: `placement.rs` — data-locality-aware backend/region resolution

**Depends on:** Epoch 2 merged.

**Files:**
- Create: `b00t-cli/src/placement.rs`
- Modify: `b00t-cli/src/lib.rs` (or wherever modules are registered — add `mod placement;`)
- Modify: `_b00t_/PROVIDER-DSTACK.provider.toml` (add `[[resource.data_residency]]` entries)
- Modify: `b00t-cli/src/job_executor.rs` (`dispatch_batch_job` calls placement before `submit_batch_job`)

**Interfaces:**
- Consumes: `BatchJobSpec.dependencies` (Epoch 2), `[[resource.data_residency]]` table read from `PROVIDER-DSTACK.provider.toml` (new).
- Produces: `pub fn resolve_placement(spec: &BatchJobSpec, residency: &[DataResidencyEntry]) -> Result<Placement>` — a pure function `dispatch_batch_job` calls to fill in `spec.backend_hint`/`spec.region_hint` before submission.

### Task 1: Define the data-residency table shape (resolves spec Open Question 3)

- [ ] **Step 1: Add `[[resource.data_residency]]` entries to `PROVIDER-DSTACK.provider.toml`**

Per the spec's lean toward the list form (a backend can span multiple regions with different local datasets):

```toml
[[resource.data_residency]]
backend  = "aws"
region   = "us-east-1"
datasets = ["s3://app4dog-ml-training/*"]

[[resource.data_residency]]
backend  = "gcp"
region   = "us-central1"
datasets = ["gs://app4dog-ml-training/*"]
```

Leave empty (no entries) if no real dataset locations are known yet at this point — the placement function below must handle zero entries correctly (falls through to "no hint", not an error, since an empty residency table isn't the same as a job with unmatched dependencies).

- [ ] **Step 2: Commit this doc-only change on its own**

```bash
git add _b00t_/PROVIDER-DSTACK.provider.toml
git commit -m "docs(dstack): add data_residency table shape to PROVIDER-DSTACK datum"
```

### Task 2: Implement `resolve_placement`

- [ ] **Step 1: Write failing tests**

Create `b00t-cli/src/placement.rs`:

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct DataResidencyEntry {
    pub backend: String,
    pub region: String,
    pub datasets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    pub backend: String,
    pub region: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PlacementError {
    #[error("no configured backend declares data residency for: {0:?}")]
    NoResidencyMatch(Vec<String>),
}

/// Matches each of `dependencies` against every `residency` entry's `datasets`
/// globs (simple prefix match today — a dataset entry like `s3://bucket/*`
/// matches any dependency URI starting with `s3://bucket/`). Returns `Ok(None)`
/// when `dependencies` is empty (nothing to place against) or when
/// `residency` is empty (no locality info configured yet — falls through to
/// dstack's own default backend selection, today's behavior). Returns an
/// error naming the unmatched dependency when at least one dependency exists
/// but no residency entry covers it — per the design's "never silently
/// dispatch cross-cloud" rule.
pub fn resolve_placement(
    dependencies: &[String],
    residency: &[DataResidencyEntry],
) -> Result<Option<Placement>, PlacementError> {
    if dependencies.is_empty() || residency.is_empty() {
        return Ok(None);
    }

    let mut unmatched = Vec::new();
    for dep in dependencies {
        let hit = residency.iter().find(|entry| {
            entry.datasets.iter().any(|glob| {
                let prefix = glob.trim_end_matches('*');
                dep.starts_with(prefix)
            })
        });
        match hit {
            Some(entry) => {
                return Ok(Some(Placement {
                    backend: entry.backend.clone(),
                    region: entry.region.clone(),
                }));
            }
            None => unmatched.push(dep.clone()),
        }
    }
    Err(PlacementError::NoResidencyMatch(unmatched))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(backend: &str, region: &str, dataset: &str) -> DataResidencyEntry {
        DataResidencyEntry {
            backend: backend.into(),
            region: region.into(),
            datasets: vec![dataset.into()],
        }
    }

    #[test]
    fn no_dependencies_returns_no_placement() {
        let residency = vec![entry("aws", "us-east-1", "s3://bucket/*")];
        assert_eq!(resolve_placement(&[], &residency).unwrap(), None);
    }

    #[test]
    fn no_residency_configured_returns_no_placement() {
        let deps = vec!["s3://bucket/file".to_string()];
        assert_eq!(resolve_placement(&deps, &[]).unwrap(), None);
    }

    #[test]
    fn matching_dependency_resolves_backend_and_region() {
        let residency = vec![entry("aws", "us-east-1", "s3://app4dog-ml-training/*")];
        let deps = vec!["s3://app4dog-ml-training/dataset-x/file.parquet".to_string()];
        let placement = resolve_placement(&deps, &residency).unwrap().unwrap();
        assert_eq!(placement.backend, "aws");
        assert_eq!(placement.region, "us-east-1");
    }

    #[test]
    fn unmatched_dependency_errors_naming_the_uri() {
        let residency = vec![entry("aws", "us-east-1", "s3://app4dog-ml-training/*")];
        let deps = vec!["gs://some-other-bucket/file".to_string()];
        let err = resolve_placement(&deps, &residency).unwrap_err();
        match err {
            PlacementError::NoResidencyMatch(uris) => {
                assert_eq!(uris, vec!["gs://some-other-bucket/file".to_string()]);
            }
        }
    }
}
```

- [ ] **Step 2: Run to verify it fails to compile** (module not registered yet)

Run: `CARGO_TARGET_DIR=~/cargo-target-scratch cargo test -p b00t-cli placement`
Expected: FAIL — module not found.

- [ ] **Step 3: Register the module**

Find where `b00t-cli`'s modules are declared (run `grep -n "^mod \|^pub mod " b00t-cli/src/lib.rs` to confirm the exact file and existing pattern first) and add `pub mod placement;` following the same style as neighboring module declarations.

- [ ] **Step 4: Run tests to verify they pass**

Run: `CARGO_TARGET_DIR=~/cargo-target-scratch cargo test -p b00t-cli placement`
Expected: all 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add b00t-cli/src/placement.rs b00t-cli/src/lib.rs
git commit -m "feat(placement): add resolve_placement for data-locality-aware backend/region hints"
```

### Task 3: Wire `resolve_placement` into `dispatch_batch_job`

- [ ] **Step 1: Write a failing test** using the existing fake-provider pattern in `job_executor.rs`'s test module (find it via `grep -n "impl ComputeProvider for FakeProvider" b00t-cli/src/job_executor.rs` and read the surrounding test setup before writing this, so the new test matches its existing construction pattern exactly rather than guessing).

The test should: construct a `JobStep` whose `batch.dependencies` matches a residency entry loaded from a test fixture `PROVIDER-DSTACK.provider.toml`-shaped TOML string, call `dispatch_batch_job`, and assert the `FakeProvider`'s `submit_batch_job` was called with a spec whose `backend_hint`/`region_hint` are populated (extend `FakeProvider` to record the last spec it was called with, if it doesn't already — check first).

- [ ] **Step 2: Run to verify it fails**

Run: `CARGO_TARGET_DIR=~/cargo-target-scratch cargo test -p b00t-cli dispatch_batch_job`

- [ ] **Step 3: Implement the wiring**

In `dispatch_batch_job` (`job_executor.rs:570`), before calling `provider.submit_batch_job(spec)`, load the residency table (read `PROVIDER-DSTACK.provider.toml`'s `[[resource.data_residency]]` — reuse whatever existing datum-loading helper this codebase already has for reading `PROVIDER-*.provider.toml` files; check `grep -rn "provider.toml\|fn load_datum" b00t-cli/src/` first rather than writing a second TOML-reading path), call `placement::resolve_placement(&spec.dependencies, &residency)`, and if it resolves a `Placement`, clone the spec with `backend_hint`/`region_hint` set before submission. If `resolve_placement` errors, propagate the error (this is the "never silently dispatch cross-cloud" behavior — a job with unmatched dependencies must fail loudly here, not fall through).

- [ ] **Step 4: Run tests to verify they pass, then run the full `b00t-cli` suite**

Run: `CARGO_TARGET_DIR=~/cargo-target-scratch cargo test -p b00t-cli`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add b00t-cli/src/job_executor.rs
git commit -m "feat(job-executor): resolve placement before dstack job submission"
```

### Epoch 3 wrap-up

- [ ] Push, open PR `feat(placement): data-locality-aware backend/region resolution`, referencing #1207 and Epoch 2's merged PR, as "Epoch 3 of 4".
- [ ] Wait for CI, merge, delete the worktree.

---

## Epoch 4: Startup-time-inclusive cost accounting via ledgrrr

**Depends on:** Epoch 3 merged. This epoch touches two repos: `_b00t_` (b00t-cli) and `ledgrrr` (ledger-core) — two separate PRs, opened together, cross-referencing each other.

**Files:**
- Modify: `b00t-cli/src/job_executor.rs` (`poll_until_terminal`, `dispatch_batch_job`)
- Create: `b00t-cli/src/job_utilization.rs` (`JobUtilizationEvent` + outbox writer)
- Modify: `ledgrrr/crates/ledger-core/src/ledger_ops.rs` (`OperationKind::RecordCost`, `RecordCostOp`)

**Interfaces:**
- Consumes: nothing new from Epoch 3 beyond `dispatch_batch_job`'s existing structure.
- Produces: `JobUtilizationEvent { job_id, provider, cold_start_seconds, run_seconds, estimated_cost }`, appended as JSON lines to a local outbox file; `RecordCostOp`/`OperationKind::RecordCost` gain two new optional fields consumed by whatever ingests that outbox (a follow-up outside this plan's scope — see Task 2's note).

### Task 1: Instrument `job_executor.rs` to record cold-start/run durations

- [ ] **Step 1: Write a failing test**

In `b00t-cli/src/job_executor.rs`'s test module, using the existing `FakeProvider`-with-scripted-statuses pattern (`with_statuses`, seen in the existing `poll_until_terminal_allows_long_pending_but_short_running_budget` test — read it first to match its construction exactly):

```rust
#[tokio::test]
async fn dispatch_batch_job_emits_utilization_event_with_separate_durations() {
    let provider = FakeProvider::with_statuses(vec![
        "status=pending".into(),
        "status=pending".into(),
        "status=running".into(),
        "status=done".into(),
    ]);
    let step = JobStep {
        name: "test-step".into(),
        depends_on: vec![],
        backend: Some("dstack".into()),
        batch: Some(BatchJobSpec {
            image: "test:latest".into(),
            config_path: "/tmp/x.json".into(),
            env: Default::default(),
            flavor: "cpu".into(),
            timeout_hours: 1.0,
            gpu_count: 1,
            volumes: vec![],
            inputs: vec![],
            dependencies: vec![],
            interruptible: false,
            backend_hint: None,
            region_hint: None,
        }),
        output_contract: None,
        // ...fill in any other required JobStep fields per its current definition
    };

    let event = dispatch_batch_job_with_event(&provider, &step).await.unwrap();
    assert!(event.cold_start_seconds >= 0.0);
    assert!(event.run_seconds >= 0.0);
}
```

(Adjust the `JobStep` literal to match its actual current field list — read `b00t-cli/src/datum_job.rs`'s `JobStep` struct definition first; do not guess fields not already confirmed.)

- [ ] **Step 2: Run to verify it fails to compile** (`dispatch_batch_job_with_event` and `JobUtilizationEvent` don't exist)

- [ ] **Step 3: Create `b00t-cli/src/job_utilization.rs`**

```rust
use serde::Serialize;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct JobUtilizationEvent {
    pub job_id: String,
    pub provider: String,
    pub cold_start_seconds: f64,
    pub run_seconds: f64,
    /// None until a real cost model exists per-backend; a follow-up concern,
    /// not blocking this event's shape.
    pub estimated_cost: Option<f64>,
}

/// Appends one JSON line per event to a local outbox file. Deliberately a
/// dumb append-only file, not a queue or database — ledgrrr's ingestion side
/// (Task 2, separate repo/PR) reads and truncates it; this function has no
/// opinion about consumption.
pub fn append_to_outbox(path: &Path, event: &JobUtilizationEvent) -> std::io::Result<()> {
    let line = serde_json::to_string(event).expect("JobUtilizationEvent always serializes");
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{line}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_to_outbox_writes_one_json_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("utilization.jsonl");
        let event = JobUtilizationEvent {
            job_id: "job-1".into(),
            provider: "dstack".into(),
            cold_start_seconds: 1.5,
            run_seconds: 10.0,
            estimated_cost: None,
        };
        append_to_outbox(&path, &event).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents.lines().count(), 1);
        assert!(contents.contains("\"job_id\":\"job-1\""));
    }
}
```

(Confirm `tempfile` is already a dev-dependency of `b00t-cli` before using it — `grep -n "tempfile" b00t-cli/Cargo.toml`; if absent, add it under `[dev-dependencies]`.)

- [ ] **Step 4: Run the new module's own test**

Run: `CARGO_TARGET_DIR=~/cargo-target-scratch cargo test -p b00t-cli job_utilization`
Expected: PASS.

- [ ] **Step 5: Instrument `poll_until_terminal` to track the Pending→Running transition timestamp**

Modify `poll_until_terminal` (`job_executor.rs:506`) to return both the final status and the elapsed pending/running durations:

```rust
struct TerminalOutcome {
    status: String,
    cold_start_seconds: f64,
    run_seconds: f64,
}

async fn poll_until_terminal(provider: &dyn ComputeProvider, handle: &JobHandle) -> Result<TerminalOutcome> {
    let started_at = std::time::Instant::now();
    let mut running_started_at: Option<std::time::Instant> = None;
    let mut pending_polls = 0u32;
    let mut running_polls = 0u32;
    loop {
        let status = provider.job_status(handle).await?;
        match classify_status(&status) {
            JobStatusBucket::Terminal => {
                let now = std::time::Instant::now();
                let cold_start_seconds = running_started_at
                    .unwrap_or(now)
                    .duration_since(started_at)
                    .as_secs_f64();
                let run_seconds = running_started_at
                    .map(|t| now.duration_since(t).as_secs_f64())
                    .unwrap_or(0.0);
                return Ok(TerminalOutcome { status, cold_start_seconds, run_seconds });
            }
            JobStatusBucket::Pending => {
                pending_polls += 1;
                if pending_polls > PENDING_MAX_POLLS {
                    anyhow::bail!(
                        "job {} (provider={}) still pending after {} polls",
                        handle.id, handle.provider, PENDING_MAX_POLLS
                    );
                }
            }
            JobStatusBucket::Running => {
                running_started_at.get_or_insert_with(std::time::Instant::now);
                running_polls += 1;
                if running_polls > RUNNING_MAX_POLLS {
                    anyhow::bail!(
                        "job {} (provider={}) still running after {} polls with no terminal status",
                        handle.id, handle.provider, RUNNING_MAX_POLLS
                    );
                }
            }
        }
        tokio::time::sleep(PROVIDER_POLL_INTERVAL).await;
    }
}
```

Update every existing call site of `poll_until_terminal` (there is exactly one, in `dispatch_batch_job` — confirm with `grep -n "poll_until_terminal(" b00t-cli/src/job_executor.rs`) to destructure `TerminalOutcome` instead of a bare `String`, and update the existing tests that call `poll_until_terminal` directly (`poll_until_terminal_allows_long_pending_but_short_running_budget`, `poll_until_terminal_errors_after_running_budget_exceeded`) to match the new return type.

- [ ] **Step 6: Wire `dispatch_batch_job` to emit the event**

```rust
async fn dispatch_batch_job(provider: &dyn ComputeProvider, step: &JobStep) -> Result<()> {
    // ...existing spec/handle setup unchanged...
    let handle = provider.submit_batch_job(spec).await.context("submit_batch_job failed")?;
    let outcome = poll_until_terminal(provider, &handle).await?;

    crate::job_utilization::append_to_outbox(
        &std::path::PathBuf::from(std::env::var("B00T_UTILIZATION_OUTBOX")
            .unwrap_or_else(|_| "/tmp/b00t-job-utilization.jsonl".into())),
        &crate::job_utilization::JobUtilizationEvent {
            job_id: handle.id.clone(),
            provider: handle.provider.clone(),
            cold_start_seconds: outcome.cold_start_seconds,
            run_seconds: outcome.run_seconds,
            estimated_cost: None,
        },
    ).ok(); // best-effort — a failed utilization write must never fail the job itself

    if is_failure_status(&outcome.status) {
        anyhow::bail!(
            "job {} (provider={}) ended in a failure status: {}",
            handle.id, handle.provider, outcome.status
        );
    }
    // ...remaining output_contract handling, updated to read outcome.status instead of final_status...
    Ok(())
}
```

- [ ] **Step 7: Run the full `b00t-cli` test suite**

Run: `CARGO_TARGET_DIR=~/cargo-target-scratch cargo test -p b00t-cli`
Expected: all PASS, including the updated `poll_until_terminal_*` tests and the new `dispatch_batch_job_emits_utilization_event_with_separate_durations` test from Step 1.

- [ ] **Step 8: Commit**

```bash
git add b00t-cli/src/job_executor.rs b00t-cli/src/job_utilization.rs b00t-cli/Cargo.toml
git commit -m "feat(job-executor): emit JobUtilizationEvent with separate cold-start/run durations"
```

### Task 2: Extend ledgrrr's `RecordCost` to carry the two duration fields

**This task is in a separate repo/worktree (`ledgrrr`, not `_b00t_`).**

- [ ] **Step 1: Write a failing test**

In `ledgrrr/crates/ledger-core/src/ledger_ops.rs`'s existing test module (find the current `RecordCostOp` test via `grep -n "RecordCostOp" ledgrrr/crates/ledger-core/src/ledger_ops.rs` around line 1875, read it first to match its exact construction style):

```rust
#[test]
fn record_cost_op_includes_duration_fields_in_hash_and_issue_message() {
    let op = RecordCostOp {
        subject: "job:abc123".into(),
        amount: "0.42".into(),
        currency: "USD".into(),
        cold_start_seconds: Some(1.5),
        run_seconds: Some(10.0),
    };
    let ctx = OperationContext { /* ...match the existing test's ctx construction... */ };
    let result = op.execute(&ctx).unwrap();
    assert!(result.success);
    assert!(result.issues[0].contains("cost:"));
}
```

- [ ] **Step 2: Run to verify it fails to compile** (fields don't exist on `RecordCostOp`)

Run: `CARGO_TARGET_DIR=~/cargo-target-scratch-ledgrrr cargo test -p ledger-core record_cost_op_includes_duration_fields`

- [ ] **Step 3: Add the two fields to both `OperationKind::RecordCost` and `RecordCostOp`, defaulted for backward compatibility**

In `ledgrrr/crates/ledger-core/src/ledger_ops.rs`:

```rust
    /// Record a cost as an immutable, content-hashed evidence entry.
    RecordCost {
        subject: String,
        amount: String,
        currency: String,
        #[serde(default)]
        cold_start_seconds: Option<f64>,
        #[serde(default)]
        run_seconds: Option<f64>,
    },
```

```rust
pub struct RecordCostOp {
    pub subject: String,
    pub amount: String,
    pub currency: String,
    pub cold_start_seconds: Option<f64>,
    pub run_seconds: Option<f64>,
}

impl LedgerOperation for RecordCostOp {
    // ...id(), description(), is_idempotent() unchanged...

    fn execute(&self, _ctx: &OperationContext) -> Result<OperationResult, LedgerOpError> {
        let hash = blake3::hash(
            format!(
                "{}|{}|{}|{}|{}",
                self.subject,
                self.amount,
                self.currency,
                self.cold_start_seconds.map(|s| s.to_string()).unwrap_or_default(),
                self.run_seconds.map(|s| s.to_string()).unwrap_or_default(),
            )
            .as_bytes(),
        )
        .to_hex()
        .to_string();
        Ok(OperationResult {
            operation_id: "record-cost".to_string(),
            success: true,
            items_processed: 1,
            items_flagged: 0,
            issues: vec![format!("Cost recorded: cost:{hash}")],
            duration_ms: 0,
            row_errors: vec![],
        })
    }
}
```

- [ ] **Step 4: Fix the existing call sites** (`ledger_ops.rs` around lines 1561-1565 and 1932 construct `RecordCostOp`/match `OperationKind::RecordCost` — add `cold_start_seconds: None, run_seconds: None` to each, or destructure the two new fields, matching whichever pattern is already there).

- [ ] **Step 5: Run the full `ledger-core` test suite**

Run: `CARGO_TARGET_DIR=~/cargo-target-scratch-ledgrrr cargo test -p ledger-core`
Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/ledger-core/src/ledger_ops.rs
git commit -m "feat(ledger-ops): carry cold_start/run duration fields on RecordCost"
```

### Epoch 4 wrap-up

- [ ] Push both branches, open two PRs (one per repo), cross-referencing each other and #1207, as "Epoch 4 of 4".
- [ ] Wait for CI on both, merge both, delete both worktrees.

---

## Final step: write the handoff for the next agent

- [ ] After Epoch 4 merges, post a comment on PR #1207 (or a new tracking issue if the spec PR is already closed) summarizing: what shipped (all 4 epochs, with PR links), what's still open (the spec's three "Open Questions" — note which were actually resolved during implementation vs. still open, per each epoch's Step 1 findings), and what's explicitly deferred (CI-runner integration consuming this system, the `ufo-types` DAG construct — including the "must use this Dependency/Constraint pattern" requirement — and Nydus cold-start acceleration). This comment **is** the handoff for the next agent; do not create a separate handoff document.

---

# Self-Review Notes (fixed inline before saving)

- **Spec coverage:** all 7 "In scope" items from the spec map onto a task above (1→Epoch1, 2→Epoch2, 3→Epoch3, 4-5→Epoch2 fleet/task builders, 6→Epoch3, 7→Epoch4). Both out-of-scope items (RunPod, generic Dependency typing) are called out in Global Constraints so no task drifts into them.
- **Placeholder scan:** no TBD/TODO left; the two genuinely unresolved facts (AWS/Azure credential shape, task-level backend/region support in dstack) are each turned into an explicit verification *step* with a concrete command to run, not a placeholder.
- **Type consistency:** `BatchJobSpec`'s four new fields (Epoch 2 Task 1) are referenced identically in Epoch 3 (`placement.rs` consumes `dependencies`) and Epoch 4 (`dispatch_batch_job` still uses the same `spec`/`handle` names as the existing code). `JobUtilizationEvent`'s field names (`job_id`, `provider`, `cold_start_seconds`, `run_seconds`, `estimated_cost`) are used identically in Epoch 4 Task 1 (b00t-cli, producer) and Task 2 (ledgrrr, consumer via the separately-named `RecordCostOp` fields `cold_start_seconds`/`run_seconds` — deliberately matching names across the repo boundary for clarity, even though they travel via an outbox file rather than a shared Rust type).
