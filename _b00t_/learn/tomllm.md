---
tomllm: Agents MUST NEVER read .tomllm files directly — always use `b00t learn &lt;topic&gt;` via MCP or CLI. Direct reads bypass guardrails, guru enrichment, and tribal knowledge annotations injected at render time.

---
PascalCase naming: .tomllm files MUST use PascalCase filenames (e.g. Vllm.InferenceProvider.tomllm, HuggingfaceMcp.mcp.tomllm). Segments separated by dots; each segment PascalCase. Type suffix (mcp, stack, cli, role, etc.) lowercase. Older kebab-case files (inference-qwen3.stack.tomllm) are legacy — new files must use PascalCase.

---
mcp.tomllm format: MCP server datums use .mcp.tomllm extension (not .mcp.toml). Fields: [b00t.mcp] with name, type (stdio/httpstream), url or command. Auth tokens via env var references. Install via `b00t mcp install &lt;Name&gt; claudecode|vscode|dotmcpjson`. httpstream MCPs need url+headers; stdio MCPs need command+args.
