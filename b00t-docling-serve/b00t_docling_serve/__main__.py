"""CLI entry point: one-shot Docling extraction.

Output is the same `document_graph/1` JSON shape reqif-opa-mcp's
`reqif_ingest_cli extract` command prints — callers (e.g. ledgrrr's
PdfIngestOp, or reqif-opa-mcp itself when configured with
REQIF_DOCLING_BACKEND=b00t) can treat the two as interchangeable.
"""

from __future__ import annotations

import argparse
import sys

from returns.result import Failure, Result
from typing import Any

from b00t_docling_serve.docling_adapter import extract_docling_document
from b00t_docling_serve.utils import json_dumps


def main() -> None:
    parser = argparse.ArgumentParser(description="b00t Docling extraction CLI")
    subparsers = parser.add_subparsers(dest="command")

    extract = subparsers.add_parser("extract", help="Extract a document graph from a source file")
    extract.add_argument("path", help="Path to a local document")
    extract.add_argument("--profile", default="auto", help="Document profile override (default: auto)")
    extract.add_argument("--source-uri", help="Optional original source URI")
    extract.add_argument("--pretty", action="store_true", help="Pretty-print JSON output")

    args = parser.parse_args()

    if args.command == "extract":
        _handle_json_result(
            extract_docling_document(args.path, source_uri=args.source_uri, profile=args.profile),
            pretty=args.pretty,
        )
        return

    parser.print_help()
    sys.exit(1)


def _handle_json_result(result: Result[Any, Exception], pretty: bool) -> None:
    if isinstance(result, Failure):
        print(f"error: {result.failure()}", file=sys.stderr)
        sys.exit(1)
    print(json_dumps(result.unwrap(), pretty=pretty))


if __name__ == "__main__":
    main()
