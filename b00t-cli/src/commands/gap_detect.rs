//! `b00t gap detect` — E6: auto-research gap detector → self-extending datum registry.
//!
//! Scans role depends_on graphs for skills absent from local datums. For each gap,
//! optionally generates a minimal `.tomllmd` stub (--generate) and auto-commits (--commit).
//!
//! # Usage
//! ```bash
//! b00t gap detect                  # report gaps for default role (worker)
//! b00t gap detect --all-roles      # scan every role datum
//! b00t gap detect --generate       # write auto-stub datums for each gap
//! b00t gap detect --generate --commit  # write + git commit stubs
//! ```

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::PathBuf;

use crate::commands::blessing::discover_missing_skills;
use crate::datum_utils::get_all_datums;

#[derive(clap::Parser, Clone)]
pub struct GapDetectArgs {
    #[clap(long, help = "Scan all roles, not just --role")]
    pub all_roles: bool,

    #[clap(long, alias = "agent", default_value = "worker", help = "Role to scan when not --all-roles")]
    pub role: String,

    #[clap(long, help = "Write auto-stub datums for each detected gap")]
    pub generate: bool,

    #[clap(long, help = "git commit generated stubs (requires --generate)")]
    pub commit: bool,

    #[clap(long, default_value = "toml", help = "Output format: toml | json")]
    pub format: String,
}

/// Scan one or more roles and return the deduplicated set of skill names that
/// have a `depends_on` entry with no matching local datum.
pub fn detect_knowledge_gaps(b00t_path: &str, roles: &[&str]) -> Vec<String> {
    let datums = get_all_datums(b00t_path).unwrap_or_default();
    let mut gaps: HashSet<String> = HashSet::new();

    for role in roles {
        for hint in discover_missing_skills(&datums, role) {
            gaps.insert(hint.skill);
        }
    }

    let mut result: Vec<String> = gaps.into_iter().collect();
    result.sort();
    result
}

/// Produce a minimal `.tomllmd` stub for a topic that has no local datum.
/// The stub is valid TOML+comments and ready for operator enrichment via
/// `b00t learn <topic>`.
pub fn generate_stub_datum(topic: &str) -> String {
    let today = chrono::Local::now().format("%Y-%m-%d");
    format!(
        r#"# Auto-generated datum stub for: {topic}
# Created by: b00t gap detect --generate on {today}
# Enrich with: b00t learn {topic}
#
# summary: stub — topic referenced in depends_on but no datum exists yet
# tags: auto-stub, gap-detect

[b00t]
name = "{topic}"
type = "skill"
hint = "Stub — run: b00t learn {topic} to enrich"
auto_generated = true
auto_generated_date = "{today}"

# b00t:map v1
# summary: auto-stub for {topic}
# tags: auto-stub, gap-detect, skill
# tier: sm0l
# cmds: b00t learn {topic}
# complexity: 1
"#
    )
}

/// Write an auto-stub datum file to `<b00t_path>/datums/AUTO-<safe_name>.tomllmd`.
/// Returns the path written.
pub fn write_stub_datum(b00t_path: &str, topic: &str) -> Result<PathBuf> {
    let safe = topic.replace(['/', '\\', ' ', ':'], "-");
    let datums_dir = PathBuf::from(b00t_path).join("datums");
    std::fs::create_dir_all(&datums_dir).context("create datums dir")?;
    let path = datums_dir.join(format!("AUTO-{safe}.tomllmd"));
    std::fs::write(&path, generate_stub_datum(topic)).context("write stub datum")?;
    Ok(path)
}

/// git commit all files matching `_b00t_/datums/AUTO-*.tomllmd`.
fn commit_stubs(written: &[PathBuf]) -> Result<()> {
    if written.is_empty() {
        return Ok(());
    }
    let paths: Vec<&str> = written.iter().filter_map(|p| p.to_str()).collect();
    let mut add = std::process::Command::new("git");
    add.args(["add", "--"]).args(&paths);
    let status = add.status().context("git add stubs")?;
    if !status.success() {
        anyhow::bail!("git add failed");
    }
    let n = written.len();
    let msg = format!("chore(auto-datum): generate {n} stub datum(s) via gap detect");
    let status = std::process::Command::new("git")
        .args(["commit", "-m", &msg])
        .status()
        .context("git commit stubs")?;
    if !status.success() {
        anyhow::bail!("git commit failed");
    }
    Ok(())
}

fn find_b00t_dir() -> Result<String> {
    let b00t = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("no home dir"))?
        .join(".b00t")
        .join("_b00t_");
    if b00t.exists() {
        return Ok(b00t.to_string_lossy().to_string());
    }
    Err(anyhow::anyhow!("_b00t_ not found — run `b00t up` first"))
}

pub fn handle_gap_detect(args: &GapDetectArgs) -> Result<()> {
    let b00t_path = find_b00t_dir()?;
    let datums = get_all_datums(&b00t_path).unwrap_or_default();

    let roles: Vec<&str> = if args.all_roles {
        datums
            .iter()
            .filter(|(_, d)| {
                d.datum_type
                    .as_ref()
                    .map(|t| format!("{t:?}").to_lowercase().contains("role"))
                    .unwrap_or(false)
                    || d.skills.as_ref().map(|s| !s.is_empty()).unwrap_or(false)
            })
            .map(|(k, _)| k.as_str())
            .collect()
    } else {
        vec![args.role.as_str()]
    };

    let gaps = detect_knowledge_gaps(&b00t_path, &roles);

    match args.format.as_str() {
        "json" => {
            let out = serde_json::json!({
                "roles_scanned": roles,
                "gap_count": gaps.len(),
                "gaps": gaps,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        _ => {
            println!("[gap_detect]");
            println!("roles_scanned = {:?}", roles);
            println!("gap_count = {}", gaps.len());
            println!();
            println!("[gap_detect.gaps]");
            if gaps.is_empty() {
                println!("# No gaps found — all depends_on targets have local datums");
            }
            for g in &gaps {
                println!("# {g}");
            }
        }
    }

    if args.generate {
        let mut written: Vec<PathBuf> = Vec::new();
        for topic in &gaps {
            match write_stub_datum(&b00t_path, topic) {
                Ok(path) => {
                    eprintln!("  wrote: {}", path.display());
                    written.push(path);
                }
                Err(e) => eprintln!("  warn: failed to write stub for {topic}: {e}"),
            }
        }
        eprintln!("generated {} stub datums", written.len());

        if args.commit && !written.is_empty() {
            commit_stubs(&written)?;
            eprintln!("committed {} stubs", written.len());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_b00t_with_gaps(dir: &TempDir) -> String {
        let p = dir.path().to_str().unwrap().to_string();
        // Role depends on two skills; only one present locally
        fs::write(
            dir.path().join("analyst.role.toml"),
            "[b00t]\nname = \"analyst\"\ntype = \"skill\"\nhint = \"Role\"\ndepends_on = [\"present.skill\", \"absent-a.skill\", \"absent-b.skill\"]\n",
        ).unwrap();
        fs::write(
            dir.path().join("present.skill.toml"),
            "[b00t]\nname = \"present\"\ntype = \"skill\"\n",
        ).unwrap();
        p
    }

    #[test]
    fn detect_knowledge_gaps_finds_absent_deps() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t_with_gaps(&dir);
        let gaps = detect_knowledge_gaps(&path, &["analyst"]);
        assert_eq!(gaps.len(), 2, "two skills absent");
        assert!(gaps.contains(&"absent-a.skill".to_string()));
        assert!(gaps.contains(&"absent-b.skill".to_string()));
        assert!(!gaps.contains(&"present.skill".to_string()));
    }

    #[test]
    fn detect_knowledge_gaps_returns_empty_when_no_gaps() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t_with_gaps(&dir);
        // role with a dep that exists
        fs::write(
            dir.path().join("simple.role.toml"),
            "[b00t]\nname = \"simple\"\ntype = \"skill\"\ndepends_on = [\"present.skill\"]\n",
        ).unwrap();
        let gaps = detect_knowledge_gaps(&path, &["simple"]);
        assert!(gaps.is_empty(), "all deps satisfied");
    }

    #[test]
    fn detect_knowledge_gaps_deduplicates_across_roles() {
        let dir = TempDir::new().unwrap();
        let path = make_b00t_with_gaps(&dir);
        fs::write(
            dir.path().join("other.role.toml"),
            "[b00t]\nname = \"other\"\ntype = \"skill\"\ndepends_on = [\"absent-a.skill\"]\n",
        ).unwrap();
        // Both roles share absent-a.skill — should appear only once
        let gaps = detect_knowledge_gaps(&path, &["analyst", "other"]);
        let count_a = gaps.iter().filter(|g| g.as_str() == "absent-a.skill").count();
        assert_eq!(count_a, 1, "deduplication: absent-a.skill appears once");
    }

    #[test]
    fn generate_stub_datum_contains_topic_name() {
        let stub = generate_stub_datum("rust-async.skill");
        assert!(stub.contains("rust-async.skill"), "topic in stub");
        assert!(stub.contains("[b00t]"), "valid TOML section");
        assert!(stub.contains("auto_generated = true"), "marked auto-generated");
        assert!(stub.contains("# b00t:map v1"), "tail-map present");
    }

    #[test]
    fn write_stub_datum_creates_file() {
        let dir = TempDir::new().unwrap();
        let path_str = dir.path().to_str().unwrap();
        let result = write_stub_datum(path_str, "test-topic.skill").unwrap();
        assert!(result.exists(), "file was created");
        let content = fs::read_to_string(&result).unwrap();
        assert!(content.contains("test-topic.skill"));
    }
}
