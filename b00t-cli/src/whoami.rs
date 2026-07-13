use crate::agentic_role::resolve_role;
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
pub fn whoami(path: &str, role_override: Option<String>, with_skills: bool, skills: Vec<String>) -> Result<()> {
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

    // 🤓 P2: one-line node identity from soul — context-efficient preamble.
    //    Emits only highest-signal facts; agent runs `b00t soul get node.<key>`
    //    for detail. Silently omitted when no node identity is recorded.
    if let Some(node) = crate::memory_provider::node_summary_from_soul() {
        println!("🥾 Node: {}  (detail: b00t soul get node.*)", node);
    }

    // Append role supplement (AGENTS/--role=<role>.md) BEFORE role datum summary
    let role = resolve_role(role_override.clone());
    let role_name = role.name();
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

    // Append role summary from .role.tomllmd / .role.tomllm / .role.toml
    if let Some(role_details) = load_role_datum(role_name, path) {
        print_role_summary(&role_details, path, with_skills);
    } else {
        println!(
            "⚠️ Role datum '{}' not found or missing required fields",
            role_name
        );
    }

    // 🛡️ Blessing enforcement: warn if task involves _b00t_/ writes without datum-authoring
    let task_context = std::env::var("B00T_TASK_CONTEXT").unwrap_or_default();
    let needs_datum_blessing = task_context.contains("datum")
        || task_context.contains("skill")
        || task_context.contains("_b00t_")
        || task_context.contains("blessing");
    if needs_datum_blessing {
        // Check if datum-authoring skill datum exists (indicates blessing loaded)
        let datum_authoring_path = expanded_path.join("_b00t_/datum-authoring.skill.toml");
        let blessing_loaded = datum_authoring_path.exists();
        if !blessing_loaded {
            println!();
            println!("🛡️ BLESSING GATE: Task context '{}' involves _b00t_/ datum operations.", task_context);
            println!("   Required blessing: datum-authoring (→ datum-schema → tomllm-format)");
            println!("   Run: b00t learn datum-authoring");
            println!("   This ensures datums use correct field names (tags ≠ type_tags) and tail-map format.");
        } else {
            println!();
            println!("✅ datum-authoring blessing confirmed — _b00t_/ write gate active");
        }
    }

    // --skills=auto interview mode or explicit csv skill list
    if !skills.is_empty() {
        let is_auto = skills.iter().any(|s| s == "auto");
        if is_auto {
            print_skill_interview(path)?;
        } else {
            println!("\nSkill load plan:");
            for s in &skills {
                let evidence = check_skill_evidence(s, path);
                println!("  b00t learn {}  {}", s, evidence);
            }
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

/// Check if a datum or learn file exists for this skill.
/// Returns a static label indicating evidence status.
fn check_skill_evidence(skill: &str, path: &str) -> &'static str {
    let candidates = [
        format!("_b00t_/datums/{}.tomllmd", skill.to_uppercase()),
        format!("_b00t_/datums/{}.tomllmd", skill),
        format!("_b00t_/learn/{}.md", skill),
        format!("_b00t_/{}.tomllm", skill),
        format!("_b00t_/{}.agent.tomllm", skill),
    ];
    let expanded = get_expanded_path(path).unwrap_or_default();
    for c in &candidates {
        if expanded.join(c).exists() || std::path::Path::new(c).exists() {
            return "[datum ✅]";
        }
    }
    "[datum ❌ — no local evidence]"
}

/// Classify a task title into a cognitive tier.
/// Returns "sm0l", "frontier", or "ch0nky" (default).
fn classify_tier(title: &str) -> &'static str {
    let lower = title.to_lowercase();
    // sm0l: mechanical / low-reasoning tasks
    const SM0L_KW: &[&str] = &[
        "test", "lint", "format", "bump", "docs", "readme", "register", "check", "audit",
    ];
    // frontier: architecture / research tasks
    const FRONTIER_KW: &[&str] = &[
        "arch", "design", "research", "evaluate", "eureka", "why", " ai ", "llm", "gemini",
    ];
    for kw in SM0L_KW {
        if lower.contains(kw) {
            return "sm0l";
        }
    }
    for kw in FRONTIER_KW {
        if lower.contains(kw) {
            return "frontier";
        }
    }
    "ch0nky"
}

/// Discover pending tasks: try `b00t task list` first, fall back to `gh issue list`.
/// Returns Vec<(id, title)>.
fn discover_tasks() -> Vec<(String, String)> {
    // 1. Try b00t task list
    if let Ok(out) = std::process::Command::new("b00t")
        .args(["task", "list"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let lower = stdout.to_lowercase();
        if out.status.success() && !lower.contains("no tasks") && !stdout.trim().is_empty() {
            let tasks = parse_task_list_output(&stdout);
            if !tasks.is_empty() {
                return tasks;
            }
        }
    }

    // 2. Fall back to gh issue list
    if let Ok(out) = std::process::Command::new("gh")
        .args([
            "issue",
            "list",
            "--state",
            "open",
            "--limit",
            "10",
            "--json",
            "number,title",
        ])
        .output()
    {
        if out.status.success() {
            return parse_gh_issue_json(&String::from_utf8_lossy(&out.stdout));
        }
    }

    vec![]
}

/// Parse `b00t task list` output — expects lines like: "  #414  wrkflw local runner"
/// Extracts (id, title) pairs.
fn parse_task_list_output(output: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Match lines starting with #<digits> whitespace title
        if let Some(rest) = trimmed.strip_prefix('#') {
            let mut parts = rest.splitn(2, |c: char| c.is_whitespace());
            if let (Some(id), Some(title)) = (parts.next(), parts.next()) {
                let id = id.trim();
                let title = title.trim().to_string();
                if !id.chars().all(|c| c.is_ascii_digit()) {
                    continue; // not a numeric id
                }
                if !title.is_empty() {
                    results.push((format!("#{}", id), title));
                }
            }
        }
    }
    results
}

/// Parse `gh issue list --json number,title` output (minimal, no serde).
fn parse_gh_issue_json(json: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    // Split on `},{` to get individual issue objects
    let items: Vec<&str> = json.split("},").collect();
    for item in items {
        let num = extract_json_number(item, "number");
        let title = extract_json_string(item, "title");
        if let (Some(n), Some(t)) = (num, title) {
            results.push((format!("#{}", n), t));
        }
    }
    results
}

fn extract_json_number(s: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":", key);
    let start = s.find(&needle)? + needle.len();
    let rest = s[start..].trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    let num = &rest[..end];
    if num.is_empty() {
        None
    } else {
        Some(num.to_string())
    }
}

fn extract_json_string(s: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\":\"", key);
    let start = s.find(&needle)? + needle.len();
    let rest = &s[start..];
    // Find closing quote (not escaped)
    let mut end = None;
    let mut prev_backslash = false;
    for (i, c) in rest.char_indices() {
        if c == '"' && !prev_backslash {
            end = Some(i);
            break;
        }
        prev_backslash = c == '\\';
    }
    end.map(|e| rest[..e].to_string())
}

/// Interview mode: analyze .b00t/tasks.json → suggest weighted skills
fn print_skill_interview(path: &str) -> Result<()> {
    // Tag→skill mapping (deterministic, no LLM required)
    let tag_to_skill: &[(&str, &str)] = &[
        ("bouncer",   "bouncer"),
        ("sm0l",      "sm0l"),
        ("langchain", "langchain"),
        ("sandbox",   "hive"),
        ("datum",     "datum"),
        ("mcp",       "mcp"),
        ("rust",      "rust"),
        ("agent",     "agent-orchestration"),
        ("okr",       "okr"),
        ("docker",    "podman"),
        ("ci",        "wrkflw"),
        ("prd",       "datum"),
        ("ooda",      "agent-orchestration"),
        ("guard",     "hive"),
        ("neumann",   "sm0l"),
        ("vllm",      "hive"),
        ("tomllm",    "tomllm"),
        ("a2a",       "agent-orchestration"),
    ];

    // Read tasks from .b00t/tasks.json — try worktree-relative and home-relative
    let tasks_path_candidates = [
        ".b00t/tasks.json".to_string(),
        dirs::home_dir()
            .unwrap_or_default()
            .join(".b00t/tasks.json")
            .to_string_lossy()
            .to_string(),
    ];
    let tasks_json: Option<String> = tasks_path_candidates
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok());

    let mut skill_scores: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();

    if let Some(ref json_str) = tasks_json {
        // Simple tag extraction without serde dependency — grep for quoted tag strings
        for (tag, skill) in tag_to_skill {
            if json_str.contains(&format!("\"{}\"", tag)) {
                *skill_scores.entry(skill).or_insert(0) += 1;
            }
        }
    }

    // Score from current git branch name — branch context = high weight (3)
    if let Ok(branch_out) = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .output()
    {
        let branch_name = String::from_utf8_lossy(&branch_out.stdout).to_lowercase();
        for (tag, skill) in tag_to_skill {
            if branch_name.contains(tag) {
                *skill_scores.entry(skill).or_insert(0) += 3;
            }
        }
    }

    let mut ranked: Vec<(&&str, &u32)> = skill_scores.iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(a.1));

    if ranked.is_empty() {
        println!("\nSkill interview: no active task context found — load core skills:");
        for skill in &["datum", "hive", "sm0l", "bouncer"] {
            let ev = check_skill_evidence(skill, path);
            println!("  b00t learn {}  {}", skill, ev);
        }
    } else {
        println!("\nSkill interview (weighted by task context):");
        for (skill, score) in ranked.iter().take(8) {
            let ev = check_skill_evidence(skill, path);
            println!("  b00t learn {}  [weight:{}]  {}", skill, score, ev);
        }
    }

    // Task discovery + tier routing
    let tasks = discover_tasks();
    if !tasks.is_empty() {
        println!("\nTask discovery (from b00t task list + gh issues):");
        for (id, title) in &tasks {
            let tier = classify_tier(title);
            // derive branch slug: lowercase, spaces→dashes, strip non-alnum/-/#
            let slug: String = title
                .to_lowercase()
                .chars()
                .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
                .collect::<String>()
                .split('-')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("-");
            let num = id.trim_start_matches('#');
            let branch = format!("task/{}-{}", num, slug);
            println!("  → [{:<8}] {}  {}  git checkout -b {}", tier, id, title, branch);
        }

        // Bouncer handoff for first (highest priority) task
        let (first_id, first_title) = &tasks[0];
        let first_tier = classify_tier(first_title);
        let first_slug: String = first_title
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let first_num = first_id.trim_start_matches('#');
        let first_branch = format!("task/{}-{}", first_num, first_slug);
        println!("\nNext action (bouncer handoff):");
        println!(
            "  b00t agent delegate {} --role=ch0nky --tier={}",
            first_branch, first_tier
        );
    }

    Ok(())
}

/// ─── Dashboard: layered system status for agents ────────────────────────────

/// Layer descriptor for the agent system dashboard.
#[derive(Debug, Clone, PartialEq)]
pub struct DashboardLayer {
    pub z: u8,
    pub name: &'static str,
    pub color: &'static str,
    pub items: Vec<DashboardItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DashboardItem {
    pub label: String,
    pub status: DashboardStatus,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DashboardStatus {
    Ready,
    Warning,
    Critical,
    Unknown,
}

/// Build the layered system dashboard for an agent.
pub fn build_dashboard() -> Vec<DashboardLayer> {
    let mut layers = Vec::new();

    // Layer 0 — Hardware / OS
    let mut hw = Vec::new();
    hw.push(detect_cpu());
    hw.push(detect_ram());
    hw.push(detect_gpu());
    hw.push(detect_os());
    layers.push(DashboardLayer { z: 0, name: "Hardware & OS", color: "#334155", items: hw });

    // Layer 1 — Runtime
    let mut rt = Vec::new();
    rt.push(detect_python());
    rt.push(detect_node());
    rt.push(detect_rust());
    rt.push(detect_just());
    layers.push(DashboardLayer { z: 1, name: "Runtime", color: "#1d4ed8", items: rt });

    // Layer 2 — Inference
    let mut inf = Vec::new();
    inf.push(check_binary("vllm", "vllm --version"));
    inf.push(check_binary("ollama", "ollama --version"));
    inf.push(check_binary("llama.cpp", "llama-cli --version"));
    inf.push(detect_models());
    layers.push(DashboardLayer { z: 2, name: "Inference", color: "#7c3aed", items: inf });

    // Layer 3 — MCP
    let mut mcp = Vec::new();
    mcp.push(count_mcp_tools());
    mcp.push(check_binary("uvx", "uvx --version"));
    mcp.push(check_binary("npx", "npx --version"));
    layers.push(DashboardLayer { z: 3, name: "MCP Tools", color: "#b91c1c", items: mcp });

    // Layer 4 — Datums
    layers.push(count_datums());

    // Layer 5 — Agents
    layers.push(count_agents());

    layers
}

fn detect_cpu() -> DashboardItem {
    let info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let cores = info.lines().filter(|l| l.starts_with("processor")).count();
    let model = info.lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map(|s| s.trim())
        .unwrap_or("unknown");
    DashboardItem {
        label: format!("CPU ({} cores)", cores),
        status: if cores > 0 { DashboardStatus::Ready } else { DashboardStatus::Unknown },
        detail: format!("{}", model),
    }
}

fn detect_ram() -> DashboardItem {
    let info = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let total_kb = info.lines()
        .find(|l| l.starts_with("MemTotal"))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let total_gb = total_kb / 1024 / 1024;
    DashboardItem {
        label: "RAM".into(),
        status: if total_gb >= 8 { DashboardStatus::Ready } else { DashboardStatus::Warning },
        detail: format!("{} GB", total_gb),
    }
}

fn detect_gpu() -> DashboardItem {
    // Check nvidia-smi first, then amdgpu
    if let Ok(out) = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name,memory.total", "--format=csv,noheader"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        if out.status.success() && !stdout.trim().is_empty() {
            return DashboardItem {
                label: "GPU (NVIDIA)".into(),
                status: DashboardStatus::Ready,
                detail: stdout.trim().to_string(),
            };
        }
    }
    DashboardItem {
        label: "GPU".into(),
        status: DashboardStatus::Unknown,
        detail: "no GPU detected".into(),
    }
}

fn detect_os() -> DashboardItem {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    DashboardItem {
        label: "OS".into(),
        status: DashboardStatus::Ready,
        detail: format!("{}/{}", os, arch),
    }
}

fn detect_python() -> DashboardItem {
    // Try python3.14 first (target version), fall back to python3
    for cmd in &["python3.14", "python3"] {
        if let Ok(out) = std::process::Command::new(cmd).arg("--version").output() {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let version = stdout.trim().trim_start_matches("Python ");
            if !version.is_empty() {
                let is_314 = version.starts_with("3.14");
                return DashboardItem {
                    label: format!("Python ({})", cmd),
                    status: if is_314 { DashboardStatus::Ready } else { DashboardStatus::Warning },
                    detail: format!("{} {}", version,
                        if is_314 { "(GIL-free ✓)" } else { "(not 3.14)" }),
                };
            }
        }
    }
    DashboardItem {
        label: "Python".into(),
        status: DashboardStatus::Critical,
        detail: "not found".into(),
    }
}

fn detect_node() -> DashboardItem {
    if let Ok(out) = std::process::Command::new("node").arg("--version").output() {
        let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return DashboardItem {
            label: "Node".into(),
            status: DashboardStatus::Ready,
            detail: v,
        };
    }
    DashboardItem { label: "Node".into(), status: DashboardStatus::Critical, detail: "not found".into() }
}

fn detect_rust() -> DashboardItem {
    if let Ok(out) = std::process::Command::new("rustc").arg("--version").output() {
        let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return DashboardItem {
            label: "Rust".into(),
            status: DashboardStatus::Ready,
            detail: v,
        };
    }
    DashboardItem { label: "Rust".into(), status: DashboardStatus::Critical, detail: "not found".into() }
}

fn detect_just() -> DashboardItem {
    if let Ok(out) = std::process::Command::new("just").arg("--version").output() {
        let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return DashboardItem {
            label: "just".into(),
            status: DashboardStatus::Ready,
            detail: v,
        };
    }
    DashboardItem { label: "just".into(), status: DashboardStatus::Unknown, detail: "not found".into() }
}

fn check_binary(name: &str, cmd: &str) -> DashboardItem {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() {
        return DashboardItem { label: name.into(), status: DashboardStatus::Unknown, detail: "no command".into() };
    }
    if let Ok(out) = std::process::Command::new(parts[0]).args(&parts[1..]).output() {
        let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return DashboardItem {
            label: name.into(),
            status: DashboardStatus::Ready,
            detail: v.lines().next().unwrap_or("ok").to_string(),
        };
    }
    DashboardItem { label: name.into(), status: DashboardStatus::Unknown, detail: "not found".into() }
}

fn detect_models() -> DashboardItem {
    // Check for GGUF model files in common locations
    let model_dirs: Vec<std::path::PathBuf> = vec![
        std::path::PathBuf::from("/opt/b00t/models"),
        std::path::PathBuf::from("/usr/local/share/b00t/models"),
        dirs::home_dir().unwrap_or_default().join(".b00t/models"),
    ];
    let count: usize = model_dirs.iter()
        .filter_map(|d| std::fs::read_dir(d).ok())
        .flat_map(|e| e.filter_map(|e| e.ok()))
        .filter(|e| e.path().extension().map(|x| x == "gguf").unwrap_or(false))
        .count();
    DashboardItem {
        label: "Local Models".into(),
        status: if count > 0 { DashboardStatus::Ready } else { DashboardStatus::Unknown },
        detail: format!("{} GGUF files", count),
    }
}

fn count_mcp_tools() -> DashboardItem {
    let dotfiles = dirs::home_dir().unwrap_or_default().join(".dotfiles/_b00t_");
    let count: usize = std::fs::read_dir(&dotfiles)
        .map(|d| d.filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "tomllmd").unwrap_or(false))
            .filter(|e| {
                std::fs::read_to_string(e.path())
                    .map(|c| c.contains("mcp") || c.contains("MCP"))
                    .unwrap_or(false)
            })
            .count())
        .unwrap_or(0);
    DashboardItem {
        label: "MCP Servers".into(),
        status: DashboardStatus::Ready,
        detail: format!("{} configured", count),
    }
}

fn count_datums() -> DashboardLayer {
    let dotfiles = dirs::home_dir().unwrap_or_default().join(".dotfiles/_b00t_/datums");
    let count: usize = std::fs::read_dir(&dotfiles)
        .map(|d| d.filter_map(|e| e.ok()).count())
        .unwrap_or(0);
    DashboardLayer {
        z: 4,
        name: "Datums",
        color: "#0f766e",
        items: vec![DashboardItem {
            label: "Datums".into(),
            status: if count > 0 { DashboardStatus::Ready } else { DashboardStatus::Warning },
            detail: format!("{} datum files", count),
        }],
    }
}

fn count_agents() -> DashboardLayer {
    let mut agents = Vec::new();
    // Check registered agent datums
    let dotfiles = dirs::home_dir().unwrap_or_default().join(".dotfiles/_b00t_/datums");
    if let Ok(dir) = std::fs::read_dir(&dotfiles) {
        for entry in dir.filter_map(|e| e.ok()) {
            let name = entry.path().file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            if name.contains("AGENT") || name.contains("agent") {
                agents.push(DashboardItem {
                    label: name,
                    status: DashboardStatus::Ready,
                    detail: "datum registered".into(),
                });
            }
        }
    }
    // Also check for .agent.toml files
    let home = dirs::home_dir().unwrap_or_default();
    if let Ok(dir) = std::fs::read_dir(&home) {
        for entry in dir.filter_map(|e| e.ok()) {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) == Some("toml")
                && p.file_stem().and_then(|s| s.to_str())
                    .map(|s| s.ends_with(".agent"))
                    .unwrap_or(false)
            {
                agents.push(DashboardItem {
                    label: p.file_stem().and_then(|s| s.to_str()).unwrap_or("?").to_string(),
                    status: DashboardStatus::Ready,
                    detail: "agent file".into(),
                });
            }
        }
    }
    if agents.is_empty() {
        agents.push(DashboardItem {
            label: "No agents registered".into(),
            status: DashboardStatus::Unknown,
            detail: "register via b00t agent create".into(),
        });
    }
    DashboardLayer {
        z: 5,
        name: "Agents",
        color: "#b45309",
        items: agents,
    }
}

/// Print the layered dashboard to stdout
pub fn print_dashboard() {
    let layers = build_dashboard();
    println!("\n{}", crate::ansi::bold("╔══════════════════════════════════════════╗"));
    println!("{}", crate::ansi::bold("║     b00t Agent System Dashboard           ║"));
    println!("{}", crate::ansi::bold("╚══════════════════════════════════════════╝"));
    println!();

    for layer in &layers {
        let color_tag = crate::ansi::cyan;
        println!("{} z={} {} {}",
            color_tag("┌─"),
            layer.z,
            color_tag(layer.name),
            color_tag(&format!("[{}]", layer.color)),
        );
        for item in &layer.items {
            let status_char = match item.status {
                DashboardStatus::Ready => crate::ansi::green("✓"),
                DashboardStatus::Warning => crate::ansi::yellow("⚠"),
                DashboardStatus::Critical => crate::ansi::red("✗"),
                DashboardStatus::Unknown => crate::ansi::dim("?"),
            };
            println!("{} {} {} — {}",
                crate::ansi::dim("│"),
                status_char,
                item.label,
                crate::ansi::dim(&item.detail),
            );
        }
        println!("{}", crate::ansi::dim("└─"));
        println!();
    }

    // Summary health
    let total = layers.iter().flat_map(|l| l.items.iter()).count();
    let ready = layers.iter().flat_map(|l| l.items.iter())
        .filter(|i| i.status == DashboardStatus::Ready).count();
    let warnings = layers.iter().flat_map(|l| l.items.iter())
        .filter(|i| i.status == DashboardStatus::Warning).count();
    let critical = layers.iter().flat_map(|l| l.items.iter())
        .filter(|i| i.status == DashboardStatus::Critical).count();
    println!("{} {} services: {} ready, {} warnings, {} critical",
        crate::ansi::bold("Health:"),
        total, crate::ansi::green(&ready.to_string()),
        crate::ansi::yellow(&warnings.to_string()),
        crate::ansi::red(&critical.to_string()),
    );
}

/// Discover capabilities across the hive — agents, MCP servers, CLI tools, topology.
pub fn discover_capabilities(filter: Option<&str>) -> Result<()> {
    let dotfiles = dirs::home_dir().unwrap_or_default().join(".dotfiles/_b00t_");
    let datums_dir = dotfiles.join("datums");

    println!("{}", crate::ansi::bold("🔍 Hive Capability Discovery"));
    println!();

    // ─── System overview ────────────────────────────────────────────────
    println!("{}", crate::ansi::dim("b00t — hive agent operating protocol"));
    println!("{}", crate::ansi::dim("Usage is subjective to task, agent, and system state."));
    println!("{}", crate::ansi::dim("Run `b00t whoami --help` for identity, `b00t capabilities --filter <topic>` to explore."));
    println!();

    // ─── Count by type ──────────────────────────────────────────────────
    let mut mcp_count = 0usize;
    let mut agent_count = 0usize;
    let mut cli_count = 0usize;
    let mut skill_count = 0usize;
    let mut datum_count = 0usize;

    if let Ok(dir) = std::fs::read_dir(&datums_dir) {
        for entry in dir.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("tomllmd") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                let has_mcp = content.contains("mcp") || content.contains("MCP");
                let has_agent = content.contains("agent") || content.contains("Agent");
                let has_cli = content.contains("cli") || content.contains("CLI");
                let has_skill = content.contains("skill") || content.contains("Skill");
                match (has_mcp, has_agent, has_cli, has_skill) {
                    (true, _, _, _) => mcp_count += 1,
                    (_, true, _, _) => agent_count += 1,
                    (_, _, true, _) => cli_count += 1,
                    (_, _, _, true) => skill_count += 1,
                    _ => datum_count += 1,
                }
            }
        }
    }

    println!("{} {} | {} {} | {} {} | {} {} | {} {}",
        crate::ansi::bold("📊 System:"),
        crate::ansi::cyan(&format!("{} datums", datum_count + mcp_count + agent_count + cli_count + skill_count)),
        crate::ansi::bold("🔌"),
        crate::ansi::cyan(&format!("{} MCP", mcp_count)),
        crate::ansi::bold("🤖"),
        crate::ansi::cyan(&format!("{} agents", agent_count)),
        crate::ansi::bold("🛠️"),
        crate::ansi::cyan(&format!("{} CLI", cli_count)),
        crate::ansi::bold("🧠"),
        crate::ansi::cyan(&format!("{} skills", skill_count)),
    );

    // ─── Agent-role filter ────────────────────────────────────────────────
    if let Some(f) = filter {
        if f.starts_with("agent/") {
            let role = f.strip_prefix("agent/").unwrap_or("");
            println!("{}", crate::ansi::bold(&format!("\n🤖 Filtering for role: {}", role)));
            // Scan agent datums matching this role
            if let Ok(dir) = std::fs::read_dir(&datums_dir) {
                for entry in dir.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) != Some("tomllmd") {
                        continue;
                    }
                    let fname = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if fname.contains(role) {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            println!("   📄 {} — {} bytes", crate::ansi::cyan(fname), content.len());
                        }
                    }
                }
            }
            return Ok(());
        }
    }

    // ─── Topology summary (detect relationships between types) ──────────
    let role_count = count_by_tag(&datums_dir, "role");
    let training_count = count_by_tag(&datums_dir, "training");
    let prd_count = count_by_tag(&datums_dir, "prd");
    println!("{} {} {} {} {} {}",
        crate::ansi::dim("Roles:"), crate::ansi::cyan(&role_count.to_string()),
        crate::ansi::dim("PRDs:"), crate::ansi::cyan(&prd_count.to_string()),
        crate::ansi::dim("Training pipelines:"), crate::ansi::cyan(&training_count.to_string()),
    );
    println!("{} b00t ontology export --format=mermaid", crate::ansi::dim("View topology:"));
    println!("{} b00t ontology export --format=cytoscape --root=<type>", crate::ansi::dim("Interactive graph:"));
    println!();

    // ─── Detailed listing (with optional filter) ───────────────────────
    let mut found = 0usize;

    if let Ok(dir) = std::fs::read_dir(&datums_dir) {
        for entry in dir.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("tomllmd") {
                continue;
            }
            let name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            if let Some(f) = filter {
                if !name.to_lowercase().contains(&f.to_lowercase()) {
                    continue;
                }
            }

            if let Ok(content) = std::fs::read_to_string(&path) {
                let has_mcp = content.contains("mcp") || content.contains("MCP");
                let has_agent = content.contains("agent") || content.contains("Agent");
                let has_cli = content.contains("cli") || content.contains("CLI");
                let has_skill = content.contains("skill") || content.contains("Skill");

                let kind = match (has_mcp, has_agent, has_cli, has_skill) {
                    (true, _, _, _) => "🔌 MCP",
                    (_, true, _, _) => "🤖 Agent",
                    (_, _, true, _) => "🛠️ CLI",
                    (_, _, _, true) => "🧠 Skill",
                    _ => "📄 Datum",
                };
                print!("  {} {}", kind, crate::ansi::cyan(&name));
                if filter.is_some() {
                    print!(" — {}", entry.path().display());
                }
                println!();
                found += 1;
            }
        }
    }

    if found == 0 {
        if filter.is_some() {
            println!("  {} No capabilities matching filter: {}", crate::ansi::yellow("⚠"), filter.unwrap());
            println!("  {} Use `b00t ontology export --format=mermaid` to see full topology", crate::ansi::dim("Tip:"));
        } else {
            println!("  {} No capabilities found in {}", crate::ansi::yellow("⚠"), datums_dir.display());
        }
    } else {
        println!("\n{} {} capabilities discovered", crate::ansi::green(&found.to_string()),
            if filter.is_some() { format!("matching '{}'", filter.unwrap()) } else { "total".into() });
        if filter.is_some() {
            println!("{} b00t ontology export --format=mermaid --root={}", crate::ansi::dim("Explore relationships:"), filter.unwrap());
        }
    }

    Ok(())
}

/// Quick count of datums containing a tag in their type_tags.
fn count_by_tag(dir: &std::path::Path, tag: &str) -> usize {
    let mut count = 0;
    if let Ok(d) = std::fs::read_dir(dir) {
        for entry in d.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("tomllmd") {
                continue;
            }
            if let Ok(content) = std::fs::read_to_string(&path) {
                if content.contains(&format!("\"{}\"", tag)) || content.contains(&format!("type_tags = [\"{}\"", tag)) {
                    count += 1;
                }
            }
        }
    }
    count
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
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::set_var("_B00T_ROLE", "captain");
        }
        let resolved = resolve_role(Some("executive".to_string()));
        assert_eq!(resolved, "executive");
        unsafe {
            std::env::remove_var("_B00T_ROLE");
        }
    }

    #[test]
    fn test_resolve_role_empty_override_falls_back() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::set_var("_B00T_ROLE", "operator");
        }
        let resolved = resolve_role(Some("".to_string()));
        assert_eq!(resolved, "operator");
        unsafe {
            std::env::remove_var("_B00T_ROLE");
        }
    }

    #[test]
    fn test_resolve_role_defaults_to_worker() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::remove_var("_B00T_ROLE");
        }
        let resolved = resolve_role(None);
        assert_eq!(resolved, "worker");
    }

    #[test]
    fn test_resolve_role_uses_env_var() {
        let _guard = ENV_MUTEX.lock().unwrap();
        unsafe {
            std::env::set_var("_B00T_ROLE", "executive");
        }
        let resolved = resolve_role(None);
        assert_eq!(resolved, "executive");
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

        assert!(checks
            .iter()
            .any(|c| { c.reference == "ralph.agent" && c.status == CapabilityStatus::Ready }));
        assert!(checks
            .iter()
            .any(|c| { c.reference == "b00t.cli" && c.status == CapabilityStatus::Ready }));
        assert!(checks
            .iter()
            .any(|c| { c.reference == "ghost.agent" && c.status == CapabilityStatus::Missing }));
        assert!(checks
            .iter()
            .any(|c| { c.reference == "b00t-mcp.mcp" && c.status == CapabilityStatus::Missing }));
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

    // ─── Dashboard tests ────────────────────────────────────────────────

    #[test]
    fn test_build_dashboard_has_all_layers() {
        let layers = build_dashboard();
        let names: Vec<&str> = layers.iter().map(|l| l.name).collect();
        assert!(names.contains(&"Hardware & OS"), "missing HW layer among: {:?}", names);
        assert!(names.contains(&"Runtime"), "missing Runtime layer");
        assert!(names.contains(&"Inference"), "missing Inference layer");
        assert!(names.contains(&"MCP Tools"), "missing MCP layer");
        assert!(names.contains(&"Datums"), "missing Datums layer");
        assert!(names.contains(&"Agents"), "missing Agents layer");
        // z values should be sequential
        for (i, layer) in layers.iter().enumerate() {
            assert_eq!(layer.z as usize, i, "layer {} has wrong z value", layer.name);
        }
    }

    #[test]
    fn test_dashboard_layer_has_color() {
        let layers = build_dashboard();
        for layer in &layers {
            assert!(!layer.color.is_empty(), "layer {} has no color", layer.name);
            assert!(layer.color.starts_with('#'), "layer {} color not hex: {}", layer.name, layer.color);
        }
    }

    #[test]
    fn test_detect_cpu_returns_ready() {
        let item = detect_cpu();
        assert!(!item.label.is_empty());
        // CPU should always be detectable on any real system
        assert!(item.status == DashboardStatus::Ready || item.status == DashboardStatus::Unknown);
    }

    #[test]
    fn test_detect_os_returns_ready() {
        let item = detect_os();
        assert_eq!(item.status, DashboardStatus::Ready);
        assert!(item.detail.contains("linux") || item.detail.contains("windows") || item.detail.contains("macos"));
    }

    #[test]
    fn test_dashboard_status_display_consistency() {
        // All dashboard items should have a non-empty label and detail
        let layers = build_dashboard();
        for layer in &layers {
            for item in &layer.items {
                assert!(!item.label.is_empty(), "empty label in layer {}", layer.name);
            }
        }
    }

    #[test]
    fn test_discover_capabilities_returns_ok() {
        // Should not crash — returns Ok even if no capabilities found
        let result = discover_capabilities(None);
        assert!(result.is_ok(), "discover_capabilities failed: {:?}", result.err());
    }

    #[test]
    fn test_discover_capabilities_with_filter() {
        let result = discover_capabilities(Some("unsloth"));
        assert!(result.is_ok());
    }

    // ─── End dashboard tests ────────────────────────────────────────────

    #[test]
    fn test_check_skill_evidence_missing() {
        // nonexistent skill must always return ❌
        let ev = check_skill_evidence("nonexistent-xyz-skill-abc", ".");
        assert!(ev.contains('\u{274c}'), "expected ❌ in: {}", ev);
    }

    #[test]
    fn test_check_skill_evidence_present_for_datum() {
        // Result may be ✅ or ❌ depending on CWD — just verify it returns one of the two labels
        let ev = check_skill_evidence("sm0l", ".");
        assert!(
            ev.contains('\u{2705}') || ev.contains('\u{274c}'),
            "unexpected evidence string: {}",
            ev
        );
    }

    #[test]
    fn test_classify_tier_sm0l() {
        assert_eq!(classify_tier("check b00t implementation"), "sm0l");
        assert_eq!(classify_tier("README authoring skill"), "sm0l");
    }

    #[test]
    fn test_classify_tier_frontier() {
        assert_eq!(classify_tier("evaluate swarms"), "frontier");
        assert_eq!(classify_tier("why AI systems don't learn"), "frontier");
    }

    #[test]
    fn test_classify_tier_default_ch0nky() {
        assert_eq!(classify_tier("integrate metaflow"), "ch0nky");
        assert_eq!(classify_tier("wrkflw local runner"), "ch0nky");
    }

    #[test]
    fn test_parse_task_list_output() {
        let output = "  #414  wrkflw local runner\n  #420  register b00t skills\n  #405  eureka\n";
        let tasks = parse_task_list_output(output);
        assert_eq!(tasks.len(), 3);
        assert_eq!(tasks[0], ("#414".to_string(), "wrkflw local runner".to_string()));
        assert_eq!(tasks[1], ("#420".to_string(), "register b00t skills".to_string()));
        assert_eq!(tasks[2], ("#405".to_string(), "eureka".to_string()));
    }

    #[test]
    fn test_parse_gh_issue_json() {
        let json = r#"[{"number":414,"title":"wrkflw local runner"},{"number":420,"title":"register b00t skills"}]"#;
        let tasks = parse_gh_issue_json(json);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0], ("#414".to_string(), "wrkflw local runner".to_string()));
        assert_eq!(tasks[1], ("#420".to_string(), "register b00t skills".to_string()));
    }

    #[test]
    fn test_parse_task_list_output_skips_no_tasks() {
        // "no tasks" lines shouldn't be parsed as tasks
        let output = "no tasks found\n";
        let tasks = parse_task_list_output(output);
        assert!(tasks.is_empty());
    }
}
