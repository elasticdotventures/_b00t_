# dstack GCP Backend Design

**Status:** Approved (2026-07-23)
**Depends on:** `feature/dstack-provider` (PR [elasticdotventures/_b00t_#857](https://github.com/elasticdotventures/_b00t_/pull/857), open, not yet merged — this branch is built on top of it)

## Context

`DstackProvider` (added in PR #857) shells out to the `dstack` CLI, which supports multiple cloud backends (RunPod, GCP, Azure, AWS, etc.) configured together under one project in `~/.dstack/server/config.yml`. PR #857 configured RunPod only — a scoping decision for the first working increment, not a conclusion that other clouds aren't needed (see `feedback_dstack_multicloud_scope` memory).

The user, now framed as this effort's product owner with the assistant as lead architect, corrected that scoping and asked for GCP as a second backend next, ahead of an idle-resource-sweep follow-up. GCP is already authenticated on this host: `gcloud config list` shows `account=brianh@elastic.ventures`, `project=app4dog`, and Application Default Credentials are already set up (`~/.config/gcloud/application_default_credentials.json` exists, a live access token was confirmed via `gcloud auth application-default print-access-token`).

## What this branch does NOT need to do

Verified directly against the installed dstack 0.20.28 source before designing, the same way PR #857's RunPod work was: `DstackProvider`'s generated fleet/task YAML (`dstack_fleet_yaml`, `dstack_task_yaml` in `b00t-cli/src/commands/provider.rs`) deliberately never hardcodes `backends: [runpod]` — that was a specific PR #857 design choice ("so it matches whatever backend(s) the operator's dstack server config.yml has configured"). This means:

- **No Rust code changes.** `ensure_fleet`, `submit_batch_job`, `submit_training_job`, `job_status`, `list_jobs` all already work against any backend dstack has configured — they only ever talk to the `dstack` CLI, never to a specific cloud's API directly.
- **No new `PROVIDER-*` datum.** GCP is another backend of the same orchestration layer (dstack), not a separate provider from b00t's `ComputeProvider` trait's point of view. It belongs in the existing `PROVIDER-DSTACK.provider.toml`.

This is a configuration/credential-generation change, not a feature-code change.

## Credential model — simpler than RunPod's

RunPod's dstack backend only accepts a literal `api_key` string in `config.yml` (no env-var interpolation exists in dstack's config loader — verified in PR #857, see `PROVIDER-DSTACK.provider.toml`'s `[resource.secrets.runpod_api_key]`). GCP's backend model (`dstack._internal.core.backends.gcp.models.py`, verified directly) supports a `GCPDefaultCreds` type (`{"type": "default"}`) that tells dstack to use ambient Application Default Credentials — the same resolution any GCP SDK client uses. Since ADC is already configured on this host, **no secret material is written into `config.yml` for GCP at all** — not even a generated one. `project_id` is the only required, non-secret field.

## Design

### 1. Extend `dstack-server-config` (existing `just` recipe, `_b00t_/justfile-dstack-sdd.just`)

The recipe currently writes a `projects: [{ name: b00t, backends: [<runpod block>] }]` config. Add a second backend entry to the same `backends:` list:

```yaml
- type: gcp
  project_id: "${GCP_PROJECT_ID}"
  creds:
    type: default
```

`GCP_PROJECT_ID` sourced from `.env` (bootstrap tier, matching the RunPod pattern) — falls back to `app4dog` if unset, since that's the project already active via `gcloud config`. No `regions:` restriction (omitted = all regions, matching the existing fleet-YAML principle of not hardcoding scope the operator didn't ask for).

### 2. Document in `PROVIDER-DSTACK.provider.toml`

- Update the file's header comment and `hint` to mention GCP as a second configured backend, not just RunPod.
- Add a `[resource.secrets.gcp_credentials]` table analogous to the existing `[resource.secrets.runpod_api_key]` one, documenting the ADC delivery mechanism instead of a `.env`-sourced literal:

  ```toml
  [resource.secrets.gcp_credentials]
  required        = false
  source_provider = "gcloud-adc"
  source_path     = "~/.config/gcloud/application_default_credentials.json"
  delivery_type   = "ambient"
  delivery_via    = "gcloud auth application-default login (one-time, outside b00t; already configured on this host)"
  ```

  `required = false` (unlike RunPod's `true`) because ADC is host-level ambient state b00t doesn't generate or manage — the datum documents the dependency so it's discoverable, not because b00t owns provisioning it.
- No changes to `[resource.run_states]` (those are dstack's own generic run states, backend-agnostic already) or `[resource.cost_control]` beyond noting GCP's own idle/cost characteristics differ from RunPod's (GCP compute typically bills differently; out of scope to fully characterize here — flag as a note, not a blocker).

### 3. Testing

- `dstack-server-config`'s output is a pure text-generation change — verify the generated YAML by hand (`python3 -c "import yaml; ..."` round-trip through dstack's own `ServerConfig`/`GCPBackendConfig` pydantic models, the same verification method used for the RunPod block in PR #857) rather than a Rust unit test, since this recipe lives entirely in shell/YAML, not Rust.
- **No live GCP spend as part of this branch.** Generating the config and confirming `dstack offer -b gcp` finds real matching offers (a read-only query, no cost) is in scope. Actually submitting and running a real GCP job is a separate, explicitly-gated step — same posture as RunPod's Task 1 in PR #857, requiring an explicit "go" before any real cloud spend, regardless of GCP's likely lower/free-tier cost.

## Out of scope (explicitly, for this branch)

- Per-job backend selection (e.g. an explicit `--backend gcp` override on `BatchJobSpec`) — not requested, YAGNI. dstack itself already lets an operator pass `-b gcp`/`-b runpod` ad hoc via its own CLI flags if needed; `b00t-cli`'s generated YAML doesn't currently expose this, and nothing in this design requires adding it.
- Azure as a third backend — not requested for this branch; same extension pattern would apply when it is.
- The idle-resource-sweep follow-up (explicitly sequenced after this branch, per user direction 2026-07-23).
