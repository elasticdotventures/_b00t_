#!/usr/bin/env python3
"""kreuzberg MCP wrapper — exposes document extraction via MCP tools.

Usage:
  uv run _b00t_/scripts/kreuzberg-mcp-wrapper.py

Or via the MCP datum:
  python3 _b00t_/scripts/kreuzberg-mcp-wrapper.py
"""
import asyncio
import json
import sys
from pathlib import Path

HAS_KREUZBERG = False
try:
    import kreuzberg
    from kreuzberg import extract_file, extract_bytes
    HAS_KREUZBERG = True
except ImportError:
    pass


TOOLS = []
if HAS_KREUZBERG:
    TOOLS = [
        {
            "name": "kreuzberg_extract_file",
            "description": "Extract text content from a document file (PDF, DOCX, images, etc.)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path to the document file"},
                },
                "required": ["path"],
            },
        },
        {
            "name": "kreuzberg_extract_bytes",
            "description": "Extract text content from raw document bytes (base64-encoded)",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "data": {"type": "string", "description": "Base64-encoded document bytes"},
                    "mime_type": {"type": "string", "description": "MIME type (e.g. application/pdf)"},
                },
                "required": ["data", "mime_type"],
            },
        },
        {
            "name": "kreuzberg_list_formats",
            "description": "List supported document formats",
            "inputSchema": {"type": "object", "properties": {}},
        },
    ]


async def handle_extract_file(args: dict) -> dict:
    path = args["path"]
    try:
        result = await extract_file(path)
        content = result.content or ""
        return {
            "content": [
                {"type": "text", "text": content},
                {"type": "text", "text": json.dumps({
                    "mime_type": result.mime_type,
                    "pages": getattr(result, "get_page_count", lambda: None)(),
                    "char_count": len(content),
                })},
            ]
        }
    except Exception as e:
        return {"content": [{"type": "text", "text": f"Error: {e}"}], "isError": True}


async def handle_extract_bytes(args: dict) -> dict:
    import base64
    data = base64.b64decode(args["data"])
    mime_type = args["mime_type"]
    try:
        result = await extract_bytes(data, mime_type=mime_type)
        content = result.content or ""
        return {"content": [{"type": "text", "text": content}]}
    except Exception as e:
        return {"content": [{"type": "text", "text": f"Error: {e}"}], "isError": True}


async def handle_list_formats() -> dict:
    return {
        "content": [
            {"type": "text", "text": json.dumps({
                "supported_formats": [
                    "text/plain", "application/pdf", "image/png", "image/jpeg",
                    "image/tiff", "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                    "text/html", "text/markdown",
                ],
                "engine": "kreuzberg",
            }, indent=2)},
        ]
    }


async def handle_request(request: dict) -> dict | None:
    method = request.get("method", "")
    req_id = request.get("id")

    if method == "initialize":
        return {
            "jsonrpc": "2.0", "id": req_id,
            "result": {
                "protocolVersion": "2025-03-26",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "kreuzberg-mcp", "version": "0.1.0"},
            },
        }
    elif method == "tools/list":
        return {"jsonrpc": "2.0", "id": req_id, "result": {"tools": TOOLS}}
    elif method == "tools/call":
        params = request.get("params", {})
        name = params.get("name", "")
        args = params.get("arguments", {})
        if not HAS_KREUZBERG:
            result = {
                "content": [{"type": "text", "text": "kreuzberg not installed. Run: uv tool install kreuzberg"}],
                "isError": True,
            }
        elif name == "kreuzberg_extract_file":
            result = await handle_extract_file(args)
        elif name == "kreuzberg_extract_bytes":
            result = await handle_extract_bytes(args)
        elif name == "kreuzberg_list_formats":
            result = await handle_list_formats()
        else:
            result = {"content": [{"type": "text", "text": f"Unknown tool: {name}"}], "isError": True}
        return {"jsonrpc": "2.0", "id": req_id, "result": result}
    elif method == "notifications/initialized":
        return None
    else:
        return {"jsonrpc": "2.0", "id": req_id, "error": {"code": -32601, "message": f"Unknown method: {method}"}}


async def main() -> None:
    while True:
        line = await asyncio.get_event_loop().run_in_executor(None, sys.stdin.readline)
        if not line:
            break
        line = line.strip()
        if not line:
            continue
        request = json.loads(line)
        response = await handle_request(request)
        if response is not None:
            sys.stdout.write(json.dumps(response) + "\n")
            sys.stdout.flush()


if __name__ == "__main__":
    asyncio.run(main())
