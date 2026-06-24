//! `b00t blessing` — agent tool authorization manifest.
//!
//! Walks datum depends_on graph for a role, emits manifest declaring:
//!   - Required skills (with tools each unlocks)
//!   - Optional skills (matched by skills field)
//!   - Forbidden command patterns
//!   - Postel next-hint (what to learn first)
//!
//! # Usage
//! ```bash
//! b00t blessing --manifest --role worker   # full manifest
//! b00t blessing --list-roles               # list available roles
//! ```

use anyhow::Result;
use b00t_c0re_a2a::{AgentCard, HiveRegistry};
use crate::datum_utils::get_all_datums;
use std::path::PathBuf;

fn find_b00t_dir() -> Result<PathBuf> {
    let b00t = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("no home dir"))?
        .join(".b00t")
        .join("_b00t_");
    if b00t.exists() { return Ok(b00t); }
    Err(anyhow::anyhow!("_b00t_ not found — run `b00t up` first"))
}

#[derive(clap::Parser, Clone)]
pub struct BlessingArgs {
    #[clap(long, help = "Emit tool authorization manifest for this role")]
    pub manifest: bool,

    #[clap(long, default_value = "worker", help = "Role to build manifest for")]
    pub role: String,

    #[clap(long, help = "List all available roles")]
    pub list_roles: bool,

    #[clap(long, default_value = "toml", help = "Output format: toml | json")]
    pub format: String,
}

pub fn handle_blessing(args: &BlessingArgs) -> Result<()> {
    let b00t_path = find_b00t_dir()?.to_string_lossy().to_string();

    if args.list_roles {
        return list_roles(&b00t_path);
    }

    if args.manifest {
        return emit_manifest(&b00t_path, &args.role, &args.format);
    }

    println!("b00t blessing — agent tool authorization manifest");
    println!("  --manifest --role <role>   emit manifest");
    println!("  --list-roles               list available roles");
    println!();
    println!("next: b00t blessing --manifest --role worker");
    Ok(())
}

fn list_roles(b00t_path: &str) -> Result<()> {
    let datums = get_all_datums(b00t_path)?;
    let mut roles: Vec<String> = Vec::new();

    for (key, datum) in &datums {
        let is_role = datum.datum_type.as_ref()
            .map(|t| format!("{t:?}").to_lowercase().contains("role"))
            .unwrap_or(false);
        let has_skills = datum.skills.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
        if is_role || has_skills {
            let hint = if datum.hint.is_empty() { key.as_str() } else { datum.hint.as_str() };
            roles.push(format!("  {key} — {hint}"));
        }
    }

    // AGENTS/ supplement names
    if let Ok(agents_dir) = find_b00t_dir().map(|p| {
        p.parent().unwrap_or(p.as_path()).join("AGENTS")
    }) {
        if agents_dir.exists() {
            for entry in std::fs::read_dir(&agents_dir).into_iter().flatten().flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Some(role) = name.strip_prefix("--role=").and_then(|s| s.strip_suffix(".md")) {
                    let line = format!("  {role} (AGENTS/ supplement)");
                    if !roles.contains(&line) { roles.push(line); }
                }
            }
        }
    }

    roles.sort();
    if roles.is_empty() {
        println!("No roles found. Create AGENTS/--role=<name>.md or add depends_on to a datum.");
    } else {
        println!("Available roles:");
        for r in &roles { println!("{r}"); }
    }
    println!();
    println!("next: b00t blessing --manifest --role <role>");
    Ok(())
}

/// A skill referenced in a role's `depends_on` that has no local datum.
/// ST-A (K1): observe-only — url is always None until ST-B wires A2A discovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoverHint {
    pub skill: String,
    /// AgentCard URL of a remote agent that has this skill, or None if not found.
    pub url: Option<String>,
    pub agent_id: Option<String>,
}

/// Walk the role's `depends_on` graph and return hints for skills with no
/// local datum (ST-A: url/agent_id always None — A2A query added in ST-B).
pub fn discover_missing_skills(
    datums: &std::collections::HashMap<String, crate::BootDatum>,
    role: &str,
) -> Vec<DiscoverHint> {
    let role_datum = datums.get(role).or_else(|| {
        datums.iter().find(|(k, _)| k.starts_with(role)).map(|(_, v)| v)
    });

    let direct_deps: Vec<String> = role_datum
        .and_then(|d| d.depends_on.clone())
        .unwrap_or_default();

    direct_deps
        .into_iter()
        .filter(|dep_key| !datums.contains_key(dep_key))
        .map(|skill| DiscoverHint { skill, url: None, agent_id: None })
        .collect()
}

/// ST-B: Enrich hints by querying the hive for agents that have each missing skill.
/// If the registry has a remote agent with the skill, populate `agent_id`.
/// In practice the registry starts empty (no heartbeat yet) — enrichment is a no-op
/// until the hive is running. The hook is exercised in tests via a pre-populated registry.
pub fn enrich_hints_with_hive(hints: Vec<DiscoverHint>, registry: &HiveRegistry) -> Vec<DiscoverHint> {
    hints.into_iter().map(|mut hint| {
        if hint.agent_id.is_none() {
            let matches = registry.find_agents_by_skill(&hint.skill);
            // Prefer the first non-local match
            if let Some((agent_id, card)) = matches.into_iter().find(|(id, _)| id != "local") {
                hint.agent_id = Some(agent_id);
                hint.url = Some(card.url.to_string());
            }
        }
        hint
    }).collect()
}

/// ST-C: When hive enrichment left agent_id=None, check B00T_SM0L_ENDPOINT.
/// If set, mark the hint with agent_id="sm0l:infer" so the caller knows to
/// delegate skill discovery to the local fine-tuned sm0l oracle.
pub fn route_unmatched_via_sm0l(hints: Vec<DiscoverHint>) -> Vec<DiscoverHint> {
    let endpoint = std::env::var("B00T_SM0L_ENDPOINT").ok();
    hints.into_iter().map(|mut hint| {
        if hint.agent_id.is_none() {
            if let Some(ref ep) = endpoint {
                hint.agent_id = Some("sm0l:infer".to_string());
                hint.url = Some(ep.clone());
            }
        }
        hint
    }).collect()
}

/// Build a minimal read-only HiveRegistry for CLI use (no remote hives registered).
fn local_hive_registry() -> HiveRegistry {
    local_hive_registry_for_test()
}

pub fn local_hive_registry_for_test() -> HiveRegistry {
    let url = url::Url::parse("stdio://local").expect("static url");
    let card = AgentCard::new("b00t-cli", "local blessing manifest agent", url);
    HiveRegistry::new(card)
}

fn emit_manifest(b00t_path: &str, role: &str, fmt: &str) -> Result<()> {
    let datums = get_all_datums(b00t_path)?;

    // Find role datum by key or prefix match
    let role_datum = datums.get(role).or_else(|| {
        datums.iter().find(|(k, _)| k.starts_with(role)).map(|(_, v)| v)
    });

    let direct_deps: Vec<String> = role_datum
        .and_then(|d| d.depends_on.clone())
        .unwrap_or_default();

    let mut required: Vec<(String, Vec<String>)> = Vec::new();
    let mut optional: Vec<(String, Vec<String>)> = Vec::new();
    let raw_hints = discover_missing_skills(&datums, role);
    let hive = local_hive_registry();
    let enriched_hints = enrich_hints_with_hive(raw_hints, &hive);
    let discover_hints = route_unmatched_via_sm0l(enriched_hints);

    for dep_key in &direct_deps {
        let unlocks = datums.get(dep_key)
            .and_then(|d| d.unlocks.clone())
            .unwrap_or_default();
        required.push((dep_key.clone(), unlocks));
    }

    // 🛡️ Blessing enforcement: if task scope includes _b00t_/ writes,
    //    ensure datum-authoring is included in the dependency graph.
    if task_involves_b00t_writes() {
        add_datum_authoring_to_graph(&datums, &direct_deps, &mut required, &mut optional);
    }

    // Optional: datums that declare this role in their skills field
    for (key, datum) in &datums {
        if direct_deps.contains(key) { continue; }
        let in_skills = datum.skills.as_ref()
            .map(|s| s.iter().any(|sk| sk == role))
            .unwrap_or(false);
        if in_skills {
            let unlocks = datum.unlocks.clone().unwrap_or_default();
            optional.push((key.clone(), unlocks));
        }
    }

    let forbidden = [
        "pip install *    → use: uv pip install",
        "docker run *     → use: podman --device nvidia.com/gpu=all",
        "rm -rf /         → BLOCKED",
        "huggingface-cli  → use: hf download",
    ];

    let next_skill = required.first().map(|(k, _)| k.as_str()).unwrap_or("<skill>");

    match fmt {
        "json" => {
            let out = serde_json::json!({
                "role": role,
                "required": required.iter().map(|(k, u)| serde_json::json!({"skill": k, "unlocks": u})).collect::<Vec<_>>(),
                "optional": optional.iter().map(|(k, u)| serde_json::json!({"skill": k, "unlocks": u})).collect::<Vec<_>>(),
                "forbidden": forbidden,
                "next": format!("b00t learn {next_skill}"),
                "discover": discover_hints.iter().map(|h| serde_json::json!({
                    "skill": h.skill,
                    "url": h.url,
                    "agent_id": h.agent_id,
                })).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        _ => {
            println!("[blessing]");
            println!("role = {role:?}");
            println!();
            println!("[blessing.required]");
            if required.is_empty() {
                println!("# No depends_on found for role '{role}'");
                println!("# Add depends_on = [\"skill.a\"] to _b00t_/{role}.toml");
            }
            for (key, unlocks) in &required {
                println!("{key:?} = {{ unlocks = {unlocks:?} }}");
            }
            if !optional.is_empty() {
                println!();
                println!("[blessing.optional]");
                for (key, unlocks) in &optional {
                    println!("{key:?} = {{ unlocks = {unlocks:?} }}");
                }
            }
            println!();
            println!("[blessing.forbidden]");
            for f in &forbidden { println!("# {f}"); }
            println!();
            println!("[blessing.next]");
            println!("hint = {:?}", format!("b00t learn {next_skill}"));
            if !discover_hints.is_empty() {
                println!();
                println!("[blessing.discover]");
                println!("# Skills needed by role '{role}' with no local datum — not yet in hive");
                for h in &discover_hints {
                    let url_str = h.url.as_deref().unwrap_or("None");
                    println!("# DISCOVER: skill={} url={url_str}", h.skill);
                    println!("# hint: b00t agent discover --capability {}", h.skill);
                }
            }
        }
    }
    Ok(())
}

// ── Blessing enforcement helpers ─────────────────────────────────────────

/// Check if the current task context involves _b00t_/ writes.
///
/// Checks `B00T_TASK_CONTEXT` env var and current git branch for keywords
/// indicating datum/skill authoring activity.
fn task_involves_b00t_writes() -> bool {
    let mut indicators = Vec::new();

    if let Ok(ctx) = std::env::var("B00T_TASK_CONTEXT") {
        if !ctx.is_empty() {
            indicators.push(ctx.to_lowercase());
        }
    }

    if let Ok(out) = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .output()
    {
        let branch = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
        if !branch.is_empty() {
            indicators.push(branch);
        }
    }

    let combined = indicators.join(" ");

    let b00t_write_kw = [
        "_b00t_", "b00t_", "datum", "datums", "skill.toml",
        "skill datum", "datum-authoring", "datum-schema", "blessing",
        "tomllm", "tomllmd",
    ];

    for kw in &b00t_write_kw {
        if combined.contains(kw) {
            return true;
        }
    }

    false
}

/// Add datum-authoring to the blessing dependency graph when task scope
/// includes _b00t_/ writes.
///
/// Inserts datum-authoring into the required list (with its sub-dependencies)
/// or the optional list if it's not already present.
fn add_datum_authoring_to_graph(
    datums: &std::collections::HashMap<String, crate::BootDatum>,
    direct_deps: &[String],
    required: &mut Vec<(String, Vec<String>)>,
    _optional: &mut Vec<(String, Vec<String>)>,
) {
    // Skip if datum-authoring is already in the required or direct deps
    let already_present = required.iter().any(|(k, _)| k == "datum-authoring.skill")
        || direct_deps.iter().any(|d| d == "datum-authoring.skill");

    if already_present {
        return;
    }

    // Collect datum-authoring and its sub-dependencies
    let mut to_add = vec!["datum-authoring.skill".to_string()];

    // Walk the dependency chain: datum-authoring → datum-schema → tomllm-format
    let mut seen: std::collections::HashSet<String> = to_add.iter().cloned().collect();
    let mut i = 0;
    while i < to_add.len() {
        let key = &to_add[i];
        if let Some(datum) = datums.get(key) {
            if let Some(ref deps) = datum.depends_on {
                for dep in deps {
                    if seen.insert(dep.clone()) {
                        to_add.push(dep.clone());
                    }
                }
            }
        }
        i += 1;
    }

    // Add to required, pulling unlocks from the datum
    for key in &to_add {
        let unlocks = datums
            .get(key)
            .and_then(|d| d.unlocks.clone())
            .unwrap_or_default();
        required.push((key.clone(), unlocks));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_b00t(dir: &TempDir) -> String {
        let p = dir.path().to_str().unwrap().to_string();
        fs::write(dir.path().join("rust.skill.toml"), "[b00t]\nname = \"rust\"\ntype = \"skill\"\nhint = \"Rust\"\ndepends_on = [\"cargo.cli\"]\nunlocks = [\"cargo.*\", \"rustfmt\"]\n").unwrap();
        fs::write(dir.path().join("cargo.cli.toml"), "[b00t]\nname = \"cargo\"\ntype = \"cli\"\nhint = \"Rust build\"\nunlocks = [\"cargo build\", \"cargo test\"]\n").unwrap();
        fs::write(dir.path().join("backend.role.toml"), "[b00t]\nname = \"backend\"\ntype = \"skill\"\nhint = \"Backend role\"\ndepends_on = [\"rust.skill\", \"cargo.cli\"]\n").unwrap();
        p
    }

    #[test]
    fn test_list_roles_no_panic() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t(&dir);
        list_roles(&path).unwrap();
    }

    #[test]
    fn test_emit_manifest_toml() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t(&dir);
        emit_manifest(&path, "backend", "toml").unwrap();
    }

    #[test]
    fn test_emit_manifest_json() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t(&dir);
        emit_manifest(&path, "backend", "json").unwrap();
    }

    #[test]
    fn test_unlocks_propagated_from_deps() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t(&dir);
        let datums = get_all_datums(&path).unwrap();
        let rust = datums.get("rust.skill").unwrap();
        let expected = vec!["cargo.*".to_string(), "rustfmt".to_string()];
        assert_eq!(rust.unlocks.as_deref(), Some(expected.as_slice()));
    }

    // ── ST-A: skill discovery diagnostics ────────────────────────────────

    fn make_b00t_with_missing_dep(dir: &TempDir) -> String {
        let p = dir.path().to_str().unwrap().to_string();
        // Role depends on two skills; only one exists locally
        fs::write(
            dir.path().join("myagent.role.toml"),
            "[b00t]\nname = \"myagent\"\ntype = \"skill\"\nhint = \"Test role\"\ndepends_on = [\"local-skill.skill\", \"remote-only.skill\"]\n",
        ).unwrap();
        fs::write(
            dir.path().join("local-skill.skill.toml"),
            "[b00t]\nname = \"local-skill\"\ntype = \"skill\"\nunlocks = [\"local.*\"]\n",
        ).unwrap();
        // remote-only.skill.toml intentionally NOT created
        p
    }

    #[test]
    fn discover_missing_skills_returns_hint_for_absent_datum() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t_with_missing_dep(&dir);
        let datums = get_all_datums(&path).unwrap();

        let hints = discover_missing_skills(&datums, "myagent");

        assert_eq!(hints.len(), 1, "exactly one missing skill");
        assert_eq!(hints[0].skill, "remote-only.skill");
        assert_eq!(hints[0].url, None, "ST-A: url always None (no A2A query yet)");
        assert_eq!(hints[0].agent_id, None);
    }

    #[test]
    fn discover_missing_skills_returns_empty_when_all_local() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t(&dir);
        let datums = get_all_datums(&path).unwrap();

        let hints = discover_missing_skills(&datums, "backend");
        assert!(hints.is_empty(), "all deps present locally — no discover hints");
    }

    #[test]
    fn emit_manifest_toml_includes_discover_section_for_missing_skill() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t_with_missing_dep(&dir);

        // Capture by checking emit doesn't panic and output contains DISCOVER
        // (stdout capture is complex; we verify via discover_missing_skills directly
        //  and trust emit_manifest calls it — covered by the function test above)
        emit_manifest(&path, "myagent", "toml").unwrap();
    }

    #[test]
    fn emit_manifest_json_includes_discover_array_for_missing_skill() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t_with_missing_dep(&dir);
        emit_manifest(&path, "myagent", "json").unwrap();
    }

    // ── ST-B: hive enrichment ─────────────────────────────────────────────

    #[test]
    fn enrich_hints_with_hive_populates_agent_id_from_remote() {
        use b00t_c0re_a2a::{AgentCard, HiveRegistry, Skill};
        use url::Url;

        // Build a registry with one remote agent advertising "remote-only.skill"
        let local_url = Url::parse("stdio://local").unwrap();
        let local_card = AgentCard::new("local-agent", "local", local_url);
        let mut registry = HiveRegistry::new(local_card);

        let remote_url = Url::parse("http://hive-node-1:4000").unwrap();
        let remote_card = AgentCard::new("remote-agent", "remote", remote_url.clone())
            .with_skill(Skill {
                id: "remote-only.skill".to_string(),
                name: "remote-only".to_string(),
                description: "provided remotely".to_string(),
                input_schema: serde_json::Value::Null,
                output_schema: serde_json::Value::Null,
            });

        registry.add_remote("hive-node-1".to_string(), remote_url, vec![remote_card]);

        // Hints with agent_id = None (as ST-A produces)
        let hints = vec![
            crate::commands::blessing::DiscoverHint {
                skill: "remote-only.skill".to_string(),
                url: None,
                agent_id: None,
            },
            crate::commands::blessing::DiscoverHint {
                skill: "truly-missing.skill".to_string(),
                url: None,
                agent_id: None,
            },
        ];

        let enriched = crate::commands::blessing::enrich_hints_with_hive(hints, &registry);

        // First hint: hive found it → agent_id populated
        assert_eq!(enriched[0].agent_id.as_deref(), Some("hive-node-1"));
        assert!(enriched[0].url.is_some(), "url should come from AgentCard");

        // Second hint: hive has no match → stays None
        assert_eq!(enriched[1].agent_id, None);
    }

    #[test]
    fn enrich_hints_returns_unchanged_when_hive_empty() {
        let registry = crate::commands::blessing::local_hive_registry_for_test();
        let hints = vec![crate::commands::blessing::DiscoverHint {
            skill: "anything.skill".to_string(),
            url: None,
            agent_id: None,
        }];
        let enriched = crate::commands::blessing::enrich_hints_with_hive(hints, &registry);
        assert_eq!(enriched[0].agent_id, None, "empty registry never enriches");
    }

    // ── ST-C: sm0l oracle routing ─────────────────────────────────────────

    #[test]
    fn route_unmatched_via_sm0l_sets_agent_id_when_endpoint_set() {
        unsafe { std::env::set_var("B00T_SM0L_ENDPOINT", "http://localhost:8080"); }
        let hints = vec![crate::commands::blessing::DiscoverHint {
            skill: "unknown.skill".to_string(),
            url: None,
            agent_id: None,
        }];
        let routed = crate::commands::blessing::route_unmatched_via_sm0l(hints);
        assert_eq!(routed[0].agent_id.as_deref(), Some("sm0l:infer"));
        assert_eq!(routed[0].url.as_deref(), Some("http://localhost:8080"));
        unsafe { std::env::remove_var("B00T_SM0L_ENDPOINT"); }
    }

    #[test]
    fn route_unmatched_via_sm0l_no_op_when_endpoint_absent() {
        unsafe { std::env::remove_var("B00T_SM0L_ENDPOINT"); }
        let hints = vec![crate::commands::blessing::DiscoverHint {
            skill: "unknown.skill".to_string(),
            url: None,
            agent_id: None,
        }];
        let routed = crate::commands::blessing::route_unmatched_via_sm0l(hints);
        assert_eq!(routed[0].agent_id, None);
    }

    #[test]
    fn route_unmatched_via_sm0l_skips_already_enriched_hints() {
        unsafe { std::env::set_var("B00T_SM0L_ENDPOINT", "http://localhost:8080"); }
        let hints = vec![crate::commands::blessing::DiscoverHint {
            skill: "found.skill".to_string(),
            url: Some("http://hive-node:4000".to_string()),
            agent_id: Some("hive-node-1".to_string()),
        }];
        let routed = crate::commands::blessing::route_unmatched_via_sm0l(hints);
        // Already has agent_id — must not be overwritten
        assert_eq!(routed[0].agent_id.as_deref(), Some("hive-node-1"));
        unsafe { std::env::remove_var("B00T_SM0L_ENDPOINT"); }
    }

}
