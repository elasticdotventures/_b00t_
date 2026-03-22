//! `b00t skill` — multi-directory skill discovery and activation
//!
//! Progressive disclosure pattern:
//! - `list` / `search` → metadata only (~50 tokens per skill)
//! - `load`            → metadata + applies_to summary
//! - `activate`        → full instruction body → stdout for LLM injection

use anyhow::Result;
use clap::Parser;
use serde_json::json;

use crate::skill_resolver::SkillResolver;

#[derive(Parser)]
pub enum SkillCommands {
    #[clap(about = "List all available skills (metadata only)")]
    List {
        #[clap(long, help = "Filter to skills declared by a role datum")]
        role: Option<String>,
        #[clap(long, help = "Output JSON")]
        json: bool,
    },

    #[clap(about = "Search skills by query (name, description, tags)")]
    Search {
        #[clap(help = "Search query")]
        query: String,
        #[clap(long, help = "Output JSON")]
        json: bool,
    },

    #[clap(about = "Show skill metadata (discovery tier — does not load instructions)")]
    Load {
        #[clap(help = "Skill name")]
        name: String,
        #[clap(long, help = "Output JSON")]
        json: bool,
    },

    #[clap(
        about = "Activate skill — emit full instruction body to stdout for LLM context injection"
    )]
    Activate {
        #[clap(help = "Skill name")]
        name: String,
        #[clap(long, help = "Prefix with role context from named role datum")]
        role: Option<String>,
    },
}

pub fn handle_skill_command(cmd: &SkillCommands, _path: &str) -> Result<()> {
    match cmd {
        SkillCommands::List { role, json } => {
            let resolver = SkillResolver::default();
            let metas = match role {
                Some(role_name) => {
                    let role_skills = load_role_skill_list(role_name)?;
                    resolver.list_for_role(&role_skills)
                }
                None => resolver.list(),
            };

            if *json {
                let out: Vec<_> = metas
                    .iter()
                    .map(|m| {
                        json!({
                            "name": m.name,
                            "description": m.description,
                            "tags": m.tags,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                if metas.is_empty() {
                    println!("No skills found. Add skills to ./skills/ or _b00t_/*.skill.toml");
                } else {
                    for m in &metas {
                        let tags = if m.tags.is_empty() {
                            String::new()
                        } else {
                            format!(" [{}]", m.tags.join(", "))
                        };
                        println!("• {} — {}{}", m.name, m.description, tags);
                    }
                    println!("\n{} skill(s) found", metas.len());
                }
            }
            Ok(())
        }

        SkillCommands::Search { query, json } => {
            let resolver = SkillResolver::default();
            let metas = resolver.search(query);

            if *json {
                let out: Vec<_> = metas
                    .iter()
                    .map(|m| {
                        json!({
                            "name": m.name,
                            "description": m.description,
                            "tags": m.tags,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else if metas.is_empty() {
                println!("No skills match '{}'", query);
            } else {
                for m in &metas {
                    println!("• {} — {}", m.name, m.description);
                }
            }
            Ok(())
        }

        SkillCommands::Load { name, json } => {
            let resolver = SkillResolver::default();
            // list() is cheap — find the meta without loading instructions
            let metas = resolver.list();
            let meta = metas
                .iter()
                .find(|m| m.name == *name)
                .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", name))?;

            if *json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "name": meta.name,
                        "description": meta.description,
                        "tags": meta.tags,
                        "source_dir": meta.source_dir,
                    }))?
                );
            } else {
                println!("🎯 {}", meta.name);
                println!("   {}", meta.description);
                if !meta.tags.is_empty() {
                    println!("   tags: {}", meta.tags.join(", "));
                }
                println!("   source: {}", meta.source_dir.display());
                println!("\n💡 Use `b00t skill activate {}` to load full instructions", name);
            }
            Ok(())
        }

        SkillCommands::Activate { name, role } => {
            let resolver = SkillResolver::default();
            let content = resolver.load(name)?;

            // Optional role context prefix
            if let Some(role_name) = role {
                if let Ok(role_context) = load_role_context_summary(role_name) {
                    println!("## Role Context: {}\n{}\n---\n", role_name, role_context);
                }
            }

            // Emit full instruction body — LLM reads this as skill activation
            println!("## Skill: {}", content.meta.name);
            println!("<!-- description: {} -->\n", content.meta.description);
            println!("{}", content.instructions);
            Ok(())
        }
    }
}

/// Load skill names declared by a role datum from _b00t_/*.role.toml(l)
fn load_role_skill_list(role_name: &str) -> Result<Vec<String>> {
    let b00t_dir = find_b00t_dir()?;
    // Try <role>.role.tomllm, <role>.role.toml
    for ext in &["role.tomllm", "role.toml"] {
        let path = b00t_dir.join(format!("{}.{}", role_name, ext));
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            // Extract skills = [...] array from TOML
            if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                if let Some(skills) = value.get("skills").and_then(|v| v.as_array()) {
                    return Ok(skills
                        .iter()
                        .filter_map(|s| s.as_str().map(String::from))
                        .collect());
                }
                // Also check [b00t].skills
                if let Some(skills) = value
                    .get("b00t")
                    .and_then(|b| b.get("skills"))
                    .and_then(|v| v.as_array())
                {
                    return Ok(skills
                        .iter()
                        .filter_map(|s| s.as_str().map(String::from))
                        .collect());
                }
            }
        }
    }

    // Not found — return empty (graceful degradation)
    Ok(vec![])
}

/// Emit a short role context summary for skill activation preamble
fn load_role_context_summary(role_name: &str) -> Result<String> {
    let b00t_dir = find_b00t_dir()?;

    for ext in &["role.tomllm", "role.toml"] {
        let path = b00t_dir.join(format!("{}.{}", role_name, ext));
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                let hint = value
                    .get("b00t")
                    .and_then(|b| b.get("hint"))
                    .or_else(|| value.get("hint"))
                    .and_then(|h| h.as_str())
                    .unwrap_or(role_name);
                return Ok(format!("Role `{}`: {}", role_name, hint));
            }
        }
    }

    Ok(format!("Role: {}", role_name))
}

/// Find the nearest _b00t_ directory (project-local or global)
fn find_b00t_dir() -> Result<std::path::PathBuf> {
    // Project-local first
    if let Ok(cwd) = std::env::current_dir() {
        let local = cwd.join("_b00t_");
        if local.is_dir() {
            return Ok(local);
        }
    }
    // Global fallback
    if let Some(home) = dirs::home_dir() {
        let global = home.join(".b00t").join("_b00t_");
        if global.is_dir() {
            return Ok(global);
        }
    }
    anyhow::bail!("No _b00t_ directory found (tried project-local and ~/.b00t/_b00t_/)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_skill_md(dir: &std::path::Path, name: &str, desc: &str) {
        let skill_dir = dir.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        let content = format!(
            "---\nname: {}\ndescription: {}\ntags:\n- test\napplies_to:\n- testing\noutput_types:\n- .txt\n---\n# {}\nDo the thing.\n",
            name, desc, name
        );
        let mut f = std::fs::File::create(skill_dir.join("SKILL.md")).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_skill_list_empty() {
        // Empty resolver — should not panic
        let resolver = SkillResolver::with_dirs(vec![]);
        let metas = resolver.list();
        assert!(metas.is_empty());
    }

    #[test]
    fn test_skill_search_no_match() {
        let resolver = SkillResolver::with_dirs(vec![]);
        let results = resolver.search("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_find_b00t_dir_global() {
        // Global b00t dir should exist on this system
        let result = find_b00t_dir();
        assert!(result.is_ok(), "expected global _b00t_ dir: {:?}", result);
    }
}
