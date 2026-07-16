use anyhow::{Context, Result};
use chrono::Utc;

use crate::{
    AiConfig, BootDatum, DatumType, RuntimeConfig,
    SessionState, UnifiedConfig,
};

// ── Config creation ────────────────────────────────────────────────────

pub fn create_ai_toml_config(ai_config: &AiConfig, path: &str) -> Result<()> {
    let toml_content =
        toml::to_string(ai_config).context("Failed to serialize AI config to TOML")?;

    let mut path_buf = std::path::PathBuf::new();
    path_buf.push(shellexpand::tilde(path).to_string());
    path_buf.push(format!("{}.ai.toml", ai_config.b00t.name));

    std::fs::write(&path_buf, toml_content).context(format!(
        "Failed to write AI config to {}",
        path_buf.display()
    ))?;

    println!("Created AI config: {}", path_buf.display());
    Ok(())
}

pub fn create_unified_toml_config(datum: &BootDatum, path: &str) -> Result<()> {
    let config = UnifiedConfig {
        b00t: datum.clone(),
        service_contract: vec![],
        env: None,
        sections: None,
    };

    let toml_content = toml::to_string(&config).context("Failed to serialize config to TOML")?;

    // Use explicit datum_type or default to Unknown
    let datum_type = datum.datum_type.clone().unwrap_or(DatumType::Unknown);
    let suffix = datum_type.file_extension();

    let mut path_buf = std::path::PathBuf::new();
    path_buf.push(shellexpand::tilde(path).to_string());
    path_buf.push(format!("{}{}", datum.name, suffix));

    std::fs::write(&path_buf, toml_content)
        .context(format!("Failed to write config to {}", path_buf.display()))?;

    println!(
        "Created {} config: {}",
        datum_type.to_string(),
        path_buf.display()
    );
    Ok(())
}

pub fn create_mcp_toml_config(package: &BootDatum, path: &str) -> Result<()> {
    create_unified_toml_config(package, path)
}

// ── Path resolution ────────────────────────────────────────────────────

pub fn get_expanded_path(path: &str) -> Result<std::path::PathBuf> {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};

    static WARNED_LEGACY: AtomicBool = AtomicBool::new(false);

    let primary = PathBuf::from(shellexpand::tilde(path).to_string());
    if primary.exists() {
        return Ok(primary);
    }

    let legacy = PathBuf::from(shellexpand::tilde("~/.dotfiles/_b00t_").to_string());
    if legacy.exists() {
        if !WARNED_LEGACY.swap(true, Ordering::SeqCst) {
            eprintln!("⚠️ Using legacy b00t path at {}", legacy.display());
        }
        return Ok(legacy);
    }

    Ok(primary)
}

/// Find a project by walking up from cwd to git root looking for .git/🥾.tomllmd
pub fn find_project_b00t() -> Option<std::path::PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let mut current = cwd.as_path();
    loop {
        let marker = current.join(".git").join("🥾.tomllmd");
        if marker.exists() {
            return current.join("_b00t_").is_dir().then(|| current.join("_b00t_"));
        }
        if current.join(".git").exists() {
            break;
        }
        current = current.parent()?;
    }
    None
}

/// Load project-local version overrides from .git/🥾.tomllmd
pub fn load_project_overrides() -> std::collections::HashMap<String, String> {
    let mut overrides = std::collections::HashMap::new();
    let path = {
        let cwd = match std::env::current_dir() {
            Ok(d) => d,
            Err(_) => return overrides,
        };
        let mut cur = cwd.as_path();
        loop {
            let git_boot = cur.join(".git").join("🥾.tomllmd");
            if git_boot.exists() {
                break Some(git_boot);
            }
            if cur.join(".git").exists() {
                break None;
            }
            cur = match cur.parent() {
                Some(p) => p,
                None => break None,
            };
        }
    };
    if let Some(path) = path {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(toml) = content.parse::<toml::Table>() {
                if let Some(o) = toml.get("overrides").and_then(|v| v.as_table()) {
                    for (k, v) in o {
                        if let Some(val) = v.as_str() {
                            overrides.insert(k.clone(), val.to_string());
                        }
                    }
                }
            }
        }
    }
    overrides
}

// ── Datum discovery ────────────────────────────────────────────────────

pub fn get_ai_tools_status(path: &str) -> Result<Vec<Box<dyn crate::StatusProvider>>> {
    use crate::datum_ai::AiDatum;
    let mut tools: Vec<Box<dyn crate::StatusProvider>> = Vec::new();
    let expanded_path = get_expanded_path(path)?;

    if let Ok(entries) = std::fs::read_dir(&expanded_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                    if file_name.ends_with(".ai.toml") {
                        if let Some(tool_name) = file_name.strip_suffix(".ai.toml") {
                            if let Ok(datum) = AiDatum::try_from((tool_name, path)) {
                                tools.push(Box::new(datum));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(tools)
}

pub fn get_config(
    command: &str,
    path: &str,
) -> Result<(UnifiedConfig, String), Box<dyn std::error::Error>> {
    // Try project-local _b00t_/ first, then fall back to global path
    let dirs: Vec<std::path::PathBuf> = {
        let mut v = Vec::new();
        if let Some(project) = find_project_b00t() {
            v.push(project);
        }
        if let Ok(expanded) = get_expanded_path(path) {
            v.push(expanded);
        }
        v
    };

    for dir in &dirs {
        for base in DatumType::all_base_suffixes() {
            for ext in [".tomllmd", ".tomllm", ".toml"] {
                let p = dir.join(format!("{}{}{}", command, base, ext));
                if p.exists() {
                    let filename =
                        p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                    let content = std::fs::read_to_string(&p)?;
                    let mut config: UnifiedConfig = toml::from_str(&content)?;
                    crate::datum_utils::apply_git_attributes_to_config(&mut config, &p);
                    return Ok((config, filename));
                }
            }
        }
        for ext in [".tomllmd", ".tomllm", ".toml"] {
            let plain = dir.join(format!("{}{}", command, ext));
            if plain.exists() {
                let filename = format!("{}{}", command, ext);
                let content = std::fs::read_to_string(&plain)?;
                let mut config: UnifiedConfig = toml::from_str(&content)?;
                crate::datum_utils::apply_git_attributes_to_config(&mut config, &plain);
                return Ok((config, filename));
            }
        }
    }

    Err(format!("{} UNDEFINED", command).into())
}

pub fn get_mcp_config(name: &str, path: &str) -> Result<BootDatum> {
    let mut path_buf = get_expanded_path(path)?;
    path_buf.push(format!("{}.mcp.toml", name));

    if !path_buf.exists() {
        anyhow::bail!(
            "MCP server '{}' not found. Use 'b00t-cli mcp add' to create it first.",
            name
        );
    }

    let content = std::fs::read_to_string(&path_buf)
        .context(format!("Failed to read MCP config from {}", path_buf.display()))?;

    let mut config: UnifiedConfig =
        toml::from_str(&content).context("Failed to parse MCP config TOML")?;
    crate::datum_utils::apply_git_attributes_to_config(&mut config, &path_buf);

    Ok(config.b00t)
}

/// Load a runtime wrapper datum, returning the parsed [RuntimeConfig].
pub fn load_runtime_datum(name: &str, path: &str) -> Result<RuntimeConfig> {
    let expanded_path = get_expanded_path(path)?;
    let suffixes = &[".runtime.toml", ".runtime.tomllmd", ".runtime.tomllm"];
    let mut found: Option<std::path::PathBuf> = None;

    for suffix in suffixes {
        let candidate = expanded_path.join(format!("{name}{suffix}"));
        if candidate.exists() {
            found = Some(candidate);
            break;
        }
    }

    let file_path = found.ok_or_else(|| {
        anyhow::anyhow!(
            "runtime datum '{name}' not found (tried {}/{name}.runtime.toml[lmd|lm])",
            expanded_path.display()
        )
    })?;

    let content = std::fs::read_to_string(&file_path)
        .context(format!("Failed to read runtime config from {}", file_path.display()))?;
    let config: UnifiedConfig =
        toml::from_str(&content).context(format!("Failed to parse {}", file_path.display()))?;

    config
        .b00t
        .runtime
        .ok_or_else(|| anyhow::anyhow!("datum '{}' missing [b00t.runtime] section", name))
}

// ── Generic datum provider loader ──────────────────────────────────────

/// Generic loader for datum providers keyed by file extension.
pub fn load_datum_providers<T>(
    path: &str,
    extension: &str,
) -> Result<Vec<Box<dyn crate::DatumProvider>>>
where
    T: crate::DatumProvider + 'static,
    T: for<'a> TryFrom<(&'a str, &'a str), Error = anyhow::Error>,
{
    let mut tools: Vec<Box<dyn crate::DatumProvider>> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Scan project-local first, then global — project overrides global
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Some(project) = find_project_b00t() {
        dirs.push(project);
    }
    if let Ok(global) = get_expanded_path(path) {
        dirs.push(global);
    }

    for dir in &dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                    if file_name.ends_with(extension) {
                        if let Some(tool_name) = file_name.strip_suffix(extension) {
                            if seen.insert(tool_name.to_string()) {
                                if let Ok(datum) = T::try_from((tool_name, path)) {
                                    tools.push(Box::new(datum));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(tools)
}

// ── Session management ────────────────────────────────────────────────

impl SessionState {
    pub fn new(agent_name: Option<String>) -> Self {
        let session_id = format!("b00t_{}", Utc::now().timestamp_millis() % 100000000);

        let agent_info = agent_name.map(|name| crate::AgentInfo {
            name: name.clone(),
            model_size: std::env::var("MODEL_SIZE").ok(),
            role: std::env::var("ROLE").ok(),
            pid: std::process::id(),
            privacy_level: std::env::var("PRIVACY").ok(),
        });

        SessionState {
            session_id,
            start_time: Utc::now(),
            commands_run: 0,
            estimated_cost: 0.0,
            budget_limit: std::env::var("B00T_BUDGET")
                .ok()
                .and_then(|s| s.parse().ok()),
            time_limit_minutes: std::env::var("B00T_TIME_LIMIT")
                .ok()
                .and_then(|s| s.parse().ok()),
            agent_info,
            hints: vec![],
            last_activity: Utc::now(),
        }
    }

    pub fn get_session_file_path() -> Result<std::path::PathBuf> {
        let session_id =
            std::env::var("B00T_SESSION_ID").unwrap_or_else(|_| "current".to_string());
        let tmp_dir = std::env::temp_dir();
        Ok(tmp_dir.join(format!("b00t_session_{}.json", session_id)))
    }

    pub fn load() -> Result<Self> {
        let path = Self::get_session_file_path()?;
        if path.exists() {
            let content =
                std::fs::read_to_string(&path).context("Failed to read session file")?;
            serde_json::from_str(&content).context("Failed to parse session file")
        } else {
            Ok(Self::new(std::env::var("_B00T_Agent").ok()))
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::get_session_file_path()?;
        let content =
            serde_json::to_string_pretty(self).context("Failed to serialize session")?;
        std::fs::write(&path, content).context("Failed to write session file")?;
        Ok(())
    }

    pub fn increment_command(&mut self, estimated_cost: f64) {
        self.commands_run += 1;
        self.estimated_cost += estimated_cost;
        self.last_activity = Utc::now();
    }

    pub fn get_status_line(&self) -> String {
        let duration = Utc::now().signed_duration_since(self.start_time);
        let elapsed_mins = duration.num_minutes();

        let cost_info = if self.estimated_cost > 0.0 {
            format!(" ${:.3}", self.estimated_cost)
        } else {
            String::new()
        };

        let time_info = if elapsed_mins > 0 {
            format!(" {}m", elapsed_mins)
        } else {
            format!(" {}s", duration.num_seconds())
        };

        let agent_info = self
            .agent_info
            .as_ref()
            .map(|a| format!(" {}", a.name))
            .unwrap_or_default();

        format!(
            "🥾 {} cmds{}{}{}",
            self.commands_run, cost_info, time_info, agent_info
        )
    }
}

// ── Session handler functions ─────────────────────────────────────────

fn check_readme_status(memory: &mut crate::session_memory::SessionMemory) -> Result<()> {
    use crate::utils::get_workspace_root;
    let git_root = get_workspace_root();
    let readme_path = std::path::PathBuf::from(&git_root).join("README.md");

    if readme_path.exists() {
        if !memory.is_readme_read() {
            println!("📖 README.md found but not yet marked as read");
            println!("💡 Run `b00t-cli session mark-readme-read` after reading it");
        } else {
            println!("✅ README.md already read this session");
        }
    } else {
        println!("ℹ️  No README.md found in git root");
    }

    Ok(())
}

/// Initialize a session and persist its state.
pub fn handle_session_init(
    budget: &Option<f64>,
    time_limit: &Option<u32>,
    agent: Option<&str>,
) -> Result<()> {
    let agent_name = agent
        .map(|s| s.to_string())
        .or_else(|| std::env::var("_B00T_Agent").ok())
        .filter(|s| !s.is_empty());

    let mut session = SessionState::new(agent_name);

    if let Some(budget) = budget {
        session.budget_limit = Some(*budget);
    }

    if let Some(time_limit) = time_limit {
        session.time_limit_minutes = Some(*time_limit);
    }

    // Set session ID in environment
    unsafe {
        std::env::set_var("B00T_SESSION_ID", &session.session_id);
    }

    session.save()?;

    // Initialize session memory and check README.md
    let mut memory = crate::session_memory::SessionMemory::load()?;
    check_readme_status(&mut memory)?;

    println!("🥾 Session {} initialized", session.session_id);

    if let Some(agent) = &session.agent_info {
        println!("🤖 Agent: {}", agent.name);
    }

    if let Some(budget) = session.budget_limit {
        println!("💰 Budget: ${:.2}", budget);
    }

    if let Some(time_limit) = session.time_limit_minutes {
        println!("⏱️  Time limit: {}m", time_limit);
    }

    Ok(())
}

/// Display current session status.
pub fn handle_session_status() -> Result<()> {
    let session = SessionState::load()?;
    println!("{}", session.get_status_line());

    if !session.hints.is_empty() {
        println!("💡 Hints:");
        for hint in &session.hints {
            println!("   • {}", hint);
        }
    }

    Ok(())
}

/// Update session state with cost and optional hint.
pub fn handle_session_update(cost: &Option<f64>, hint: Option<&str>) -> Result<()> {
    let mut session = SessionState::load()?;

    if let Some(cost) = cost {
        session.increment_command(*cost);
    } else {
        session.increment_command(0.0);
    }

    if let Some(hint) = hint {
        session.hints.push(hint.to_string());
    }

    session.save()?;
    Ok(())
}

/// End the current session and clear persisted state.
pub fn handle_session_end() -> Result<()> {
    let session = SessionState::load()?;
    let path = SessionState::get_session_file_path()?;

    println!("🥾 Session {} ended", session.session_id);
    println!("📊 Final stats: {}", session.get_status_line());

    if path.exists() {
        std::fs::remove_file(&path).context("Failed to remove session file")?;
    }

    unsafe {
        std::env::remove_var("B00T_SESSION_ID");
    }
    Ok(())
}

/// Print a one-line status prompt for the current session.
pub fn handle_session_prompt() -> Result<()> {
    let session = SessionState::load()?;
    print!("{}", session.get_status_line());
    Ok(())
}
