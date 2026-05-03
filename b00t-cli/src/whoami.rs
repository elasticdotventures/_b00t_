use crate::entanglement::parse_entanglement_ref;
use crate::skill_resolver::SkillResolver;
use crate::{DatumType, UnifiedConfig, get_config, get_expanded_path};
use anyhow::{Context, Result};
use b00t_c0re_lib::TemplateRenderer;
use std::fs;

/// Detect current AI agent based on environment variables
pub fn detect_agent(ignore_env: bool) -> String {
    // Check if _B00T_Agent is already set and we're not ignoring env
    if !ignore_env {
        if let Ok(agent) = std::env::var("_B00T_Agent") {
            if !agent.is_empty() {
                return agent;
            }
        }
    }

    // Check for Claude Code
    if std::env::var("CLAUDECODE").unwrap_or_default() == "1" {
        return "claude".to_string();
    }

    // TODO: Add detection for other agents based on their shell environment:
    // - gemini: specific environment vars set by gemini-cli shell
    // - codex: specific environment vars set by codex shell
    // - other agents: their respective shell environment indicators

    // Return empty string if no agent detected
    "".to_string()
}

/// Display agent identity information from AGENT.md template and role datum (if available)
pub fn whoami(path: &str, role_override: Option<String>, with_skills: bool) -> Result<()> {
    let expanded_path = get_expanded_path(path)?;
    let agent_md_path = expanded_path.join("AGENT.md");

    if !agent_md_path.exists() {
        anyhow::bail!(
            "AGENT.md not found in {}. This file contains agent identity information.",
            expanded_path.display()
        );
    }

    let template_content = fs::read_to_string(&agent_md_path).context(format!(
        "Failed to read AGENT.md from {}",
        agent_md_path.display()
    ))?;

    // Use b00t-c0re-lib template renderer
    let renderer =
        TemplateRenderer::with_defaults().context("Failed to create template renderer")?;

    let rendered = renderer
        .render(&template_content)
        .context("Failed to render template")?;

    println!("{}", rendered);

    // Append role supplement (AGENTS/--role=<role>.md) BEFORE role datum summary
    // 🤓 supplement = full AGENTS/--role=*.md content (MCP patterns, crew scaling, etc.)
    if let Some(role_name) = resolve_role(role_override.clone()) {
        let supplement_candidates = [
            std::path::PathBuf::from("AGENTS").join(format!("--role={}.md", role_name)),
            dirs::home_dir()
                .unwrap_or_default()
                .join(".b00t/AGENTS")
                .join(format!("--role={}.md", role_name)),
        ];
        for p in &supplement_candidates {
            if let Ok(content) = fs::read_to_string(p) {
                println!("\n{}", content);
                break;
            }
        }
    }

    // Append role summary from .role.tomllmd / .role.tomllm / .role.toml
    // and .agent.tomllmd / .agent.tomllm / .agent.toml datum.
    // 🤓 role datums are executable + introspectable: skills, capabilities, entanglements
    // .tomllmd currently downgrades to the generic .tomllm/TOML path.
    if let Some(role) = resolve_role(role_override) {
        if let Some(role_details) = load_role_datum(&role, path) {
            print_role_summary(&role_details, path, with_skills);
        } else {
            println!(
                "⚠️ Role datum '{}' not found or missing required fields",
                role
            );
        }
    }

    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoleDetails {
    pub name: String,
    pub hint: String,
    pub skills: Vec<String>,
    pub compliance: Vec<String>,
    pub entangled_agents: Vec<String>,
    pub entangled_cli: Vec<String>,
    pub entangled_mcp: Vec<String>,
    pub channel_prefix: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct CapabilityCheck {
    reference: String,
    expected_type: DatumType,
    status: CapabilityStatus,
}

#[derive(Clone, Debug, PartialEq)]
enum CapabilityStatus {
    Ready,
    Missing,
    TypeMismatch { found: DatumType },
    InvalidReference,
}

fn resolve_role(role_override: Option<String>) -> Option<String> {
    role_override
        .filter(|r| !r.trim().is_empty())
        .or_else(|| std::env::var("_B00T_ROLE").ok())
        .map(|r| r.to_lowercase())
}

fn load_role_datum(role: &str, path: &str) -> Option<RoleDetails> {
    // 🤓 prefer .role.tomllmd / .role.tomllm / .role.toml over other typed datums with same name
    let (config, _) = get_config_with_type_preference(role, &DatumType::Role, path).ok()?;
    let datum = config.b00t;

    if let Some(datum_type) = &datum.datum_type {
        if datum_type != &DatumType::Role {
            return None;
        }
    } else {
        // Without an explicit type we assume a generic datum is intended for this role
    }

    let skills = datum.skills.unwrap_or_default();
    let compliance = datum.compliance.unwrap_or_default();
    let entangled_agents = datum.entangled_agents.unwrap_or_default();
    let entangled_cli = datum.entangled_cli.unwrap_or_default();
    let entangled_mcp = datum.entangled_mcp.unwrap_or_default();
    let channel_prefix = datum.channel_prefix;

    Some(RoleDetails {
        name: datum.name,
        hint: datum.hint,
        skills,
        compliance,
        entangled_agents,
        entangled_cli,
        entangled_mcp,
        channel_prefix,
    })
}

fn summarize_list(items: &[String], max_items: usize) -> Option<String> {
    if items.is_empty() {
        return None;
    }

    let shown: Vec<String> = items.iter().take(max_items).cloned().collect();
    let remaining = items.len().saturating_sub(shown.len());

    let mut summary = shown.join(", ");
    if remaining > 0 {
        summary.push_str(&format!(" (+{} more)", remaining));
    }

    Some(summary)
}

fn collect_capability_checks(role: &RoleDetails, path: &str) -> Vec<CapabilityCheck> {
    let mut checks = Vec::new();

    for reference in &role.entangled_agents {
        checks.push(check_role_capability(reference, DatumType::Agent, path));
    }
    for reference in &role.entangled_cli {
        checks.push(check_role_capability(reference, DatumType::Cli, path));
    }
    for reference in &role.entangled_mcp {
        checks.push(check_role_capability(reference, DatumType::Mcp, path));
    }

    checks
}

fn check_role_capability(reference: &str, fallback_type: DatumType, path: &str) -> CapabilityCheck {
    let (name, ref_type) = match parse_entanglement_ref(reference) {
        Ok((name, ref_type)) => (name, ref_type),
        Err(_) => {
            return CapabilityCheck {
                reference: reference.to_string(),
                expected_type: fallback_type,
                status: CapabilityStatus::InvalidReference,
            };
        }
    };

    let expected_type = ref_type.unwrap_or(fallback_type);
    let (config, filename) = match get_config_with_type_preference(&name, &expected_type, path) {
        Ok(config) => config,
        Err(_) => {
            return CapabilityCheck {
                reference: reference.to_string(),
                expected_type,
                status: CapabilityStatus::Missing,
            };
        }
    };

    let found = config.b00t.get_datum_type(Some(&filename));
    if found == expected_type {
        CapabilityCheck {
            reference: reference.to_string(),
            expected_type,
            status: CapabilityStatus::Ready,
        }
    } else {
        CapabilityCheck {
            reference: reference.to_string(),
            expected_type,
            status: CapabilityStatus::TypeMismatch { found },
        }
    }
}

fn get_config_with_type_preference(
    name: &str,
    expected_type: &DatumType,
    path: &str,
) -> Result<(UnifiedConfig, String), Box<dyn std::error::Error>> {
    let expanded_path = get_expanded_path(path)?;
    let base = expected_type.base_suffix();
    if let Some(result) = get_config_typed(name, base, &expanded_path) {
        return Ok(result);
    }
    get_config(name, path)
}

/// Try typed extensions for `name` — .tomllmd first, then .tomllm, then .toml.
/// Returns the first match, or falls back to generic get_config.
fn get_config_typed<'a>(
    name: &str,
    base: &str,
    expanded_path: &std::path::Path,
) -> Option<(UnifiedConfig, String)> {
    for ext in [".tomllmd", ".tomllm", ".toml"] {
        let filename = format!("{}{}{}", name, base, ext);
        let config_path = expanded_path.join(&filename);
        if config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                if let Ok(config) = toml::from_str::<UnifiedConfig>(&content) {
                    return Some((config, filename));
                }
            }
        }
    }
    None
}

fn print_role_summary(role: &RoleDetails, path: &str, with_skills: bool) {
    println!("🎭 Role: {}", role.name);
    println!("💡 {}", role.hint);

    if with_skills && !role.skills.is_empty() {
        // Resolve skill metadata for all declared skills (discovery tier)
        // Prefer resolving skills relative to the expanded `path` (repository root),
        // falling back to the current directory if anything goes wrong.
        let mut resolved = Vec::new();

        if let Ok(expanded_path) = get_expanded_path(path) {
            if let Ok(original_dir) = std::env::current_dir() {
                if std::env::set_current_dir(&expanded_path).is_ok() {
                    let resolver = SkillResolver::default();
                    resolved = resolver.list_for_role(&role.skills);
                    // Best-effort restore of the original working directory.
                    let _ = std::env::set_current_dir(original_dir);
                }
            }
        }

        // If resolution relative to `path` failed, fall back to the original behavior.
        if resolved.is_empty() {
            let resolver = SkillResolver::default();
            resolved = resolver.list_for_role(&role.skills);
        }

        println!(
            "🧠 Skills ({} declared, {} resolved):",
            role.skills.len(),
            resolved.len()
        );
        for m in &resolved {
            let tags = if m.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", m.tags.join(", "))
            };
            println!("   • {} — {}{}", m.name, m.description, tags);
        }
        // Show unresolved skill names so agent knows what to find
        let resolved_names: std::collections::HashSet<_> =
            resolved.iter().map(|m| m.name.as_str()).collect();
        let unresolved: Vec<_> = role
            .skills
            .iter()
            .filter(|s| !resolved_names.contains(s.as_str()))
            .collect();
        if !unresolved.is_empty() {
            println!(
                "   ⚠️ Unresolved: {} (no skill datum found)",
                unresolved
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    } else if let Some(skills_summary) = summarize_list(&role.skills, 5) {
        println!(
            "🧠 Skills: {} (use --with-skills to resolve)",
            skills_summary
        );
    }

    if let Some(compliance_summary) = summarize_list(&role.compliance, 3) {
        println!("⚖️ Compliance: {}", compliance_summary);
    }

    if let Some(agent_summary) = summarize_list(&role.entangled_agents, 5) {
        println!("🤖 Sub-agents: {}", agent_summary);
    }

    if let Some(cli_summary) = summarize_list(&role.entangled_cli, 5) {
        println!("🛠️ CLI tools: {}", cli_summary);
    }

    if let Some(mcp_summary) = summarize_list(&role.entangled_mcp, 5) {
        println!("🔌 MCP tools: {}", mcp_summary);
    }

    let checks = collect_capability_checks(role, path);
    if !checks.is_empty() {
        println!("🩺 Capability check:");
        for check in checks {
            match check.status {
                CapabilityStatus::Ready => {
                    println!("   ✅ {} [{}]", check.reference, check.expected_type);
                }
                CapabilityStatus::Missing => {
                    println!(
                        "   ❌ {} [{} missing]",
                        check.reference, check.expected_type
                    );
                }
                CapabilityStatus::TypeMismatch { found } => {
                    println!(
                        "   ❌ {} [expected {}, found {}]",
                        check.reference, check.expected_type, found
                    );
                }
                CapabilityStatus::InvalidReference => {
                    println!("   ❌ {} [invalid reference format]", check.reference);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Serialize env-mutating tests — process env is global state
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_detect_agent_claude() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::set_var("_B00T_Agent", "test-agent");
            std::env::set_var("CLAUDECODE", "1");
        }
        assert_eq!(detect_agent(true), "claude");
        unsafe {
            std::env::remove_var("CLAUDECODE");
            std::env::remove_var("_B00T_Agent");
        }
    }

    #[test]
    fn test_detect_agent_env_variable() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("CLAUDECODE");
            std::env::set_var("_B00T_Agent", "test-agent");
        }
        assert_eq!(detect_agent(false), "test-agent");
        unsafe {
            std::env::remove_var("_B00T_Agent");
        }
    }

    #[test]
    fn test_detect_agent_ignore_env() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("CLAUDECODE");
            std::env::set_var("_B00T_Agent", "test-agent");
        }
        assert_eq!(detect_agent(true), "");
        unsafe {
            std::env::remove_var("_B00T_Agent");
        }
    }

    #[test]
    fn test_resolve_role_prefers_override() {
        unsafe {
            std::env::set_var("_B00T_ROLE", "captain");
        }
        let resolved = resolve_role(Some("executive".to_string()));
        assert_eq!(resolved, Some("executive".to_string()));
        unsafe {
            std::env::remove_var("_B00T_ROLE");
        }
    }

    #[test]
    fn test_summarize_list_limits_output() {
        let items = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let summary = summarize_list(&items, 2).unwrap();
        assert_eq!(summary, "a, b (+2 more)");
    }

    #[test]
    fn test_load_role_datum_includes_entangled_capabilities() {
        let temp = TempDir::new().unwrap();
        let role_path = temp.path().join("orchestrator.role.toml");
        fs::write(
            role_path,
            r#"[b00t]
name = "orchestrator"
type = "role"
hint = "orchestrator"
skills = ["delegation"]
compliance = ["monitor chat"]
entangled_agents = ["ralph.agent"]
entangled_cli = ["b00t.cli"]
entangled_mcp = ["ralph.mcp"]
channel_prefix = "agent:executive:"
"#,
        )
        .unwrap();

        let role = load_role_datum("orchestrator", temp.path().to_str().unwrap()).unwrap();
        assert_eq!(role.name, "orchestrator");
        assert_eq!(role.entangled_agents, vec!["ralph.agent".to_string()]);
        assert_eq!(role.entangled_cli, vec!["b00t.cli".to_string()]);
        assert_eq!(role.entangled_mcp, vec!["ralph.mcp".to_string()]);
        assert_eq!(role.channel_prefix, Some("agent:executive:".to_string()));
    }

    #[test]
    fn test_collect_capability_checks_reports_ready_and_missing() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        fs::write(
            path.join("orchestrator.role.toml"),
            r#"[b00t]
name = "orchestrator"
type = "role"
hint = "orchestrator"
entangled_agents = ["ralph.agent", "ghost.agent"]
entangled_cli = ["b00t.cli"]
entangled_mcp = ["ralph.mcp", "b00t-mcp.mcp"]
"#,
        )
        .unwrap();
        fs::write(
            path.join("ralph.agent.toml"),
            r#"[b00t]
name = "ralph"
type = "agent"
hint = "ralph agent"
"#,
        )
        .unwrap();
        fs::write(
            path.join("b00t.cli.toml"),
            r#"[b00t]
name = "b00t"
type = "cli"
hint = "b00t cli"
"#,
        )
        .unwrap();
        fs::write(
            path.join("ralph.mcp.toml"),
            r#"[b00t]
name = "ralph"
type = "mcp"
hint = "ralph mcp"
"#,
        )
        .unwrap();

        let role = load_role_datum("orchestrator", path.to_str().unwrap()).unwrap();
        let checks = collect_capability_checks(&role, path.to_str().unwrap());

        assert!(
            checks
                .iter()
                .any(|c| { c.reference == "ralph.agent" && c.status == CapabilityStatus::Ready })
        );
        assert!(
            checks
                .iter()
                .any(|c| { c.reference == "b00t.cli" && c.status == CapabilityStatus::Ready })
        );
        assert!(
            checks
                .iter()
                .any(|c| { c.reference == "ghost.agent" && c.status == CapabilityStatus::Missing })
        );
        assert!(
            checks.iter().any(|c| {
                c.reference == "b00t-mcp.mcp" && c.status == CapabilityStatus::Missing
            })
        );
    }

    #[test]
    fn test_check_role_capability_detects_invalid_reference() {
        let check = check_role_capability("ralph.agent.extra", DatumType::Agent, "/tmp");
        assert_eq!(check.status, CapabilityStatus::InvalidReference);
    }

    #[test]
    fn test_check_role_capability_prefers_typed_file_extension() {
        let temp = TempDir::new().unwrap();
        let path = temp.path();

        fs::write(
            path.join("ralph.agent.toml"),
            r#"[b00t]
name = "ralph"
type = "agent"
hint = "ralph agent"
"#,
        )
        .unwrap();
        fs::write(
            path.join("ralph.mcp.toml"),
            r#"[b00t]
name = "ralph"
type = "mcp"
hint = "ralph mcp"
"#,
        )
        .unwrap();

        let check = check_role_capability("ralph.mcp", DatumType::Mcp, path.to_str().unwrap());
        assert_eq!(check.status, CapabilityStatus::Ready);
    }
}
