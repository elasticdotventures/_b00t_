#!/usr/bin/env python3
"""SAM3 local runner — reads request.json, writes result.json.

Request schema: Sam3JobRequest (see b00t-c0re-lib/src/ai_capability.rs)
Output schema:  SamSegmentationOutput

Usage (direct):  python3 run.py /workspace/request.json
Usage (podman):  podman run --device nvidia.com/gpu=all \\
                   -v /host/request.json:/workspace/request.json:ro \\
                   -v /host/output:/workspace:rw \\
                   app4dog/sam3-runner:local
"""
from __future__ import annotations

import base64
import json
import sys
import time
from io import BytesIO
from pathlib import Path
from typing import Any

import numpy as np
from PIL import Image


# ── Schema constants ─────────────────────────────────────────────────────────

DEFAULT_MODEL_ID = "facebook/sam3"
DEFAULT_SCORE_THRESHOLD = 0.0
DEFAULT_OUTPUT_FORMAT = "rle"  # "rle" | "png_base64" | "polygon"


# ── RLE helpers ───────────────────────────────────────────────────────────────

def mask_to_rle(mask: np.ndarray) -> dict[str, Any]:
    """COCO-style RLE encoding for a binary mask."""
    flat = mask.flatten(order="F").astype(np.uint8)
    counts, current = [], 0
    for px in flat:
        if px == current:
            counts[-1] += 1 if counts else None
            if not counts:
                counts.append(1)
            else:
                counts[-1] += 1
        else:
            counts.append(1)
            current = px
    # rebuild correctly
    counts = []
    prev = 0
    run = 0
    for px in flat:
        if px == prev:
            run += 1
        else:
            counts.append(run)
            run = 1
            prev = px
    counts.append(run)
    return {"size": list(mask.shape), "counts": counts}


def mask_to_png_b64(mask: np.ndarray) -> str:
    img = Image.fromarray((mask * 255).astype(np.uint8), mode="L")
    buf = BytesIO()
    img.save(buf, format="PNG")
    return base64.b64encode(buf.getvalue()).decode()


def mask_to_polygon(mask: np.ndarray) -> list[list[float]]:
    try:
        import cv2
        contours, _ = cv2.findContours(
            mask.astype(np.uint8), cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE
        )
        if not contours:
            return []
        c = max(contours, key=cv2.contourArea)
        return c.reshape(-1, 2).tolist()
    except ImportError:
        return []


def encode_mask(mask: np.ndarray, fmt: str) -> dict[str, Any]:
    if fmt == "png_base64":
        return {"format": "png_base64", "data": mask_to_png_b64(mask)}
    if fmt == "polygon":
        return {"format": "polygon", "data": mask_to_polygon(mask)}
    return {"format": "rle", "data": mask_to_rle(mask)}


# ── Model loading ─────────────────────────────────────────────────────────────

def load_model(model_id: str, device: str):
    """Load SAM3 via transformers pipeline or direct model API."""
    try:
        from transformers import pipeline as hf_pipeline
        # SAM3 uses the "mask-generation" or "image-segmentation" task
        pipe = hf_pipeline(
            "image-segmentation",
            model=model_id,
            device=0 if device == "cuda" else -1,
        )
        return ("pipeline", pipe)
    except Exception:
        pass

    # Fallback: direct AutoModel load
    from transformers import AutoProcessor, AutoModelForImageSegmentation
    import torch
    processor = AutoProcessor.from_pretrained(model_id)
    model = AutoModelForImageSegmentation.from_pretrained(
        model_id, torch_dtype=torch.float16 if device == "cuda" else torch.float32
    )
    model = model.to(device)
    return ("automodel", (processor, model))


# ── Inference ─────────────────────────────────────────────────────────────────

def run_inference(model_handle, image: Image.Image, prompts: list[dict], req: dict) -> list[dict]:
    """Run SAM3 inference, returning list of segment dicts."""
    device = req.get("device", "cuda")
    score_threshold = float(req.get("score_threshold", DEFAULT_SCORE_THRESHOLD))
    output_fmt = req.get("output_format", DEFAULT_OUTPUT_FORMAT)

    kind, handle = model_handle

    results = []

    if kind == "pipeline":
        pipe = handle
        # Build prompt kwargs for SAM3 — text, box, point
        text_prompts = [p["value"] for p in prompts if p["type"] == "text"]
        box_prompts = [p["value"] for p in prompts if p["type"] == "box"]
        point_prompts = [(p["value"]["coords"], p["value"]["label"])
                         for p in prompts if p["type"] == "point"]

        pipe_kwargs: dict[str, Any] = {}
        if text_prompts:
            pipe_kwargs["candidate_labels"] = text_prompts
        if box_prompts:
            pipe_kwargs["boxes"] = [box_prompts]
        if point_prompts:
            pipe_kwargs["points_per_side"] = None  # disable auto-gen

        segments = pipe(image, **pipe_kwargs) if pipe_kwargs else pipe(image)
        if segments is None:
            segments = []

        for seg in segments:
            score = float(seg.get("score") or 0.0)
            if score < score_threshold:
                continue
            mask_arr = np.array(seg["mask"]) if isinstance(seg["mask"], Image.Image) \
                       else np.array(seg["mask"])
            box = seg.get("box") or {}
            results.append({
                "mask": encode_mask(mask_arr > 0, output_fmt),
                "score": score,
                "label": seg.get("label", ""),
                "box": [box.get("xmin", 0), box.get("ymin", 0),
                        box.get("xmax", 0), box.get("ymax", 0)],
            })

    else:
        # automodel path
        processor, model = handle
        import torch

        # Build inputs
        text_prompts = [p["value"] for p in prompts if p["type"] == "text"] or None
        inputs = processor(images=image, text=text_prompts, return_tensors="pt")
        inputs = {k: v.to(device) for k, v in inputs.items()}

        with torch.no_grad():
            outputs = model(**inputs)

        # Post-process masks
        masks = processor.post_process_segmentation(outputs, target_sizes=[image.size[::-1]])
        for i, seg in enumerate(masks[0].get("segments_info", [])):
            score = float(seg.get("score", 0.0))
            if score < score_threshold:
                continue
            mask_id = seg["id"]
            mask_arr = (masks[0]["segmentation"].cpu().numpy() == mask_id)
            results.append({
                "mask": encode_mask(mask_arr, output_fmt),
                "score": score,
                "label": seg.get("label_id", ""),
                "box": [0, 0, 0, 0],
            })

    return results


# ── Main ──────────────────────────────────────────────────────────────────────

def main(request_path: str) -> None:
    req_file = Path(request_path)
    if not req_file.exists():
        print(f"ERROR: request file not found: {request_path}", file=sys.stderr)
        sys.exit(1)

    req = json.loads(req_file.read_text())
    output_path = Path(req.get("output_path", "/workspace/result.json"))
    model_id = req.get("model_id", DEFAULT_MODEL_ID)
    device = req.get("device", "cuda")

    # Load image
    image_path = req.get("image_path")
    image_url = req.get("image_url")
    image_b64 = req.get("image_base64")

    if image_path:
        image = Image.open(image_path).convert("RGB")
    elif image_url:
        import requests as req_lib
        image = Image.open(BytesIO(req_lib.get(image_url, timeout=30).content)).convert("RGB")
    elif image_b64:
        image = Image.open(BytesIO(base64.b64decode(image_b64))).convert("RGB")
    else:
        print("ERROR: request must contain image_path, image_url, or image_base64", file=sys.stderr)
        sys.exit(1)

    prompts = req.get("prompts", [])
    print(f"SAM3 runner: model={model_id} device={device} image={image.size} prompts={len(prompts)}")

    t0 = time.monotonic()
    model_handle = load_model(model_id, device)
    t_load = time.monotonic() - t0

    t1 = time.monotonic()
    segments = run_inference(model_handle, image, prompts, req)
    t_infer = time.monotonic() - t1

    result = {
        "schema_version": "sam3.v1",
        "model_id": model_id,
        "image_size": list(image.size),
        "segments": segments,
        "segment_count": len(segments),
        "timing": {"load_s": round(t_load, 3), "infer_s": round(t_infer, 3)},
    }

    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(result, indent=2))
    print(f"SAM3: {len(segments)} segments → {output_path} (load={t_load:.1f}s infer={t_infer:.1f}s)")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "/workspace/request.json")
