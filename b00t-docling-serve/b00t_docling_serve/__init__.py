"""b00t's own Docling-backed document extraction.

Relocated out of `reqif-opa-mcp` (2026-08-23) so Docling extraction is a
standalone, independently-versioned b00t capability rather than bundled
inside a ReqIF/OPA/SARIF-specific tool. `reqif-opa-mcp` now depends on
this package (as its preferred backend, configurable back to its own
in-process extractor) instead of owning the Docling wrapper itself — see
`reqif_ingest_cli/docling_backend.py` there.
"""
