//! Multi-directory skill resolver — progressive disclosure across all skill sources.
//!
//! Searches in priority order:
//! 1. `./skills/`              — project-local, SKILL.md format
//! 2. `./_b00t_/*.skill.toml`  — project b00t native
//! 3. `~/.claude/skills/`      — Claude Code native, SKILL.md format
//! 4. `~/.b00t/_b00t_/*.skill.toml` — global b00t
//!
//! # Progressive Disclosure
//! `search()` / `list()` → SkillMeta only (~50 tokens each).
//! `load()` → full SkillContent (instructions_inline or file read).
//!
//! # Usage
//! ```rust,ignore
//! let resolver = SkillResolver::default();
//! let matches = resolver.search("rust cli");        // discovery tier
//! let content = resolver.load("fast-rust")?;        // activation tier
//! ```

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::datum_skill::SkillDatum;

/// Source format of a discovered skill
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillFormat {
    /// agentskills.io SKILL.md with YAML frontmatter
    SkillMd,
    /// b00t native `.skill.toml`
    TomlDatum,
}

/// Lightweight skill descriptor — discovery tier (~50 tokens)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    /// Source directory containing this skill
    pub source_dir: PathBuf,
    /// Format the skill was loaded from
    #[serde(skip)]
    pub format: SkillFormat,
}

impl Default for SkillFormat {
    fn default() -> Self {
        SkillFormat::TomlDatum
    }
}

/// Full skill content — activation tier
#[derive(Debug, Clone)]
pub struct SkillContent {
    pub meta: SkillMeta,
    /// Full instruction body (from inline or file)
    pub instructions: String,
}

/// Configured skill search directory
#[derive(Debug, Clone)]
struct SkillDir {
    path: PathBuf,
    format: SkillFormat,
}

/// Resolves skills across multiple directories with priority ordering.
pub struct SkillResolver {
    dirs: Vec<SkillDir>,
}

impl Default for SkillResolver {
    /// Build resolver with standard search path (project-local → global).
    fn default() -> Self {
        let mut dirs = Vec::new();

        // 1. Project-local SKILL.md skills/
        if let Ok(cwd) = std::env::current_dir() {
            let local_skills = cwd.join("skills");
            if local_skills.is_dir() {
                dirs.push(SkillDir { path: local_skills, format: SkillFormat::SkillMd });
            }
            // 2. Project-local b00t datums
            let local_b00t = cwd.join("_b00t_");
            if local_b00t.is_dir() {
                dirs.push(SkillDir { path: local_b00t, format: SkillFormat::TomlDatum });
            }
        }

        // 3. Claude Code native skills
        if let Some(home) = dirs::home_dir() {
            let claude_skills = home.join(".claude").join("skills");
            if claude_skills.is_dir() {
                dirs.push(SkillDir { path: claude_skills, format: SkillFormat::SkillMd });
            }
            // 4. Global b00t datums
            let global_b00t = home.join(".b00t").join("_b00t_");
            if global_b00t.is_dir() {
                dirs.push(SkillDir { path: global_b00t, format: SkillFormat::TomlDatum });
            }
        }

        SkillResolver { dirs }
    }
}

impl SkillResolver {
    /// Create resolver with explicit directory list (for testing).
    pub fn with_dirs(dirs: Vec<(PathBuf, SkillFormat)>) -> Self {
        SkillResolver {
            dirs: dirs
                .into_iter()
                .map(|(path, format)| SkillDir { path, format })
                .collect(),
        }
    }

    /// Discover all skills — returns metadata only (discovery tier).
    pub fn list(&self) -> Vec<SkillMeta> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for dir in &self.dirs {
            if !dir.path.is_dir() {
                continue;
            }
            let metas = match dir.format {
                SkillFormat::SkillMd => scan_skill_md_dir(&dir.path),
                SkillFormat::TomlDatum => scan_toml_skill_dir(&dir.path),
            };
            for meta in metas {
                // first occurrence wins (priority order)
                if seen.insert(meta.name.clone()) {
                    out.push(meta);
                }
            }
        }

        out
    }

    /// Search skills by query — simple substring match on name + description + tags.
    /// Returns metadata only (discovery tier).
    pub fn search(&self, query: &str) -> Vec<SkillMeta> {
        let q = query.to_lowercase();
        self.list()
            .into_iter()
            .filter(|m| {
                m.name.to_lowercase().contains(&q)
                    || m.description.to_lowercase().contains(&q)
                    || m.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// Filter skills by role's declared skill list.
    pub fn list_for_role(&self, role_skills: &[String]) -> Vec<SkillMeta> {
        let all = self.list();
        all.into_iter()
            .filter(|m| role_skills.iter().any(|rs| rs == &m.name))
            .collect()
    }

    /// Load full skill content by name — activation tier.
    pub fn load(&self, name: &str) -> Result<SkillContent> {
        for dir in &self.dirs {
            if !dir.path.is_dir() {
                continue;
            }
            match dir.format {
                SkillFormat::SkillMd => {
                    // Look for <dir>/<name>/SKILL.md
                    let skill_md = dir.path.join(name).join("SKILL.md");
                    if skill_md.exists() {
                        let datum = SkillDatum::from_skill_md(&skill_md)?;
                        let cfg = datum.skill_config()?;
                        let instructions = datum.load_instructions(&dir.path.join(name))?;
                        return Ok(SkillContent {
                            meta: SkillMeta {
                                name: datum.datum.name.clone(),
                                description: cfg.description.clone(),
                                tags: cfg.tags.clone(),
                                source_dir: dir.path.join(name),
                                format: SkillFormat::SkillMd,
                            },
                            instructions,
                        });
                    }
                }
                SkillFormat::TomlDatum => {
                    // Look for <dir>/<name>.skill.toml
                    let toml_path = dir.path.join(format!("{}.skill.toml", name));
                    if toml_path.exists() {
                        let path_str = dir.path.to_string_lossy();
                        let datum = SkillDatum::from_config(name, &path_str)?;
                        let cfg = datum.skill_config()?;
                        let instructions = datum.load_instructions(&dir.path)?;
                        return Ok(SkillContent {
                            meta: SkillMeta {
                                name: datum.datum.name.clone(),
                                description: cfg.description.clone(),
                                tags: cfg.tags.clone(),
                                source_dir: dir.path.clone(),
                                format: SkillFormat::TomlDatum,
                            },
                            instructions,
                        });
                    }
                }
            }
        }
        anyhow::bail!("Skill '{}' not found in any configured skill directory", name)
    }
}

// --- private scanners ---

fn scan_skill_md_dir(dir: &PathBuf) -> Vec<SkillMeta> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let skill_md = entry.path().join("SKILL.md");
        if skill_md.exists() {
            if let Ok(datum) = SkillDatum::from_skill_md(&skill_md) {
                if let Ok(cfg) = datum.skill_config() {
                    out.push(SkillMeta {
                        name: datum.datum.name.clone(),
                        description: cfg.description.clone(),
                        tags: cfg.tags.clone(),
                        source_dir: entry.path(),
                        format: SkillFormat::SkillMd,
                    });
                }
            }
        }
    }
    out
}

fn scan_toml_skill_dir(dir: &PathBuf) -> Vec<SkillMeta> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(fname) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        if !fname.ends_with(".skill.toml") {
            continue;
        }
        let name = fname.trim_end_matches(".skill.toml");
        let path_str = dir.to_string_lossy();
        if let Ok(datum) = SkillDatum::from_config(name, &path_str) {
            if let Ok(cfg) = datum.skill_config() {
                out.push(SkillMeta {
                    name: datum.datum.name.clone(),
                    description: cfg.description.clone(),
                    tags: cfg.tags.clone(),
                    source_dir: dir.clone(),
                    format: SkillFormat::TomlDatum,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_skill_md_dir(dir: &std::path::Path, name: &str, desc: &str, tags: &[&str]) {
        let skill_dir = dir.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        let tags_yaml = if tags.is_empty() {
            "tags: []".to_string()
        } else {
            let items = tags
                .iter()
                .map(|t| format!("- {}", t))
                .collect::<Vec<_>>()
                .join("\n");
            format!("tags:\n{}", items)
        };
        let content = format!(
            "---\nname: {}\ndescription: {}\n{}\napplies_to:\n- general\noutput_types:\n- .txt\n---\n# {}\nInstructions for {}.\n",
            name, desc, tags_yaml, name, name
        );
        let mut f = std::fs::File::create(skill_dir.join("SKILL.md")).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_resolver_list_skill_md_dir() {
        let tmp = tempfile::tempdir().unwrap();
        make_skill_md_dir(tmp.path(), "fast-rust", "Fast Rust code", &["rust"]);
        make_skill_md_dir(tmp.path(), "friendly-python", "Friendly Python", &["python"]);

        let resolver =
            SkillResolver::with_dirs(vec![(tmp.path().to_path_buf(), SkillFormat::SkillMd)]);
        let metas = resolver.list();
        assert_eq!(metas.len(), 2);
        let names: Vec<_> = metas.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"fast-rust"));
        assert!(names.contains(&"friendly-python"));
    }

    #[test]
    fn test_resolver_search_by_query() {
        let tmp = tempfile::tempdir().unwrap();
        make_skill_md_dir(tmp.path(), "fast-rust", "Fast Rust code. Use for Rust.", &["rust"]);
        make_skill_md_dir(tmp.path(), "friendly-python", "Friendly Python code.", &["python"]);

        let resolver =
            SkillResolver::with_dirs(vec![(tmp.path().to_path_buf(), SkillFormat::SkillMd)]);

        let results = resolver.search("rust");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "fast-rust");

        let empty = resolver.search("golang");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_resolver_load_skill_md() {
        let tmp = tempfile::tempdir().unwrap();
        make_skill_md_dir(tmp.path(), "my-skill", "My skill desc", &["demo"]);

        let resolver =
            SkillResolver::with_dirs(vec![(tmp.path().to_path_buf(), SkillFormat::SkillMd)]);

        let content = resolver.load("my-skill").unwrap();
        assert_eq!(content.meta.name, "my-skill");
        assert!(content.instructions.contains("Instructions for my-skill."));
    }

    #[test]
    fn test_resolver_load_missing_errors() {
        let resolver = SkillResolver::with_dirs(vec![]);
        assert!(resolver.load("nonexistent").is_err());
    }

    #[test]
    fn test_resolver_priority_first_wins() {
        let tmp1 = tempfile::tempdir().unwrap();
        let tmp2 = tempfile::tempdir().unwrap();
        make_skill_md_dir(tmp1.path(), "shared-skill", "From dir1", &[]);
        make_skill_md_dir(tmp2.path(), "shared-skill", "From dir2", &[]);

        let resolver = SkillResolver::with_dirs(vec![
            (tmp1.path().to_path_buf(), SkillFormat::SkillMd),
            (tmp2.path().to_path_buf(), SkillFormat::SkillMd),
        ]);

        let metas = resolver.list();
        assert_eq!(metas.len(), 1); // deduped
        assert_eq!(metas[0].description, "From dir1"); // first wins
    }

    #[test]
    fn test_resolver_list_for_role() {
        let tmp = tempfile::tempdir().unwrap();
        make_skill_md_dir(tmp.path(), "rust-skill", "Rust skill", &["rust"]);
        make_skill_md_dir(tmp.path(), "python-skill", "Python skill", &["python"]);

        let resolver =
            SkillResolver::with_dirs(vec![(tmp.path().to_path_buf(), SkillFormat::SkillMd)]);

        let role_skills = vec!["rust-skill".to_string()];
        let filtered = resolver.list_for_role(&role_skills);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "rust-skill");
    }
}
