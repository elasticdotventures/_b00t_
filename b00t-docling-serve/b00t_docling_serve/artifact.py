"""Artifact registration and hashing (mirrors reqif-opa-mcp's
`reqif_ingest_cli.artifact`)."""

from __future__ import annotations

import hashlib
import mimetypes
from pathlib import Path

from returns.result import Failure, Result, Success

from b00t_docling_serve.models import ArtifactRecord
from b00t_docling_serve.utils import stable_id, utc_now_iso

_EXTENSION_TO_FORMAT: dict[str, str] = {
    ".docx": "docx",
    ".md": "markdown",
    ".pdf": "pdf",
}
_EXTENSION_TO_MEDIA_TYPE: dict[str, str] = {
    ".docx": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ".md": "text/markdown",
    ".pdf": "application/pdf",
}


def register_artifact(
    path: str | Path,
    source_uri: str | None = None,
    document_profile: str | None = None,
) -> Result[ArtifactRecord, Exception]:
    """Register an immutable artifact record for a local file."""
    try:
        resolved = Path(path).expanduser().resolve()
        if not resolved.exists() or not resolved.is_file():
            return Failure(FileNotFoundError(f"Artifact not found: {resolved}"))

        sha256 = _compute_sha256(resolved)
        suffix = resolved.suffix.lower()
        media_type = _EXTENSION_TO_MEDIA_TYPE.get(suffix) or mimetypes.guess_type(resolved.name)[0]
        file_format = _EXTENSION_TO_FORMAT.get(suffix, suffix.lstrip(".") or "binary")

        artifact = ArtifactRecord(
            schema="artifact/1",
            artifact_id=stable_id("artifact", resolved.name, sha256),
            sha256=sha256,
            filename=resolved.name,
            source_path=str(resolved),
            source_uri=source_uri,
            media_type=media_type or "application/octet-stream",
            file_format=file_format,
            size_bytes=resolved.stat().st_size,
            ingested_at=utc_now_iso(),
            document_profile=document_profile,
        )
        return Success(artifact)
    except Exception as exc:
        return Failure(exc)


def _compute_sha256(path: Path) -> str:
    """Hash a file with a stable chunk size."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()
