---
name: rust
description: "Rust development skill — requires rust-doc MCP server. Enforces agentic guards for safe code generation."
version: 1.0.0
platforms: [linux, macos]
---

# Rust Development Skill

## Guard: MCP Server Required

**This skill REFUSES to operate without the `rust-doc` MCP server running.**

Before any Rust code generation, the agent MUST:
1. Check that `vendor/rust-docs-mcp-server/target/release/rustdocs_mcp_server` exists
2. If not built, run: `cd vendor/rust-docs-mcp-server && cargo build --release`
3. Start the server in background: `vendor/rust-docs-mcp-server/target/release/rustdocs_mcp_server "serde@^1.0" &`

## Agentic Guards

When generating Rust code, the agent MUST:
- Query the rust-doc MCP server for crate APIs before writing code
- Never invent function signatures — verify via `query_rust_docs`
- Use the latest documentation, not training data
- Prefer safe Rust patterns (avoid `unsafe` without explicit justification)

## Preferences

- Dependency management: `cargo add` over manual Cargo.toml edits
- Testing: `#[cfg(test)]` with `cargo test` 
- Formatting: `cargo fmt` before commit
- Clippy: `cargo clippy -- -D warnings` for CI
- Edition: 2024 preferred
- Error handling: `anyhow` for applications, `thiserror` for libraries

## Quick Reference

```bash
# Build and start the MCP server
cd vendor/rust-docs-mcp-server && cargo build --release
./target/release/rustdocs_mcp_server "serde@^1.0" &

# Query docs via MCP tool
# (available after server starts, via query_rust_docs tool)
```
