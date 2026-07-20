//! ledgrrr — finops usage ledger for collaborative autonomy.
//!
//! Every capability a b00t agent executes on behalf of a project is a
//! billable/auditable event. This module makes that explicit: the agent
//! creates a **receipt of data** ([`UsageReceipt`]) for each execution and
//! registers it with `ledgrrr`, which mints a **finops usage code**
//! ([`FinopsCode`]). Multiple collaborating agents share one ledger
//! (local-first JSONL, or the `ledgerr-mcp` server) so usage is attributable
//! per agent / project / capability — the accounting backbone of
//! collaborative autonomy.
//!
//! Two implementations:
//! - [`LocalLedgrrr`] — persistent JSONL ledger, milled codes, constraint
//!   validation. Fully realized, no external server; the default.
//! - [`McpLedgrrr`] — bridges to the running `ledgerr-mcp` server over stdio
//!   (mirrors `b00t-mcp`'s existing `call_ledgerr_mcp_stdio` bridge, driven by
//!   `LEDGERR_MCP_CMD`). The production seam when a server is available.

use crate::error::{ChatError, ChatResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// A finops usage code minted by ledgrrr on receipt registration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinopsCode(pub String);

impl FinopsCode {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FinopsCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Constraint a [`UsageReceipt`] must satisfy before a code is minted.
/// Aligns with the Tax-Lawyer `Satisfies<Constraint>` pattern used across
/// `ledgerr_tax` / `ufo-types`.
#[derive(Debug, Clone, Default)]
pub struct ReceiptConstraint {
    pub require_project: bool,
    pub require_capability: bool,
    pub nonneg_units: bool,
}

impl ReceiptConstraint {
    pub fn strict() -> Self {
        Self {
            require_project: true,
            require_capability: true,
            nonneg_units: true,
        }
    }
}

/// A data receipt describing one agent capability execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReceipt {
    pub receipt_id: String,
    pub agent_id: String,
    pub project: String,
    pub capability: String,
    /// Quantified usage (messages, tokens, requests…). Drives finops cost.
    pub units: u64,
    /// Epoch milliseconds when the capability executed.
    pub occurred_at: u64,
    /// Whether the receipt passed [`ReceiptConstraint`] validation.
    pub constraint_satisfied: bool,
    /// Finops code assigned by ledgrrr on registration (None until registered).
    pub finops_code: Option<String>,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
}

impl UsageReceipt {
    pub fn new(
        agent_id: impl Into<String>,
        project: impl Into<String>,
        capability: impl Into<String>,
        units: u64,
    ) -> Self {
        let occurred_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        Self {
            receipt_id: uuid::Uuid::new_v4().to_string(),
            agent_id: agent_id.into(),
            project: project.into(),
            capability: capability.into(),
            units,
            occurred_at,
            constraint_satisfied: false,
            finops_code: None,
            metadata: serde_json::Value::Null,
        }
    }

    /// Evaluate the receipt against a constraint (Tax-Lawyer `Satisfies`).
    pub fn check(&mut self, c: &ReceiptConstraint) -> bool {
        let ok = (!c.require_project || !self.project.is_empty())
            && (!c.require_capability || !self.capability.is_empty())
            && (!c.nonneg_units || self.units > 0);
        self.constraint_satisfied = ok;
        ok
    }
}

/// The ledgrrr ledger contract. Implemented by local and MCP backends.
pub trait Ledgrrr: Send + Sync {
    /// Register a receipt; mint and return its finops usage code.
    fn register(&self, receipt: &UsageReceipt) -> ChatResult<FinopsCode>;
    /// All finops codes recorded for a project.
    fn codes_for(&self, project: &str) -> Vec<FinopsCode>;
    /// Total units recorded for a project (collaborative cost rollup).
    fn units_for(&self, project: &str) -> u64;
}

fn slug(project: &str) -> String {
    project
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// Local-first persistent ledger. Append-only JSONL; milled, monotonic finops
/// codes per project. Safe for multiple collaborating agents on one host when
/// they point at the same `path` (each append is a single line write).
pub struct LocalLedgrrr {
    constraint: ReceiptConstraint,
    by_project: Mutex<HashMap<String, Vec<UsageReceipt>>>,
    path: Option<PathBuf>,
    code_prefix: String,
}

impl LocalLedgrrr {
    /// In-memory ledger (no persistence), prefix `FINOPS`.
    pub fn memory() -> Self {
        Self {
            constraint: ReceiptConstraint::strict(),
            by_project: Mutex::new(HashMap::new()),
            path: None,
            code_prefix: "FINOPS".to_string(),
        }
    }

    /// Mock ledger used while real ledgerr integration is pending: synthetic
    /// `MOCK-FINOPS-*` codes, in-memory only. Keeps the nats mesh focus
    /// unblocked from a running `ledgerr-mcp` server.
    pub fn mock() -> Self {
        Self {
            constraint: ReceiptConstraint::strict(),
            by_project: Mutex::new(HashMap::new()),
            path: None,
            code_prefix: "MOCK-FINOPS".to_string(),
        }
    }

    /// Persistent ledger backed by a JSONL file. Existing lines are replayed to
    /// continue the per-project code sequence.
    pub fn file(path: impl AsRef<Path>) -> ChatResult<Self> {
        let path = path.as_ref().to_path_buf();
        let mut by_project: HashMap<String, Vec<UsageReceipt>> = HashMap::new();
        if path.exists() {
            let file = std::fs::File::open(&path)
                .map_err(|e| ChatError::Other(format!("ledgrrr open {path:?}: {e}")))?;
            for line in BufReader::new(file).lines().flatten() {
                if let Ok(r) = serde_json::from_str::<UsageReceipt>(&line) {
                    by_project.entry(r.project.clone()).or_default().push(r);
                }
            }
        }
        Ok(Self {
            constraint: ReceiptConstraint::strict(),
            by_project: Mutex::new(by_project),
            path: Some(path),
            code_prefix: "FINOPS".to_string(),
        })
    }

    fn next_code(&self, project: &str, map: &HashMap<String, Vec<UsageReceipt>>) -> FinopsCode {
        let seq = map.get(project).map(|v| v.len() as u64 + 1).unwrap_or(1);
        FinopsCode(format!("{}-{}-{:06}", self.code_prefix, slug(project), seq))
    }

    fn append_line(&self, receipt: &UsageReceipt) -> ChatResult<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| ChatError::Other(format!("ledgrrr append {path:?}: {e}")))?;
        let line = serde_json::to_string(receipt)?;
        writeln!(file, "{line}")
            .map_err(|e| ChatError::Other(format!("ledgrrr write {path:?}: {e}")))?;
        Ok(())
    }
}

impl Ledgrrr for LocalLedgrrr {
    fn register(&self, receipt: &UsageReceipt) -> ChatResult<FinopsCode> {
        let mut receipt = receipt.clone();
        if !receipt.check(&self.constraint) {
            return Err(ChatError::Other(format!(
                "ledgrrr: receipt {} failed constraint",
                receipt.receipt_id
            )));
        }
        let mut map = self.by_project.lock().unwrap();
        let code = self.next_code(&receipt.project, &map);
        receipt.finops_code = Some(code.0.clone());
        self.append_line(&receipt)?;
        map.entry(receipt.project.clone())
            .or_default()
            .push(receipt);
        Ok(code)
    }

    fn codes_for(&self, project: &str) -> Vec<FinopsCode> {
        self.by_project
            .lock()
            .unwrap()
            .get(project)
            .map(|v| {
                v.iter()
                    .filter_map(|r| r.finops_code.clone().map(FinopsCode))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn units_for(&self, project: &str) -> u64 {
        self.by_project
            .lock()
            .unwrap()
            .get(project)
            .map(|v| v.iter().map(|r| r.units).sum())
            .unwrap_or(0)
    }
}

/// Mock ledger alias (see [`LocalLedgrrr::mock`]); synthetic `MOCK-FINOPS-*`
/// codes, no external server. Used by the nats-focused mesh work until real
/// ledgerr integration (and iroh backing) lands.
pub type MockLedgrrr = LocalLedgrrr;

/// MCP bridge to the `ledgerr-mcp` server (production seam).
///
/// Mirrors `b00t-mcp`'s `call_ledgerr_mcp_stdio`: spawns the binary named by
/// `LEDGERR_MCP_CMD` as a stdio subprocess, performs the MCP `initialize`
/// handshake, then issues `tools/call` for `ledgerr_register_receipt`. The
/// returned finops code is parsed from the tool result.
///
/// ⚠️ The exact register-tool name/shape should be confirmed against the
/// deployed `ledgerr-mcp`; it is the integration surface, not a local default.
pub struct McpLedgrrr {
    command: String,
    register_tool: String,
}

impl McpLedgrrr {
    pub fn from_env() -> ChatResult<Self> {
        let command = std::env::var("LEDGERR_MCP_CMD")
            .map_err(|_| ChatError::Other("LEDGERR_MCP_CMD not set".into()))?;
        Ok(Self {
            command,
            register_tool: "ledgerr_register_receipt".to_string(),
        })
    }

    pub fn with_command(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            register_tool: "ledgerr_register_receipt".to_string(),
        }
    }
}

impl Ledgrrr for McpLedgrrr {
    fn register(&self, receipt: &UsageReceipt) -> ChatResult<FinopsCode> {
        let payload = serde_json::json!({
            "name": self.register_tool,
            "arguments": { "receipt": receipt }
        });
        let result = call_ledgerr_stdio(&self.command, &payload)
            .map_err(|e| ChatError::Other(format!("ledgrrr-mcp: {e}")))?;
        let code = result
            .get("finops_code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChatError::Other("ledgrrr-mcp: no finops_code in response".into()))?;
        Ok(FinopsCode(code.to_string()))
    }

    fn codes_for(&self, _project: &str) -> Vec<FinopsCode> {
        // Remote ledger: not enumerated locally by the bridge.
        Vec::new()
    }

    fn units_for(&self, _project: &str) -> u64 {
        0
    }
}

/// Spawn `ledgerr-mcp` as a stdio subprocess, handshake, call one tool.
/// Returns the parsed `result` object from the tool response.
fn call_ledgerr_stdio(cmd: &str, payload: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
    use std::io::{BufRead, Write};
    use std::process::{Command, Stdio};

    let mut child = Command::new(cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| anyhow::anyhow!("spawn ledgerr-mcp failed: {e}"))?;

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("no stdin"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("no stdout"))?;

    let initialize = serde_json::json!({
        "jsonrpc": "2.0", "id": 1,
        "method": "initialize",
        "params": { "protocolVersion": "2024-11-05", "capabilities": {} }
    });
    writeln!(stdin, "{initialize}").ok();
    stdin.flush().ok();

    let initialized = serde_json::json!({
        "jsonrpc": "2.0", "method": "notifications/initialized", "params": {}
    });
    writeln!(stdin, "{initialized}").ok();
    stdin.flush().ok();

    let call = serde_json::json!({
        "jsonrpc": "2.0", "id": 2,
        "method": "tools/call",
        "params": payload
    });
    writeln!(stdin, "{call}").ok();
    stdin.flush().ok();

    let reader = std::io::BufReader::new(stdout);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
            if v.get("id").and_then(|i| i.as_i64()) == Some(2) {
                return Ok(v
                    .get("result")
                    .and_then(|r| r.get("content"))
                    .and_then(|c| c.as_array())
                    .and_then(|a| a.first())
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                    .and_then(|t| serde_json::from_str::<serde_json::Value>(t).ok())
                    .unwrap_or_else(|| serde_json::json!({})));
            }
        }
    }
    Err(anyhow::anyhow!("no response from ledgerr-mcp"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_ledger_mints_monotonic_codes() {
        let ledger = LocalLedgrrr::memory();
        let r1 = UsageReceipt::new("alpha", "app4dog", "nats.send", 1);
        let r2 = UsageReceipt::new("beta", "app4dog", "nats.broadcast", 3);
        let c1 = ledger.register(&r1).unwrap();
        let c2 = ledger.register(&r2).unwrap();
        assert_eq!(c1.as_str(), "FINOPS-app4dog-000001");
        assert_eq!(c2.as_str(), "FINOPS-app4dog-000002");
        assert_eq!(ledger.codes_for("app4dog").len(), 2);
        assert_eq!(ledger.units_for("app4dog"), 4);
    }

    #[test]
    fn constraint_rejects_empty_project() {
        let ledger = LocalLedgrrr::memory();
        let bad = UsageReceipt::new("alpha", "", "nats.send", 1);
        assert!(ledger.register(&bad).is_err());
    }

    #[test]
    fn slugifies_project_names() {
        assert_eq!(slug("App4.Dog"), "app4-dog");
    }

    #[test]
    fn receipt_check_validates_units() {
        let mut r = UsageReceipt::new("a", "p", "c", 0);
        assert!(!r.check(&ReceiptConstraint::strict()));
        let mut r2 = UsageReceipt::new("a", "p", "c", 5);
        assert!(r2.check(&ReceiptConstraint::strict()));
    }
}
