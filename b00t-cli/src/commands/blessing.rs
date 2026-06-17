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

    for dep_key in &direct_deps {
        let unlocks = datums.get(dep_key)
            .and_then(|d| d.unlocks.clone())
            .unwrap_or_default();
        required.push((dep_key.clone(), unlocks));
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
        }
    }
    Ok(())
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
}
