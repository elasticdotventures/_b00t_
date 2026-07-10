---
mcp path configuration: Use --path flag to specify MCP datum location when datums are in subdirectory. Default searches ~/.dotfiles/_b00t_/ but datum organization may use nested _b00t_/_b00t_/ structure. Verify with: b00t-cli --path ~/.dotfiles/_b00t_/_b00t_ mcp list

dotmcpjson target for self-install: Install b00t-mcp to project .mcp.json using 'b00t-cli mcp install b00t-mcp dotmcpjson'. Added in commit f499a97. Enables self-bootstrapping of b00t MCP server into any project. Use just b00t::mcp-self-install for convenience.

rustc version upgrades: When dependencies require newer rustc (e.g., rig-core needs 1.88+ for unstable let expressions), upgrade with 'rustup update stable && rustup override set stable' from project root. Never use manual workarounds or skip builds - fix the toolchain.

just module invocation: Justfile modules (e.g., 'mod b00t') must be invoked from the justfile root where module is declared. Use 'just b00t::recipe' not 'just recipe' when in subdirectory. cd to project root or use -f flag.


---
🦨 b00t-run-hallucination: The help text says b00t run <name> but b00t run is not a CLI command. It's a hidden shortcut RunDatum that the help text describes as datum dispatch. Should be b00t <name> directly, not b00t run <name>. Fix help text and auto-dispatch.
datum validation paths: Use absolute paths when validating project-local datum files from recipes; relative paths may resolve as global datum keys.

---
salvage-first meta-pattern: an outer layer (CLI/MCP) must never validate stricter than the layer it fronts; degrade malformed input to salvage plus telemetry, never bail-and-discard — and keep diagnostics out of data channels (return empty vecs, print hints to stderr)

---
durable-first persistence: record_lesson tried vector store before filesystem write — SIGPIPE from a truncating pipe (lfmf ... | head -1) killed the process between them and lost the payload; cheap local store MUST come first, enrichment second
