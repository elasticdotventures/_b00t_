use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Parser)]
pub enum WhatismyCommands {
    #[clap(about = "Detect current AI agent (claude, gemini, etc.)")]
    Agent {
        #[clap(long, help = "Ignore _B00T_Agent environment variable")]
        no_env: bool,
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(about = "Detect current session information")]
    Session {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(about = "Detect current environment setup")]
    Environment {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(about = "Show session-aware system status with OODA context")]
    Status {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(about = "Export template in specified format")]
    Template {
        #[clap(help = "Template name (e.g., 'status')")]
        name: String,
        #[clap(help = "Output format: toml, yaml, json, or tera")]
        format: String,
    },
    #[clap(about = "Show current agent role and blessed tools")]
    Role {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
        #[clap(long, help = "Show available tools for role")]
        show_tools: bool,
        /// Infer and rank skills from role datum + git log context
        #[clap(long, help = "Infer skills for role from datum + repo context")]
        skills: bool,
    },
}

impl WhatismyCommands {
    pub fn execute(&self, _path: &str) -> Result<()> {
        match self {
            WhatismyCommands::Agent { no_env, json } => {
                use crate::session_memory::SessionMemory;
                let memory = SessionMemory::load()?;
                let agent = detect_agent(&memory, *no_env);

                if *json {
                    println!(
                        "{}",
                        serde_json::to_string(&serde_json::json!({
                            "agent": agent,
                            "pid": std::process::id(),
                            "ppid": get_parent_pid(),
                        }))?
                    );
                } else {
                    println!("{}", agent);
                }
                Ok(())
            }
            WhatismyCommands::Session { json } => {
                use crate::session_memory::SessionMemory;
                let memory = SessionMemory::load()?;

                if *json {
                    println!(
                        "{}",
                        serde_json::to_string(&serde_json::json!({
                            "session_id": memory.metadata.session_id,
                            "pid": std::process::id(),
                            "created_at": memory.metadata.created_at,
                            "updated_at": memory.metadata.updated_at,
                            "branch": memory.metadata.initial_branch,
                        }))?
                    );
                } else {
                    println!("{}", memory.get_summary());
                }
                Ok(())
            }
            WhatismyCommands::Environment { json } => {
                use crate::session_memory::SessionMemory;
                let memory = SessionMemory::load()?;
                let env_info = memory.collect_tracked_env();

                if *json {
                    println!("{}", serde_json::to_string(&env_info)?);
                } else {
                    println!("🌍 Environment: {:?}", env_info);
                }
                Ok(())
            }
            WhatismyCommands::Status { json } => {
                use crate::session_memory::SessionMemory;
                let mut memory = SessionMemory::load()?;

                if *json {
                    let context = memory.get_agent_context();
                    println!("{}", serde_json::to_string_pretty(&context)?);
                } else {
                    // Try template rendering first, fall back to detailed diagnostics
                    match memory.render_status_template() {
                        Ok(rendered) => println!("{}", rendered),
                        Err(_) => {
                            // Fallback: Run the enhanced diagnostics
                            crate::commands::init::run_system_diagnostics(&mut memory)?;
                        }
                    }
                }
                Ok(())
            }
            WhatismyCommands::Template { name, format } => {
                use crate::session_memory::SessionMemory;
                let memory = SessionMemory::load()?;

                match name.as_str() {
                    "status" => {
                        match format.as_str() {
                            "tera" => {
                                // Output the raw Tera template
                                let template_content = memory
                                    .load_default_status_template()
                                    .unwrap_or_else(|_| "Error loading template".to_string());
                                println!("{}", template_content);
                            }
                            "toml" => {
                                // Output template config in TOML format
                                let context = memory.get_agent_context();
                                println!("[template.status]");
                                println!("# Agent context for template rendering");
                                println!("agent_name = \"{}\"", context.agent_name);
                                println!("session_id = \"{}\"", context.session_id);
                                println!("session_duration = {}", context.session_duration);
                                println!("current_branch = \"{}\"", context.current_branch);
                                println!("shell_count = {}", context.shell_count);
                                println!("build_count = {}", context.build_count);
                                println!("compile_count = {}", context.compile_count);
                                println!("test_count = {}", context.test_count);
                            }
                            "yaml" => {
                                // Output template config in YAML format
                                let context = memory.get_agent_context();
                                println!("template:");
                                println!("  status:");
                                println!("    # Agent context for template rendering");
                                println!("    agent_name: \"{}\"", context.agent_name);
                                println!("    session_id: \"{}\"", context.session_id);
                                println!("    session_duration: {}", context.session_duration);
                                println!("    current_branch: \"{}\"", context.current_branch);
                                println!("    shell_count: {}", context.shell_count);
                                println!("    build_count: {}", context.build_count);
                                println!("    compile_count: {}", context.compile_count);
                                println!("    test_count: {}", context.test_count);
                            }
                            "json" => {
                                // Output template config in JSON format
                                let context = memory.get_agent_context();
                                let template_data = serde_json::json!({
                                    "template": {
                                        "status": {
                                            "agent_name": context.agent_name,
                                            "session_id": context.session_id,
                                            "session_duration": context.session_duration,
                                            "current_branch": context.current_branch,
                                            "shell_count": context.shell_count,
                                            "build_count": context.build_count,
                                            "compile_count": context.compile_count,
                                            "test_count": context.test_count
                                        }
                                    }
                                });
                                println!("{}", serde_json::to_string_pretty(&template_data)?);
                            }
                            _ => {
                                return Err(anyhow::anyhow!(
                                    "Unsupported format: {}. Use: toml, yaml, json, or tera",
                                    format
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Unknown template: {}. Available: status",
                            name
                        ));
                    }
                }
                Ok(())
            }
            WhatismyCommands::Role { json, show_tools, skills } => {
                use crate::session_memory::SessionMemory;
                let memory = SessionMemory::load()?;

                // Detect current role based on agent patterns
                let agent = detect_agent(&memory, false);
                let role = detect_role_from_agent(&agent);

                // Load role supplement (AGENTS/--role=<role>.md) if present
                let supplement = load_role_supplement(&role);

                // Infer skills from role datum + git log context (if --skills or --json)
                let inferred_skills = if *skills || *json {
                    infer_skills_for_role(&role)
                } else {
                    Vec::new()
                };

                if *json {
                    let mut role_data = if *show_tools {
                        get_role_with_tools(&role)?
                    } else {
                        serde_json::json!({
                            "role": role,
                            "agent": agent,
                            "session_id": memory.metadata.session_id
                        })
                    };

                    // Merge skill inference into JSON output
                    if let Some(obj) = role_data.as_object_mut() {
                        obj.insert("inferred_skills".to_string(), serde_json::json!(inferred_skills));
                        if let Some(s) = &supplement {
                            obj.insert("role_supplement".to_string(), serde_json::json!(s));
                        }
                    }
                    println!("{}", serde_json::to_string_pretty(&role_data)?);
                } else {
                    println!("🎭 Role: {}", role);
                    println!("🤖 Agent: {}", agent);

                    if let Some(s) = &supplement {
                        println!("📋 Supplement: {}", s);
                    }

                    if *show_tools {
                        show_blessed_tools(&role)?;
                    }

                    if *skills && !inferred_skills.is_empty() {
                        println!("\n🎓 Inferred skills for role '{}' (ranked by context):", role);
                        for (i, skill) in inferred_skills.iter().enumerate() {
                            println!("  {}. {}", i + 1, skill);
                        }
                    } else if *skills {
                        println!("ℹ️  No skills inferred (no role datum or git context found)");
                    }
                }
                Ok(())
            }
        }
    }
}

use crate::session_memory::SessionMemory;

pub fn detect_agent(memory: &SessionMemory, no_env: bool) -> String {
    // Check environment variable first (unless no_env flag is set)
    if !no_env {
        if let Some(agent) = memory.get_env_var("_B00T_Agent") {
            if !agent.is_empty() {
                return agent;
            }
        }
    }

    // Detect based on parent process and environment
    let pid = std::process::id();
    let ppid = get_parent_pid();

    // 🤖 AAIII: Abstract AI Inference Interface detection
    // Priority: qwen > claude > codex > gemini > others
    
    // Detect Qwen Code CLI (new priority)
    if memory.get_env_var("QWEN_CODE").is_some()
        || std::env::vars().any(|(k, _)| k.starts_with("QWEN_"))
    {
        return format!("🤖 Qwen Code PID:{}", pid);
    }
    
    // Check if qwen CLI is available and we're in a qwen session
    if let Some(parent_cmd) = get_parent_command() {
        if parent_cmd.contains("qwen")
            && duct::cmd!("qwen", "--version").read().is_ok()
        {
            return format!("🤖 Qwen Code PID:{}", pid);
        }
    }

    // Detect OpenAI Codex sandbox (bun wrapper)
    if memory
        .get_env_var("CODEX_MANAGED_BY_BUN")
        .or_else(|| memory.get_env_var("CODEX_SANDBOX_NETWORK_DISABLED"))
        .is_some()
    {
        return format!("🤖 OpenAI Codex PID:{}", pid);
    }

    // Detect Gemini environments via env vars
    if std::env::vars().any(|(k, _)| k.starts_with("GEMINI_")) {
        return format!("🤖 Gemini PID:{}", pid);
    }

    // Check for Claude Code
    if memory.get_env_var("CLAUDECODE").as_deref() == Some("1") {
        return format!("🤖 Claude Code PID:{}", pid);
    }

    // Check for other AI environments using tracked variables
    if memory.get_env_var("ANTHROPIC_API_KEY").is_some()
        || memory.get_env_var("CLAUDE_API_KEY").is_some()
    {
        return format!("🤖 Claude Agent PID:{}", pid);
    }

    // Check parent process name against configured patterns
    if let Some(parent_cmd) = get_parent_command() {
        for (pattern, display) in &memory.config.agent_patterns {
            if parent_cmd.contains(pattern) {
                return format!("{} PID:{}", display, pid);
            }
        }
    }

    // Default: not an agent
    format!("🧑‍💻 Human PID:{} PPID:{}", pid, ppid.unwrap_or(0))
}

fn get_parent_pid() -> Option<u32> {
    #[cfg(unix)]
    {
        unsafe {
            let ppid = libc::getppid();
            if ppid > 0 { Some(ppid as u32) } else { None }
        }
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn get_parent_command() -> Option<String> {
    if let Some(ppid) = get_parent_pid() {
        // Try to read the parent command from /proc on Linux
        #[cfg(target_os = "linux")]
        {
            if let Ok(cmdline) = std::fs::read_to_string(format!("/proc/{}/comm", ppid)) {
                return Some(cmdline.trim().to_string());
            }
        }

        // Fallback: use ps command
        if let Ok(output) = duct::cmd!("ps", "-o", "comm=", "-p", ppid.to_string()).read() {
            return Some(output.trim().to_string());
        }
    }
    None
}

/// Agent blessing configuration
#[derive(Debug, Deserialize, Serialize)]
struct AgentBlessing {
    description: String,
    tools: Vec<String>,
    required_for_role: bool,
}

/// Detect role from agent string and environment
fn detect_role_from_agent(agent: &str) -> String {
    // Check _B00T_ROLE environment variable first
    if let Ok(role) = std::env::var("_B00T_ROLE") {
        if !role.is_empty() {
            return role.to_lowercase();
        }
    }

    // Fallback to agent-based detection
    if agent.contains("Claude") {
        "captain".to_string()
    } else if agent.contains("GPT") {
        "operator".to_string()
    } else {
        "unknown".to_string()
    }
}

/// Load blessings configuration
fn load_blessings() -> Result<HashMap<String, AgentBlessing>> {
    let config_dir = crate::session_memory::SessionMemory::get_config_path()?;
    let blessings_path = config_dir
        .join("_b00t_")
        .join("cake.🍰")
        .join("agents")
        .join("blessings.toml");

    if !blessings_path.exists() {
        return Ok(HashMap::new());
    }

    let content = std::fs::read_to_string(&blessings_path)?;
    let blessings: HashMap<String, AgentBlessing> = toml::from_str(&content)?;
    Ok(blessings)
}

/// Get role data with tools
fn get_role_with_tools(role: &str) -> Result<serde_json::Value> {
    let blessings = load_blessings()?;

    if let Some(blessing) = blessings.get(role) {
        Ok(serde_json::json!({
            "role": role,
            "description": blessing.description,
            "tools": blessing.tools,
            "required_for_role": blessing.required_for_role
        }))
    } else {
        Ok(serde_json::json!({
            "role": role,
            "description": "Unknown role",
            "tools": [],
            "required_for_role": false
        }))
    }
}

/// Show blessed tools for role
fn show_blessed_tools(role: &str) -> Result<()> {
    let blessings = load_blessings()?;

    if let Some(blessing) = blessings.get(role) {
        println!("📜 Description: {}", blessing.description);
        println!("🛠️  Blessed Tools:");
        for tool in &blessing.tools {
            let status = if tool_is_available(tool) {
                "✅"
            } else {
                "❌"
            };
            println!("  {} {}", status, tool);
        }
        println!(
            "🎯 Required: {}",
            if blessing.required_for_role {
                "Yes"
            } else {
                "No"
            }
        );
    } else {
        println!("❓ No blessings found for role: {}", role);
    }

    Ok(())
}

/// Check if a tool datum is available
fn tool_is_available(tool_name: &str) -> bool {
    let config_dir = crate::session_memory::SessionMemory::get_config_path().unwrap_or_default();
    let tool_path = config_dir.join("_b00t_").join(tool_name);
    tool_path.exists()
}

// ── Role supplement + skill inference ────────────────────────────────────────

/// Load `AGENTS/--role=<role>.md` supplement — returns the tail-map summary if present
fn load_role_supplement(role: &str) -> Option<String> {
    // Search: project-local AGENTS/ first, then ~/.b00t/AGENTS/
    let candidates = [
        std::path::PathBuf::from("AGENTS").join(format!("--role={}.md", role)),
        dirs::home_dir()
            .unwrap_or_default()
            .join(".b00t/AGENTS")
            .join(format!("--role={}.md", role)),
    ];

    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            // Extract tail-map summary line
            let summary = content
                .lines()
                .find(|l| l.trim_start_matches("# ").starts_with("summary:"))
                .map(|l| l.trim_start_matches("# ").trim_start_matches("summary:").trim().to_string())
                .unwrap_or_else(|| format!("{}({:?})", role, path));
            return Some(summary);
        }
    }
    None
}

/// Infer skills for a role by combining:
///   1. Role datum `skills = [...]` array from `_b00t_/<role>.role.tom(llm)`
///   2. SkillResolver search on git log topic tokens (last 20 commits)
///
/// Returns deduped skill names ranked by: role-datum declaration → git frequency
fn infer_skills_for_role(role: &str) -> Vec<String> {
    use crate::skill_resolver::SkillResolver;
    let mut skills: HashMap<String, usize> = HashMap::new();

    // 1. Role datum declared skills (high base weight = role-declared skills rank first)
    let declared = load_role_datum_skills(role);
    for s in declared {
        *skills.entry(s).or_insert(0) += 10;
    }

    // 2. Git log topic tokens → SkillResolver search (boost frequency-matched skills)
    let resolver = SkillResolver::default();
    if let Ok(tokens) = extract_git_log_topics() {
        for token in &tokens {
            let matches = resolver.search(token);
            for m in matches {
                *skills.entry(m.name).or_insert(0) += 1;
            }
        }
    }

    // Sort by weight descending, preserve insertion order for equal weights
    let mut sorted: Vec<(String, usize)> = skills.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.into_iter().map(|(name, _)| name).collect()
}

/// Parse `skills = [...]` from `_b00t_/<role>.role.toml(l)` datum
fn load_role_datum_skills(role: &str) -> Vec<String> {
    let candidates = [
        dirs::home_dir().unwrap_or_default().join(format!(".dotfiles/_b00t_/{}.role.tomllm", role)),
        dirs::home_dir().unwrap_or_default().join(format!(".dotfiles/_b00t_/{}.role.toml", role)),
        dirs::home_dir().unwrap_or_default().join(format!(".b00t/_b00t_/{}.role.tomllm", role)),
        dirs::home_dir().unwrap_or_default().join(format!(".b00t/_b00t_/{}.role.toml", role)),
    ];

    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            // Strip tomllm comment lines before TOML parsing
            let clean: String = content
                .lines()
                .filter(|l| !l.trim_start().starts_with("# "))
                .collect::<Vec<_>>()
                .join("\n");
            if let Ok(val) = toml::from_str::<toml::Value>(&clean) {
                if let Some(skills) = val.get("skills").and_then(|v| v.as_array()) {
                    return skills
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                }
            }
        }
    }
    Vec::new()
}

/// Extract topic-like tokens from `git log --oneline -20`
fn extract_git_log_topics() -> Result<Vec<String>> {
    let output = duct::cmd!("git", "log", "--oneline", "-20")
        .stderr_null()
        .read()
        .map_err(|e| anyhow::anyhow!("git log: {}", e))?;

    // Extract tokens: skip hash (first col), split on spaces/punctuation
    // Keep tokens ≥4 chars that are alphanumeric (likely identifiers/topics)
    let tokens: Vec<String> = output
        .lines()
        .flat_map(|line| {
            line.splitn(2, ' ')
                .nth(1)  // skip the hash
                .unwrap_or("")
                .split(|c: char| !c.is_alphanumeric() && c != '-')
                .filter(|t| t.len() >= 4)
                .map(|t| t.to_lowercase())
                .collect::<Vec<_>>()
        })
        .collect();

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whatismy_commands_exist() {
        let agent_cmd = WhatismyCommands::Agent {
            no_env: false,
            json: false,
        };

        assert!(agent_cmd.execute("test").is_ok());
    }
}
