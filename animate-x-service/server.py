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
