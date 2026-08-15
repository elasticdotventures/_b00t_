# Animate-X Transmogrify Service Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a containerized, RunPod-hosted Animate-X service (async job-queue HTTP API over the batch image+motion→video pipeline), register it as a b00t container datum implementing the new `transmogrify` interface, and validate it end-to-end on real RunPod GPU hardware for under $2.

**Architecture:** A FastAPI job-queue server (`POST /jobs`, `GET /jobs/{id}`) wraps Animate-X's two-stage CLI pipeline (`process_data.py` → `inference.py`), run as subprocesses by a single background worker thread. Packaged in a CUDA container, checkpoints supplied by a RunPod network volume (populated by a separate idempotent tool, not baked into the image). Deployed as a one-shot RunPod GPU Pod for validation, not a persistent Serverless endpoint.

**Tech Stack:** Python 3.10+, FastAPI, uvicorn, pytest (server/tool unit tests, stubbed — no GPU needed), Docker/Containerfile (podman-buildable), RunPod (`b00t provider runpod`), existing Animate-X repo at `~/promptexecution/animate-x` (reference-only, untouched).

## Global Constraints

- Reference spec: `docs/superpowers/specs/2026-08-15-animate-x-transmogrify-service-design.md`.
- `animate-x` source tree is never modified — it's a third-party reference clone (task #35), consumed by path, not forked.
- Validation run must stay under a **$2.00 hard cost cap**; every pod-lifecycle step must be paired with an explicit stop, and cost checked via `b00t provider runpod list` before/after.
- No synchronous HTTP request ever blocks on the actual generation — job submission and status polling only.
- `RUNPOD_API_KEY` is required for the final validation task and is **not currently set** in this environment — Task 7 cannot proceed without the user supplying it.
- New files live under a new top-level `animate-x-service/` directory in `.dotfiles`, matching the existing root-level `Containerfile.b00t-inference` convention.

---

## File Structure

```
.dotfiles/
├── animate-x-service/
│   ├── Containerfile
│   ├── server.py                # FastAPI app: POST /jobs, GET /jobs/{id}
│   ├── job_store.py              # in-memory job state + worker thread
│   ├── pipeline.py               # subprocess wrapper around process_data.py/inference.py
│   ├── populate_volume.py        # idempotent checkpoint fetch/dedupe tool
│   ├── checkpoint_manifest.py    # the 5 required checkpoint files: source, size, sha256
│   └── tests/
│       ├── test_job_store.py
│       ├── test_server.py
│       └── test_populate_volume.py
└── _b00t_/
    └── animate-x.container.toml  # the b00t datum
```

---

### Task 1: Checkpoint manifest + volume-populate tool

**Files:**
- Create: `animate-x-service/checkpoint_manifest.py`
- Create: `animate-x-service/populate_volume.py`
- Test: `animate-x-service/tests/test_populate_volume.py`

**Interfaces:**
- Produces: `checkpoint_manifest.py` → `CHECKPOINTS: list[CheckpointSpec]`, where `CheckpointSpec` is a `NamedTuple(url: str, relpath: str, size_bytes: int, sha256: str | None)`.
- Produces: `populate_volume.py` → `is_present(dest_dir: Path, spec: CheckpointSpec) -> bool`, `populate(dest_dir: Path, checkpoints: list[CheckpointSpec], fetch=urllib_fetch) -> list[str]` (returns list of relpaths actually fetched; `fetch` is injectable for testing).

- [ ] **Step 1: Write `checkpoint_manifest.py`**

```python
"""checkpoint_manifest.py - the 5 files Animate-X needs, sourced from
Hugging Face. Sizes are approximate upper bounds used for presence checks;
sha256 is None where the source doesn't publish one (populate_volume must
then fall back to size-only verification for that file)."""
from typing import NamedTuple


class CheckpointSpec(NamedTuple):
    url: str
    relpath: str
    size_bytes: int
    sha256: str | None


CHECKPOINTS: list[CheckpointSpec] = [
    CheckpointSpec(
        url="https://huggingface.co/Shuaishuai0219/Animate-X/resolve/main/animate-x.pth",
        relpath="animate-x.pth",
        size_bytes=3_400_000_000,
        sha256=None,
    ),
    CheckpointSpec(
        url="https://huggingface.co/Shuaishuai0219/Animate-X/resolve/main/dw-ll_ucoco_384.onnx",
        relpath="dw-ll_ucoco_384.onnx",
        size_bytes=134_000_000,
        sha256=None,
    ),
    CheckpointSpec(
        url="https://huggingface.co/Shuaishuai0219/Animate-X/resolve/main/open_clip_pytorch_model.bin",
        relpath="open_clip_pytorch_model.bin",
        size_bytes=1_900_000_000,
        sha256=None,
    ),
    CheckpointSpec(
        url="https://huggingface.co/Shuaishuai0219/Animate-X/resolve/main/v2-1_512-ema-pruned.ckpt",
        relpath="v2-1_512-ema-pruned.ckpt",
        size_bytes=5_200_000_000,
        sha256=None,
    ),
    CheckpointSpec(
        url="https://huggingface.co/Shuaishuai0219/Animate-X/resolve/main/yolox_l.onnx",
        relpath="yolox_l.onnx",
        size_bytes=207_000_000,
        sha256=None,
    ),
]
```

- [ ] **Step 2: Write the failing tests for `populate_volume.py`**

```python
# animate-x-service/tests/test_populate_volume.py
import hashlib
from pathlib import Path

import pytest

from animate_x_service.checkpoint_manifest import CheckpointSpec
from animate_x_service.populate_volume import is_present, populate


def make_spec(tmp_relpath: str, content: bytes, sha256: str | None) -> CheckpointSpec:
    return CheckpointSpec(
        url=f"https://example.invalid/{tmp_relpath}",
        relpath=tmp_relpath,
        size_bytes=len(content),
        sha256=sha256,
    )


def test_is_present_false_when_file_missing(tmp_path):
    spec = make_spec("missing.bin", b"x" * 10, None)
    assert is_present(tmp_path, spec) is False


def test_is_present_false_when_size_mismatch(tmp_path):
    spec = make_spec("partial.bin", b"x" * 100, None)
    (tmp_path / "partial.bin").write_bytes(b"x" * 40)
    assert is_present(tmp_path, spec) is False


def test_is_present_true_when_size_matches_and_no_hash_expected(tmp_path):
    content = b"x" * 50
    spec = make_spec("ok.bin", content, None)
    (tmp_path / "ok.bin").write_bytes(content)
    assert is_present(tmp_path, spec) is True


def test_is_present_false_when_hash_mismatch(tmp_path):
    content = b"x" * 50
    real_sha = hashlib.sha256(content).hexdigest()
    spec = make_spec("hashed.bin", content, real_sha)
    (tmp_path / "hashed.bin").write_bytes(b"y" * 50)  # same size, wrong content
    assert is_present(tmp_path, spec) is False


def test_is_present_true_when_hash_matches(tmp_path):
    content = b"x" * 50
    real_sha = hashlib.sha256(content).hexdigest()
    spec = make_spec("hashed_ok.bin", content, real_sha)
    (tmp_path / "hashed_ok.bin").write_bytes(content)
    assert is_present(tmp_path, spec) is True


def test_populate_skips_already_present_files(tmp_path):
    content = b"x" * 20
    spec = make_spec("already.bin", content, None)
    (tmp_path / "already.bin").write_bytes(content)

    fetch_calls = []

    def fake_fetch(url, dest):
        fetch_calls.append(url)

    fetched = populate(tmp_path, [spec], fetch=fake_fetch)
    assert fetched == []
    assert fetch_calls == []


def test_populate_fetches_missing_files(tmp_path):
    content = b"x" * 20
    spec = make_spec("missing2.bin", content, None)

    def fake_fetch(url, dest):
        Path(dest).write_bytes(content)

    fetched = populate(tmp_path, [spec], fetch=fake_fetch)
    assert fetched == ["missing2.bin"]
    assert (tmp_path / "missing2.bin").read_bytes() == content
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd /home/brianh/.dotfiles/animate-x-service && python3 -m pytest tests/test_populate_volume.py -v`
Expected: FAIL/ERROR — `ModuleNotFoundError: No module named 'animate_x_service'` (or `populate_volume` doesn't exist yet).

- [ ] **Step 4: Write `populate_volume.py`**

```python
"""populate_volume.py - idempotent checkpoint fetch. Only downloads a file
if it's missing, wrong-sized, or (when a sha256 is known) hash-mismatched.
Never trusts a partial/corrupt prior download as complete."""
import hashlib
import urllib.request
from pathlib import Path
from typing import Callable

from checkpoint_manifest import CHECKPOINTS, CheckpointSpec


def _sha256_of(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def is_present(dest_dir: Path, spec: CheckpointSpec) -> bool:
    path = dest_dir / spec.relpath
    if not path.exists():
        return False
    if path.stat().st_size != spec.size_bytes:
        return False
    if spec.sha256 is not None and _sha256_of(path) != spec.sha256:
        return False
    return True


def _urllib_fetch(url: str, dest: str) -> None:
    urllib.request.urlretrieve(url, dest)


def populate(
    dest_dir: Path,
    checkpoints: list[CheckpointSpec] = CHECKPOINTS,
    fetch: Callable[[str, str], None] = _urllib_fetch,
) -> list[str]:
    dest_dir.mkdir(parents=True, exist_ok=True)
    fetched: list[str] = []
    for spec in checkpoints:
        if is_present(dest_dir, spec):
            continue
        dest_path = dest_dir / spec.relpath
        dest_path.parent.mkdir(parents=True, exist_ok=True)
        fetch(spec.url, str(dest_path))
        fetched.append(spec.relpath)
    return fetched


if __name__ == "__main__":
    import sys

    target = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/runpod-volume/checkpoints")
    result = populate(target)
    print(f"Fetched {len(result)} checkpoint(s): {result}")
    print(f"Already present: {len(CHECKPOINTS) - len(result)}")
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd /home/brianh/.dotfiles/animate-x-service && python3 -m pytest tests/test_populate_volume.py -v`
Expected: `7 passed`

- [ ] **Step 6: Commit**

```bash
cd /home/brianh/.dotfiles
git add animate-x-service/checkpoint_manifest.py animate-x-service/populate_volume.py animate-x-service/tests/test_populate_volume.py
git commit -m "feat(animate-x): add checkpoint manifest and idempotent volume-populate tool"
```

---

### Task 2: Job store — in-memory queue + worker thread

**Files:**
- Create: `animate-x-service/job_store.py`
- Test: `animate-x-service/tests/test_job_store.py`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `job_store.py` → class `JobStore` with methods `submit(image_path: str, motion_path: str) -> str` (returns job_id), `get(job_id: str) -> dict | None` (returns `{"status": "queued"|"running"|"complete"|"failed", "output_path": str | None, "error": str | None}`), constructor `JobStore(run_job: Callable[[str, str, str], str])` where `run_job(job_id, image_path, motion_path) -> output_path` is the injectable pipeline function (raises on failure). One background thread processes jobs strictly one at a time (single GPU).

- [ ] **Step 1: Write the failing tests**

```python
# animate-x-service/tests/test_job_store.py
import threading
import time

from job_store import JobStore


def test_submit_returns_job_id_immediately():
    store = JobStore(run_job=lambda jid, img, mot: "/out.mp4")
    job_id = store.submit("/img.png", "/motion.mp4")
    assert isinstance(job_id, str) and len(job_id) > 0


def test_job_starts_queued_or_running():
    started = threading.Event()

    def slow_job(jid, img, mot):
        started.set()
        time.sleep(0.2)
        return "/out.mp4"

    store = JobStore(run_job=slow_job)
    job_id = store.submit("/img.png", "/motion.mp4")
    status = store.get(job_id)["status"]
    assert status in ("queued", "running")


def test_job_reaches_complete_with_output_path():
    store = JobStore(run_job=lambda jid, img, mot: "/workspace/out.mp4")
    job_id = store.submit("/img.png", "/motion.mp4")

    deadline = time.time() + 2
    while time.time() < deadline:
        result = store.get(job_id)
        if result["status"] == "complete":
            break
        time.sleep(0.01)

    result = store.get(job_id)
    assert result["status"] == "complete"
    assert result["output_path"] == "/workspace/out.mp4"
    assert result["error"] is None


def test_failed_job_reports_error_not_silently_dropped():
    def failing_job(jid, img, mot):
        raise RuntimeError("checkpoint missing")

    store = JobStore(run_job=failing_job)
    job_id = store.submit("/img.png", "/motion.mp4")

    deadline = time.time() + 2
    while time.time() < deadline:
        result = store.get(job_id)
        if result["status"] == "failed":
            break
        time.sleep(0.01)

    result = store.get(job_id)
    assert result["status"] == "failed"
    assert "checkpoint missing" in result["error"]


def test_get_unknown_job_returns_none():
    store = JobStore(run_job=lambda jid, img, mot: "/out.mp4")
    assert store.get("nonexistent") is None


def test_jobs_run_one_at_a_time_not_concurrently():
    concurrent_count = {"value": 0, "max": 0}
    lock = threading.Lock()

    def job(jid, img, mot):
        with lock:
            concurrent_count["value"] += 1
            concurrent_count["max"] = max(concurrent_count["max"], concurrent_count["value"])
        time.sleep(0.1)
        with lock:
            concurrent_count["value"] -= 1
        return "/out.mp4"

    store = JobStore(run_job=job)
    ids = [store.submit("/img.png", "/motion.mp4") for _ in range(3)]

    deadline = time.time() + 3
    while time.time() < deadline:
        if all(store.get(i)["status"] == "complete" for i in ids):
            break
        time.sleep(0.02)

    assert concurrent_count["max"] == 1
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /home/brianh/.dotfiles/animate-x-service && python3 -m pytest tests/test_job_store.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'job_store'`

- [ ] **Step 3: Write `job_store.py`**

```python
"""job_store.py - single-worker async job queue. One GPU, one job at a
time; a synchronous HTTP call would time out on Animate-X's real runtime,
so callers submit and poll instead."""
import queue
import threading
import uuid
from typing import Callable, Optional


class JobStore:
    def __init__(self, run_job: Callable[[str, str, str], str]):
        self._run_job = run_job
        self._jobs: dict[str, dict] = {}
        self._lock = threading.Lock()
        self._queue: "queue.Queue[str]" = queue.Queue()
        self._worker = threading.Thread(target=self._worker_loop, daemon=True)
        self._worker.start()

    def submit(self, image_path: str, motion_path: str) -> str:
        job_id = str(uuid.uuid4())
        with self._lock:
            self._jobs[job_id] = {
                "status": "queued",
                "output_path": None,
                "error": None,
                "image_path": image_path,
                "motion_path": motion_path,
            }
        self._queue.put(job_id)
        return job_id

    def get(self, job_id: str) -> Optional[dict]:
        with self._lock:
            job = self._jobs.get(job_id)
            if job is None:
                return None
            return {
                "status": job["status"],
                "output_path": job["output_path"],
                "error": job["error"],
            }

    def _worker_loop(self) -> None:
        while True:
            job_id = self._queue.get()
            with self._lock:
                job = self._jobs[job_id]
                job["status"] = "running"
                image_path = job["image_path"]
                motion_path = job["motion_path"]
            try:
                output_path = self._run_job(job_id, image_path, motion_path)
                with self._lock:
                    self._jobs[job_id]["status"] = "complete"
                    self._jobs[job_id]["output_path"] = output_path
            except Exception as exc:  # noqa: BLE001 - job failure must be captured, not crash the worker
                with self._lock:
                    self._jobs[job_id]["status"] = "failed"
                    self._jobs[job_id]["error"] = str(exc)
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /home/brianh/.dotfiles/animate-x-service && python3 -m pytest tests/test_job_store.py -v`
Expected: `6 passed`

- [ ] **Step 5: Commit**

```bash
cd /home/brianh/.dotfiles
git add animate-x-service/job_store.py animate-x-service/tests/test_job_store.py
git commit -m "feat(animate-x): add single-worker async job store"
```

---

### Task 3: Pipeline wrapper (subprocess invocation of Animate-X)

**Files:**
- Create: `animate-x-service/pipeline.py`

**Interfaces:**
- Consumes: nothing directly (invoked by `server.py` in Task 4, matching `JobStore`'s `run_job` signature from Task 2: `run_job(job_id: str, image_path: str, motion_path: str) -> str`).
- Produces: `pipeline.py` → `run_animate_x_job(job_id: str, image_path: str, motion_path: str, *, animate_x_root: str, workspace_root: str, checkpoints_dir: str) -> str` (returns absolute output video path; raises `RuntimeError` with subprocess stderr tail on any non-zero exit).

This task has no isolated unit test — it shells out to the real `process_data.py`/`inference.py` and has no meaningful behavior without a GPU and real checkpoints. It's covered by the Task 7 RunPod validation instead. Keep it a thin, obviously-correct wrapper.

- [ ] **Step 1: Write `pipeline.py`**

```python
"""pipeline.py - thin subprocess wrapper around Animate-X's two-stage CLI
(process_data.py, inference.py). No unit test: this has no meaningful
behavior without a GPU + real checkpoints, and is validated by the actual
RunPod run (Task 7) instead."""
import os
import subprocess
from pathlib import Path


def run_animate_x_job(
    job_id: str,
    image_path: str,
    motion_path: str,
    *,
    animate_x_root: str,
    workspace_root: str,
    checkpoints_dir: str,
) -> str:
    job_dir = Path(workspace_root) / job_id
    job_dir.mkdir(parents=True, exist_ok=True)
    saved_pkl_dir = job_dir / "saved_pkl"
    saved_pose_dir = job_dir / "saved_pose"
    saved_frame_dir = job_dir / "saved_frames"
    for d in (saved_pkl_dir, saved_pose_dir, saved_frame_dir):
        d.mkdir(parents=True, exist_ok=True)

    env = dict(os.environ)
    env["ANIMATE_X_CHECKPOINTS_DIR"] = checkpoints_dir

    preprocess_cmd = [
        "python3",
        "process_data.py",
        "--source_video_paths",
        motion_path,
        "--saved_pose_dir",
        str(saved_pkl_dir),
        "--saved_pose",
        str(saved_pose_dir),
        "--saved_frame_dir",
        str(saved_frame_dir),
    ]
    _run_step(preprocess_cmd, cwd=animate_x_root, env=env, step_name="process_data.py")

    output_dir = job_dir / "results"
    output_dir.mkdir(parents=True, exist_ok=True)
    inference_cmd = [
        "python3",
        "inference.py",
        "--cfg",
        "configs/Animate_X_infer.yaml",
        "--source_image",
        image_path,
        "--output_dir",
        str(output_dir),
    ]
    _run_step(inference_cmd, cwd=animate_x_root, env=env, step_name="inference.py")

    outputs = sorted(output_dir.glob("*.mp4"))
    if not outputs:
        raise RuntimeError(f"inference.py completed but produced no .mp4 in {output_dir}")
    return str(outputs[0])


def _run_step(cmd: list[str], *, cwd: str, env: dict, step_name: str) -> None:
    result = subprocess.run(
        cmd, cwd=cwd, env=env, capture_output=True, text=True, timeout=1800
    )
    if result.returncode != 0:
        stderr_tail = "\n".join(result.stderr.splitlines()[-40:])
        raise RuntimeError(f"{step_name} failed (exit {result.returncode}):\n{stderr_tail}")
```

- [ ] **Step 2: Commit**

```bash
cd /home/brianh/.dotfiles
git add animate-x-service/pipeline.py
git commit -m "feat(animate-x): add subprocess pipeline wrapper for process_data.py + inference.py"
```

---

### Task 4: FastAPI job server

**Files:**
- Create: `animate-x-service/server.py`
- Test: `animate-x-service/tests/test_server.py`

**Interfaces:**
- Consumes: `job_store.JobStore` (Task 2) — constructed with `pipeline.run_animate_x_job` (Task 3) partially applied via a lambda, wired to real `animate_x_root`/`workspace_root`/`checkpoints_dir` from environment variables.
- Produces: FastAPI `app` object. `POST /jobs` (multipart `image`, `motion` files) → `202 {"job_id": str}`. `GET /jobs/{job_id}` → `200 {"status": ..., "output_path": ..., "error": ...}` or `404` if unknown.

- [ ] **Step 1: Write the failing tests**

```python
# animate-x-service/tests/test_server.py
import io

from fastapi.testclient import TestClient

from job_store import JobStore
from server import build_app


def make_client(run_job):
    store = JobStore(run_job=run_job)
    app = build_app(store)
    return TestClient(app)


def test_post_jobs_returns_job_id():
    client = make_client(run_job=lambda jid, img, mot: "/out.mp4")
    response = client.post(
        "/jobs",
        files={
            "image": ("img.png", io.BytesIO(b"fakeimg"), "image/png"),
            "motion": ("motion.mp4", io.BytesIO(b"fakevideo"), "video/mp4"),
        },
    )
    assert response.status_code == 202
    body = response.json()
    assert "job_id" in body and len(body["job_id"]) > 0


def test_get_unknown_job_returns_404():
    client = make_client(run_job=lambda jid, img, mot: "/out.mp4")
    response = client.get("/jobs/does-not-exist")
    assert response.status_code == 404


def test_full_roundtrip_submit_then_poll_to_complete():
    client = make_client(run_job=lambda jid, img, mot: "/workspace/out.mp4")
    submit = client.post(
        "/jobs",
        files={
            "image": ("img.png", io.BytesIO(b"fakeimg"), "image/png"),
            "motion": ("motion.mp4", io.BytesIO(b"fakevideo"), "video/mp4"),
        },
    )
    job_id = submit.json()["job_id"]

    import time

    deadline = time.time() + 2
    status = None
    while time.time() < deadline:
        poll = client.get(f"/jobs/{job_id}")
        status = poll.json()
        if status["status"] == "complete":
            break
        time.sleep(0.01)

    assert status["status"] == "complete"
    assert status["output_path"] == "/workspace/out.mp4"


def test_failed_job_surfaces_error_via_polling():
    def failing(jid, img, mot):
        raise RuntimeError("boom")

    client = make_client(run_job=failing)
    submit = client.post(
        "/jobs",
        files={
            "image": ("img.png", io.BytesIO(b"fakeimg"), "image/png"),
            "motion": ("motion.mp4", io.BytesIO(b"fakevideo"), "video/mp4"),
        },
    )
    job_id = submit.json()["job_id"]

    import time

    deadline = time.time() + 2
    status = None
    while time.time() < deadline:
        poll = client.get(f"/jobs/{job_id}")
        status = poll.json()
        if status["status"] == "failed":
            break
        time.sleep(0.01)

    assert status["status"] == "failed"
    assert "boom" in status["error"]
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /home/brianh/.dotfiles/animate-x-service && python3 -m pytest tests/test_server.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'server'`

- [ ] **Step 3: Write `server.py`**

```python
"""server.py - async job-queue HTTP API for the Animate-X transmogrify
backend. POST /jobs submits a job and returns immediately; GET /jobs/{id}
polls status. No synchronous inference over HTTP -- Animate-X's real
runtime would time out a request/response call."""
import os
import tempfile
from pathlib import Path

from fastapi import FastAPI, File, HTTPException, UploadFile
from fastapi.responses import JSONResponse

from job_store import JobStore
from pipeline import run_animate_x_job


def build_app(store: JobStore) -> FastAPI:
    app = FastAPI(title="animate-x-transmogrify")
    upload_dir = Path(tempfile.mkdtemp(prefix="animate-x-uploads-"))

    @app.post("/jobs", status_code=202)
    async def submit_job(image: UploadFile = File(...), motion: UploadFile = File(...)):
        image_path = upload_dir / f"{image.filename}"
        motion_path = upload_dir / f"{motion.filename}"
        image_path.write_bytes(await image.read())
        motion_path.write_bytes(await motion.read())
        job_id = store.submit(str(image_path), str(motion_path))
        return {"job_id": job_id}

    @app.get("/jobs/{job_id}")
    async def get_job(job_id: str):
        result = store.get(job_id)
        if result is None:
            raise HTTPException(status_code=404, detail="unknown job_id")
        return JSONResponse(result)

    return app


def _real_run_job(job_id: str, image_path: str, motion_path: str) -> str:
    return run_animate_x_job(
        job_id,
        image_path,
        motion_path,
        animate_x_root=os.environ["ANIMATE_X_ROOT"],
        workspace_root=os.environ.get("ANIMATE_X_WORKSPACE", "/workspace/jobs"),
        checkpoints_dir=os.environ["ANIMATE_X_CHECKPOINTS_DIR"],
    )


app = build_app(JobStore(run_job=_real_run_job))
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd /home/brianh/.dotfiles/animate-x-service && python3 -m pytest tests/test_server.py -v`
Expected: `4 passed`

- [ ] **Step 5: Commit**

```bash
cd /home/brianh/.dotfiles
git add animate-x-service/server.py animate-x-service/tests/test_server.py
git commit -m "feat(animate-x): add FastAPI async job-queue server"
```

---

### Task 5: Containerfile + local build smoke test

**Files:**
- Create: `animate-x-service/Containerfile`
- Create: `animate-x-service/requirements-service.txt`

**Interfaces:**
- Consumes: `server.py`, `job_store.py`, `pipeline.py`, `populate_volume.py`, `checkpoint_manifest.py` (Tasks 1-4), plus the `animate-x` source tree, mounted or copied at build/run time.
- Produces: a buildable container image `localhost/animate-x-service:latest` exposing port 8000, expecting `/checkpoints` (network volume mount) and `ANIMATE_X_ROOT=/opt/animate-x`.

- [ ] **Step 1: Write `requirements-service.txt`**

```
fastapi>=0.110
uvicorn[standard]>=0.27
python-multipart>=0.0.9
```

- [ ] **Step 2: Write `Containerfile`**

```dockerfile
FROM docker.io/pytorch/pytorch:2.1.0-cuda11.8-cudnn8-runtime

RUN apt-get update && apt-get install -y --no-install-recommends git ffmpeg libgl1 && \
    rm -rf /var/lib/apt/lists/*

# Animate-X source (reference-only third-party code, copied not forked)
COPY animate-x-src /opt/animate-x
WORKDIR /opt/animate-x
RUN pip install --no-cache-dir -r requirements.txt

# Job-queue service layer
COPY requirements-service.txt /opt/service/requirements-service.txt
RUN pip install --no-cache-dir -r /opt/service/requirements-service.txt
COPY server.py job_store.py pipeline.py populate_volume.py checkpoint_manifest.py /opt/service/
ENV PYTHONPATH=/opt/service:$PYTHONPATH

ENV ANIMATE_X_ROOT=/opt/animate-x
ENV ANIMATE_X_WORKSPACE=/workspace/jobs
ENV ANIMATE_X_CHECKPOINTS_DIR=/checkpoints

RUN mkdir -p /workspace/jobs /checkpoints
VOLUME /checkpoints
EXPOSE 8000

CMD ["uvicorn", "server:app", "--app-dir", "/opt/service", "--host", "0.0.0.0", "--port", "8000"]
```

- [ ] **Step 3: Copy the Animate-X source into the build context**

Run: `cd /home/brianh/.dotfiles/animate-x-service && cp -r /home/brianh/promptexecution/animate-x animate-x-src && rm -rf animate-x-src/.git`
Expected: `animate-x-src/` now contains the reference repo's files with no `.git` (avoids nesting a git repo, matching Global Constraints — this is a build-context copy, not a fork).

- [ ] **Step 4: Build the image locally (CPU-capable build; GPU only needed at run time)**

Run: `cd /home/brianh/.dotfiles/animate-x-service && podman build -t localhost/animate-x-service:latest -f Containerfile .`
Expected: build completes successfully (exit 0). This validates the image builds and all service-layer dependencies install — it does not run inference (no checkpoints, no GPU needed for this step).

- [ ] **Step 5: Smoke-test the container starts and the job endpoint responds (no GPU, expect job to fail fast on missing checkpoints — that's OK, we're only checking the server boots)**

Run: `podman run -d --name animate-x-smoke -p 8000:8000 localhost/animate-x-service:latest && sleep 3 && curl -s http://localhost:8000/jobs/nonexistent -w '\n%{http_code}\n'`
Expected: `404` (server is up and routing correctly)

Run: `podman rm -f animate-x-smoke`

- [ ] **Step 6: Commit**

```bash
cd /home/brianh/.dotfiles
git add animate-x-service/Containerfile animate-x-service/requirements-service.txt
echo "animate-x-service/animate-x-src/" >> .gitignore
git add .gitignore
git commit -m "feat(animate-x): add Containerfile and local build smoke test"
```

---

### Task 6: b00t container datum + transmogrify interface

**Files:**
- Create: `_b00t_/animate-x.container.toml`

**Interfaces:**
- Consumes: image name from Task 5 (`localhost/animate-x-service:latest`), and the job-server API shape from Task 4 (`POST /jobs`, `GET /jobs/{id}`).
- Produces: a `datum validate`-passing TOML, documenting the new `transmogrify` interface convention.

- [ ] **Step 1: Write `_b00t_/animate-x.container.toml`**

```toml
[b00t]
name = "animate-x"
type = "docker"
hint = "Animate-X character image animation — async job-queue service. POST /jobs (image, motion) -> job_id; GET /jobs/{id} -> status. Not OpenAI-compatible: generation is too slow for sync request/response."

[b00t.container]
image = "localhost/animate-x-service:latest"
build_file = "animate-x-service/Containerfile"
runtime = "podman"
port = 8000
api = "b00t-job-queue"
gpu_device = "nvidia.com/gpu=all"

[[b00t.container.volumes]]
host = "runpod-network-volume:checkpoints"
container = "/checkpoints"

# ─── transmogrify interface (new b00t capability, first backend) ─────────────
# General contract: image/audio/video in, transformed media out, one or more
# steps. This backend implements image+motion -> animated video.
[b00t.interfaces.transmogrify]
description = "image + motion video -> animated character video"
submit = { method = "POST", path = "/jobs", tokens = ["image", "motion"] }
status = { method = "GET", path = "/jobs/{job_id}" }

# b00t:map v1
# summary: Animate-X async job-queue service — image+motion -> animated video, first transmogrify backend
# tags: container, animate-x, transmogrify, character-animation, runpod, gpu, async-job-queue
# tier: ch0nky
# cmds: b00t datum call animate-x.container transmogrify --token image=<path> --token motion=<path>
# complexity: 5
```

- [ ] **Step 2: Validate the datum**

Run: `b00t datum validate /home/brianh/.dotfiles/_b00t_/animate-x.container.toml`
Expected: `valid — no issues found`

If invalid (e.g., `[b00t.interfaces.transmogrify]` isn't a recognized schema section yet), fall back to documenting the interface as a `[[b00t.usage]]` entry instead:

```toml
[[b00t.usage]]
description = "Submit a transmogrify job (image + motion -> animated video)"
command = "curl -s -X POST http://localhost:8000/jobs -F image=@<path> -F motion=@<path>"

[[b00t.usage]]
description = "Poll job status"
command = "curl -s http://localhost:8000/jobs/<job_id>"
```

and re-run `b00t datum validate` until it passes.

- [ ] **Step 3: Commit**

```bash
cd /home/brianh/.dotfiles
git add _b00t_/animate-x.container.toml
git commit -m "feat(animate-x): add b00t container datum with transmogrify interface"
```

---

### Task 7: RunPod validation run ($2 budget)

**Files:** none (operational task — pushes the image, provisions/populates the volume, runs one real job, tears down).

**Prerequisite:** `RUNPOD_API_KEY` must be set in the environment (from `console.runpod.io` → Settings → API Keys, per the `PROVIDER-RUNPOD` datum). **This is not currently available — get it from the user before starting this task.** Do not attempt any RunPod API call without it.

- [ ] **Step 1: Confirm budget tracking baseline**

Run: `b00t provider runpod list`
Expected: current pod list and cost baseline recorded before any new spend (should show $0 attributable to this task so far).

- [ ] **Step 2: Push the built image to a registry RunPod can pull from**

Tag and push `localhost/animate-x-service:latest` to whatever registry the user's RunPod account is configured to pull from (confirm target registry with the user if not already established elsewhere in this environment — do not assume Docker Hub credentials exist).

- [ ] **Step 3: Provision the network volume and populate it (cheapest available step — check RunPod's pod-type options; use a non-GPU pod for this if RunPod allows attaching a volume without a GPU instance, per the spec's deferred open item)**

Run `populate_volume.py` against the mounted volume from whatever pod type ends up hosting it; confirm all 5 checkpoints report `is_present() == True` afterward via a re-run (second `populate()` call should fetch 0 files).

- [ ] **Step 4: Submit the one-shot validation pod**

Run: `b00t provider runpod submit` with the pushed image, the populated network volume attached at `/checkpoints`, GPU type `NVIDIA RTX 3090` (sm0l tier, $0.44/hr per `PROVIDER-RUNPOD` datum).

- [ ] **Step 5: Run the validation job**

Once the pod's job-queue server is reachable, `POST /jobs` with a tiny test image and short (1-2s) driving clip from `animate-x-src/data/` (pick the smallest available pair). Poll `GET /jobs/{id}` until `complete` or `failed`.

- [ ] **Step 6: Verify the output**

Confirm the resulting video file exists, has non-zero size, and is a valid video (e.g., `ffprobe` reports a video stream and non-zero duration).

- [ ] **Step 7: Stop the pod immediately**

Run: `b00t provider runpod stop` for this pod, regardless of job outcome — success or failure, the paid clock stops here.

- [ ] **Step 8: Record actual cost**

Run: `b00t provider runpod list` again; compute and report the actual dollar cost of this task against the $2.00 cap.

- [ ] **Step 9: Commit validation notes**

```bash
cd /home/brianh/.dotfiles
# add a short RESULTS.md or amend the design spec's "Open items" section with:
# actual cost, GPU type used, wall-clock time, output confirmation
git add docs/superpowers/specs/2026-08-15-animate-x-transmogrify-service-design.md
git commit -m "docs(animate-x): record RunPod validation results"
```

---

## Self-Review Notes

- **Spec coverage:** Architecture (Task 4+5), components 1-5 (Tasks 5,4,3,1,6 respectively), data flow (Task 7), error handling (Tasks 1 sha/size checks, Task 2 failed-job path, Task 7 wall-clock/cost guard), testing (Tasks 1,2 unit tests; Task 3,7 real-hardware validation as the spec specifies) — all covered.
- **No placeholders:** every step has complete code or an exact command; Task 7's registry-push step explicitly asks for user confirmation rather than assuming credentials, which is a real external dependency, not a placeholder.
- **Type consistency:** `run_job(job_id: str, image_path: str, motion_path: str) -> str` signature is identical across `job_store.py` (Task 2), `pipeline.py` (Task 3), and `server.py`'s wiring (Task 4). `JobStore.get()` return shape (`status`/`output_path`/`error`) is identical across Task 2's tests, Task 4's server responses, and the datum's documented interface (Task 6).
