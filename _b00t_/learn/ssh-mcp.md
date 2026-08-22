---
install with --locked: 'cargo install ssh-mcp' (v0.1.3) fails to compile against a freshly-resolved poem-mcpserver -- its #[Tools] macro expects a ToolsCallResponse/output_schema shape poem-mcpserver 0.2.3's latest patch doesn't provide. 'cargo install ssh-mcp --locked' uses the crate's own published Cargo.lock and builds clean, producing both ssh-mcp and ssh-mcp-stdio binaries.
