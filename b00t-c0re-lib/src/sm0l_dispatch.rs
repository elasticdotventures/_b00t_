//! # SmolDispatch — cognitive tier routing for sm0l local inference
//!
//! Internal alias for "delegate this to the sm0l tier and get back compressed output."
//! ch0nky/frontier models call SmolDispatch; sm0l processes without tool calls.
//!
//! Architecture (OpenHarness mantra): "The model is the agent. The code is the harness."
//!   ch0nky decides WHAT to delegate → SmolDispatch routes HOW → sm0l executes passively.
//!
//! Endpoint priority (matches sm0l-filter.py):
//!   1. B00T_AI_SM0L_BASE env   (explicit override)
//!   2. :8001/v1               (ch0nky llamacpp MTP — currently Qwen3.6-27B)
//!   3. :8000/v1               (any local OpenAI-compat server)
//!   4. HF Inference API       (serverless, requires HF_TOKEN)
//!   5. Err(NoEndpoint)        — governance gate, never silently pass-through
//!
//! Session logs: /tmp/b00t-sm0l-<session-id>/ — ephemeral, cleared by session_cleanup().
//! ch0nky receives log_path in SmolOutput.log_path and MAY read it; ignores if unneeded.

use anyhow::{Result, anyhow};
use std::env;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Behavior chains ──────────────────────────────────────────────────────────

/// Named behavior chains for sm0l delegation.
/// Each variant maps to a system prompt template + output contract.
#[derive(Debug, Clone, PartialEq)]
pub enum SmolBehavior {
    /// Filter build/test/runtime output → errors only. Most common delegation.
    FilterErrors(FilterTask),
    /// Collapse long output to N key points. For ch0nky pre-digestion.
    Summarize { max_output_lines: usize },
    /// Binary pass/fail: did this command succeed? Returns "ok" or first failure line.
    CheckOutputOk,
}

/// Task-specific error filter templates. Each has a specialized system prompt.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterTask {
    Cargo,
    Podman,
    Systemd,
    Hive,
    General,
}

impl FilterTask {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Cargo => "cargo",
            Self::Podman => "podman",
            Self::Systemd => "systemd",
            Self::Hive => "hive",
            Self::General => "general",
        }
    }

    fn system_prompt(&self) -> &'static str {
        let _suffix =
            " Return ONLY matching lines. If none: respond with exactly: -\nNo commentary.";
        match self {
            Self::Cargo => concat!(
                "You are a Rust build/test log filter. ",
                "Extract ONLY: compile errors (error[E*]), test failures (FAILED, panicked at), ",
                "linker errors, missing crate errors. ",
                "Deduplicate repeated error codes with ×N count suffix. ",
                "Suppress: warnings, note:, help:, arrow source paths.",
                " Return ONLY matching lines. If none: respond with exactly: -\nNo commentary."
            ),
            Self::Podman => concat!(
                "You are a container runtime log filter. ",
                "Extract ONLY: pull failures, image not found, permission denied, OOM killed, ",
                "exit code non-zero, CDI errors, ERRO level lines.",
                " Return ONLY matching lines. If none: respond with exactly: -\nNo commentary."
            ),
            Self::Systemd => concat!(
                "You are a systemd/journalctl log filter. ",
                "Extract ONLY: Failed, failed, Error, error, killed, segfault, start-limit-hit.",
                " Return ONLY matching lines. If none: respond with exactly: -\nNo commentary."
            ),
            Self::Hive => concat!(
                "You are a b00t hive log filter. ",
                "Extract ONLY: resource gate failures, service start failures, ",
                "exclusion group conflicts, guard BLOCK violations.",
                " Return ONLY matching lines. If none: respond with exactly: -\nNo commentary."
            ),
            Self::General => concat!(
                "You are a terse error extractor. ",
                "Extract ONLY lines containing: error, failed, panic, fatal, denied, killed ",
                "(case-insensitive). Deduplicate with ×N count suffix.",
                " Return ONLY matching lines. If none: respond with exactly: -\nNo commentary."
            ),
        }
    }
}

impl SmolBehavior {
    fn system_prompt(&self) -> String {
        match self {
            Self::FilterErrors(task) => task.system_prompt().to_string(),
            Self::Summarize { max_output_lines } => format!(
                "You are a terse summarizer. Collapse the input to at most {max_output_lines} \
                 bullet points covering only novel information. Remove duplicates. \
                 Output bullets only, no preamble."
            ),
            Self::CheckOutputOk => concat!(
                "You are a pass/fail checker. ",
                "If the input shows a successful outcome with no errors: respond with exactly: ok\n",
                "Otherwise: respond with the first failure line verbatim. No other output."
            ).to_string(),
        }
    }

    fn log_tag(&self) -> String {
        match self {
            Self::FilterErrors(t) => format!("filter-{}", t.as_str()),
            Self::Summarize { .. } => "summarize".to_string(),
            Self::CheckOutputOk => "check-ok".to_string(),
        }
    }
}

// ─── Endpoint discovery ───────────────────────────────────────────────────────

/// Resolved sm0l endpoint. Cheap to clone; construction probes the network.
#[derive(Debug, Clone)]
pub struct SmolEndpoint {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub source: EndpointSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EndpointSource {
    EnvOverride,
    LocalPort(u16),
    HfInference,
}

impl fmt::Display for SmolEndpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.model, self.base_url)
    }
}

impl SmolEndpoint {
    /// Probe in priority order; return first live endpoint.
    pub fn discover() -> Result<Self> {
        Self::discover_with_env(|key| env::var(key).ok())
    }

    pub fn discover_with_env<F: Fn(&str) -> Option<String>>(env_fn: F) -> Result<Self> {
        Self::discover_with(env_fn, Self::probe)
    }

    /// Full DI variant — both env lookup and port probe are injected. Used in tests.
    pub fn discover_with<F, P>(env_fn: F, probe_fn: P) -> Result<Self>
    where
        F: Fn(&str) -> Option<String>,
        P: Fn(&str) -> bool,
    {
        // 1. explicit override
        if let Some(base_url) = env_fn("B00T_AI_SM0L_BASE") {
            return Ok(SmolEndpoint {
                model: env_fn("B00T_AI_SM0L_MODEL").unwrap_or_else(|| "sm0l".to_string()),
                api_key: env_fn("B00T_AI_SM0L_KEY").unwrap_or_else(|| "local-b00t".to_string()),
                base_url,
                source: EndpointSource::EnvOverride,
            });
        }

        // 2–3. probe local ports
        for (port, model) in [(8001u16, "ch0nky"), (8000u16, "sm0l")] {
            let url = format!("http://127.0.0.1:{port}/v1");
            if probe_fn(&url) {
                return Ok(SmolEndpoint {
                    base_url: url,
                    model: model.to_string(),
                    api_key: "local-b00t".to_string(),
                    source: EndpointSource::LocalPort(port),
                });
            }
        }

        // 4. HF Inference API
        if let Some(token) = env_fn("HF_TOKEN") {
            if !token.is_empty() {
                return Ok(SmolEndpoint {
                    base_url: "https://api-inference.huggingface.co/v1".to_string(),
                    model: "Qwen/Qwen2.5-3B-Instruct".to_string(),
                    api_key: token,
                    source: EndpointSource::HfInference,
                });
            }
        }

        Err(anyhow!(
            "SmolDispatch: no endpoint. Run: b00t hive activate inference-qwen36-27b-mtp-podman"
        ))
    }

    fn probe(base_url: &str) -> bool {
        let client = match reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
        {
            Ok(c) => c,
            Err(_) => return false, // fail closed — TLS/env issues
        };
        for path in ["/health", "/v1/models"] {
            let url = format!("{}{}", base_url.trim_end_matches('/'), path);
            if let Ok(resp) = client.get(&url).send() {
                if resp.status().as_u16() < 500 {
                    return true;
                }
            }
        }
        false
    }
}

// ─── Session logging ───────────────────────────────────────────────────────────

/// Ephemeral session log directory. Persists on disk until explicit cleanup.
/// ch0nky receives log_path in SmolOutput and MAY read the full I/O; ignores by default.
pub struct SmolSession {
    pub id: String,
    pub dir: PathBuf,
    counter: AtomicUsize,
}

impl SmolSession {
    /// Create new session under /tmp/b00t-sm0l-<timestamp>-<pid>/
    pub fn new() -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let pid = std::process::id();
        let id = format!("{ts}-{pid}");
        let dir = PathBuf::from(format!("/tmp/b00t-sm0l-{id}"));
        let _ = fs::create_dir_all(&dir);
        Self {
            id,
            dir,
            counter: AtomicUsize::new(0),
        }
    }

    /// Load existing session by ID (for ch0nky to reference prior sm0l output).
    pub fn load(id: &str) -> Option<Self> {
        let dir = PathBuf::from(format!("/tmp/b00t-sm0l-{id}"));
        if dir.exists() {
            Some(Self {
                id: id.to_string(),
                dir,
                counter: AtomicUsize::new(0),
            })
        } else {
            None
        }
    }

    fn next_n(&self) -> usize {
        self.counter.fetch_add(1, Ordering::Relaxed)
    }

    fn write_log(&self, n: usize, tag: &str, kind: &str, content: &str) -> PathBuf {
        let path = self.dir.join(format!("{n:04}-{tag}.{kind}"));
        if let Ok(mut f) = fs::File::create(&path) {
            let _ = f.write_all(content.as_bytes());
        }
        path
    }

    /// Log a sm0l interaction. Returns (prompt_path, input_path, output_path).
    pub fn log(
        &self,
        tag: &str,
        prompt: &str,
        input: &str,
        output: &str,
    ) -> (PathBuf, PathBuf, PathBuf) {
        let n = self.next_n();
        let p = self.write_log(n, tag, "prompt", prompt);
        let i = self.write_log(n, tag, "input", input);
        let o = self.write_log(n, tag, "output", output);
        (p, i, o)
    }

    /// Remove all session files. Call at session end.
    pub fn cleanup(&self) {
        let _ = fs::remove_dir_all(&self.dir);
    }

    /// Remove all sm0l session dirs older than max_age_secs.
    pub fn cleanup_old(max_age_secs: u64) {
        let tmp = Path::new("/tmp");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let Ok(entries) = fs::read_dir(tmp) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("b00t-sm0l-") {
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                if let Ok(modified) = meta.modified() {
                    if let Ok(age) = modified.duration_since(UNIX_EPOCH) {
                        if now.saturating_sub(age.as_secs()) > max_age_secs {
                            let _ = fs::remove_dir_all(entry.path());
                        }
                    }
                }
            }
        }
    }
}

impl Default for SmolSession {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Dispatch output ──────────────────────────────────────────────────────────

/// What ch0nky gets back from a sm0l delegation.
#[derive(Debug)]
pub struct SmolOutput {
    /// Filtered/processed content. None = clean (no issues found).
    pub result: Option<String>,
    /// Path to the full I/O log. ch0nky reads this only if it needs to investigate.
    pub log_path: Option<PathBuf>,
    pub stats: SmolStats,
    pub endpoint: String,
}

impl SmolOutput {
    /// True if sm0l found no issues (result is None or sentinel "-").
    pub fn is_clean(&self) -> bool {
        match &self.result {
            None => true,
            Some(s) => s.trim() == "-",
        }
    }

    /// One-line summary for executive context. Keeps frontier tokens minimal.
    pub fn summary_line(&self) -> String {
        if self.is_clean() {
            format!(
                "[sm0l:{}] ✅ clean ({} raw → {} unique)",
                self.endpoint, self.stats.raw_lines, self.stats.unique_lines
            )
        } else {
            let preview = self
                .result
                .as_deref()
                .unwrap_or("")
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(120)
                .collect::<String>();
            let log_ref = self
                .log_path
                .as_ref()
                .map(|p| format!(" log:{}", p.display()))
                .unwrap_or_default();
            format!("[sm0l:{}] ⚠ {preview}…{log_ref}", self.endpoint)
        }
    }
}

#[derive(Debug, Default)]
pub struct SmolStats {
    pub raw_lines: usize,
    pub unique_lines: usize,
    pub was_truncated: bool,
}

// ─── Dispatch entry point ─────────────────────────────────────────────────────

/// Main dispatch function. Deduplicate, call sm0l, log, return SmolOutput.
pub fn dispatch(
    behavior: &SmolBehavior,
    input: &str,
    session: Option<&SmolSession>,
    max_input_bytes: usize,
) -> Result<SmolOutput> {
    let endpoint = SmolEndpoint::discover()?;
    dispatch_with_endpoint(behavior, input, session, max_input_bytes, &endpoint)
}

pub fn dispatch_with_endpoint(
    behavior: &SmolBehavior,
    input: &str,
    session: Option<&SmolSession>,
    max_input_bytes: usize,
    endpoint: &SmolEndpoint,
) -> Result<SmolOutput> {
    // 1. Preprocess: dedup + strip ANSI + truncate
    let (processed, stats) = preprocess(input, max_input_bytes);
    let system = behavior.system_prompt();
    let tag = behavior.log_tag();

    // 2. Call sm0l
    let raw_result = call_endpoint(endpoint, &system, &processed)?;
    let result = if raw_result.trim() == "-" || raw_result.trim().is_empty() {
        None
    } else {
        Some(raw_result.clone())
    };

    // 3. Log to session if provided
    let log_path = session.map(|s| {
        let (_, _, out_path) = s.log(&tag, &system, &processed, &raw_result);
        out_path
    });

    Ok(SmolOutput {
        result,
        log_path,
        stats,
        endpoint: endpoint.to_string(),
    })
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

static ANSI_RE: OnceLock<regex::Regex> = OnceLock::new();

fn preprocess(raw: &str, max_bytes: usize) -> (String, SmolStats) {
    // Strip ANSI escape sequences — compiled once via OnceLock
    let ansi_re = ANSI_RE.get_or_init(|| regex::Regex::new(r"\x1b\[[0-9;]*[mGKH]").unwrap());

    let mut seen: std::collections::HashMap<[u8; 8], usize> = Default::default();
    let mut order: Vec<String> = Vec::new();
    let max_unique = 500;

    for line in raw.lines() {
        let norm = ansi_re.replace_all(line, "").trim().to_string();
        if norm.is_empty() {
            continue;
        }
        // 8-byte hash for dedup key
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        norm.hash(&mut h);
        let key = h.finish().to_le_bytes();

        *seen.entry(key).or_insert(0) += 1;
        if seen[&key] == 1 {
            order.push(norm);
        }
        if order.len() >= max_unique {
            break;
        }
    }

    let raw_lines = raw.lines().count();
    let unique_lines = order.len();
    let was_truncated = unique_lines >= max_unique;

    let mut text = String::new();
    for line in &order {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        line.hash(&mut h);
        let key = h.finish().to_le_bytes();
        let n = seen[&key];
        if n > 1 {
            text.push_str(&format!("{line}  ×{n}\n"));
        } else {
            text.push_str(line);
            text.push('\n');
        }
        if text.len() >= max_bytes {
            break;
        }
    }

    (
        text,
        SmolStats {
            raw_lines,
            unique_lines,
            was_truncated,
        },
    )
}

fn call_endpoint(endpoint: &SmolEndpoint, system: &str, content: &str) -> Result<String> {
    let payload = serde_json::json!({
        "model": endpoint.model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user",   "content": content},
        ],
        "max_tokens": 1024,
        "temperature": 0.0,
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let resp = client
        .post(format!(
            "{}/chat/completions",
            endpoint.base_url.trim_end_matches('/')
        ))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", endpoint.api_key))
        .json(&payload)
        .send()
        .map_err(|e| anyhow!("sm0l endpoint call failed: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        let body_text = resp.text().unwrap_or_default();
        return Err(anyhow!("sm0l endpoint HTTP {status}: {body_text}"));
    }
    let body: serde_json::Value = resp.json()?;
    let text = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("-")
        .trim()
        .to_string();
    Ok(text)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_task_prompts_non_empty() {
        for task in [
            FilterTask::Cargo,
            FilterTask::Podman,
            FilterTask::Systemd,
            FilterTask::Hive,
            FilterTask::General,
        ] {
            let p = task.system_prompt();
            assert!(!p.is_empty());
            assert!(
                p.contains("Return ONLY"),
                "sentinel instruction missing for {:?}",
                task
            );
        }
    }

    #[test]
    fn test_smol_output_is_clean() {
        let clean = SmolOutput {
            result: None,
            log_path: None,
            stats: SmolStats::default(),
            endpoint: "ch0nky(http://127.0.0.1:8001/v1)".to_string(),
        };
        assert!(clean.is_clean());

        let sentinel = SmolOutput {
            result: Some("-".to_string()),
            log_path: None,
            stats: SmolStats::default(),
            endpoint: "ch0nky(http://127.0.0.1:8001/v1)".to_string(),
        };
        assert!(sentinel.is_clean());

        let with_errors = SmolOutput {
            result: Some("error[E0499]: cannot borrow".to_string()),
            log_path: None,
            stats: SmolStats::default(),
            endpoint: "ch0nky(http://127.0.0.1:8001/v1)".to_string(),
        };
        assert!(!with_errors.is_clean());
    }

    #[test]
    fn test_endpoint_discover_env_override() {
        // Full DI: env + probe both injected — no network, no env mutation
        let ep = SmolEndpoint::discover_with(
            |key| match key {
                "B00T_AI_SM0L_BASE" => Some("http://test:9999/v1".to_string()),
                _ => None,
            },
            |_| false, // probe returns nothing live
        )
        .unwrap();
        assert_eq!(ep.base_url, "http://test:9999/v1");
        assert_eq!(ep.source, EndpointSource::EnvOverride);
    }

    #[test]
    fn test_endpoint_discover_hf_fallback() {
        // Full DI: probe returns false (no local ports live) → falls through to HF
        let ep = SmolEndpoint::discover_with(
            |key| match key {
                "HF_TOKEN" => Some("hf_test_token".to_string()),
                _ => None,
            },
            |_| false,
        )
        .unwrap();
        assert_eq!(ep.source, EndpointSource::HfInference);
        assert!(ep.base_url.contains("huggingface"));
    }

    #[test]
    fn test_preprocess_dedup() {
        let input = "error[E0499]: foo\nwarning: bar\nerror[E0499]: foo\n";
        let (processed, stats) = preprocess(input, 65536);
        assert_eq!(stats.raw_lines, 3);
        assert_eq!(stats.unique_lines, 2); // foo deduped
        assert!(processed.contains("×2"));
    }

    #[test]
    fn test_smol_session_lifecycle() {
        let session = SmolSession::new();
        assert!(session.dir.exists());
        let (_, _, out) = session.log("test", "system", "input", "output");
        assert!(out.exists());
        session.cleanup();
        assert!(!session.dir.exists());
    }

    #[test]
    fn test_behavior_log_tags() {
        assert_eq!(
            SmolBehavior::FilterErrors(FilterTask::Cargo).log_tag(),
            "filter-cargo"
        );
        assert_eq!(SmolBehavior::CheckOutputOk.log_tag(), "check-ok");
        assert_eq!(
            SmolBehavior::Summarize {
                max_output_lines: 5
            }
            .log_tag(),
            "summarize"
        );
    }
}
