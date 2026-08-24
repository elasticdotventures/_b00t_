# b00t-docling-serve

b00t's own Docling-backed document extraction (PDF/DOCX/Markdown), relocated
out of [`reqif-opa-mcp`](https://github.com/PromptExecution/reqif-opa-mcp)
(2026-08-23) so it's a standalone, independently-versioned capability rather
than bundled inside a ReqIF/OPA/SARIF-specific tool.

Two ways to use it, both non-perpetual:

- **One-shot CLI**: `uv run python -m b00t_docling_serve extract <path> --profile auto`
  — starts, extracts, exits. This is what `reqif-opa-mcp` and `ledgrrr`'s
  `PdfIngestOp` call directly.
- **On-demand NATS Micro Service** (`b00t_docling_serve.nats_service`,
  service `ledgrrr-docling`, endpoint `extract`): for shared/networked use
  across multiple consumers. Tracks time since its last request and exits
  cleanly after `IDLE_TIMEOUT_SECONDS` (default 300s) idle — see
  `deploy/quadlet/docling-nats-service.container` for the Podman Quadlet
  unit, which deliberately has no `[Install]` section so it's never
  auto-started at boot.

Output is the same `document_graph/1` JSON schema `reqif-opa-mcp`'s
`reqif_ingest_cli extract` produces — the two are interchangeable.

## Install

```bash
uv sync --extra docling          # one-shot CLI
uv sync --extra docling --extra nats-service   # + NATS service mode
```

## Consumers

- `reqif-opa-mcp`: configurable via `REQIF_DOCLING_BACKEND` (`b00t` is the
  default/preferred backend, `local` falls back to its own in-process
  extractor) — see `reqif_ingest_cli/docling_backend.py`.
- `ledgrrr`'s `PdfIngestOp` (`ledger-core/src/ledger_ops.rs`): shells out to
  this package's CLI directly.
