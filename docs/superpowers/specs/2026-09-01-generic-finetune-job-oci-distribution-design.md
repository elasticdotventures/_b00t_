# Generic fine-tuning job type + OCI-layer-per-datum distribution

Part of epic `epic/topical-rag-mesh-a2a-acp` (task #50). This is sub-project
**B** of the fine-tuning thread (decomposition order: B → C → retrofit A —
see task #50 for the full epic).

## Why

`_b00t_/ai-finetune.just` already runs real Unsloth QLoRA fine-tunes today,
both on `sm3lly`'s RTX 3090 (via `b00t hive activate finetune`) and in the
cloud (`hf jobs run` against `ghcr.io/elasticdotventures/b00t-training-image`).
It works, but it's bespoke to one model family at a time (currently `qwen36`
GGUF profiles; `qwen2.5` sunset noted but not retargeted) and its only
output path is Hugging Face Hub (`elasticdotventures/b00t-training` dataset
repo, `elasticdotventures/b00t-qwen3-coder-30b`-style adapter repos).

Two things this blocks:
- **Sub-project C** (two agents racing to fine-tune the same base model)
  needs both competitors running the *identical* job shape — you can't
  compare results fairly if one path is hand-edited justfile recipes and
  the other is something new built for the race.
- Brian's stated target: model/dataset artifacts distributed as **one OCI
  container layer per AI datum**, stored on S3-compatible object storage
  (AWS S3 `ap-southeast-4`, or the `containers/zerofs/` ZeroFS-backed store
  once that build-verifies — infrastructure#192), not HF-Hub-only.

## Non-goals (this sub-project)

- Not implementing Axolotl (`axolotl.ai.toml` stays a comparison datum,
  "not yet wired into b00t-finetune.cli" per its own comment — out of
  scope here).
- Not building the competing-dual-agent orchestration itself (sub-project
  C consumes this job spec; doesn't extend it).
- Not touching `meta_harness.py` (harness/prompt evolution) — that's a
  distinct concept from fine-tuning model weights, already noted in
  task #50.
- Not replacing HF Hub outright — it stays a valid *optional* mirror
  target for convenience/visibility; S3/ZeroFS becomes the canonical
  output, not the only one.

## Architecture

```
job manifest (TOML, one per run)
        │
        ▼
  b00t-finetune runner  ──┬── local path: sm3lly RTX3090
  (same entrypoint,       │   (b00t hive activate finetune,
   same manifest schema)  │    existing Unsloth Python heredoc)
                          │
                          └── cloud path: HF Jobs
                              (ghcr.io/.../b00t-training-image)
        │
        ▼
  LoRA adapter (safetensors + adapter_config.json)
        │
        ▼
  OCI-layer packaging (one layer = one AI datum version)
        │
        ▼
  push to S3-compatible storage, spire-agent credentials
  (AWS S3 ap-southeast-4, or ZeroFS-backed once #192 lands)
        │
        ▼
  register/update the AI datum (e.g. qwen38-peer.ai.toml)
  pointing at the new OCI digest
```

## Components

**1. Job manifest schema** (new, TOML — matches existing datum-file
conventions rather than inventing a new format):

```toml
[job]
id = "qwen38-peer-2026-09-01"          # datum-name-shaped, not a UUID
base_model = "unsloth/Qwen3.8-27B-unsloth-bnb-4bit"
framework = "unsloth"                    # only value implemented; axolotl reserved

[dataset]
source = "hf://datasets/elasticdotventures/b00t-training"
# OR: generate = true, using finetune/generate_dataset.py's existing
# ChatML/SYSTEM_PROMPTS/TASK_SCENARIOS templates unchanged

[target]
kind = "local" | "cloud"
# local: hive_profile = "finetune" (existing gpu_free_mb >= 15000 gate)
# cloud: image = "ghcr.io/elasticdotventures/b00t-training-image:latest", flavor = "a100-large"

[output]
datum = "qwen38-peer"                    # → qwen38-peer.ai.toml gets written/updated
storage = "s3" | "zerofs"
s3_bucket = "..."                        # ap-southeast-4, only when storage = "s3"
mirror_to_hf = true                      # optional, default true during transition
```

**2. Runner abstraction.** A thin Rust (`b00t-cli` subcommand, alongside
existing `model.rs`/`model_manager.rs`) or `just` wrapper that reads the
manifest and dispatches to whichever existing mechanism matches
`[target].kind` — **not a rewrite of the training script itself**. The
Unsloth Python heredoc in `ai-finetune.just` stays as-is; the manifest just
parameterizes what today is hardcoded (`config-cloud-coder.yaml`,
`config-smol.yaml`). This is deliberately a thin layer, not a new training
framework.

**3. OCI-layer packaging.** After training, wrap the adapter directory
(`adapter_model.safetensors`, `adapter_config.json`, tokenizer files) as a
single-layer OCI image using `oras` (already the de facto tool for
non-container OCI artifacts — no existing dependency on it yet, would be a
new tool addition) or a minimal custom packer if `oras`turns out to be a
bad fit for size or S3-target reasons. Layer digest becomes the version
identifier written into the AI datum.

**4. Storage push — SPIRE-federated, not root creds.** Following
infrastructure#189's just-landed pattern: the push step authenticates via
`profile = "spire-agent"` (credential_process, short-lived) for AWS S3, not
a static/root key. For ZeroFS: mount via the container from
infrastructure#192 (once build-verified) and write through the POSIX
interface — same credential story underneath (ZeroFS's own `[aws]` config
block also takes access_key/secret, which should likewise come from
`secretEnvy`/`globalEnvy`, not a hand-typed value).

**5. AI datum registration.** On successful push, write/update
`qwen38-peer.ai.toml` (mirroring `qwen36-peer.ai.toml`'s existing shape)
with the new OCI digest, storage location, and training-run metadata
(base model, dataset ref, timestamp, framework). This is the "one OCI
layer per AI datum" contract — each datum version = one immutable layer.

## Data flow / error handling

- **GPU OOM (local path)**: existing `finetune.hive.toml` gate
  (`gpu_free_mb >= 15000`) runs *before* the job starts, stopping
  `inference-qwen36-*` services first. Runner should re-check free VRAM
  right before launching training (gate-then-race condition is possible if
  something else claims the GPU between gate-check and launch) and fail
  fast with a clear error rather than let Unsloth OOM mid-run.
- **Cloud job failure**: `hf jobs run` failures surface via HF's own job
  status API — runner polls and surfaces the failure, no new retry logic
  needed beyond what's already there.
- **Storage push failure**: retry with backoff (the dependency graph
  already pulls in `backon`/`tryhard` — reuse rather than hand-roll).
- **Credential expiry mid-push**: spire-agent credentials are short-lived
  by design; runner should re-fetch rather than cache across a long push,
  especially for a large adapter over a slow link.

## Testing

- Unit-test the manifest parser and OCI-layer-digest computation in
  isolation (no GPU needed).
- Integration test: run the *smallest* existing config
  (`config-smol.yaml`'s target, or an equivalent tiny model) through the
  full new path end-to-end — local hive-gate → training → OCI packaging →
  push to a real (test-scoped) S3 prefix → datum file written — before
  trusting this for the actual `qwen38-27B` run, which is expensive and
  shouldn't be the first thing that exercises untested plumbing.
- Explicitly do NOT gate this sub-project's completion on a successful
  `qwen38-27B` run — that's sub-project C's job (the competing-agents
  race). This sub-project's success criterion is: the *pipe* works
  end-to-end with a cheap model.

## Open questions for sub-project C to inherit

- Exact `oras`-vs-custom-packer choice for OCI layering — deferred, not
  blocking B's core design.
- Whether `mirror_to_hf` stays default-on long-term or is a transition
  crutch to drop once S3/ZeroFS is proven — revisit after C's race
  produces real artifacts to distribute.
