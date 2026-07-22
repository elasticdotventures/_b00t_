# b00t dstack Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `DstackProvider` compute backend to b00t so cloud AI jobs (mesh3d today, Sapiens2 next) get reliable, provider-agnostic status tracking and real PASS/FAIL outcome reporting, instead of the RunPod-only bespoke polling that currently gives up after 10 seconds.

**Architecture:** `DstackProvider` implements the existing `ComputeProvider` trait by shelling out to the `dstack` CLI (no official Rust SDK exists), mirroring the `HfProvider` pattern already in `provider.rs`. It's registered in `get_provider()` alongside `runpod`/`hf`/`local`. Separately, `job_executor.rs` gains real enforcement of `JobStep.output_contract` (currently schema-only, never read) so a job's actual PASS/FAIL — not just its container exit code — gates `JobCondition.when` for downstream steps. `cloud_mesh.sh` is migrated off manual polling onto this.

**Tech Stack:** Rust (`b00t-cli` crate), `dstack` CLI (Python, installed via `uv tool install 'dstack[all]'`), `serde_json`, existing `duct`/`std::process::Command` shell-out conventions.

## Global Constraints

- No new Rust crate dependency for dstack — shell out to the CLI, matching `HfProvider`'s `run_hf()` pattern exactly (spec's Open Question 1: no official `dstackai/dstack` Rust SDK exists; `crates.io/crates/dstack-sdk` is an unrelated project — never depend on it).
- `RunpodProvider`/`HfProvider`/`LocalProvider` are not modified in behavior, only additive changes (new `"dstack"` branch in `get_provider()`).
- dstack's real `RunStatus` values (verified against `dstackai/dstack` source): `pending`, `submitted`, `provisioning`, `running`, `terminating`, `terminated`, `failed`, `done`. `finished_statuses() = [terminated, failed, done]`. There is no distinct "pulling" state — cold-start time is absorbed into `provisioning`.
- Every provider method returns `anyhow::Result<T>` with `.context(...)` on shell-out failures, matching existing style in `provider.rs`.
- Spec: `~/.b00t/docs/superpowers/specs/2026-07-22-b00t-dstack-provider-design.md`

---

## Task 1: Install dstack CLI and capture real output fixtures

**Files:**
- Create: `b00t-cli/tests/fixtures/dstack_ps_json.txt`
- Create: `b00t-cli/tests/fixtures/dstack_apply_output.txt`

**Interfaces:**
- Produces: two fixture files containing real `dstack` CLI output, consumed by Task 3's parser and its unit test.

The exact JSON field names `dstack ps --json` emits aren't documented publicly in a form this plan can verify without running the tool — this task captures ground truth instead of guessing.

- [ ] **Step 1: Install the dstack CLI**

```bash
uv tool install 'dstack[all]'
dstack --version
```

Expected: prints a version string (confirms install; `which dstack` currently returns "not found" on this machine per the spec's investigation).

- [ ] **Step 2: Configure a backend and start the server**

Follow `https://dstack.ai/docs/reference/server/config.yml` to configure at least one real cloud backend (or a local backend if testing without cloud credentials), then:

```bash
dstack server &
dstack init
```

Expected: server starts, `dstack init` completes without error in the current directory.

- [ ] **Step 3: Submit a trivial task and capture the apply output**

Create a minimal task config:

```bash
cat > /tmp/dstack-echo-task.yml <<'EOF'
type: task
name: b00t-fixture-capture
image: ubuntu:24.04
commands:
  - echo "PASS"
EOF
dstack apply -f /tmp/dstack-echo-task.yml -y -d | tee b00t-cli/tests/fixtures/dstack_apply_output.txt
```

Expected: exits 0, `dstack_apply_output.txt` contains the real submit output (including however dstack prints the run name).

- [ ] **Step 4: Capture the `dstack ps --json` output for that run**

```bash
dstack ps --json -a | tee b00t-cli/tests/fixtures/dstack_ps_json.txt
```

Expected: valid JSON array/object written to the fixture file, containing an entry for `b00t-fixture-capture` with its status field.

- [ ] **Step 5: Commit the fixtures**

```bash
cd ~/.b00t
git add b00t-cli/tests/fixtures/dstack_ps_json.txt b00t-cli/tests/fixtures/dstack_apply_output.txt
git commit -m "test: capture real dstack CLI output fixtures for DstackProvider parser"
```

---

## Task 2: `DstackProvider` struct + task-config YAML builder + `submit_batch_job`/`submit_training_job`

**Files:**
- Modify: `b00t-cli/src/commands/provider.rs`

**Interfaces:**
- Consumes: `BatchJobSpec { image, config_path, env, flavor, timeout_hours }`, `TrainingJobSpec { config_path, image, flavor, timeout_hours }`, `JobHandle { id, provider }` (all already defined in this file, unchanged).
- Produces: `pub struct DstackProvider;`, `fn dstack_task_yaml(name: &str, spec: &BatchJobSpec) -> String` (pure function, testable without the CLI installed — mirrors `hf_batch_args`), `impl DstackProvider { fn run_dstack(&self, args: &[&str]) -> Result<String> }`.

- [ ] **Step 1: Write the failing test for the YAML builder**

Add to the bottom of `provider.rs`'s existing `#[cfg(test)] mod tests` block (create one in the same style as other provider tests if none exists near `HfProvider`):

```rust
#[test]
fn dstack_task_yaml_includes_image_env_and_command() {
    let mut env = std::collections::HashMap::new();
    env.insert("MESH_GPU".to_string(), "auto".to_string());
    let spec = BatchJobSpec {
        image: "docker.io/elasticdotventures/mesh-runner:v6".into(),
        config_path: "/workspace/request.json".into(),
        env,
        flavor: "RTX_4090".into(),
        timeout_hours: 2.0,
    };
    let yaml = dstack_task_yaml("b00t-job-abc123", &spec);
    assert!(yaml.contains("type: task"));
    assert!(yaml.contains("name: b00t-job-abc123"));
    assert!(yaml.contains("image: docker.io/elasticdotventures/mesh-runner:v6"));
    assert!(yaml.contains("MESH_GPU: \"auto\""));
    assert!(yaml.contains("/workspace/request.json"));
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd ~/.b00t && cargo test -p b00t-cli dstack_task_yaml_includes_image_env_and_command
```

Expected: FAIL — `dstack_task_yaml` not found.

- [ ] **Step 3: Implement `DstackProvider`, the YAML builder, and submit methods**

Add after the `HfProvider` `impl ComputeProvider for HfProvider` block (before the `// ── Local (podman/docker) provider` section):

```rust
// ── dstack provider ────────────────────────────────────────────────────────

pub struct DstackProvider;

impl DstackProvider {
    pub fn new() -> Self {
        Self
    }

    fn run_dstack(&self, args: &[&str]) -> Result<String> {
        let out = Command::new("dstack")
            .args(args)
            .output()
            .context("dstack CLI not found — run: uv tool install 'dstack[all]'")?;
        if !out.status.success() {
            bail!(
                "dstack {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }
}

/// Pure YAML builder, split out so tests can assert the exact task config
/// without the `dstack` CLI installed — same rationale as `hf_batch_args`.
/// The image's own ENTRYPOINT runs; `config_path` is passed through as the
/// entrypoint's argument, same convention as `RunpodProvider`/`HfProvider`.
fn dstack_task_yaml(name: &str, spec: &BatchJobSpec) -> String {
    let mut env_lines = String::new();
    for (key, value) in &spec.env {
        env_lines.push_str(&format!("  {key}: \"{value}\"\n"));
    }
    format!(
        "type: task\nname: {name}\nimage: {image}\ncommands:\n  - exec \"$@\" -- {config_path}\nenv:\n{env_lines}",
        image = spec.image,
        config_path = spec.config_path,
    )
}

#[async_trait]
impl ComputeProvider for DstackProvider {
    fn name(&self) -> &str {
        "dstack"
    }

    async fn deploy_inference_endpoint(&self, _cfg: &EndpointConfig) -> Result<EndpointHandle> {
        bail!("dstack provider does not yet support inference endpoints in b00t (batch/training jobs only) — use provider=runpod")
    }

    async fn endpoint_status(&self, _id: &str) -> Result<EndpointHandle> {
        bail!("dstack provider has no endpoint management yet; use provider=runpod")
    }

    async fn teardown_endpoint(&self, _id: &str) -> Result<()> {
        bail!("dstack provider has no endpoint management yet; use provider=runpod")
    }

    async fn list_endpoints(&self) -> Result<Vec<EndpointHandle>> {
        Ok(vec![])
    }

    async fn submit_training_job(&self, spec: &TrainingJobSpec) -> Result<JobHandle> {
        let name = format!("b00t-train-{}", uuid_suffix());
        let batch_spec = BatchJobSpec {
            image: spec.image.clone(),
            config_path: spec.config_path.clone(),
            env: Default::default(),
            flavor: spec.flavor.clone(),
            timeout_hours: spec.timeout_hours,
        };
        let yaml = dstack_task_yaml(&name, &batch_spec);
        submit_dstack_yaml(self, &name, &yaml)
    }

    async fn submit_batch_job(&self, spec: &BatchJobSpec) -> Result<JobHandle> {
        let name = format!("b00t-job-{}", uuid_suffix());
        let yaml = dstack_task_yaml(&name, spec);
        submit_dstack_yaml(self, &name, &yaml)
    }

    async fn job_status(&self, handle: &JobHandle) -> Result<String> {
        todo_task3_job_status(self, handle)
    }

    async fn cancel_job(&self, handle: &JobHandle) -> Result<()> {
        self.run_dstack(&["stop", &handle.id, "-y"])?;
        Ok(())
    }

    async fn list_jobs(&self) -> Result<Vec<JobHandle>> {
        todo_task3_list_jobs(self)
    }
}

fn uuid_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    format!(
        "{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

/// Write `yaml` to a temp file and `dstack apply -f <file> -y -d` it.
/// Split out so submit_batch_job/submit_training_job share one path.
fn submit_dstack_yaml(provider: &DstackProvider, name: &str, yaml: &str) -> Result<JobHandle> {
    let tmp = std::env::temp_dir().join(format!("{name}.dstack.yml"));
    std::fs::write(&tmp, yaml).context("writing dstack task config")?;
    provider.run_dstack(&[
        "apply",
        "-f",
        tmp.to_str().unwrap(),
        "-y",
        "-d",
    ])?;
    Ok(JobHandle {
        id: name.to_string(),
        provider: "dstack".into(),
    })
}
```

`job_status`/`list_jobs` are stubbed as `todo_task3_job_status`/`todo_task3_list_jobs` calls deliberately — Task 3 implements and replaces them using the real fixture captured in Task 1. This keeps Task 2's test (the YAML builder) independently green without blocking on the fixture.

Add stub functions right below `submit_dstack_yaml` so the file compiles for this task:

```rust
fn todo_task3_job_status(_provider: &DstackProvider, _handle: &JobHandle) -> Result<String> {
    unimplemented!("implemented in Task 3 against the captured dstack ps --json fixture")
}

fn todo_task3_list_jobs(_provider: &DstackProvider) -> Result<Vec<JobHandle>> {
    unimplemented!("implemented in Task 3 against the captured dstack ps --json fixture")
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cd ~/.b00t && cargo test -p b00t-cli dstack_task_yaml_includes_image_env_and_command
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add b00t-cli/src/commands/provider.rs
git commit -m "feat: add DstackProvider skeleton + submit_batch_job/submit_training_job"
```

---

## Task 3: `DstackProvider::job_status` / `list_jobs` against the real fixture

**Files:**
- Modify: `b00t-cli/src/commands/provider.rs`
- Test fixture (read-only, from Task 1): `b00t-cli/tests/fixtures/dstack_ps_json.txt`

**Interfaces:**
- Consumes: fixture file from Task 1, `JobHandle` from Task 2.
- Produces: `fn parse_dstack_ps_json(json: &str, run_name: Option<&str>) -> Result<Vec<(JobHandle, String)>>` — returns handle + raw dstack status string per run; `job_status`/`list_jobs` now call this instead of the `todo_task3_*` stubs from Task 2.

- [ ] **Step 1: Open the captured fixture and note the real field names**

```bash
cat ~/.b00t/b00t-cli/tests/fixtures/dstack_ps_json.txt
```

Identify the JSON key holding the run name (likely `"name"` or `"run_name"`) and the key holding status (likely `"status"`), and whether the top-level shape is a bare array or `{"runs": [...]}`. Use these exact keys in Step 3 below — do not guess past this point.

- [ ] **Step 2: Write the failing test against the fixture**

```rust
#[test]
fn parses_real_dstack_ps_json_fixture() {
    let json = include_str!("../tests/fixtures/dstack_ps_json.txt");
    let parsed = parse_dstack_ps_json(json, None).expect("fixture should parse");
    assert!(!parsed.is_empty(), "fixture should contain at least one run");
    let (handle, status) = &parsed[0];
    assert_eq!(handle.provider, "dstack");
    assert!(!status.is_empty());
}
```

- [ ] **Step 3: Run test to verify it fails**

```bash
cd ~/.b00t && cargo test -p b00t-cli parses_real_dstack_ps_json_fixture
```

Expected: FAIL — `parse_dstack_ps_json` not found.

- [ ] **Step 4: Implement the parser using the field names identified in Step 1**

Replace the `todo_task3_job_status`/`todo_task3_list_jobs` stubs with:

```rust
/// Parse `dstack ps --json` output. Field names below are taken from the
/// real fixture captured in Task 1 (`tests/fixtures/dstack_ps_json.txt`) —
/// adjust the `.get("...")` keys here if dstack's schema differs from what
/// was captured on this machine's dstack version.
fn parse_dstack_ps_json(json: &str, run_name: Option<&str>) -> Result<Vec<(JobHandle, String)>> {
    let value: serde_json::Value = serde_json::from_str(json).context("parsing dstack ps --json output")?;
    let runs = value
        .as_array()
        .cloned()
        .or_else(|| value.get("runs").and_then(|r| r.as_array()).cloned())
        .ok_or_else(|| anyhow::anyhow!("unexpected dstack ps --json shape: {json}"))?;

    let mut out = Vec::new();
    for run in runs {
        let name = run
            .get("name")
            .or_else(|| run.get("run_name"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("run entry missing name field: {run}"))?
            .to_string();
        if let Some(filter) = run_name {
            if name != filter {
                continue;
            }
        }
        let status = run
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        out.push((
            JobHandle {
                id: name,
                provider: "dstack".into(),
            },
            status,
        ));
    }
    Ok(out)
}
```

Then wire it into the trait impl (replacing the two `todo_task3_*` calls from Task 2):

```rust
    async fn job_status(&self, handle: &JobHandle) -> Result<String> {
        let out = self.run_dstack(&["ps", "--json", "-a"])?;
        let matches = parse_dstack_ps_json(&out, Some(&handle.id))?;
        let (_, status) = matches
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("dstack run '{}' not found in `dstack ps`", handle.id))?;
        Ok(format!("run={} status={}", handle.id, status))
    }

    async fn list_jobs(&self) -> Result<Vec<JobHandle>> {
        let out = self.run_dstack(&["ps", "--json", "-a"])?;
        let matches = parse_dstack_ps_json(&out, None)?;
        Ok(matches.into_iter().map(|(h, _)| h).collect())
    }
```

Remove the now-unused `todo_task3_job_status`/`todo_task3_list_jobs` stub functions.

- [ ] **Step 5: Run test to verify it passes**

```bash
cd ~/.b00t && cargo test -p b00t-cli parses_real_dstack_ps_json_fixture
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add b00t-cli/src/commands/provider.rs
git commit -m "feat: implement DstackProvider::job_status/list_jobs against real dstack ps --json fixture"
```

---

## Task 4: Register `"dstack"` in `get_provider()`

**Files:**
- Modify: `b00t-cli/src/commands/provider.rs:109-116`

**Interfaces:**
- Consumes: `DstackProvider::new()` from Task 2.
- Produces: `get_provider("dstack")` now resolves instead of erroring.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn get_provider_resolves_dstack() {
    let provider = get_provider("dstack").expect("dstack provider should resolve");
    assert_eq!(provider.name(), "dstack");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd ~/.b00t && cargo test -p b00t-cli get_provider_resolves_dstack
```

Expected: FAIL — `get_provider` returns `Err("unknown provider 'dstack'...")`.

- [ ] **Step 3: Update `get_provider`**

```rust
pub fn get_provider(name: &str) -> Result<Box<dyn ComputeProvider>> {
    match name {
        "runpod" => Ok(Box::new(RunpodProvider::new()?)),
        "hf" => Ok(Box::new(HfProvider::new())),
        "local" => Ok(Box::new(LocalProvider::new())),
        "dstack" => Ok(Box::new(DstackProvider::new())),
        other => bail!("unknown provider '{}'; supported: runpod, hf, local, dstack", other),
    }
}
```

- [ ] **Step 4: Update the `JobStep.backend` doc comment**

In `b00t-cli/src/datum_job.rs`, find the `backend` field doc comment (currently reads `accepted values are whatever get_provider accepts ("local" | "runpod" | "hf")`) and change it to:

```rust
    /// `Some(name)` dispatches via `ComputeProvider::submit_batch_job` from
    /// `commands::provider::get_provider(name)` — accepted values are whatever
    /// `get_provider` accepts ("local" | "runpod" | "hf" | "dstack"). Requires
    /// `batch` to also be set; the step's `task` is ignored in that case.
```

- [ ] **Step 5: Run test to verify it passes**

```bash
cd ~/.b00t && cargo test -p b00t-cli get_provider_resolves_dstack
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add b00t-cli/src/commands/provider.rs b00t-cli/src/datum_job.rs
git commit -m "feat: register dstack in get_provider(), update JobStep.backend docs"
```

---

## Task 5: State-aware status buckets in `job_executor.rs`

**Files:**
- Modify: `b00t-cli/src/job_executor.rs:417-467`

**Interfaces:**
- Consumes: raw status strings from any `ComputeProvider::job_status` (RunPod's `"pod=... status=..."`, HF's raw `hf jobs inspect` text, dstack's `"run=... status=..."` from Task 3).
- Produces: `enum JobStatusBucket { Pending, Running, Terminal }`, `fn classify_status(status: &str) -> JobStatusBucket` (replaces the boolean `is_terminal_status`), keeps `is_failure_status` as-is (still only meaningful once `Terminal`).

- [ ] **Step 1: Write the failing test**

Add near the existing `is_terminal_status`/`is_failure_status` tests (search `#[cfg(test)]` in this file):

```rust
#[test]
fn classify_status_buckets_dstack_states_correctly() {
    assert_eq!(classify_status("run=x status=pending"), JobStatusBucket::Pending);
    assert_eq!(classify_status("run=x status=submitted"), JobStatusBucket::Pending);
    assert_eq!(classify_status("run=x status=provisioning"), JobStatusBucket::Pending);
    assert_eq!(classify_status("run=x status=running"), JobStatusBucket::Running);
    assert_eq!(classify_status("run=x status=done"), JobStatusBucket::Terminal);
    assert_eq!(classify_status("run=x status=failed"), JobStatusBucket::Terminal);
    assert_eq!(classify_status("run=x status=terminated"), JobStatusBucket::Terminal);
    // existing RunPod/HF/local markers still classify as Terminal
    assert_eq!(classify_status("pod=x status=Exited"), JobStatusBucket::Terminal);
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd ~/.b00t && cargo test -p b00t-cli classify_status_buckets_dstack_states_correctly
```

Expected: FAIL — `classify_status`/`JobStatusBucket` not found.

- [ ] **Step 3: Implement `JobStatusBucket` and `classify_status`, keep `is_terminal_status`/`is_failure_status`**

Add above the existing `is_terminal_status` function:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobStatusBucket {
    /// Provider is still allocating/pulling — dstack's pending/submitted/
    /// provisioning, or any provider's equivalent. Not stuck by itself;
    /// generous timeout applies here (see poll_until_terminal).
    Pending,
    /// Job is actively executing.
    Running,
    /// Success or failure, no further state changes expected.
    Terminal,
}

/// Buckets a raw provider status string. dstack states (pending, submitted,
/// provisioning, running, terminating, terminated, failed, done) are checked
/// first since they're unambiguous; falls back to the existing substring
/// markers for RunPod/HF/local providers.
fn classify_status(status: &str) -> JobStatusBucket {
    let s = status.to_lowercase();
    if s.contains("status=pending") || s.contains("status=submitted") || s.contains("status=provisioning") {
        return JobStatusBucket::Pending;
    }
    if s.contains("status=running") {
        return JobStatusBucket::Running;
    }
    if is_terminal_status(&s) {
        return JobStatusBucket::Terminal;
    }
    JobStatusBucket::Pending
}
```

`is_terminal_status`/`is_failure_status` are unchanged — `classify_status` composes them rather than replacing their logic, so existing callers/tests on those two functions keep working.

- [ ] **Step 4: Run test to verify it passes**

```bash
cd ~/.b00t && cargo test -p b00t-cli classify_status_buckets_dstack_states_correctly
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add b00t-cli/src/job_executor.rs
git commit -m "feat: add JobStatusBucket/classify_status for pending-vs-running-vs-terminal"
```

---

## Task 6: State-aware timeout in `poll_until_terminal`

**Files:**
- Modify: `b00t-cli/src/job_executor.rs:407-467`

**Interfaces:**
- Consumes: `classify_status` from Task 5.
- Produces: `poll_until_terminal` now allows a longer budget while `Pending`, shorter once `Running`, replacing the flat `PROVIDER_MAX_POLLS=50` for all real (non-test) providers.

- [ ] **Step 1: Write the failing test**

Using the existing `FakeProvider::with_statuses` test double (search for it in this file's test module):

```rust
#[tokio::test]
async fn poll_until_terminal_allows_long_pending_but_short_running_budget() {
    // 40 pending polls (would have exceeded the old flat 50-cap alongside
    // real polls) followed by running -> done should still succeed.
    let mut statuses: Vec<&str> = vec!["run=x status=pending"; 40];
    statuses.push("run=x status=running");
    statuses.push("run=x status=done");
    let provider = FakeProvider::with_statuses(&statuses);
    let handle = JobHandle { id: "x".into(), provider: "dstack".into() };
    let result = poll_until_terminal(&provider, &handle).await;
    assert!(result.is_ok(), "expected success, got {result:?}");
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd ~/.b00t && cargo test -p b00t-cli poll_until_terminal_allows_long_pending_but_short_running_budget
```

Expected: FAIL — old flat `PROVIDER_MAX_POLLS=50` cap trips (42 polls needed, but only if `PROVIDER_POLL_INTERVAL`/count aren't yet bucket-aware; confirm the failure mode matches "did not reach a terminal status" before proceeding).

- [ ] **Step 3: Implement bucket-aware polling**

Replace the existing `poll_until_terminal` function body:

```rust
/// Poll budgets are asymmetric: `Pending` (still pulling/provisioning) gets
/// a much longer budget than `Running`, because cold starts are legitimately
/// slow and provider-dependent, while a job that's already `Running` should
/// be making progress — see the spec's state-aware timeout rationale.
const PENDING_MAX_POLLS: u32 = 300; // 300 * 200ms = 60s test-speed; real providers see the same ratio scaled by their own poll interval
const RUNNING_MAX_POLLS: u32 = 50;

async fn poll_until_terminal(provider: &dyn ComputeProvider, handle: &JobHandle) -> Result<String> {
    let mut pending_polls = 0u32;
    let mut running_polls = 0u32;
    loop {
        let status = provider.job_status(handle).await?;
        match classify_status(&status) {
            JobStatusBucket::Terminal => return Ok(status),
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

- [ ] **Step 4: Run test to verify it passes**

```bash
cd ~/.b00t && cargo test -p b00t-cli poll_until_terminal_allows_long_pending_but_short_running_budget
```

Expected: PASS

- [ ] **Step 5: Run the full existing job_executor test suite to check for regressions**

```bash
cd ~/.b00t && cargo test -p b00t-cli job_executor
```

Expected: all PASS — the old flat-budget tests should still pass since `PENDING_MAX_POLLS`/`RUNNING_MAX_POLLS` are each individually >= the old `PROVIDER_MAX_POLLS` for their respective bucket.

- [ ] **Step 6: Commit**

```bash
git add b00t-cli/src/job_executor.rs
git commit -m "feat: bucket-aware poll timeout — generous while pending, tight while running"
```

---

## Task 7: `output_contract` enforcement (PASS/FAIL evidence)

**Files:**
- Modify: `b00t-cli/src/job_executor.rs:472-493` (`dispatch_batch_job`)

**Interfaces:**
- Consumes: `JobStep.output_contract: Option<String>` (already defined in `datum_job.rs`, currently unread anywhere), terminal status string from `poll_until_terminal`.
- Produces: `fn evaluate_output_contract(contract: Option<&str>, job_output: &str) -> Result<()>` — errors if a contract is declared but the job's last non-empty line doesn't start with `PASS` or `FAIL:`; `dispatch_batch_job` now takes the step (not just the spec) so it can read `output_contract`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn evaluate_output_contract_rejects_missing_pass_fail_marker() {
    let result = evaluate_output_contract(Some("PASS|FAIL:<5lines>"), "some log line\nno marker here");
    assert!(result.is_err());
}

#[test]
fn evaluate_output_contract_accepts_pass_marker() {
    let result = evaluate_output_contract(Some("PASS|FAIL:<5lines>"), "running tests...\nPASS");
    assert!(result.is_ok());
}

#[test]
fn evaluate_output_contract_surfaces_fail_detail() {
    let result = evaluate_output_contract(Some("PASS|FAIL:<5lines>"), "running tests...\nFAIL: assertion mismatch at line 42");
    let err = result.expect_err("FAIL: line should error");
    assert!(err.to_string().contains("assertion mismatch at line 42"));
}

#[test]
fn evaluate_output_contract_none_always_passes() {
    assert!(evaluate_output_contract(None, "anything at all").is_ok());
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd ~/.b00t && cargo test -p b00t-cli evaluate_output_contract
```

Expected: FAIL — function not found.

- [ ] **Step 3: Implement `evaluate_output_contract` and wire it into `dispatch_batch_job`**

Add near `is_failure_status`:

```rust
/// Enforces `JobStep.output_contract` (format documented in datum_job.rs as
/// `"PASS|FAIL:<5lines>"`). A step with no contract always passes — this
/// only tightens behavior for steps that opt in. `job_output` is the job's
/// captured stdout (or `result.json` contents, read by the caller); the last
/// non-empty line must start with `PASS` or `FAIL:`.
fn evaluate_output_contract(contract: Option<&str>, job_output: &str) -> Result<()> {
    let Some(_contract) = contract else {
        return Ok(());
    };
    let last_line = job_output
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("");
    if last_line.trim() == "PASS" {
        return Ok(());
    }
    if let Some(detail) = last_line.trim().strip_prefix("FAIL:") {
        anyhow::bail!("job output_contract reported FAIL:{}", detail);
    }
    anyhow::bail!(
        "job declared an output_contract but its last output line was not PASS/FAIL: got '{}'",
        last_line
    );
}
```

Update `dispatch_batch_job` to take the step and enforce the contract after the terminal status is reached:

```rust
async fn dispatch_batch_job(provider: &dyn ComputeProvider, step: &JobStep) -> Result<()> {
    let spec = step.batch.as_ref().ok_or_else(|| {
        anyhow::anyhow!("step '{}' declares a backend but has no batch spec", step.name)
    })?;
    let handle = provider
        .submit_batch_job(spec)
        .await
        .context("submit_batch_job failed")?;

    let final_status = poll_until_terminal(provider, &handle).await?;

    if is_failure_status(&final_status) {
        anyhow::bail!(
            "job {} (provider={}) ended in a failure status: {}",
            handle.id,
            handle.provider,
            final_status
        );
    }

    if step.output_contract.is_some() {
        let job_output = provider.job_status(&handle).await
            .context("re-reading job status for output_contract evaluation")?;
        evaluate_output_contract(step.output_contract.as_deref(), &job_output)?;
    }

    Ok(())
}
```

Update the one caller (`execute_provider_step`, a few lines above) to pass `step` instead of `spec`:

```rust
async fn execute_provider_step(backend: &str, step: &JobStep) -> Result<()> {
    let provider =
        get_provider(backend).with_context(|| format!("resolving backend '{}'", backend))?;
    dispatch_batch_job(provider.as_ref(), step).await
}
```

(The `step.batch.as_ref().ok_or_else(...)` check moves inside `dispatch_batch_job` now, so remove the duplicate check that previously lived in `execute_provider_step` — confirm by re-reading the function before/after this edit that there's exactly one such check.)

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd ~/.b00t && cargo test -p b00t-cli evaluate_output_contract
cd ~/.b00t && cargo test -p b00t-cli execute_provider_step
```

Expected: all PASS

- [ ] **Step 5: Commit**

```bash
git add b00t-cli/src/job_executor.rs
git commit -m "feat: enforce JobStep.output_contract PASS/FAIL evidence for provider-dispatched steps"
```

---

## Task 8: `PROVIDER-DSTACK.provider.tomllmd` datum

**Files:**
- Create: `_b00t_/datums/PROVIDER-DSTACK.provider.tomllmd`

**Interfaces:**
- None (documentation datum, no code dependency) — follows the exact shape of `PROVIDER-RUNPOD.provider.tomllmd`/`PROVIDER-HF.provider.tomllmd`.

- [ ] **Step 1: Create the datum**

```toml
# dstack — Multi-Cloud GPU Orchestration
#
# dstack provisions GPU workloads across RunPod, AWS, GCP, Azure, Lambda,
# Kubernetes, and bare metal through one CLI/API. b00t uses it as the
# provider-agnostic backend so RunPod (or any single cloud) is optional
# infrastructure, not a hard dependency for running AI jobs.
#
# Auth: configured via `dstack server` backend config (see
#   https://dstack.ai/docs/reference/server/config.yml); no single
#   DSTACK_API_KEY env var — each backend cloud has its own credentials.
# Install: uv tool install 'dstack[all]'
#
# No official Rust SDK exists for dstackai/dstack (CLI + Python SDK + REST
# API only) — b00t's DstackProvider shells out to the CLI, same pattern as
# HfProvider's `hf jobs` wrapper.
#
# Run states (verified against dstackai/dstack source): pending, submitted,
# provisioning, running, terminating, terminated, failed, done.
# finished_statuses = [terminated, failed, done]. No separate "pulling"
# state — cold-start time is absorbed into `provisioning`.
#
# b00t CLI: b00t provider job submit-batch|status|cancel|list --provider dstack

[b00t.schema]
version   = "1"
type      = "api"
type_tags = ["provider", "training", "batch", "gpu", "cloud", "multi-cloud", "orchestration"]

[resource]
name        = "dstack"
console_url = "https://dstack.ai"
cli_install = "uv tool install 'dstack[all]'"

[resource.run_states]
pending      = { bucket = "pending" }
submitted    = { bucket = "pending" }
provisioning = { bucket = "pending" }
running      = { bucket = "running" }
terminating  = { bucket = "terminal" }
terminated   = { bucket = "terminal", failure = true }
failed       = { bucket = "terminal", failure = true }
done         = { bucket = "terminal", failure = false }

[[resource.usages]]
description = "Submit a batch job via b00t CLI (provider-agnostic)"
command     = "b00t-cli provider job submit-batch --provider dstack --image <image> --config <path> --flavor <flavor>"

[[resource.usages]]
description = "Check job status"
command     = "b00t-cli provider job status --provider dstack <run-name>"

[[resource.usages]]
description = "Cancel a job"
command     = "b00t-cli provider job cancel --provider dstack <run-name>"

# b00t:map v1
# summary: dstack multi-cloud GPU orchestration provider — CLI shell-out backend for ComputeProvider
# tags: provider, dstack, orchestration, multi-cloud, gpu, batch, training
# tier: frontier
# cmds: b00t-cli provider job submit-batch --provider dstack
# complexity: 3
```

- [ ] **Step 2: Validate the datum loads**

```bash
cd ~/.b00t && b00t-cli datum validate _b00t_/datums/PROVIDER-DSTACK.provider.tomllmd
```

Expected: PASS (no schema errors). If `datum validate` isn't the exact subcommand name, run `b00t-cli datum --help` first to confirm — do not skip validation.

- [ ] **Step 3: Commit**

```bash
git add _b00t_/datums/PROVIDER-DSTACK.provider.tomllmd
git commit -m "docs: add PROVIDER-DSTACK datum"
```

---

## Task 9: Replace `cloud_mesh.sh`'s manual-poll invocation with a `.job.toml` + `b00t job run`

**Files:**
- Create: `_b00t_/mesh3d-cloud.job.toml` (in `~/.b00t`)
- Modify: `app4dog/game-play/pipelines/photo-critter/cloud_mesh.sh`

**Interfaces:**
- Consumes: `backend = "dstack"` (Task 4), `JobStep.output_contract` enforcement (Task 7), the existing `mesh3d-batch.hive.toml` resource gate (already in the repo, unmodified).
- Produces: `b00t job run mesh3d-cloud` as the one-command, flag-free entry point — submit, poll (bucket-aware), and PASS/FAIL enforcement all happen inside `JobExecutor::run_job`, not in bash.

Per the operator's steer during this plan's review: reduce how much specific CLI syntax an LLM/operator has to reproduce correctly per invocation, and make pre/post steps (the resource gate check, the PASS/FAIL check) deterministic data in a job definition rather than hand-written bash re-derived each time. The real, wired job schema is `[[b00t.job.steps]]` (`JobStep`/`JobTask` in `datum_job.rs`, executed by `commands/job.rs`'s `b00t job run`) — confirmed against `_b00t_/example-workflow.job.toml`. A second, older `type = "job"` / `[b00t.orchestration]` schema also exists in `_b00t_/llm-batch-job.job.toml` but its `b00t job deploy`/`to-manifest` commands are **not** in the real `JobCommands` enum (`commands/job.rs`) — that pattern is dead/unwired and must not be used as a reference.

**Known open gap, not solved by this task:** `dstack_task_yaml` (Task 2) only handles `image`/`env`/entrypoint-command — it does not stage local files (the input photo, `request.json`) into the remote container. `cloud_mesh.sh` today builds `request.json` + copies the photo into a local temp dir that RunPod's volume mount reaches; dstack's equivalent (its own `files:`/volume mechanism, or a pre-upload-to-object-storage step) has not been designed. This task wires the orchestration/polling/PASS-FAIL path end-to-end using a *pre-staged* `config_path`; actually staging a new local photo into that path each run is follow-up work, called out explicitly rather than papered over with unverified YAML.

- [ ] **Step 1: Author the job definition**

```toml
# _b00t_/mesh3d-cloud.job.toml
[b00t]
name = "mesh3d-cloud"
parent = "wrkflw.cli"
type = "job"
hint = "Pixal3D image->GLB cloud batch job via dstack, resource-gated, PASS/FAIL-enforced"
version = "0.1.0"
depends_on = []

[b00t.job]
description = "Submit the mesh3d Pixal3D container to dstack, watch to completion, enforce PASS/FAIL"
tags = ["mesh3d", "pixal3d", "dstack", "app4dog"]

[b00t.job.config]
mode = "sequential"
checkpoint_mode = "off"
continue_on_failure = false

[[b00t.job.steps]]
name = "resource-gate"
description = "Dry-run the existing mesh3d-batch hive resource gate before spending cloud time/money"

[b00t.job.steps.task]
type = "bash"
command = "b00t hive plan mesh3d-batch"

[b00t.job.steps.condition]
when = "always"

[[b00t.job.steps]]
name = "mesh3d-generate"
description = "Submit Pixal3D GLB generation to dstack and enforce PASS/FAIL"
depends_on = ["resource-gate"]
backend = "dstack"
output_contract = "PASS|FAIL:<5lines>"

[b00t.job.steps.batch]
image = "docker.io/elasticdotventures/mesh-runner:v6"
config_path = "/workspace/request.json"
flavor = "RTX_4090"
timeout_hours = 2.0

[b00t.job.steps.condition]
when = "on_success"

[b00t.job.env]
MESH_RESOLUTION = "1024"
MESH_LOW_VRAM = "true"
```

- [ ] **Step 2: Validate the job definition parses**

```bash
cd ~/.b00t && b00t-cli job plan mesh3d-cloud
```

Expected: prints the two-step plan (`resource-gate` → `mesh3d-generate`) with no parse errors.

- [ ] **Step 3: Dry-run it**

```bash
cd ~/.b00t && b00t-cli job run mesh3d-cloud --dry-run
```

Expected: shows what would execute without actually submitting to dstack.

- [ ] **Step 4: Reduce `cloud_mesh.sh` to request-staging only, delegate execution**

`cloud_mesh.sh` keeps its existing responsibility (building `request.json` from the input photo — the part this task does *not* solve for the remote-file-staging gap noted above) but drops the `b00t provider job submit-batch` call and the "Monitor:"/"Cancel:" footer entirely. Replace the tail of the script (from `b00t provider job submit-batch \` through the `echo "Cancel: ..."` line) with:

```bash
echo "=== Cloud Mesh (via dstack job) ==="
echo "Request staged: $WORK_DIR/request.json"
echo ""
echo "Run: b00t job run mesh3d-cloud"
echo "(request.json staging into the remote container is a known follow-up gap — see plan Task 9)"
```

This intentionally stops short of calling `b00t job run` automatically from bash — until the file-staging gap is resolved, auto-invoking would submit a job against a stale/placeholder `config_path`. Printing the one command to run keeps the operator-facing surface to a single flag-free command instead of the old submit+manual-poll dance, without silently pretending file-staging works.

- [ ] **Step 5: Commit**

```bash
cd ~/.b00t
git add _b00t_/mesh3d-cloud.job.toml
git commit -m "feat: add mesh3d-cloud.job.toml — dstack batch step with resource gate + PASS/FAIL enforcement"

cd /home/brianh/promptexecution/app4dog
git add game-play/pipelines/photo-critter/cloud_mesh.sh
git commit -m "refactor: cloud_mesh.sh stages the request only, delegates execution to 'b00t job run mesh3d-cloud'"
```

---

## Task 10: End-to-end smoke test (trace-or-filler evidence)

**Files:** none (manual verification, no code change)

- [ ] **Step 1: Low-level provider check (bypasses the job.toml, isolates Tasks 2-6)**

```bash
cd ~/.b00t
b00t-cli provider job submit-batch --provider dstack --image ubuntu:24.04 --config /dev/null --flavor cpu
b00t-cli provider job status --provider dstack <job-id>   # repeat until terminal
```

Confirm it does NOT give up after ~10 seconds the way the old RunPod-only path did.

- [ ] **Step 2: Intent-based smoke test (the actual operator-facing surface from Task 9)**

Author a trivial `_b00t_/dstack-smoke.job.toml` with one `backend = "dstack"` step (`image = "ubuntu:24.04"`, a command that echoes `PASS`, `output_contract = "PASS|FAIL:<5lines>"`), then:

```bash
b00t-cli job run dstack-smoke
```

Expected: exits 0, and the job's PASS/FAIL evidence line (Task 7's `evaluate_output_contract`) is what determined success — not just container exit code.

- [ ] **Step 3: Paste the evidence line**

Record the final output of both commands verbatim as the closing evidence for this plan — per b00t's own trace-or-filler law, this is the PASS/FAIL line that closes out the work, not a narrative claim that it works.

---

# b00t:map v1
# summary: Implementation plan for DstackProvider — 10 tasks covering CLI install/fixture capture, provider methods, get_provider routing, bucket-aware polling, output_contract enforcement, datum, and cloud_mesh.sh migration
# tags: provider, dstack, orchestration, plan, gpu, multi-cloud
# tier: frontier
# cmds: superpowers:subagent-driven-development or superpowers:executing-plans
# complexity: 6
