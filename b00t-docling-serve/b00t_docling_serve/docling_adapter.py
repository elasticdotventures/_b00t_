"""Docling-backed extraction for PDF, DOCX, and Markdown.

Relocated from reqif-opa-mcp's `reqif_ingest_cli.docling_adapter` (2026-08-23) —
identical extraction logic, same `document_graph/1` output schema.
"""

from __future__ import annotations

from pathlib import Path

from returns.result import Failure, Result, Success

from b00t_docling_serve.artifact import register_artifact
from b00t_docling_serve.models import ArtifactRecord, DocumentGraph, DocumentNode, SourceAnchor
from b00t_docling_serve.utils import normalize_text, split_paragraphs, stable_id

_SUFFIX_TO_PROFILE: dict[str, str] = {
    ".docx": "docx_docling_v1",
    ".md": "markdown_docling_v1",
    ".pdf": "pdf_docling_v1",
}


def extract_docling_document(
    path: str | Path,
    source_uri: str | None = None,
    profile: str = "auto",
) -> Result[DocumentGraph, Exception]:
    """Extract a canonical document graph from Docling-supported formats."""
    try:
        resolved = Path(path).expanduser().resolve()
        suffix = resolved.suffix.lower()
        resolved_profile = _SUFFIX_TO_PROFILE.get(suffix)
        if profile != "auto":
            resolved_profile = profile
        if resolved_profile is None:
            return Failure(ValueError(f"Docling extraction is not configured for: {resolved.name}"))

        artifact_result = register_artifact(
            resolved,
            source_uri=source_uri,
            document_profile=resolved_profile,
        )
        if isinstance(artifact_result, Failure):
            return artifact_result
        artifact = artifact_result.unwrap()

        if suffix == ".pdf":
            try:
                pdf_graph = _extract_pdf_with_pypdf(resolved, artifact, None)
                if pdf_graph.nodes:
                    return Success(pdf_graph)
            except (ModuleNotFoundError, ImportError) as exc:
                return Failure(_missing_pypdf_error(resolved, exc))
            except Exception:
                # Fall back to Docling-based extraction on unexpected PyPDF errors.
                pass

            try:
                from docling.datamodel.base_models import InputFormat
                from docling.datamodel.pipeline_options import PdfPipelineOptions
                from docling.document_converter import DocumentConverter, PdfFormatOption
            except (ImportError, ModuleNotFoundError) as exc:
                return Failure(_missing_docling_error(resolved, exc))

            try:
                converter = DocumentConverter(
                    allowed_formats=[InputFormat.PDF],
                    format_options={
                        InputFormat.PDF: PdfFormatOption(
                            pipeline_options=PdfPipelineOptions(
                                do_ocr=False,
                                do_table_structure=False,
                                force_backend_text=True,
                            )
                        )
                    },
                )
                conversion = converter.convert(resolved)
                document = conversion.document
                if document is None:
                    return Failure(ValueError(f"Docling produced no document for: {resolved.name}"))
            except Exception as exc:
                return Success(_extract_pdf_with_pypdf(resolved, artifact, exc))
        else:
            try:
                from docling.document_converter import DocumentConverter
            except (ImportError, ModuleNotFoundError) as exc:
                return Failure(_missing_docling_error(resolved, exc))
            converter = DocumentConverter()
            conversion = converter.convert(resolved)
            document = conversion.document
            if document is None:
                return Failure(ValueError(f"Docling produced no document for: {resolved.name}"))

        nodes: list[DocumentNode] = []
        heading_path: list[str] = []

        for item, level in document.iterate_items(with_groups=False):
            text = normalize_text(getattr(item, "text", ""))
            if not text:
                continue

            label = getattr(getattr(item, "label", None), "value", "text")
            if label == "section_header":
                heading_path = heading_path[: max(level - 1, 0)]
                heading_path.append(text)
                section_node_id = stable_id("section", artifact.artifact_id, *heading_path)
                nodes.append(
                    DocumentNode(
                        node_id=section_node_id,
                        node_type="section",
                        text=text,
                        parent_id=None,
                        semantic_id=stable_id("semantic", *heading_path),
                        attributes={"label": label, "level": level},
                        anchors=[
                            SourceAnchor(
                                kind="docling_section",
                                artifact_id=artifact.artifact_id,
                                page=_first_page(item),
                                heading_path=list(heading_path),
                                semantic_id=stable_id("semantic", *heading_path),
                            )
                        ],
                    )
                )
                continue

            parent_id = stable_id("section", artifact.artifact_id, *heading_path) if heading_path else None
            semantic_id = stable_id(
                "semantic",
                *heading_path,
                text[:96],
            )
            for paragraph_index, paragraph in enumerate(split_paragraphs(text), start=1):
                node_id = stable_id(
                    "paragraph",
                    artifact.artifact_id,
                    label,
                    _first_page(item),
                    paragraph_index,
                    paragraph[:96],
                )
                nodes.append(
                    DocumentNode(
                        node_id=node_id,
                        node_type="paragraph",
                        text=paragraph,
                        parent_id=parent_id,
                        semantic_id=semantic_id,
                        attributes={
                            "label": label,
                            "heading_path": " > ".join(heading_path),
                        },
                        anchors=[
                            SourceAnchor(
                                kind="docling_paragraph",
                                artifact_id=artifact.artifact_id,
                                page=_first_page(item),
                                paragraph=paragraph_index,
                                heading_path=list(heading_path),
                                semantic_id=semantic_id,
                            )
                        ],
                    )
                )

        return Success(
            DocumentGraph(
                schema="document_graph/1",
                artifact=artifact,
                profile=resolved_profile,
                nodes=nodes,
                metadata={"extractor": "docling"},
            )
        )
    except Exception as exc:
        return Failure(exc)


def _missing_pypdf_error(path: Path, exc: Exception) -> Exception:
    """Build a helpful error explaining that the PyPDF-based extractor is unavailable."""
    return RuntimeError(
        "PyPDF-based PDF extraction failed for "
        f"'{path}': {exc}. "
        "The 'pypdf' dependency is optional; install the 'docling' extra, e.g.:\n"
        "    uv sync --extra docling"
    )


def _first_page(item: object) -> int | None:
    """Get the first available page number from a Docling item."""
    provenance = getattr(item, "prov", None)
    if not provenance:
        return None
    return getattr(provenance[0], "page_no", None)


def _extract_pdf_with_pypdf(
    path: Path,
    artifact: ArtifactRecord,
    docling_error: Exception | None,
) -> DocumentGraph:
    """Fallback PDF extractor when Docling cannot initialize offline models."""
    from pypdf import PdfReader

    reader = PdfReader(str(path))
    nodes: list[DocumentNode] = []
    for page_number, page in enumerate(reader.pages, start=1):
        text = normalize_text(page.extract_text() or "")
        if not text:
            continue
        for paragraph_index, paragraph in enumerate(split_paragraphs(text), start=1):
            semantic_id = stable_id("semantic", path.name, page_number, paragraph[:96])
            node_id = stable_id(
                "paragraph",
                artifact.artifact_id,
                page_number,
                paragraph_index,
                paragraph[:96],
            )
            nodes.append(
                DocumentNode(
                    node_id=node_id,
                    node_type="paragraph",
                    text=paragraph,
                    parent_id=None,
                    semantic_id=semantic_id,
                    attributes={"label": "pdf_text", "extractor": "pypdf"},
                    anchors=[
                        SourceAnchor(
                            kind="pdf_page_paragraph",
                            artifact_id=artifact.artifact_id,
                            page=page_number,
                            paragraph=paragraph_index,
                            semantic_id=semantic_id,
                        )
                    ],
                )
            )

    return DocumentGraph(
        schema="document_graph/1",
        artifact=artifact,
        profile="pdf_docling_v1",
        nodes=nodes,
        metadata={
            "extractor": "pypdf",
            "fallback_reason": str(docling_error) if docling_error is not None else None,
        },
    )


def _missing_docling_error(path: Path, import_error: Exception) -> ValueError:
    """Return a guided install error when the docling extra is missing."""
    return ValueError(
        f"Docling extractor is not available for {path.name}. "
        f"Install optional dependency group 'docling' via 'uv sync --extra docling'. "
        f"Import error: {import_error}"
    )
