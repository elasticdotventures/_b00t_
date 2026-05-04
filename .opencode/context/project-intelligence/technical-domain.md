<!-- Context: project-intelligence/technical | Priority: critical | Version: 1.1 | Updated: 2026-05-04 -->

# Technical Domain

> b00t is a Rust 2024 edition workspace (0.8.0) providing an agentic CLI framework with TOML-based datum configuration, MCP server management, hive CMDB, and RHAI scripting for AI engineering workflows.

## Quick Reference

- **Purpose**: Understand how b00t works technically — CLI architecture, datum system, MCP management, hive CMDB, RHAI scripting
- **Update When**: New CLI commands, datum types, guard patterns, MCP targets, dependency changes
- **Audience**: Developers, DevOps, AI engineers extending b00t

## Tech Stack

| Layer | Technology | Version | Rationale |
|-------|-----------|---------|-----------|
| Language | Rust | 2024 edition | Safety, performance, cross-compilation |
| CLI Framework | clap | 4.5.28 | Derive-based Parser with nested subcommands |
| Async Runtime | tokio | 1.44.2 | Full async for MCP proxy, registry, IPC |
| Scripting | rhai | 1.24 (sync) | Embedded scripting for guards, hooks, pipelines |
| MCP Protocol | rmcp | 0.8.5 | Client/server/transport for Model Context Protocol |
| Config | toml + serde | 0.9 / 1.0 | Datum configs in `_b00t_/*.toml` files |
| IPC | zbus | 5 (tokio) | D-Bus integration for system services |
| Telemetry | opentelemetry | 0.31 | Tracing and observability |

## Architecture Pattern

```
Type: Agent-based CLI framework
Pattern: Monorepo workspace with 11 crates, TOML-driven datum registry, RHAI scripting engine
```

### Why This Architecture?

Rust provides the safety and performance needed for a CLI tool that manages system state (installs, guards, MCP servers). The workspace split isolates concerns: `b00t-cli` (CLI dispatch), `b00t-c0re-lib` (shared engine), `b00t-mcp` (MCP protocol), `b00t-grok` (RAG), `k0mmand3r` (agent coordination). TOML datums replace hardcoded config with discoverable, typed files.

## Project Structure

```
_b00t_/
├── b00t-cli/              # Main CLI binary (clap Parser, 40+ subcommands)
├── b00t-c0re-lib/         # Shared engine: rhai, grok, irontology bridge
├── b00t-mcp/              # MCP server binary
├── b00t-grok/             # RAG knowledge base client
├── b00t-lib-chat/         # Chat library
├── b00t-ipc/              # Inter-process communication
├── b00t-azure-cp/         # Azure control plane
├── b00t-l3dg3rr-viz/      # Visualization
├── b00t-py/               # Python bindings
├── k0mmand3r/             # Agent coordination protocol
├── b00t-ast/              # AST utilities
└── _b00t_/                # Datum TOML files, scripts, hive profiles
```

## Key Technical Decisions

| Decision | Rationale | Impact |
|----------|-----------|--------|
| TOML datum files | Discoverable, typed, git-friendly config per tool | All tools defined as `_b00t_/<name>.<type>.toml` |
| RHAI for scripting | Safe embedded scripting without shell injection | Guards, hooks, pipelines in rhai not bash |
| Gate preconditions | Late-binding install checks from datum metadata | `[[b00t.gate]]` auto-derived from requires/env |
| Hive CMDB | System state profiles with resource gating | Mutual exclusion between inference/download profiles |
| MCP multi-target | Single source of truth → multiple agent platforms | `b00t-cli mcp install <name> <target>` |

## Key Patterns

### Datum System
Every tool/config is a `.toml` file in `_b00t_/` with a typed suffix (`.cli.toml`, `.mcp.toml`, `.hive.toml`, `.bash.toml`, etc.). The `BootDatum` struct (b00t-cli/src/lib.rs:207) deserializes all fields: `name`, `type`, `install`, `version`, `gate`, `depends_on`, `mcp`, `env`, `require`, `hook_*`, etc.

### Gate System
`[[b00t.gate]]` on BootDatum defines preconditions: `command` (on PATH), `file` (exists), `env` (set), `rhai` (expression). Auto-derived from `requires`/`env` fields. Evaluated by `gate_check()` in rhai engine (b00t-c0re-lib/src/rhai_engine.rs:190-252). Uses `which` for commands, `~` expansion for files, `.env` fallback for env vars.

### MCP Management
`b00t-cli mcp` subcommands: `register` (JSON/command mode), `list` (with `--search`/`--installed`/`--running`/`--suspended`/`--all`/`--max-threshold` filters), `install` (targets: claudecode, vscode, geminicli, dotmcpjson, roocode, codex, stdout), `sync` (bidirectional push/pull), `execute` (stdio proxy), `output`, `registry`, `depends`, `status`. Threshold guard: >10 servers requires filter.

### Hive CMDB
`b00t hive` subcommands: `status` (RAM/GPU/CPU/services), `list` (profiles), `plan` (dry-run gate check), `activate` (stop/start systemd services), `run` (guard evaluation), `peers` (discovery/gossip), `cyber` (trust boundary). Guards defined in `_b00t_/hive-guards.hive.toml` with rhai macros (`pip_guard`, `docker_guard`, etc.) and action types: `warn`, `block`, `redirect`.

### RHAI Pipeline
Scripts in `_b00t_/scripts/*.rhai` use map/filter/reduce pattern. Registered functions: `gate_check(kind, spec)`, `kg_query(kind, val)`, `session_track(event, detail)`, `run_cmd(cmd)`, `command_exists(cmd)`, `file_exists(path)`, `read_file(path)`, `get_env(var)`, `log_info/warn/error/success`. Example: `install-mcp-recommended.rhai` discovers datums, derives gates, evaluates, batch-installs.

### Naming Conventions
- `snake_case` for files and directories
- `PascalCase` for types, enums, structs
- `camelCase` for functions and methods
- `SCREAMING_SNAKE_CASE` for constants
- TOML for all configuration
- Datum files: `<name>.<type>.toml` (e.g., `docker.cli.toml`, `filesystem.mcp.toml`)

### Security
- `gate_check` uses `Command::new("which")` not `sh -c` — no shell injection
- `kg_query` pipes to `b00t-mcp --stdio` via stdin, never string interpolation
- Telemetry written to `~/.b00t/telemetry.jsonl`
- Hive guards block destructive commands (`rm -rf /`, `git push --force` to main)

## 📂 Codebase References

| File | What It Contains |
|------|-----------------|
| `b00t-cli/src/lib.rs` | BootDatum, GateSpec, exit_code, McpListFilter, McpListItem, ansi module |
| `b00t-cli/src/main.rs` | Cli Parser with 40+ subcommands, Commands enum |
| `b00t-cli/src/commands/mcp.rs` | McpCommands (Register/List/Install/Sync/Output/Registry/Execute/Status) |
| `b00t-cli/src/commands/hive.rs` | HiveCommands (Status/List/Plan/Activate/Run/Peers), guard evaluation |
| `b00t-cli/src/commands/install.rs` | install_datum with dependency resolution |
| `b00t-cli/src/session_memory.rs` | SessionMemory, SessionConfig with mcp_list_threshold |
| `b00t-cli/src/whoami.rs` | whoami with --json, role resolution, skill loading |
| `b00t-cli/src/k0mmand3r/mod.rs` | K0mmand3rCmd::parse, LoopSpec::from_tokens |
| `b00t-cli/src/wow.rs` | CheckResult, BuildIntegrityCheck, spline tests |
| `b00t-c0re-lib/src/rhai_engine.rs` | gate_check, kg_query, session_track registered functions |
| `b00t-c0re-lib/src/irontology_bridge.rs` | DatumNode, IrontologyBridgeClient, NeumannStore stubs |
| `b00t-c0re-lib/src/dual_grok.rs` | GrokBackend enum (Raglite/Irontology/CodebaseMemory/Both) |
| `_b00t_/hive-guards.hive.toml` | Guard patterns with rhai macros |
| `_b00t_/scripts/install-mcp-recommended.rhai` | Map/filter/reduce pipeline |
| `Cargo.toml` | Workspace config, version 0.8.0, edition 2024 |

## Related Files

- `business-domain.md` — Why b00t exists (AI engineering agent framework)
- `navigation.md` — Quick routes to all project intelligence
