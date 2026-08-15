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
