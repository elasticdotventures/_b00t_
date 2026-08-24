use std::io::{self, BufRead, IsTerminal, Read, Write};

use anyhow::{Context, Result};

use crate::{
    BootDatum, DatumType, GateSpec, McpListFilter, McpListItem, McpMethods, PolysemeRef,
    RuntimeConfig, UnifiedConfig, check_command_available, get_expanded_path, load_runtime_datum,
};

// ── Datum Dispatch ─────────────────────────────────────────────────────

/// Datum dispatch resolution — what action to take for `b00t <name>`.
#[derive(Clone)]
pub enum DatumDispatch {
    /// Launch via sandbox (runtime datum)
    Runtime(RuntimeConfig),
    /// Execute the datum's command + passthrough args (cli datum)
    CliPassthrough { command: String, args: Vec<String> },
    /// Show polyseme resolution options
    Polyseme {
        name: String,
        refs: Vec<PolysemeRef>,
    },
    /// Found but not directly dispatchable (mcp, ai, etc.)
    Info(String),
}

// ── Dispatch Mode Trait Chain (#706) ─────────────────────────────────────
//
// Each datum kind resolves independently via `DispatchMode::try_resolve`.
// `default_dispatch_chain()` is the ordered Vec<Box<dyn DispatchMode>> that
// `resolve_all_datum_dispatches` walks; adding a new dispatch kind (e.g. a
// future DockerMode or AgentMode) means appending a new implementor here,
// not editing resolve_all_datum_dispatches' body. Cross-mode precedence
// (e.g. "CLI is suppressed when a Runtime datum also matches") is NOT
// encoded per-mode — it's handled uniformly afterward by the existing
// `result_is_implied_by` stereotype filter, so modes stay independent.

/// A single resolution strategy for `b00t <name>` dispatch.
pub trait DispatchMode {
    /// Attempt to resolve `candidate` (looked up under `path`) into a dispatch action.
    fn try_resolve(&self, candidate: &str, path: &str) -> Option<DatumDispatch>;
}

struct RuntimeMode;
impl DispatchMode for RuntimeMode {
    fn try_resolve(&self, candidate: &str, path: &str) -> Option<DatumDispatch> {
        let expanded = get_expanded_path(path).ok()?;
        let suffixes = [".runtime.toml", ".runtime.tomllmd", ".runtime.tomllm"];
        for suffix in &suffixes {
            let p = expanded.join(format!("{candidate}{suffix}"));
            if p.exists() {
                if let Ok(cfg) = load_runtime_datum(candidate, path) {
                    return Some(DatumDispatch::Runtime(cfg));
                }
            }
        }
        None
    }
}

struct CliPassthroughMode;
impl DispatchMode for CliPassthroughMode {
    fn try_resolve(&self, candidate: &str, path: &str) -> Option<DatumDispatch> {
        let expanded = get_expanded_path(path).ok()?;
        let suffixes = [".cli.toml", ".cli.tomllmd", ".cli.tomllm"];
        for suffix in &suffixes {
            let p = expanded.join(format!("{candidate}{suffix}"));
            if p.exists() {
                if let Ok(datum) = load_cli_datum(candidate, path) {
                    let command = datum.command.unwrap_or_else(|| candidate.to_string());
                    let args: Vec<String> = datum.args.unwrap_or_default();
                    return Some(DatumDispatch::CliPassthrough { command, args });
                }
            }
        }
        None
    }
}

struct PolysemeMode;
impl DispatchMode for PolysemeMode {
    fn try_resolve(&self, candidate: &str, path: &str) -> Option<DatumDispatch> {
        let expanded = get_expanded_path(path).ok()?;
        let suffixes = [".polyseme.toml", ".polyseme.tomllmd", ".polyseme.tomllm"];
        for suffix in &suffixes {
            let p = expanded.join(format!("{candidate}{suffix}"));
            if p.exists() {
                if let Ok(refs) = crate::load_polyseme_refs(candidate, path) {
                    return Some(DatumDispatch::Polyseme {
                        name: candidate.to_string(),
                        refs,
                    });
                }
            }
        }
        None
    }
}

struct OodaMode;
impl DispatchMode for OodaMode {
    fn try_resolve(&self, candidate: &str, path: &str) -> Option<DatumDispatch> {
        let expanded = get_expanded_path(path).ok()?;
        let suffixes = [".ooda.toml", ".ooda.tomllmd", ".ooda.tomllm"];
        for suffix in &suffixes {
            let p = expanded.join(format!("{candidate}{suffix}"));
            if p.exists() {
                return Some(DatumDispatch::Info(format!(
                    "ooda loop '{}' — run with: b00t ooda run {}",
                    candidate, candidate
                )));
            }
        }
        None
    }
}

struct McpInfoMode;
impl DispatchMode for McpInfoMode {
    fn try_resolve(&self, candidate: &str, path: &str) -> Option<DatumDispatch> {
        let expanded = get_expanded_path(path).ok()?;
        let suffixes = [".mcp.toml", ".mcp.tomllmd", ".mcp.tomllm"];
        for suffix in &suffixes {
            let p = expanded.join(format!("{candidate}{suffix}"));
            if p.exists() {
                return Some(DatumDispatch::Info(format!(
                    "mcp datum '{}' — use 'b00t mcp list' or 'b00t mcp execute {} <tool>'",
                    candidate, candidate
                )));
            }
        }
        None
    }
}

/// Ordered chain of dispatch strategies, tried in priority order
/// (most-specific/actionable first). Extend by appending a new
/// `Box<dyn DispatchMode>` implementor — no match-block edits required.
pub fn default_dispatch_chain() -> Vec<Box<dyn DispatchMode>> {
    vec![
        Box::new(RuntimeMode),
        Box::new(CliPassthroughMode),
        Box::new(PolysemeMode),
        Box::new(OodaMode),
        Box::new(McpInfoMode),
    ]
}

/// Search the datum space for `candidate` and resolve ALL matching dispatch actions.
/// Returns multiple matches when a name is polysemous or has multiple datum types.
pub fn resolve_all_datum_dispatches(candidate: &str, path: &str) -> Vec<DatumDispatch> {
    if get_expanded_path(path).is_err() {
        return Vec::new();
    }

    let mut results: Vec<DatumDispatch> = default_dispatch_chain()
        .iter()
        .filter_map(|mode| mode.try_resolve(candidate, path))
        .collect();

    // ── Stereotype hierarchy: eliminate less-specific matches ──────────────
    if results.len() > 1 {
        let mut filtered: Vec<DatumDispatch> = Vec::new();
        for result in &results {
            let is_implied = results.iter().any(|other| {
                std::mem::discriminant(result) != std::mem::discriminant(other)
                    && result_is_implied_by(result, other)
            });
            if !is_implied {
                filtered.push(result.clone());
            }
        }
        if !filtered.is_empty() {
            results = filtered;
        }
    }

    results
}

/// Returns true if `a` is implied by `b` (a is less specific than b).
fn result_is_implied_by(a: &DatumDispatch, b: &DatumDispatch) -> bool {
    use DatumDispatch::*;
    match (a, b) {
        (CliPassthrough { .. }, Runtime(_)) => true,
        (Info(_), _) | (_, Info(_)) => false,
        _ => false,
    }
}

/// Single-match convenience — returns the first runtime or CLI dispatch, or the polyseme if present.
pub fn resolve_datum_dispatch(candidate: &str, path: &str) -> Option<DatumDispatch> {
    let mut all = resolve_all_datum_dispatches(candidate, path);
    if all.is_empty() {
        return None;
    }
    // Prefer runtime over CLI over polyseme for single-match
    if let Some(pos) = all.iter().position(|d| {
        matches!(
            d,
            DatumDispatch::Runtime(_)
                | DatumDispatch::CliPassthrough { .. }
                | DatumDispatch::Polyseme { .. }
        )
    }) {
        return Some(all.swap_remove(pos));
    }
    all.into_iter().next()
}

/// Interactive polyseme selection prompt (#580).
pub fn prompt_polyseme_selection(name: &str, refs: &[PolysemeRef]) -> Option<String> {
    if !io::stdin().is_terminal() {
        return None;
    }

    eprintln!("\n🔀 '{name}' has multiple resolutions:");
    for (i, r) in refs.iter().enumerate() {
        eprintln!("  {}) {} — {}", i + 1, r.name, r.description);
    }
    eprint!("  Select [1-{}]: ", refs.len());
    io::stdout().flush().ok();

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input).ok()?;
    let choice: usize = input.trim().parse().ok()?;
    refs.get(choice.wrapping_sub(1)).map(|r| r.name.clone())
}

/// Load a CLI datum and return its BootDatum.
fn load_cli_datum(name: &str, path: &str) -> Result<BootDatum> {
    let expanded = get_expanded_path(path)?;
    let suffixes = [".cli.toml", ".cli.tomllmd", ".cli.tomllm"];
    let mut found = None;
    for suffix in &suffixes {
        let p = expanded.join(format!("{name}{suffix}"));
        if p.exists() {
            found = Some(p);
            break;
        }
    }
    let file_path = found.ok_or_else(|| anyhow::anyhow!("CLI datum '{name}' not found"))?;
    let content =
        std::fs::read_to_string(&file_path).context(format!("read {}", file_path.display()))?;
    let config: UnifiedConfig =
        toml::from_str(&content).context(format!("parse {}", file_path.display()))?;
    Ok(config.b00t)
}

// ── JSON helpers ──────────────────────────────────────────────────────

pub fn extract_comments_and_clean_json(input: &str) -> (String, Option<String>) {
    let comment_re = regex::Regex::new(r"//.*$").unwrap();
    let block_comment_re = regex::Regex::new(r"/\*.*?\*/").unwrap();

    let (mut cleaned_input, mut first_comment) = (String::new(), None);

    // First, remove block comments /* ... */
    let input_without_blocks = block_comment_re.replace_all(input, "").to_string();

    // Then process line comments
    for line in input_without_blocks.lines() {
        if let Some(cap) = comment_re.find(line) {
            if first_comment.is_none() {
                let comment_text = cap.as_str().trim_start_matches("//").trim();
                if !comment_text.is_empty() {
                    first_comment = Some(comment_text.to_string());
                }
            }
            let line_without_comment = line[..cap.start()].trim_end();
            if !line_without_comment.is_empty() {
                cleaned_input.push_str(line_without_comment);
                cleaned_input.push('\n');
            }
        } else {
            cleaned_input.push_str(line);
            cleaned_input.push('\n');
        }
    }

    // Also handle trailing commas (JSON5 style)
    let trailing_comma_re = regex::Regex::new(r",(\s*[}\]])").unwrap();
    cleaned_input = trailing_comma_re
        .replace_all(&cleaned_input, "$1")
        .to_string();

    // Handle trailing commas at end of lines more aggressively
    let lines: Vec<String> = cleaned_input
        .lines()
        .map(|line| {
            let trimmed = line.trim_end();
            if trimmed.ends_with(',')
                && (line.contains('}')
                    || line.contains(']')
                    || cleaned_input
                        .lines()
                        .skip_while(|l| l != &line)
                        .nth(1)
                        .map(|next| next.trim().starts_with('}') || next.trim().starts_with(']'))
                        .unwrap_or(false))
            {
                trimmed.strip_suffix(',').unwrap_or(trimmed).to_string()
            } else {
                line.to_string()
            }
        })
        .collect();
    cleaned_input = lines.join("\n");

    (cleaned_input.trim().to_string(), first_comment)
}

pub fn clean_json_for_dwiw(input: &str) -> String {
    extract_comments_and_clean_json(input).0
}

// ── MCP JSON normalization ────────────────────────────────────────────

/// Convert legacy JSON command/args to new multi-method format
fn create_mcp_datum_from_json(
    name: String,
    hint: Option<String>,
    server_config: &serde_json::Value,
) -> BootDatum {
    let command = server_config
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("npx")
        .to_string();
    let args = server_config
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // Detect transport type and requirements based on command
    let (requires, transport_type) = match command.as_str() {
        "docker" => (vec!["docker".to_string()], "stdio"),
        "uvx" | "python" | "python3" => (vec!["python".to_string()], "stdio"),
        "npx" | "node" => (vec!["node".to_string()], "stdio"),
        _ => (vec![], "stdio"),
    };

    let cli_method = serde_json::json!({
        "command": command,
        "args": args,
        "priority": 0,
        "requires": requires,
        "transport": transport_type
    });

    BootDatum {
        model_hf_id: None,
        model_size_gb: None,
        model_size_4bit_gb: None,
        name,
        datum_type: Some(DatumType::Mcp),
        hint: hint
            .or_else(|| {
                server_config
                    .get("hint")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "MCP server".to_string()),
        env: server_config
            .get("env")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            }),
        require: server_config
            .get("require")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            }),
        mcp: Some(McpMethods {
            stdio: Some(vec![
                cli_method
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            ]),
            httpstream: None,
        }),
        gate: server_config
            .get("gate")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|g| {
                        let cmd = g.get("command").and_then(|v| v.as_str());
                        let file = g.get("file").and_then(|v| v.as_str());
                        let env = g.get("env").and_then(|v| v.as_str());
                        let rhai = g.get("rhai").and_then(|v| v.as_str());
                        let knowledge_backend = g.get("knowledge_backend").and_then(|v| v.as_str());
                        let hint = g
                            .get("hint")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        if cmd.is_none()
                            && file.is_none()
                            && env.is_none()
                            && rhai.is_none()
                            && knowledge_backend.is_none()
                        {
                            return None;
                        }
                        Some(GateSpec {
                            command: cmd.map(|s| s.to_string()),
                            file: file.map(|s| s.to_string()),
                            env: env.map(|s| s.to_string()),
                            rhai: rhai.map(|s| s.to_string()),
                            knowledge_backend: knowledge_backend.map(|s| s.to_string()),
                            justfile: None,
                            hint,
                        })
                    })
                    .collect()
            }),
        ..BootDatum::default()
    }
}

pub fn normalize_mcp_json(input: &str, dwiw: bool) -> Result<BootDatum> {
    let (cleaned_input, hint) = if dwiw {
        extract_comments_and_clean_json(input)
    } else {
        (input.to_string(), None)
    };

    let json_value: serde_json::Value = serde_json::from_str(&cleaned_input)?;

    // Handle direct format: {"name": "...", "command": "...", ...}
    if let Some(name) = json_value.get("name") {
        let name_str = name.as_str().unwrap_or("unknown").to_string();

        // Check if this is an HTTP server (has URL field)
        if let Some(url) = json_value.get("url") {
            let http_method = serde_json::json!({
                "url": url.as_str().unwrap_or(""),
                "priority": 0,
                "requires": ["internet"],
                "requires_internet": true,
                "requires_auth": false,
                "transport": "httpstream"
            });

            return Ok(BootDatum {
                model_hf_id: None,
                model_size_gb: None,
                model_size_4bit_gb: None,
                name: name_str,
                datum_type: Some(DatumType::Mcp),
                hint: hint
                    .clone()
                    .unwrap_or_else(|| "MCP HTTP server".to_string()),
                env: json_value
                    .get("env")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    }),
                require: json_value
                    .get("require")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect()
                    }),
                mcp: Some(McpMethods {
                    stdio: None,
                    httpstream: Some(
                        http_method
                            .as_object()
                            .unwrap()
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                    ),
                }),
                ..BootDatum::default()
            });
        }

        return Ok(create_mcp_datum_from_json(
            name_str,
            hint.clone(),
            &json_value,
        ));
    }

    // Handle mcpServers wrapper format
    if let Some(mcp_servers) = json_value.get("mcpServers") {
        let keys: Vec<_> = mcp_servers
            .as_object()
            .map(|obj| obj.keys().collect())
            .unwrap_or_default();

        if keys.len() == 1 {
            let server_name = keys[0].clone();
            let server_config = &mcp_servers[&server_name];
            return Ok(create_mcp_datum_from_json(
                server_name,
                hint.clone(),
                server_config,
            ));
        } else if keys.len() > 1 {
            let server_name = keys[0].clone();
            let server_config = &mcp_servers[&server_name];
            eprintln!(
                "⚠️  Multiple servers found in mcpServers, using first: {}",
                server_name
            );
            eprintln!("💡 To register multiple servers, use separate commands for each");
            return Ok(create_mcp_datum_from_json(
                server_name,
                hint.clone(),
                server_config,
            ));
        }
    }

    // Handle single server format: {"server_name": {...}}
    let keys: Vec<_> = json_value
        .as_object()
        .map(|obj| obj.keys().collect())
        .unwrap_or_default();

    if keys.len() == 1 {
        let server_name = keys[0].clone();
        let server_config = &json_value[&server_name];
        return Ok(create_mcp_datum_from_json(server_name, hint, server_config));
    }

    anyhow::bail!("Unable to parse MCP server configuration from JSON");
}

// ── MCP file operations ────────────────────────────────────────────────

pub fn get_mcp_toml_files(path: &str) -> Result<Vec<String>> {
    let expanded_path = get_expanded_path(path)?;
    let entries = std::fs::read_dir(&expanded_path)
        .with_context(|| format!("Error reading directory {}", expanded_path.display()))?;

    let mut mcp_files = Vec::new();
    for entry in entries {
        if let Ok(entry) = entry {
            let entry_path = entry.path();
            if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                if file_name.ends_with(".mcp.toml") {
                    if let Some(server_name) = file_name.strip_suffix(".mcp.toml") {
                        mcp_files.push(server_name.to_string());
                    }
                }
            }
        }
    }
    Ok(mcp_files)
}

pub fn mcp_list(path: &str, json_output: bool, filter: McpListFilter) -> Result<()> {
    let mcp_files = get_mcp_toml_files(path)?;
    let total_count = mcp_files.len();
    let mut mcp_items: Vec<McpListItem> = Vec::new();

    if total_count > 20 && !json_output {
        eprint!("🔍 Checking {} MCP servers...", total_count);
        let _ = std::io::stderr().flush();
    }
    let mut checked = 0usize;

    for server_name in mcp_files {
        checked += 1;
        if total_count > 20 && !json_output && checked % 10 == 0 {
            eprint!(" {}/{}", checked, total_count);
            let _ = std::io::stderr().flush();
        }
        match crate::get_mcp_config(&server_name, path) {
            Ok(datum) => {
                let (command, args) =
                    if let Some(mcp) = &datum.mcp {
                        if let Some(stdio_methods) = &mcp.stdio {
                            if let Some(first_method) = stdio_methods.first() {
                                let command = first_method
                                    .get("command")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                let args = first_method.get("args").and_then(|v| v.as_array()).map(
                                    |arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str())
                                            .map(|s| s.to_string())
                                            .collect::<Vec<String>>()
                                    },
                                );
                                (command, args)
                            } else {
                                (None, None)
                            }
                        } else if let Some(httpstream) = &mcp.httpstream {
                            let url = httpstream
                                .get("url")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            (Some("HTTP".to_string()), url.map(|u| vec![u]))
                        } else {
                            (None, None)
                        }
                    } else {
                        (datum.command.clone(), datum.args.clone())
                    };

                let is_installed = command
                    .as_deref()
                    .map(|c| {
                        if c == "HTTP" {
                            return true;
                        }
                        if check_command_available(c) {
                            return true;
                        }
                        let claude_cfg = dirs::home_dir()
                            .map(|h| h.join(".claude").join("settings.json"))
                            .filter(|p| p.exists());
                        if let Some(cfg) = claude_cfg {
                            if let Ok(content) = std::fs::read_to_string(&cfg) {
                                if content.contains(&format!("\"{}\"", server_name)) {
                                    return true;
                                }
                            }
                        }
                        false
                    })
                    .unwrap_or(false);
                let is_running = command
                    .as_deref()
                    .and_then(|c| {
                        if c == "HTTP" {
                            return Some(false);
                        }
                        let cname = std::path::Path::new(c)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(c);
                        let pgrep_result = duct::cmd!("pgrep", "-x", cname)
                            .stderr_null()
                            .read()
                            .ok()
                            .map(|s| !s.trim().is_empty());
                        if pgrep_result == Some(true) {
                            return Some(true);
                        }
                        duct::cmd!("pgrep", "-f", &format!("[{}]{}", &cname[..1], &cname[1..]))
                            .stderr_null()
                            .read()
                            .ok()
                            .map(|s| !s.trim().is_empty())
                    })
                    .unwrap_or(false);

                let is_suspended = datum
                    .mcp
                    .as_ref()
                    .and_then(|m| m.stdio.as_ref())
                    .and_then(|methods| methods.first())
                    .and_then(|m| m.get("enabled"))
                    .and_then(|v| v.as_bool())
                    .map(|enabled| !enabled)
                    .unwrap_or(false);

                let restart_hint = if is_installed && !is_running && !is_suspended {
                    let cmd = command.as_deref().unwrap_or("");
                    if let Some(mcp) = &datum.mcp {
                        if let Some(http) = &mcp.httpstream {
                            http.get("url")
                                .and_then(|v| v.as_str())
                                .map(|url| format!("restart: connect to httpstream at {}", url))
                        } else if let Some(methods) = &mcp.stdio {
                            methods.first().map(|m| {
                                let cmd = m.get("command").and_then(|v| v.as_str()).unwrap_or(cmd);
                                let args: Vec<&str> = m
                                    .get("args")
                                    .and_then(|v| v.as_array())
                                    .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                                    .unwrap_or_default();
                                format!("restart: {} {}", cmd, args.join(" "))
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                let transport = datum
                    .mcp
                    .as_ref()
                    .map(|m| {
                        if m.httpstream.is_some() {
                            "httpstream"
                        } else {
                            "stdio"
                        }
                    })
                    .map(|s| s.to_string());

                let item = McpListItem {
                    name: server_name.clone(),
                    command,
                    args,
                    hint: Some(datum.hint.clone()),
                    error: None,
                    is_installed,
                    is_running,
                    is_suspended,
                    transport,
                    restart_hint,
                };

                let search_pass = filter
                    .search
                    .as_ref()
                    .map(|q| item.name.to_lowercase().contains(&q.to_lowercase()))
                    .unwrap_or(true);
                let installed_pass = filter
                    .is_installed
                    .map(|f| item.is_installed == f)
                    .unwrap_or(true);
                let running_pass = filter
                    .is_running
                    .map(|f| item.is_running == f)
                    .unwrap_or(true);
                let suspended_pass = filter
                    .is_suspended
                    .map(|f| item.is_suspended == f)
                    .unwrap_or(true);

                if search_pass && installed_pass && running_pass && suspended_pass {
                    mcp_items.push(item);
                }
            }
            Err(e) => {
                let item = McpListItem {
                    name: server_name,
                    command: None,
                    args: None,
                    hint: None,
                    error: Some(e.to_string()),
                    is_installed: false,
                    is_running: false,
                    is_suspended: false,
                    transport: None,
                    restart_hint: None,
                };
                mcp_items.push(item);
            }
        }
    }

    // Threshold guard
    let threshold = filter.max_threshold.unwrap_or_else(|| {
        crate::session_memory::SessionMemory::load()
            .ok()
            .and_then(|m| {
                let t = m.config.mcp_list_threshold;
                if t > 0 { Some(t) } else { None }
            })
            .unwrap_or(10)
    });

    let has_active_filter = filter.search.is_some()
        || filter.is_installed.is_some()
        || filter.is_running.is_some()
        || filter.is_suspended.is_some();

    let truncated =
        !filter.bypass_threshold && !has_active_filter && mcp_items.len() > threshold as usize;

    if truncated {
        mcp_items.truncate(threshold as usize);
        if let Ok(memory) = crate::session_memory::SessionMemory::load() {
            if let Some(role) = memory.strings.get("last_role") {
                let role_lower = role.to_lowercase();
                let matching: Vec<&McpListItem> = mcp_items
                    .iter()
                    .filter(|item| {
                        item.name.to_lowercase().contains(&role_lower)
                            || item
                                .hint
                                .as_deref()
                                .unwrap_or("")
                                .to_lowercase()
                                .contains(&role_lower)
                    })
                    .collect();
                if !matching.is_empty() {
                    eprintln!(
                        "🎯 {role} role matched {count}/{total} MCP servers",
                        role = role,
                        count = matching.len(),
                        total = total_count
                    );
                }
            }
        }
    }

    if json_output {
        let expanded_path = get_expanded_path(path)?;
        let output = crate::McpListOutput {
            servers: mcp_items,
            path: expanded_path.display().to_string(),
            truncated,
            threshold,
            total_count,
        };
        let json_str = serde_json::to_string_pretty(&output)
            .context("Failed to serialize MCP list to JSON")?;
        println!("{}", json_str);
    } else {
        if total_count > 20 {
            eprint!("\r\x1b[K");
        }
        let expanded_path = get_expanded_path(path)?;
        if mcp_items.is_empty() && total_count == 0 {
            println!(
                "{}  No MCP server configurations found in {}",
                crate::ansi::yellow("⚠️"),
                expanded_path.display()
            );
            println!("   Use 'b00t-cli mcp add <json>' to add MCP server configurations.");
        } else if mcp_items.is_empty() && total_count > 0 {
            println!(
                "{}  No MCP servers match your filters ({} total available).",
                crate::ansi::yellow("⚠️"),
                crate::ansi::bold(&total_count.to_string()),
            );
            println!(
                "   {}  Try: b00t-cli mcp list --all",
                crate::ansi::dim("💡")
            );
        } else {
            if truncated {
                println!(
                    "{}  Showing {shown}/{total} MCP servers (threshold={threshold}). Use --search or --installed/--running/--suspended to filter.",
                    crate::ansi::yellow("⚠️"),
                    shown = mcp_items.len(),
                    total = total_count,
                    threshold = threshold,
                );
                println!(
                    "   {}  Override: --max-threshold <N> or --all to bypass guard.",
                    crate::ansi::dim("ℹ️")
                );
                println!();
            }
            let installed_count = mcp_items.iter().filter(|i| i.is_installed).count();
            let running_count = mcp_items.iter().filter(|i| i.is_running).count();
            let suspended_count = mcp_items.iter().filter(|i| i.is_suspended).count();
            if has_active_filter || total_count > threshold as usize {
                println!(
                    "{}  {} shown  {}  {} total  {}  {} installed  {}  {} running  {}  {} suspended",
                    crate::ansi::bold("📊"),
                    crate::ansi::cyan(&mcp_items.len().to_string()),
                    crate::ansi::dim("|"),
                    crate::ansi::bold(&total_count.to_string()),
                    crate::ansi::dim("|"),
                    crate::ansi::green(&installed_count.to_string()),
                    crate::ansi::dim("|"),
                    crate::ansi::green(&running_count.to_string()),
                    crate::ansi::dim("|"),
                    crate::ansi::yellow(&suspended_count.to_string()),
                );
            } else {
                println!(
                    "{}  Available MCP servers in {}:  ({})",
                    crate::ansi::bold("📋"),
                    crate::ansi::cyan(&expanded_path.display().to_string()),
                    crate::ansi::bold(&format!("{} total", total_count)),
                );
            }
            if !truncated && total_count > threshold as usize {
                println!("  (all {total} shown)", total = total_count);
            }
            println!();
            for item in &mcp_items {
                let status = if item.is_suspended {
                    "⏸️"
                } else if item.is_running {
                    "▶️"
                } else if item.is_installed {
                    "📋"
                } else {
                    "❌"
                };
                match (&item.command, &item.args) {
                    (Some(command), Some(args)) => {
                        println!("{status} {} ({command})", item.name);
                        if !args.is_empty() {
                            println!("   args: {}", args.join(" "));
                        }
                        if item.is_suspended {
                            println!(
                                "   ⏸️  SUSPENDED — enable with: b00t-cli mcp register --restore {}",
                                item.name
                            );
                        }
                        if !item.is_running && !item.is_suspended && item.is_installed {
                            if let Some(hint) = &item.restart_hint {
                                println!("   🔄  Not running — {hint}");
                            }
                        }
                    }
                    _ => {
                        println!("{status} {} (error reading config)", item.name);
                    }
                }
            }
            if truncated {
                println!();
                println!(
                    "💡 {total} total servers, showing first {threshold}. Use --search, --installed, --is-running, or --all to see more.",
                    total = total_count,
                    threshold = threshold
                );
            }
            let _ = crate::session_memory::SessionMemory::load().map(|mut m| {
                let key = format!("mcp_list_view_{}", chrono::Utc::now().format("%Y%m%d"));
                let _ = m.incr(&key);
                let _ = m.set("mcp_last_view_count", &total_count.to_string());
            });
            crate::write_event("mcp_list_view", &total_count.to_string());
            println!();
            println!("To install to VSCode: b00t-cli vscode install mcp <name>");
            println!("To install to Claude Code: b00t-cli claude-code install mcp <name>");
        }
    }

    Ok(())
}

/// Register an MCP server configuration from JSON input
pub fn mcp_add_json(json: &str, dwiw: bool, path: &str) -> Result<()> {
    let json_content = if json == "-" {
        let mut buffer = String::new();

        if io::stdin().is_terminal() {
            eprintln!("📋 Paste your MCP server JSON configuration and press Ctrl+D when done:");
            eprintln!("💡 Supported formats:");
            eprintln!("   • Direct: {{\"name\":\"server\",\"command\":\"npx\",\"args\":[...]}}");
            eprintln!("   • mcpServers: {{\"mcpServers\":{{\"server\":{{...}}}}}}");
            eprintln!("   • Named: {{\"server-name\":{{\"command\":\"npx\",...}}}}");
            eprintln!("");
        }

        match io::stdin().read_to_string(&mut buffer) {
            Ok(_) => {
                let trimmed = buffer.trim();
                if trimmed.is_empty() {
                    anyhow::bail!(
                        "No input provided. Pipe JSON content or press Ctrl+D after pasting."
                    );
                }
                trimmed.to_string()
            }
            Err(e) => {
                anyhow::bail!(
                    "Failed to read from stdin: {}. Pipe JSON content or use Ctrl+D after input.",
                    e
                );
            }
        }
    } else {
        json.trim().to_string()
    };

    let datum = normalize_mcp_json(&json_content, dwiw)?;
    crate::create_mcp_toml_config(&datum, path)?;

    println!("MCP server '{}' configuration saved.", datum.name);
    println!(
        "To install to VSCode: b00t-cli vscode install mcp {}",
        datum.name
    );

    Ok(())
}

/// Remove an MCP server configuration by name
pub fn mcp_remove(name: &str, path: &str) -> Result<()> {
    let expanded_path = get_expanded_path(path)?;
    let mcp_path = expanded_path.join(format!("{}.mcp.toml", name));

    if mcp_path.exists() {
        std::fs::remove_file(&mcp_path).with_context(|| {
            format!(
                "Failed to remove MCP server configuration: {}",
                mcp_path.display()
            )
        })?;
        println!("Removed MCP server configuration: {}", name);
    } else {
        anyhow::bail!("MCP server configuration not found: {}", name);
    }

    Ok(())
}

pub fn mcp_output(path: &str, use_mcp_servers_wrapper: bool, servers: &str) -> Result<()> {
    let requested_servers: Vec<&str> = servers.split(',').map(|s| s.trim()).collect();
    let mut server_configs = serde_json::Map::new();

    for server_name in requested_servers {
        if server_name.is_empty() {
            continue;
        }

        match crate::get_mcp_config(server_name, path) {
            Ok(datum) => {
                let (command, args) = extract_mcp_command_args(&datum);
                let mut server_config = serde_json::Map::new();
                server_config.insert("command".to_string(), serde_json::Value::String(command));
                server_config.insert(
                    "args".to_string(),
                    serde_json::Value::Array(
                        args.into_iter().map(serde_json::Value::String).collect(),
                    ),
                );

                server_configs.insert(
                    server_name.to_string(),
                    serde_json::Value::Object(server_config),
                );
            }
            Err(_) => {
                let mut error_config = serde_json::Map::new();
                error_config.insert(
                    "command".to_string(),
                    serde_json::Value::String("b00t:💩🪵".to_string()),
                );

                let utc_timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let utc_time = chrono::DateTime::from_timestamp(utc_timestamp as i64, 0)
                    .unwrap()
                    .format("%Y-%m-%dT%H:%M:%SZ")
                    .to_string();

                error_config.insert(
                    "args".to_string(),
                    serde_json::Value::Array(vec![
                        serde_json::Value::String(utc_time),
                        serde_json::Value::String(format!(
                            "server '{}' not found in _b00t_ directory",
                            server_name
                        )),
                    ]),
                );

                server_configs.insert(
                    server_name.to_string(),
                    serde_json::Value::Object(error_config),
                );
            }
        }
    }

    let output = if use_mcp_servers_wrapper {
        let mut wrapper = serde_json::Map::new();
        wrapper.insert(
            "mcpServers".to_string(),
            serde_json::Value::Object(server_configs),
        );
        serde_json::Value::Object(wrapper)
    } else {
        serde_json::Value::Object(server_configs)
    };

    let json_str =
        serde_json::to_string_pretty(&output).context("Failed to serialize MCP servers to JSON")?;
    println!("{}", json_str);

    Ok(())
}

/// Extract command and args from MCP datum, handling both new multi-method and legacy formats
fn extract_mcp_command_args(datum: &BootDatum) -> (String, Vec<String>) {
    if let Some(mcp) = &datum.mcp {
        if let Some(stdio_methods) = &mcp.stdio {
            if let Some(first_method) = stdio_methods.first() {
                let command = first_method
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("npx")
                    .to_string();
                let args = first_method
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default();
                return (command, args);
            }
        }
    }

    // Fallback to legacy fields for backwards compatibility
    (
        datum.command.clone().unwrap_or_else(|| "npx".to_string()),
        datum.args.clone().unwrap_or_default(),
    )
}

/// Resolve the active MCP method (stdio/httpstream) and return command details.
fn select_mcp_method(
    datum: &BootDatum,
    stdio_command: Option<&str>,
    use_httpstream: bool,
) -> Result<(
    String,
    Vec<String>,
    Option<std::collections::HashMap<String, String>>,
    &'static str,
)> {
    if let Some(methods) = &datum.mcp {
        if use_httpstream {
            if let Some(httpstream_method) = &methods.httpstream {
                let url = httpstream_method
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing url in httpstream method"))?;

                return Ok((url.to_string(), vec![], None, "httpstream"));
            } else {
                anyhow::bail!("No httpstream method available for MCP '{}'", datum.name);
            }
        }

        if let Some(stdio_command_filter) = stdio_command {
            if let Some(stdio_methods) = &methods.stdio {
                let matching_method = stdio_methods.iter().find(|method| {
                    method
                        .get("command")
                        .and_then(|v| v.as_str())
                        .map(|cmd| cmd == stdio_command_filter)
                        .unwrap_or(false)
                });

                if let Some(method) = matching_method {
                    let command = method
                        .get("command")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow::anyhow!("Missing command in stdio method"))?;
                    let args = method
                        .get("args")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    let env = method.get("env").and_then(|v| v.as_object()).map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    });

                    return Ok((command.to_string(), args, env, "stdio"));
                } else {
                    anyhow::bail!(
                        "No stdio method with command '{}' found for MCP '{}'. Available commands: {}",
                        stdio_command_filter,
                        datum.name,
                        stdio_methods
                            .iter()
                            .filter_map(|m| m.get("command").and_then(|v| v.as_str()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            } else {
                anyhow::bail!("No stdio methods available for MCP '{}'", datum.name);
            }
        } else if let Some(stdio_methods) = &methods.stdio {
            if stdio_methods.is_empty() {
                anyhow::bail!("No stdio methods available for MCP '{}'", datum.name);
            }

            let method = &stdio_methods[0];
            let command = method
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing command in stdio method"))?;
            let args = method
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let env = method.get("env").and_then(|v| v.as_object()).map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            });

            return Ok((command.to_string(), args, env, "stdio"));
        } else {
            anyhow::bail!("No stdio methods available for MCP '{}'", datum.name);
        }
    }

    let (command, args) = extract_mcp_command_args(datum);
    Ok((command, args, datum.env.clone(), "stdio"))
}

// ── MCP Installation Functions ─────────────────────────────────────────

/// Resolve command/args/env for an MCP install, including generic AI-backend
/// provisioning: if `datum.ai_provision` is set, mints a b00t-server API key
/// scoped to it (`b00t server key create`) and merges the resulting
/// key/base-url into the env map on top of whatever static `env` the stdio
/// method already declares. This is what makes provisioning "the default
/// within b00t" for ANY MCP datum, not a rust-doc-specific special case —
/// any future datum opts in by setting `[b00t.ai_provision]`.
///
/// Shells out to `b00t server key create` rather than linking b00t-mcp's
/// LlmState in-process: b00t-mcp already depends on b00t-cli, so the reverse
/// dependency would be circular. Same duct::cmd! pattern already used below
/// for `claude mcp add-json` etc.
fn resolve_provisioned_command_args_env(
    datum: &BootDatum,
) -> Result<(String, Vec<String>, Option<std::collections::HashMap<String, String>>)> {
    let (command, args, static_env, _transport) = select_mcp_method(datum, None, false)?;

    let mut env = static_env.unwrap_or_default();
    if let Some(provision) = &datum.ai_provision {
        let key_output = duct::cmd!(
            "b00t",
            "server",
            "key",
            "create",
            "--consumer",
            &datum.name,
            "--access",
            &provision.scope
        )
        .read()
        .with_context(|| {
            format!(
                "failed to provision AI-backend key for '{}' (scope: {})",
                datum.name, provision.scope
            )
        })?;
        let key = key_output.trim().to_string();
        env.insert(provision.inject_key_as.clone(), key);
        env.insert(provision.inject_base_as.clone(), provision.server_url.clone());
    }

    Ok((command, args, if env.is_empty() { None } else { Some(env) }))
}

pub fn claude_code_install_mcp(name: &str, path: &str) -> Result<()> {
    let datum = crate::get_mcp_config(name, path)?;
    let (command, args, env) = resolve_provisioned_command_args_env(&datum)?;

    let mut claude_json = serde_json::json!({
        "name": datum.name,
        "command": command,
        "args": args
    });
    if let Some(env) = env {
        claude_json["env"] = serde_json::json!(env);
    }

    let json_str =
        serde_json::to_string(&claude_json).context("Failed to serialize JSON for Claude Code")?;

    let result = duct::cmd!("claude", "mcp", "add-json", &datum.name, &json_str).run();

    match result {
        Ok(_) => {
            println!(
                "Successfully installed MCP server '{}' to Claude Code",
                datum.name
            );
            println!(
                "Claude Code command: claude mcp add-json {} '{}'",
                datum.name, json_str
            );
        }
        Err(e) => {
            eprintln!("Failed to install MCP server to Claude Code: {}", e);
            eprintln!(
                "Manual command: claude mcp add-json {} '{}'",
                datum.name, json_str
            );
            return Err(anyhow::anyhow!("Claude Code installation failed: {}", e));
        }
    }

    Ok(())
}

pub fn vscode_install_mcp(name: &str, path: &str) -> Result<()> {
    let datum = crate::get_mcp_config(name, path)?;
    let (command, args, env) = resolve_provisioned_command_args_env(&datum)?;

    let mut vscode_json = serde_json::json!({
        "name": datum.name,
        "command": command,
        "args": args
    });
    if let Some(env) = env {
        vscode_json["env"] = serde_json::json!(env);
    }

    let json_str =
        serde_json::to_string(&vscode_json).context("Failed to serialize JSON for VSCode")?;

    let result = duct::cmd!("code", "--add-mcp", &json_str).run();

    match result {
        Ok(_) => {
            println!(
                "Successfully installed MCP server '{}' to VSCode",
                datum.name
            );
            println!("VSCode command: code --add-mcp '{}'", json_str);
        }
        Err(e) => {
            eprintln!("Failed to install MCP server to VSCode: {}", e);
            eprintln!("Manual command: code --add-mcp '{}'", json_str);
            return Err(anyhow::anyhow!("VSCode installation failed: {}", e));
        }
    }

    Ok(())
}

pub fn gemini_install_mcp(name: &str, path: &str, use_repo: bool) -> Result<()> {
    let datum = crate::get_mcp_config(name, path)?;
    let (command, args, env) = resolve_provisioned_command_args_env(&datum)?;

    let mut gemini_json = serde_json::json!({
        "name": datum.name,
        "command": command,
        "args": args
    });
    if let Some(env) = env {
        gemini_json["env"] = serde_json::json!(env);
    }

    let json_str =
        serde_json::to_string(&gemini_json).context("Failed to serialize JSON for Gemini CLI")?;

    let location_flag = if use_repo { "--repo" } else { "--user" };
    let result = duct::cmd!(
        "gemini",
        "mcp",
        "add-json",
        location_flag,
        &datum.name,
        &json_str
    )
    .run();

    match result {
        Ok(_) => {
            let location = if use_repo {
                "repository"
            } else {
                "user global"
            };
            println!(
                "Successfully installed MCP server '{}' to Gemini CLI ({})",
                datum.name, location
            );
            println!(
                "Gemini CLI command: gemini mcp add-json {} {} '{}'",
                location_flag, datum.name, json_str
            );
        }
        Err(e) => {
            let location = if use_repo {
                "repository"
            } else {
                "user global"
            };
            eprintln!(
                "Failed to install MCP server to Gemini CLI ({}): {}",
                location, e
            );
            eprintln!(
                "Manual command: gemini mcp add-json {} {} '{}'",
                location_flag, datum.name, json_str
            );
            return Err(anyhow::anyhow!("Gemini CLI installation failed: {}", e));
        }
    }

    Ok(())
}

pub fn codex_install_mcp(
    name: &str,
    path: &str,
    _use_repo: bool,
    stdio_command: Option<&str>,
    use_httpstream: bool,
) -> Result<()> {
    let datum = crate::get_mcp_config(name, path)?;
    let (command, args, env, method_type) =
        select_mcp_method(&datum, stdio_command, use_httpstream)?;

    let mut codex_args = vec!["mcp".to_string(), "add".to_string()];

    if let Some(env_map) = env {
        for (key, value) in env_map {
            codex_args.push("--env".to_string());
            codex_args.push(format!("{key}={value}"));
        }
    }

    codex_args.push(name.to_string());
    if method_type == "httpstream" {
        codex_args.push("--url".to_string());
        codex_args.push(command.clone());
    } else {
        codex_args.push("--".to_string());
        codex_args.push(command.clone());
        codex_args.extend(args.clone());
    }

    let result = duct::cmd("codex", &codex_args).run();

    match result {
        Ok(_) => {
            println!(
                "Successfully installed MCP server '{}' to Codex",
                datum.name
            );
            println!("Codex command: codex {}", codex_args.join(" "));
        }
        Err(e) => {
            eprintln!("Failed to install MCP server to Codex: {}", e);
            eprintln!("Manual command: codex {}", codex_args.join(" "));
            return Err(anyhow::anyhow!("Codex installation failed: {}", e));
        }
    }

    Ok(())
}

pub fn dotmcpjson_install_mcp(
    name: &str,
    path: &str,
    stdio_command: Option<&str>,
    use_httpstream: bool,
) -> Result<()> {
    use crate::utils::get_workspace_root;

    let datum = crate::get_mcp_config(name, path)?;
    let repo_root = get_workspace_root();
    let mcp_json_path = std::path::Path::new(&repo_root).join(".mcp.json");

    if !mcp_json_path.exists() {
        anyhow::bail!("No .mcp.json file found in repo root: {}", repo_root);
    }

    let existing_content =
        std::fs::read_to_string(&mcp_json_path).context("Failed to read .mcp.json file")?;

    let mut mcp_config: serde_json::Value =
        serde_json::from_str(&existing_content).context("Failed to parse .mcp.json file")?;

    if !mcp_config.is_object() {
        mcp_config = serde_json::json!({});
    }
    if !mcp_config["mcpServers"].is_object() {
        mcp_config["mcpServers"] = serde_json::json!({});
    }

    let (command, args, env, method_type) =
        select_mcp_method(&datum, stdio_command, use_httpstream)?;

    let server_config = if method_type == "httpstream" {
        serde_json::json!({ "url": command })
    } else {
        serde_json::json!({ "command": command, "args": args })
    };

    if let Some(method_env) = env {
        if let Some(server_obj) = server_config.as_object() {
            let mut new_config = server_obj.clone();
            new_config.insert("env".to_string(), serde_json::to_value(method_env)?);
            mcp_config["mcpServers"][&datum.name] = serde_json::Value::Object(new_config);
        }
    } else {
        mcp_config["mcpServers"][&datum.name] = server_config;
    }

    let updated_content = serde_json::to_string_pretty(&mcp_config)
        .context("Failed to serialize updated .mcp.json")?;

    std::fs::write(&mcp_json_path, updated_content)
        .context("Failed to write updated .mcp.json file")?;

    println!(
        "✅ Successfully installed MCP server '{}' to .mcp.json",
        datum.name
    );

    if method_type == "httpstream" {
        println!("🌐 Used httpstream method");
    } else if let Some(cmd) = stdio_command {
        println!("🎯 Used stdio method with command: {}", cmd);
    } else {
        println!("📡 Used default stdio method");
    }

    println!("📁 Updated: {}", mcp_json_path.display());
    Ok(())
}

/// Install an MCP server to opencode's config (~/.config/opencode/opencode.json).
pub fn opencode_install_mcp(
    name: &str,
    path: &str,
    stdio_command: Option<&str>,
    use_httpstream: bool,
) -> Result<()> {
    let datum = crate::get_mcp_config(name, path)?;
    let (command, args, env, method_type) =
        select_mcp_method(&datum, stdio_command, use_httpstream)?;

    let mut command_arr = vec![command.clone()];
    command_arr.extend(args.clone());

    let mut server_entry = serde_json::json!({
        "enabled": true,
        "type": "local",
        "command": command_arr
    });

    if let Some(env_map) = &env {
        if let Some(obj) = server_entry.as_object_mut() {
            obj.insert("env".to_string(), serde_json::to_value(env_map)?);
        }
    }

    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let config_path = home.join(".config").join("opencode").join("opencode.json");

    let config_path = if config_path.exists() {
        config_path
    } else {
        home.join(".config").join("opencode").join("opencode.jsonc")
    };

    let mut config: serde_json::Value = if config_path.exists() {
        let content =
            std::fs::read_to_string(&config_path).context("Failed to read opencode config")?;
        serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse opencode config: {}", e))?
    } else {
        serde_json::json!({})
    };

    if !config["mcp"].is_object() {
        config["mcp"] = serde_json::json!({});
    }

    config["mcp"][name] = server_entry;

    let updated =
        serde_json::to_string_pretty(&config).context("Failed to serialize opencode config")?;
    std::fs::write(&config_path, updated).context("Failed to write opencode config")?;

    println!(
        "✅ Successfully installed MCP server '{}' to opencode",
        name
    );
    println!("📁 Config: {}", config_path.display());

    if method_type == "httpstream" {
        println!("🌐 Used httpstream method");
    } else if let Some(cmd) = stdio_command {
        println!("🎯 Used stdio method with command: {}", cmd);
    } else {
        println!("📡 Used stdio method");
    }

    Ok(())
}

/// Push all repo .mcp.json servers into Codex CLI config via `codex mcp add`.
pub fn codex_sync_dotmcpjson(path: &str, use_repo: bool) -> Result<()> {
    use crate::utils::get_workspace_root;
    use std::path::Path;

    let _ = path;
    let repo_root = get_workspace_root();
    let mcp_json_path = Path::new(&repo_root).join(".mcp.json");

    if !mcp_json_path.exists() {
        anyhow::bail!("No .mcp.json file found in repo root: {}", repo_root);
    }

    let content = std::fs::read_to_string(&mcp_json_path)
        .context("Failed to read .mcp.json for Codex sync")?;
    let value: serde_json::Value =
        serde_json::from_str(&content).context("Failed to parse .mcp.json for Codex sync")?;
    let servers = value
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("Missing mcpServers in {}", mcp_json_path.display()))?;

    if servers.is_empty() {
        anyhow::bail!("No MCP servers present in {}", mcp_json_path.display());
    }

    let mut failures = Vec::new();

    for (name, config) in servers {
        let mut codex_cmd = std::process::Command::new("codex");
        codex_cmd.args(["mcp", "add"]);

        if let Some(env) = config.get("env").and_then(|v| v.as_object()) {
            for (key, value) in env {
                if let Some(value) = value.as_str() {
                    codex_cmd.args(["--env", &format!("{}={}", key, value)]);
                }
            }
        }

        if let Some(url) = config.get("url").and_then(|v| v.as_str()) {
            codex_cmd.args([name, "--url", url]);
        } else {
            let command = match config.get("command").and_then(|v| v.as_str()) {
                Some(command) => command,
                None => {
                    failures.push((name.clone(), "missing command or url".to_string()));
                    continue;
                }
            };
            codex_cmd.arg(name).arg("--").arg(command);
            if let Some(args) = config.get("args").and_then(|v| v.as_array()) {
                codex_cmd.args(args.iter().filter_map(|v| v.as_str()));
            }
        }

        match codex_cmd.status() {
            Ok(status) if status.success() => println!("Codex synced '{}'", name),
            Ok(status) => {
                failures.push((name.clone(), format!("exited with status {}", status)));
            }
            Err(e) => failures.push((name.clone(), e.to_string())),
        }
    }

    if failures.is_empty() {
        let location = if use_repo {
            "repository"
        } else {
            "user global"
        };
        println!(
            "✅ Synced {} MCP servers from {} into Codex ({})",
            servers.len(),
            mcp_json_path.display(),
            location
        );
        Ok(())
    } else {
        let details = failures
            .iter()
            .map(|(name, err)| format!("{}: {}", name, err))
            .collect::<Vec<_>>()
            .join("; ");
        Err(anyhow::anyhow!(
            "Failed to sync {} servers to Codex: {}",
            failures.len(),
            details
        ))
    }
}

/// Bidirectional MCP sync between b00t datums and external agent platforms.
pub fn mcp_sync_bidirectional(
    path: &str,
    operation: &str,
    source: &str,
    dest: &str,
    _agent: Option<&str>,
) -> Result<()> {
    let is_known_platform = |platform: &str| {
        matches!(
            platform,
            "b00t" | "kiro" | "claude" | "claudecode" | "codex" | "dotmcpjson" | "roocode"
        )
    };

    let normalized_op = operation.to_lowercase();
    let normalized_source = source.to_lowercase();
    let normalized_dest = dest.to_lowercase();

    if !is_known_platform(&normalized_source) {
        anyhow::bail!("Unknown platform '{}'", source);
    }
    if !is_known_platform(&normalized_dest) {
        anyhow::bail!("Unknown platform '{}'", dest);
    }

    match normalized_op.as_str() {
        "push" => {
            if normalized_source != "b00t" {
                anyhow::bail!("Push operation requires source to be 'b00t'");
            }

            match normalized_dest.as_str() {
                "codex" => codex_sync_dotmcpjson(path, true),
                "dotmcpjson" | "roocode" => {
                    for server_name in get_mcp_toml_files(path)? {
                        dotmcpjson_install_mcp(&server_name, path, None, false)?;
                    }
                    Ok(())
                }
                _ => anyhow::bail!(
                    "Push to platform '{}' is not implemented yet. Supported push destinations: codex, dotmcpjson, roocode",
                    dest
                ),
            }
        }
        "pull" => {
            if normalized_dest != "b00t" {
                anyhow::bail!("Pull operation requires destination to be 'b00t'");
            }
            anyhow::bail!(
                "Pull from platform '{}' is not implemented yet. Use MCP register/output as workaround",
                source
            )
        }
        _ => anyhow::bail!("Invalid operation '{}'", operation),
    }
}

// ── Dispatch Mode Trait Chain (#706) tests ──────────────────────────────
//
// resolve_all_datum_dispatches() used to be a linear function that tried
// runtime -> cli -> polyseme -> ooda -> mcp in sequence, inline. These
// tests exercise the trait-based replacement: each datum kind is now an
// independent `DispatchMode` implementor, and the chain is an ordered
// Vec<Box<dyn DispatchMode>>. Adding a new dispatch kind means appending a
// new implementor, not editing a match block.
#[cfg(test)]
mod dispatch_mode_tests {
    use super::*;

    /// Proves the chain is extensible: a brand-new mode, defined entirely
    /// in this test, participates in resolution without touching any of
    /// the built-in modes or resolve_all_datum_dispatches' body.
    struct AlwaysHitMode;
    impl DispatchMode for AlwaysHitMode {
        fn try_resolve(&self, candidate: &str, _path: &str) -> Option<DatumDispatch> {
            Some(DatumDispatch::Info(format!("always-hit:{candidate}")))
        }
    }

    #[test]
    fn default_chain_has_one_mode_per_datum_kind() {
        // Runtime, CliPassthrough, Polyseme, Ooda, Mcp
        assert_eq!(default_dispatch_chain().len(), 5);
    }

    #[test]
    fn chain_is_extensible_without_editing_existing_modes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();

        // Nothing on disk matches any built-in mode.
        let chain = default_dispatch_chain();
        assert!(chain.iter().all(|m| m.try_resolve("nope", path).is_none()));

        // Appending a new implementor is the only change needed to add a
        // dispatch kind — no match block to edit.
        let mut chain = default_dispatch_chain();
        chain.push(Box::new(AlwaysHitMode));
        let hit = chain.iter().find_map(|m| m.try_resolve("nope", path));
        assert!(matches!(hit, Some(DatumDispatch::Info(_))));
    }

    #[test]
    fn runtime_mode_matches_only_runtime_datum() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        std::fs::write(
            dir.path().join("rt.runtime.toml"),
            "[b00t]\nname = \"rt\"\ntype = \"runtime\"\n\n[b00t.runtime]\nbinary = \"/bin/true\"\n",
        )
        .unwrap();

        assert!(matches!(
            RuntimeMode.try_resolve("rt", path),
            Some(DatumDispatch::Runtime(_))
        ));
        assert!(CliPassthroughMode.try_resolve("rt", path).is_none());
    }

    #[test]
    fn cli_mode_matches_only_cli_datum() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        std::fs::write(
            dir.path().join("c.cli.toml"),
            "[b00t]\nname = \"c\"\ntype = \"cli\"\ncommand = \"echo\"\n",
        )
        .unwrap();

        match CliPassthroughMode.try_resolve("c", path) {
            Some(DatumDispatch::CliPassthrough { command, .. }) => assert_eq!(command, "echo"),
            other => panic!("expected CliPassthrough, got {:?}", other.is_some()),
        }
        assert!(RuntimeMode.try_resolve("c", path).is_none());
    }
}
