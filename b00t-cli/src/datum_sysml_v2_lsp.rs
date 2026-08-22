//! SysML v2 LSP + bundled MCP server datum — wraps `daltskin/sysml-v2-lsp`
//! (npm package `sysml-v2-lsp`, bin `sysml-mcp`), a TypeScript LSP with a
//! bundled Model Context Protocol server (stdio transport) for AI-assisted
//! SysML v2 modelling: parse/validate/diagnostics/symbols/hover/goto-def/
//! rename/complexity-analysis/Mermaid preview.
//!
//! Decision provenance: PromptExecution/ledgrrr `docs/sysml-v2-tooling-survey.md`
//! — "Wrap `daltskin/sysml-v2-lsp` now. Accept the TypeScript runtime
//! dependency inside b00t as a pragmatic exception to the Rust-first rule, in
//! exchange for shipping in days/weeks instead of building a Rust LSP from
//! scratch on an unverified parser crate." Nothing had implemented that
//! decision yet — this module is the first cut.
//!
//! Structural template: `datum_podman.rs` (external-process DatumType, no
//! persistent `BootDatum` schema changes needed — reuses `desires`/`require`).
//!
//! 🦨 SPIKE STATUS — read before assuming this is production-ready:
//! - `is_installed`/`version_status` are real, network-free checks (node/npx
//!   presence only — see doc comment on `is_installed` for why they stop there).
//! - `resolve_entrypoint`/`start_server`/`probe_tools` are REAL, not stubs —
//!   verified manually against the actual published npm package
//!   (`sysml-v2-lsp@0.25.0`) during development: a live process was spawned
//!   and a genuine MCP `initialize` + `tools/list` handshake was completed,
//!   returning the server's real 20 tool names (parse, validate,
//!   getDiagnostics, getSymbols, getDefinition, getReferences, getHierarchy,
//!   getModelSummary, getComplexity, preview, and visualise/visualize
//!   spelling variants).
//! - NOT wired: this DatumType is not yet plumbed into `commands/mcp.rs`'s
//!   install/sync flow (the code path that writes an entry into Claude
//!   Desktop's/Codex's own MCP config so *they* can spawn+call it). That is
//!   the "same MCP tool-exposure path other DatumTypes use" for the generic
//!   `DatumType::Mcp`/`McpDatum` case (see `datum_mcp.rs`) — a `.mcp.toml`
//!   stdio-method datum already gets that for free with zero Rust code (see
//!   `_b00t_/ssh-mcp.mcp.toml` for a directly analogous existing example: an
//!   external MCP server, spawned via a cargo-installed binary instead of
//!   npx, otherwise the same shape). This SysmlV2Lsp DatumType exists
//!   alongside that, not instead of it — it adds LSP/npm-specific capability
//!   checks (`is_installed`, `resolve_entrypoint`, `probe_tools`) that
//!   `McpDatum`'s generic stdio-method schema doesn't attempt.
//!
//! Known gotcha (found the hard way): the installed `sysml-mcp` bin symlink
//! (`node_modules/.bin/sysml-mcp -> ../sysml-v2-lsp/dist/server/mcpServer.js`)
//! has NO shebang line. Executing it directly via PATH lookup (`npx sysml-mcp`,
//! or `npx -p sysml-v2-lsp sysml-mcp`) fails with a shell syntax error
//! (`sysml-mcp: 1: Syntax error: "(" unexpected`) because /bin/sh tries to
//! interpret the minified JS as a shell script. It MUST be invoked as
//! `node <resolved-path-to-mcpServer.js>`, never as a bare command —
//! `resolve_entrypoint`/`start_server` below do that.
//!
//! Also found the hard way: the bundled MCP server uses newline-delimited
//! JSON-RPC over stdio (one JSON object per line), NOT LSP-style
//! `Content-Length:`-framed messages — a first attempt using LSP framing
//! hung forever waiting for headers that never came.

use crate::traits::*;
use crate::{BootDatum, check_command_available, get_config};
use anyhow::{Context, Result};
use duct::cmd;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// npm package name published to the registry (verified live: `npm view sysml-v2-lsp`).
pub const NPM_PACKAGE: &str = "sysml-v2-lsp";
/// bin name inside the package — resolves to `dist/server/mcpServer.js`.
pub const BIN_NAME: &str = "sysml-mcp";

pub struct SysmlV2LspDatum {
    pub datum: BootDatum,
}

impl SysmlV2LspDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(SysmlV2LspDatum { datum: config.b00t })
    }

    /// `<package>@<version>` if `desires` is set, else the bare package name
    /// (npx/npm resolve that to `latest`).
    fn package_spec(&self) -> String {
        match self.desired_version() {
            Some(v) => format!("{NPM_PACKAGE}@{v}"),
            None => NPM_PACKAGE.to_string(),
        }
    }

    /// Resolve the on-disk path to the bundled MCP server entrypoint
    /// (`dist/server/mcpServer.js`) via `npx -p <pkg> node -e require.resolve(...)`.
    ///
    /// Has real network side effects on first call — npm/npx installs the
    /// package into the npx cache (`~/.npm/_npx/...`) if not already present.
    /// Deliberately NOT called from `is_installed`/`version_status`, which
    /// must stay cheap and network-free for routine `b00t status` runs.
    pub fn resolve_entrypoint(&self) -> Result<std::path::PathBuf> {
        let spec = self.package_spec();
        let script =
            format!("console.log(require.resolve('{NPM_PACKAGE}/dist/server/mcpServer.js'))");
        let output = cmd!("npx", "--yes", "-p", &spec, "node", "-e", &script)
            .read()
            .with_context(|| format!("failed to resolve {NPM_PACKAGE} entrypoint via npx"))?;
        let path = output.trim();
        if path.is_empty() {
            anyhow::bail!("npx returned an empty path for {NPM_PACKAGE}'s mcpServer.js");
        }
        Ok(std::path::PathBuf::from(path))
    }

    /// Spawn the bundled MCP server as a stdio subprocess (`node <entrypoint>`
    /// — see module doc for why it can't be exec'd directly as `sysml-mcp`).
    /// Caller owns the child's lifecycle; it is not killed on drop.
    pub fn start_server(&self) -> Result<Child> {
        let entrypoint = self.resolve_entrypoint()?;
        Command::new("node")
            .arg(&entrypoint)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to spawn node {}", entrypoint.display()))
    }

    /// Perform a real MCP `initialize` + `tools/list` handshake against a
    /// freshly-spawned server instance, over newline-delimited JSON-RPC on
    /// stdio (see module doc — NOT LSP `Content-Length` framing). Returns the
    /// advertised tool names. Best-effort with a hard overall timeout;
    /// intended for diagnostics/verification, not a hot path — full tool-call
    /// passthrough into b00t's own MCP surface is NOT implemented here.
    pub fn probe_tools(&self, timeout: Duration) -> Result<Vec<String>> {
        let mut child = self.start_server()?;
        let mut stdin = child.stdin.take().context("child has no stdin handle")?;
        let stdout = child.stdout.take().context("child has no stdout handle")?;

        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let send = |stdin: &mut std::process::ChildStdin, msg: &serde_json::Value| -> Result<()> {
            writeln!(stdin, "{}", msg)?;
            stdin.flush()?;
            Ok(())
        };

        send(
            &mut stdin,
            &serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": {"name": "b00t-cli", "version": env!("CARGO_PKG_VERSION")}
                }
            }),
        )?;

        let wait_for_id = |rx: &std::sync::mpsc::Receiver<String>,
                            id: i64,
                            timeout: Duration|
         -> Option<serde_json::Value> {
            let deadline = std::time::Instant::now() + timeout;
            while std::time::Instant::now() < deadline {
                if let Ok(line) = rx.recv_timeout(Duration::from_millis(200)) {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                        if v.get("id").and_then(|i| i.as_i64()) == Some(id) {
                            return Some(v);
                        }
                    }
                }
            }
            None
        };

        if wait_for_id(&rx, 1, timeout).is_none() {
            let _ = child.kill();
            anyhow::bail!(
                "timed out waiting for `initialize` response from {}",
                BIN_NAME
            );
        }

        send(
            &mut stdin,
            &serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}),
        )?;
        send(
            &mut stdin,
            &serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        )?;

        let tools = match wait_for_id(&rx, 2, timeout) {
            Some(v) => v["result"]["tools"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default(),
            None => {
                let _ = child.kill();
                anyhow::bail!("timed out waiting for `tools/list` response from {}", BIN_NAME);
            }
        };

        let _ = child.kill();
        let _ = child.wait();
        Ok(tools)
    }
}

impl DatumChecker for SysmlV2LspDatum {
    /// Cheap, network-free check: node/npx runtime present. Deliberately does
    /// NOT attempt to resolve/download the npm package here — that has real
    /// network side effects and belongs in `resolve_entrypoint`/`start_server`,
    /// called explicitly, never from a status check that runs on every
    /// `b00t status`.
    fn is_installed(&self) -> bool {
        check_command_available("npx") && check_command_available("node")
    }

    fn current_version(&self) -> Option<String> {
        None // would require a network round-trip (npm view / npx resolve); not attempted here
    }

    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }

    fn version_status(&self) -> VersionStatus {
        if !DatumChecker::is_installed(self) {
            VersionStatus::Missing
        } else {
            VersionStatus::Unknown // no cheap way to compare installed vs desired npm version
        }
    }
}

impl StatusProvider for SysmlV2LspDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }

    fn subsystem(&self) -> &str {
        "sysml_v2_lsp"
    }

    fn hint(&self) -> &str {
        &self.datum.hint
    }

    fn is_disabled(&self) -> bool {
        !DatumChecker::is_installed(self)
    }
}

impl FilterLogic for SysmlV2LspDatum {
    fn is_available(&self) -> bool {
        self.is_available_default()
    }

    fn prerequisites_satisfied(&self) -> bool {
        if let Some(require) = &self.datum.require {
            self.evaluate_constraints(require)
        } else {
            check_command_available("npx") && check_command_available("node")
        }
    }

    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
}

crate::impl_boot_datum_accessors!(SysmlV2LspDatum);

#[cfg(test)]
mod tests {
    use super::*;

    fn bare_datum() -> SysmlV2LspDatum {
        SysmlV2LspDatum {
            datum: BootDatum {
                name: "sysml-v2-lsp".into(),
                hint: "test".into(),
                ..Default::default()
            },
        }
    }

    #[test]
    fn package_spec_uses_desires_when_set() {
        let mut datum = bare_datum();
        datum.datum.desires = Some("0.25.0".into());
        assert_eq!(datum.package_spec(), "sysml-v2-lsp@0.25.0");
    }

    #[test]
    fn package_spec_bare_name_without_desires() {
        let datum = bare_datum();
        assert_eq!(datum.package_spec(), "sysml-v2-lsp");
    }

    #[test]
    fn is_installed_reflects_node_npx_presence_no_network() {
        let datum = bare_datum();
        // Pins the no-network fast path only — NOT full package resolution
        // (that's resolve_entrypoint, exercised by the ignored live test below).
        assert_eq!(
            DatumChecker::is_installed(&datum),
            check_command_available("npx") && check_command_available("node")
        );
    }

    #[test]
    fn version_status_missing_without_node_npx() {
        // Can't easily fake PATH here; just pin the branch logic directly.
        let datum = bare_datum();
        if !check_command_available("npx") || !check_command_available("node") {
            assert_eq!(datum.version_status(), VersionStatus::Missing);
        } else {
            assert_eq!(datum.version_status(), VersionStatus::Unknown);
        }
    }

    // Live integration test — spawns the real subprocess and performs a real
    // MCP `initialize` + `tools/list` handshake over stdio against the actual
    // published npm package. Requires network on first run (npx installs the
    // package into its cache) and node>=20. Not run by default `cargo test`
    // (matches this crate's existing convention of gating network-dependent
    // tests behind #[ignore] — see job_ipc.rs) — opt in with
    // `cargo test -- --ignored` once npx/node are known available.
    #[test]
    #[ignore]
    fn live_probe_tools_returns_known_tool_names() {
        let datum = bare_datum();
        let tools = datum
            .probe_tools(Duration::from_secs(60))
            .expect("live MCP probe failed");
        assert!(tools.contains(&"parse".to_string()));
        assert!(tools.contains(&"getSymbols".to_string()));
    }
}
