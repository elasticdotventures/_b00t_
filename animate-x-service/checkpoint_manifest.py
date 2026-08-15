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
