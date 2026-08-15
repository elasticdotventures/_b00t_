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
    dest_dir = Path(dest_dir)
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
