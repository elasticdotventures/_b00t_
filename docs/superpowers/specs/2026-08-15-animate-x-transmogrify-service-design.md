# Animate-X Transmogrify Service — Design

**Date:** 2026-08-15
**Context:** `animate-x` (antgroup/animate-x, cloned reference-only at `~/promptexecution/animate-x`, backlog task `#35` in `.dotfiles/.b00t/tasks.json`) is a Stable-Diffusion-2.1-based latent diffusion model for character image animation: given a character image and a driving motion video, it produces an animated video of the character performing that motion. This design turns it into the first concrete implementation of a new general b00t capability — **transmogrify** — and makes it callable directly as a containerized service from the b00t AI subsystem.

## Goal

1. Define **transmogrify** as a general b00t interface for multi-step media transformation (image/audio/video: transform, merge, split) — Animate-X is the first backend, not the only one ever intended.
2. Ship a containerized Animate-X service, callable via `b00t datum call animate-x.container transmogrify --token image=<path> --token motion=<path>`.
3. Validate the full pipeline end-to-end on real RunPod GPU hardware for a single tiny input, within a hard **$2.00** cost cap, with costs monitored at each step.

## Non-goals

- Building out transmogrify backends for audio or video merge/split — the interface is defined generally, but only the image+motion→video (Animate-X) backend is implemented now.
- A persistent/always-on RunPod Serverless deployment — out of scope for this pass (see "Deployment target" below); revisit once there's a real call-volume need.
- Vendoring/forking `animate-x` under `elasticdotventures` — it stays a reference-only third-party clone (per task #35's original decision); the container only depends on its *published* checkpoints and pinned source, not a fork.
- Optimizing Animate-X's own model (frame count, resolution, quality) — default config, minimal test inputs.

## Why RunPod, not local

Local GPU is a GTX 1050 (4GB VRAM). Animate-X's checkpoint set (SD2.1 base ~5GB + Animate-X weights + OpenCLIP + pose-estimation models, ~8-12GB total) needs materially more VRAM than that to run at all. RunPod GPU cloud is used instead, at the `sm0l` tier (RTX 3090, 24GB VRAM, $0.44/hr per the existing `PROVIDER-RUNPOD` datum) — cheapest tier with comfortable headroom.

RunPod's own "Cached Models" feature was evaluated and does not fit: it's Serverless-only (conflicts with the one-shot Pod approach below) and limited to one conventional Hugging Face model repo per endpoint, whereas Animate-X needs 5 checkpoint files from mixed sources. No existing RunPod infrastructure (network volumes, `RUNPOD_API_KEY`) exists in this environment to reuse — this is a fresh build.

## Architecture

```
b00t datum call animate-x.container transmogrify --token image=... --token motion=...
        │
        ▼
POST /jobs (image, motion video)  ──►  202 { job_id }
        │
        ▼ (background worker, single-GPU, one job at a time)
  process_data.py  (pose/motion extraction)
        │
        ▼
  inference.py     (Animate-X generation)
        │
        ▼
  output video written to /workspace/jobs/<job_id>/output.mp4

GET /jobs/<job_id>  ──►  { status: queued|running|complete|failed, output_path?, error? }
```

The container is a **batch pipeline wrapped in an async job-queue HTTP layer** — not synchronous request/response (generation is too slow, would time out the caller), not a bare CLI script (wouldn't be callable as a service). This matches the existing `b00t-inference.container.toml` / `llama-cpp-server.container.toml` convention (`port`, `api` datum fields) while fitting Animate-X's actual latency profile.

## Components

1. **`Containerfile.animate-x`** — CUDA-enabled base image, Animate-X's Python deps (from `environment.yaml`/`requirements.txt`), source copied in, job-server entrypoint. Checkpoints are *not* baked into the image — they come from the mounted network volume.
2. **Job server** (`server.py`, FastAPI) —
   - `POST /jobs`: accepts an image file + motion video file (multipart), writes them to a per-job workspace dir, enqueues the job, returns `{job_id}` immediately.
   - `GET /jobs/{job_id}`: returns current status; once `complete`, includes the output path; once `failed`, includes an error message.
   - One background worker thread/process pulls from the queue — single GPU, no concurrency.
   - Worker invokes `process_data.py` then `inference.py` as subprocesses against the job's workspace dir, using default Animate-X config.
3. **`_b00t_/animate-x.container.toml`** — the b00t container datum: `image`, `build_file`, `gpu_device`, `port`, `api = "b00t-job-queue"` (new convention, documented inline — see "transmogrify interface" below), plus a reference to the network volume ID.
4. **Volume-populate tool** (`populate_volume.py` or `.sh`) — manifest-driven, idempotent: given the 5 required checkpoint files (source URL, expected size, sha256 where Hugging Face provides one), checks what's already present on the mounted volume and fetches only what's missing or incomplete. This is the general "don't pay to re-store what's already there" mechanism requested — scoped narrowly to this checkpoint set, not a generic framework.
5. **`transmogrify` interface convention** — documented as a `[b00t.interfaces.transmogrify]`-style block (or equivalent `datum call`-resolvable entry) on the datum: tokens `image` and `motion` resolve to a `POST /jobs` call against the running pod's URL. Future transmogrify backends (audio, merge/split) would define their own token sets against the same interface name.

## Data flow (validation run)

1. Populate the RunPod network volume once (`populate_volume.py` run against the volume — one-time, outside the timed GPU-pod budget where possible, or as the first step of the timed pod run if a pre-population path isn't available).
2. `b00t provider runpod submit` the built image, mounting the volume, with a tiny test image + a short (1-2s) driving clip and a low `max_frames`.
3. Pod starts, job server boots, worker runs the pipeline once.
4. Poll `GET /jobs/{id}` until `complete` or `failed`.
5. Retrieve/verify the output video exists and is non-trivial (non-zero size, valid video).
6. `b00t provider runpod stop` immediately — no idle time.
7. Record actual cost (`b00t provider runpod list`) in the implementation notes.

## Error handling

- Job failures (bad input, OOM, missing checkpoint) → `status: failed` with a message field; never silently dropped or left `running` forever.
- `populate_volume.py` fail-safe: a file is only considered "present" if its size (and sha256, where available) matches the manifest — a partial/corrupt prior download is treated as missing and re-fetched, never trusted.
- **Budget guard:** a hard wall-clock timeout on the validation pod (independent of job status) — if the job hangs, the pod gets stopped by the timeout regardless, so a stuck run can't silently burn past $2.
- If any step's real-world cost approaches the $2 cap before validation completes, stop and report rather than continuing.

## Testing

- Job-queue server logic (submit → queued → running → complete/failed transitions, concurrent-request handling) is unit-tested locally with a stubbed pipeline function — no GPU required.
- `populate_volume.py`'s dedupe logic is unit-tested against a scratch directory (pre-existing complete file / partial file / missing file cases).
- The actual GPU inference path has no local equivalent (no capable GPU) — it is validated exactly once, for real, on RunPod, as described above.

## Open items deferred to implementation

- Exact test image/clip to use for the validation run (small, license-clear, e.g. a still + a few-second clip already in `animate-x/data/`).
- Whether volume pre-population can happen before the timed GPU pod starts (RunPod may allow attaching a volume to a cheap CPU-only pod for population, separate from the GPU pod used for the actual test) — cheaper if possible, needs confirming against RunPod's actual pod-type options during implementation.
