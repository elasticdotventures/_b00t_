---
PyO3 feature control: default-features = false in Cargo workspace dependencies may not fully disable PyO3 if other crates enable it transitively

b00t is big - use metaprogramming: generics, macros, compiler plugins to reduce context
DRY idiomatic semantic abstractions.

# Rust - b00t Gospel

## Core Patterns (2026-05, v0.7.48)

### 1. Clap Reflection → MCP Tool Generation
- trait: `McpReflection` (in `b00t-mcp/src/clap_reflection.rs`)
- Derives MCP `Tool` from clap `Parser` via `CommandFactory`
- `generate_json_schema()` maps clap args → JSON Schema automatically
- `impl_mcp_tool!()` macro wraps struct → command_path + tool name

```rust
#[derive(Parser, Clone)]
pub struct McpListCommand {
    #[arg(long, help = "...")]
    pub json: bool,
}
impl_mcp_tool!(McpListCommand, "b00t_mcp_list", ["mcp", "list"]);
```

Register in `create_mcp_registry()` → execute via `McpCommandRegistry.execute(tool_name, params)`

### 2. ServerHandler Pattern (rmcp SDK)
- `B00tMcpServerRusty` implements `rmcp::handler::server::ServerHandler`
- Override: `get_info()`, `list_tools()`, `call_tool()`, `list_resources()`, `read_resource()`, `on_initialized()`
- `RequestContext<RoleServer>` provides `peer.peer_info()` for client detection

### 3. Client Detection via MCP Initialize
```rust
// In on_initialized(): capture client identity
let client_name = context.peer.peer_info()
    .map(|p| p.client_info.name.clone().unwrap_or_default());

// Or per-call in call_tool(): extract from context
let client_name = context.peer.peer_info()
    .map(|p| p.client_info.name.clone().unwrap_or_default());
```
- Hermes, claude-code, opencode all send `Initialize` with `client_info`
- Use to customize response format, available tools, error verbosity

### 4. Trait-Based Tool Registry (No Runtime Parsing)
- `McpCommandRegistryBuilder` → compile-time tool generation
- Zero runtime parsing failures, full CLAP structure sync
- Registry.execute() dispatches via tool_name → struct instantiation

### 5. B00tContext Library Pattern
```rust
use b00t_c0re_lib::{B00tContext, TemplateRenderer, utils};
use b00t_c0re_lib::learn::get_learn_lesson;

let lesson = get_learn_lesson(path, topic)?;
let renderer = TemplateRenderer::with_defaults()?;
let rendered = renderer.render(&lesson)?;
```

### 6. ACL System
- `AclFilter` checks command against allow/deny regex patterns
- ACL config at `~/.dotfiles/b00t-mcp-acl.toml`
- `allow = ["*"]` for development, specific patterns for production

### 7. Chat Runtime Pattern
- `ChatRuntime::global()` → singleton
- `drain_indicator()` → extracts `<🥾>` markers from command output
- Inject into `CallToolResult` for hermes parsing

### 8. Chat Indicator in Results
```rust
CallToolResult::success(vec![Content::text(serde_json::to_string_pretty(&B00tOutput {
    output: decorated_output,
    success: true,
    server_type: "rusty",
    working_dir: self.working_dir.display().to_string(),
    indicator: indicator.to_string(),
}))])
```

## Workspace Structure
- `b00t-mcp/` - MCP server (rmcp SDK)
- `b00t-cli/` - CLI binary
- `b00t-c0re-lib/` - shared library (templates, context, grok client)
- `b00t-browser-ext/`, `b00t-lib-chat/` - optional components
- workspace Cargo.toml at repo root
- edition = "2024" in all crates (2026-05+)

## Error Handling
- `anyhow::Result` everywhere, `.with_context()` for chain messages
- `McpError` for MCP protocol errors
- `CallToolResult::success()/error()` with `Content::text()`

## B00t-Specific Conventions
- Use 🤓 for non-obvious tribal knowledge comments
- DRY: McpReflection trait eliminates manual schema writing
- `b00t grok learn` for knowledge assimilation
- `b00t lfmf datum abstract` for lesson capture
- Always update learn/rust.md when discovering new patterns

## Pitfalls
- 🦨 `rustc` standalone lint fails on `async fn` (edition not set) — use `cargo check`
- PyO3 transitive features may not respect `default-features=false`
- OAuth disabled until handler trait fixed (see `lib.rs` comments)
- ACP hive + acp_tools disabled — complex NATS dependency
- `b00t-cli-target` vs `b00t-mcp-target`: separate build dirs
- b00t is big - use metaprogramming to reduce context, never paste large blocks

# b00t:map v1
summary: b00t org Rust patterns — clap→MCP reflection, rmcp ServerHandler, client detection, trait registry
tags: rust, mcp, clap, reflection, traits, server-handler, client-detection, workspace, rmcp
tier: ch0nky
cmds: cargo check, cargo test, b00t learn rust
complexity: 6
---
network types: Use std::net types (IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr) instead of raw String for addresses. String matching on IP patterns (h.starts_with("192.168.")) is fragile and wrong. Rust std provides is_loopback(), is_private(), is_link_local(), is_unique_local() on IpAddr — use them. For hostnames, use a PeerAddr enum with type variants, not string parsing. url crate available for URL addresses. Trait-based matching over enum variants beats string match every time.

---
access-control: Implement trust zones and peer permissions as Zanzibar-style relation tuples (user:node relation:can_access object:resource), not ad-hoc enum match. OpenFGA/Auth0 FGA provide existing Rust SDKs. Relation tuples compose naturally across trust zones: zone becomes an object attribute. Use existing std::net types (IpAddr::is_loopback(), is_private()) + url::Host (already in tree) instead of hand-rolled string IP parsing.
