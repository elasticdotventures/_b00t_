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
