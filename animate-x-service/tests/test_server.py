import io
import time

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
