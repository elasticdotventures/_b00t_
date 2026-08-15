"""pipeline.py - thin subprocess wrapper around Animate-X's two-stage CLI
(process_data.py, inference.py). No unit test: this has no meaningful
behavior without a GPU + real checkpoints, and is validated by the actual
RunPod run (Task 7) instead.

Two real-repo details this accounts for (verified against
~/promptexecution/animate-x source, not assumed from the README alone):
  1. inference.py takes NO --source_image/--output_dir flags -- all
     per-run parameters (image, pose/frame paths, output dir, checkpoint
     paths) come from the YAML config's `test_list_path` / `log_dir` /
     `test_model` keys. We generate a per-job config from the base one.
  2. dwpose/wholebody.py hardcodes onnx checkpoint paths as
     "checkpoints/yolox_l.onnx" and "checkpoints/dw-ll_ucoco_384.onnx",
     relative to the process cwd -- not overridable via config or CLI. A
     `checkpoints` symlink inside animate_x_root pointing at the mounted
     checkpoints_dir satisfies this, and also covers the config's default
     (matching-filename) `open_clip_pytorch_model.bin` /
     `v2-1_512-ema-pruned.ckpt` paths. Only `test_model` needs an explicit
     override, since the config's default filename
     (`animate-x_ckpt.pth`) doesn't match the published checkpoint name
     (`animate-x.pth`) in checkpoint_manifest.py.
"""
import os
import subprocess
from pathlib import Path

import yaml


def _ensure_checkpoints_symlink(animate_x_root: str, checkpoints_dir: str) -> None:
    link = Path(animate_x_root) / "checkpoints"
    target = Path(checkpoints_dir).resolve()
    if link.is_symlink() or link.exists():
        if link.resolve() == target:
            return
        link.unlink()
    link.symlink_to(target)


def _generate_job_config(
    *,
    animate_x_root: str,
    base_config_relpath: str,
    image_path: str,
    saved_pkl_path: str,
    saved_pose_dir: str,
    saved_frame_dir: str,
    output_dir: str,
    seed: int,
) -> Path:
    base_config_path = Path(animate_x_root) / base_config_relpath
    with open(base_config_path) as f:
        cfg = yaml.safe_load(f)

    cfg["test_list_path"] = [
        [1, image_path, saved_pose_dir, saved_frame_dir, saved_pkl_path, seed]
    ]
    cfg["log_dir"] = output_dir
    cfg["test_model"] = "checkpoints/animate-x.pth"

    job_config_path = Path(output_dir) / "job_config.yaml"
    job_config_path.parent.mkdir(parents=True, exist_ok=True)
    with open(job_config_path, "w") as f:
        yaml.safe_dump(cfg, f)
    return job_config_path


def run_animate_x_job(
    job_id: str,
    image_path: str,
    motion_path: str,
    *,
    animate_x_root: str,
    workspace_root: str,
    checkpoints_dir: str,
) -> str:
    _ensure_checkpoints_symlink(animate_x_root, checkpoints_dir)

    job_dir = Path(workspace_root) / job_id
    saved_pkl_dir = job_dir / "saved_pkl"
    saved_pose_dir = job_dir / "saved_pose"
    saved_frame_dir = job_dir / "saved_frames"
    for d in (saved_pkl_dir, saved_pose_dir, saved_frame_dir):
        d.mkdir(parents=True, exist_ok=True)

    env = dict(os.environ)

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

    motion_basename = Path(motion_path).stem
    saved_pkl_path = str(saved_pkl_dir / f"{motion_basename}.pkl")
    per_video_pose_dir = str(saved_pose_dir / motion_basename)
    per_video_frame_dir = str(saved_frame_dir / motion_basename)

    output_dir = job_dir / "results"
    job_config_path = _generate_job_config(
        animate_x_root=animate_x_root,
        base_config_relpath="configs/Animate_X_infer.yaml",
        image_path=image_path,
        saved_pkl_path=saved_pkl_path,
        saved_pose_dir=per_video_pose_dir,
        saved_frame_dir=per_video_frame_dir,
        output_dir=str(output_dir),
        seed=42,
    )

    inference_cmd = ["python3", "inference.py", "--cfg", str(job_config_path)]
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
