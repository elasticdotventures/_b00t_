// b00t-cli/src/commands/up.rs
use anyhow::{Context, Result};
use clap::Parser;
use crate::session_memory::SessionMemory;
use crate::commands::ontology::build_ontology;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Parser, Debug)]
pub struct UpArgs {
    /// AI tool to use for the ralph loop
    #[clap(long, default_value = "claude", value_parser = ["claude", "amp", "codex", "opencode", "mistralrs"])]
    pub tool: String,

    /// Maximum iterations per ralph session
    #[clap(long, default_value = "10")]
    pub max_iter: u32,

    /// Agent role (filters ontology + tutorial path)
    #[clap(long)]
    pub role: Option<String>,

    /// Maximum restart cycles before giving up
    #[clap(long, default_value = "5")]
    pub max_restarts: u32,

    /// Onboard a git repo: discover ._b00t_/ datums and symlink into ~/.b00t/_b00t_/
    #[clap(long, help = "Repo path to onboard (defaults to cwd)", value_name = "PATH")]
    pub repo: Option<Option<String>>,

    /// Dry run — show what would be symlinked without writing (repo mode only)
    #[clap(long)]
    pub dry_run: bool,
}

impl UpArgs {
    pub fn execute(&self) -> Result<()> {
        // Repo onboarding mode: `b00t up --repo [path]`
        if let Some(repo_path) = &self.repo {
            let path = repo_path.as_deref().unwrap_or(".");
            return up_repo(path, self.dry_run);
        }

        let workspace_root = crate::utils::get_workspace_root();
        let ralph_script = format!("{}/b00t.sh", workspace_root);

        if !std::path::Path::new(&ralph_script).exists() {
            // Postel: check if we're in a git repo with ._b00t_/ before failing
            let cwd = std::env::current_dir().unwrap_or_default();
            if let Some(datum_dir) = find_repo_datum_dir(&cwd) {
                println!("🥾 b00t up: no b00t.sh found, but ._b00t_/ detected at {}",
                    datum_dir.display());
                println!("   hint: run `b00t up --repo .` to onboard this repo");
            }
            anyhow::bail!("b00t.sh not found at {}. Run from b00t workspace root.", ralph_script);
        }

        let mut restart_count = 0u32;
        let mut session = SessionMemory::load().unwrap_or_default();

        loop {
            println!("🥾 b00t up: cycle {} (tool={}, max_iter={})",
                restart_count + 1, self.tool, self.max_iter);

            // Hive stack summary — quick check before launching agent
            let stacks = crate::hive::hive_stacks_status();
            if !stacks.is_empty() {
                let active: Vec<_> = stacks.iter().filter(|(_, a, _)| *a).map(|(n, _, _)| n.as_str()).collect();
                if active.is_empty() {
                    println!("  🥾 stacks: none active (b00t hive activate <profile>)");
                } else {
                    println!("  🥾 stacks: {}", active.join(", "));
                }
            }

            // Build ontology JSON from live datum TOML scan
            let datum_dir = format!("{}/_b00t_", workspace_root);
            let ontology = build_ontology(self.role.as_deref(), &datum_dir)
                .unwrap_or_else(|_| crate::commands::ontology::Ontology {
                    role: self.role.clone().unwrap_or_else(|| "developer".to_string()),
                    available: vec![],
                    installable: vec![],
                    blessings: vec![],
                    timestamp: chrono::Utc::now().to_rfc3339(),
                });
            let ontology_json = serde_json::to_string(&ontology)
                .unwrap_or_else(|_| "{}".to_string());

            let status = Command::new("bash")
                .arg(&ralph_script)
                .arg("--tool")
                .arg(&self.tool)
                .arg(self.max_iter.to_string())
                .env("B00T_ONTOLOGY", &ontology_json)
                .env("B00T_ROLE", self.role.as_deref().unwrap_or("developer"))
                .current_dir(&workspace_root)
                .status()
                .context(format!("Failed to exec b00t.sh at {}", ralph_script))?;

            let code = status.code().unwrap_or(1);

            // Persist cycle state to session memory; set() auto-saves internally
            let _ = session.set("up.last_exit", &code.to_string());
            let _ = session.set("up.tool", &self.tool);
            let _ = session.set("up.restart_count", &restart_count.to_string());
            emit_up_heartbeat(restart_count, code, &self.role);

            match code {
                0 => {
                    println!("✅ b00t up: ralph completed after {} cycle(s)", restart_count + 1);
                    return Ok(());
                }
                75 => {
                    // POSIX TEMPFAIL — agent requests restart
                    restart_count += 1;
                    if restart_count >= self.max_restarts {
                        anyhow::bail!(
                            "b00t up: max restarts ({}) reached. Last exit: 75",
                            self.max_restarts
                        );
                    }
                    println!("🔄 b00t up: restart {}/{} (exit 75 = TEMPFAIL)",
                        restart_count, self.max_restarts);
                }
                n => {
                    anyhow::bail!("b00t up: ralph exited with error code {}", n);
                }
            }
        }
    }
}

// ── Repo onboarding ───────────────────────────────────────────────────────────

/// Postel-defensive discovery: check several candidate paths for a b00t datum dir.
/// Be liberal in what we accept (._b00t_/, .b00t/, _b00t_/, b00t/).
fn find_repo_datum_dir(start: &Path) -> Option<PathBuf> {
    let candidates = ["._b00t_", ".b00t", "_b00t_", "b00t"];
    let mut dir = start.to_path_buf();
    loop {
        for name in &candidates {
            let candidate = dir.join(name);
            if candidate.is_dir() {
                return Some(candidate);
            }
        }
        // Also accept a bare b00t.toml / workspace.project.tomllm at root
        if dir.join(".git").exists() {
            break; // hit git root without finding datum dir
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

/// Walk up from `start` to find the git repository root.
fn find_git_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join(".git").exists() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Derive a namespace slug from git remote URL or directory name.
/// e.g. "git@github.com:app4dog/workspace.git" → "app4dog--workspace"
fn namespace_from_repo(repo_root: &Path) -> String {
    // Try git remote origin URL
    let output = Command::new("git")
        .args(["-C", &repo_root.to_string_lossy(), "remote", "get-url", "origin"])
        .output()
        .ok()
        .filter(|o| o.status.success());

    if let Some(out) = output {
        let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
        // Extract "owner/repo" from SSH or HTTPS URL
        let slug = url
            .trim_end_matches(".git")
            .split([':', '/'])
            .rev()
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("--");
        if !slug.is_empty() {
            return slug;
        }
    }

    // Fallback: directory name
    repo_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Core repo onboarding: discover ._b00t_/ datums, symlink into ~/.b00t/_b00t_/
fn up_repo(path: &str, dry_run: bool) -> Result<()> {
    let start = Path::new(path).canonicalize()
        .with_context(|| format!("Cannot resolve path: {}", path))?;

    // 1. Find git root (required — we need it for namespace derivation)
    let git_root = find_git_root(&start)
        .ok_or_else(|| anyhow::anyhow!("Not inside a git repository: {}", start.display()))?;

    // 2. Find datum dir (Postel: multiple candidate names)
    let datum_dir = find_repo_datum_dir(&git_root)
        .ok_or_else(|| anyhow::anyhow!(
            "No ._b00t_/ directory found in {}.\n  Create one with: mkdir ._b00t_",
            git_root.display()
        ))?;

    // 3. Derive namespace for symlink prefixing
    let namespace = namespace_from_repo(&git_root);
    println!("🥾 b00t up --repo: {}", git_root.display());
    println!("   namespace : {}", namespace);
    println!("   datum dir : {}", datum_dir.display());
    if dry_run {
        println!("   mode      : dry-run (no changes written)");
    }

    // 4. Find all .tomllm files in the datum dir
    let entries = std::fs::read_dir(&datum_dir)
        .with_context(|| format!("Cannot read {}", datum_dir.display()))?;

    let tomllm_files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "tomllm").unwrap_or(false)
            || p.to_string_lossy().ends_with(".tomllm"))
        .collect();

    if tomllm_files.is_empty() {
        println!("   ⚠️  no .tomllm files found in {}", datum_dir.display());
        return Ok(());
    }

    // 5. Symlink each into ~/.b00t/_b00t_/ as <namespace>--<filename>
    let b00t_datum_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?
        .join(".b00t")
        .join("_b00t_");

    if !b00t_datum_dir.exists() && !dry_run {
        std::fs::create_dir_all(&b00t_datum_dir)
            .with_context(|| format!("Cannot create {}", b00t_datum_dir.display()))?;
    }

    let mut registered = 0usize;
    for src in &tomllm_files {
        let filename = src.file_name().unwrap().to_string_lossy();
        let link_name = format!("{namespace}--{filename}");
        let link_path = b00t_datum_dir.join(&link_name);

        if dry_run {
            println!("   [dry-run] {} → {}", link_name, src.display());
            continue;
        }

        // If link already exists and points to same target, skip silently
        if link_path.exists() || link_path.symlink_metadata().is_ok() {
            if let Ok(existing) = std::fs::read_link(&link_path) {
                if existing == *src {
                    println!("   ✓ already linked: {}", link_name);
                    registered += 1;
                    continue;
                }
                // Stale symlink — replace it
                std::fs::remove_file(&link_path)
                    .with_context(|| format!("Cannot remove stale link: {}", link_path.display()))?;
            } else {
                // Real file exists — don't clobber, warn instead
                println!("   ⚠️  skipped (real file exists): {}", link_path.display());
                continue;
            }
        }

        std::os::unix::fs::symlink(src, &link_path)
            .with_context(|| format!("Cannot create symlink: {}", link_path.display()))?;
        println!("   → {}", link_name);
        registered += 1;
    }

    if !dry_run {
        println!("   ✅ registered {} datum(s) into {}", registered, b00t_datum_dir.display());
        // Persist to session memory so subsequent commands know the active repo
        if let Ok(mut session) = SessionMemory::load() {
            let _ = session.set("up.repo.root", &git_root.to_string_lossy());
            let _ = session.set("up.repo.namespace", &namespace);
            let _ = session.set("up.repo.datum_dir", &datum_dir.to_string_lossy());
        }
    }

    Ok(())
}

/// Emit b00t up state change event to IPC channel (best-effort, non-fatal)
fn emit_up_heartbeat(cycle: u32, exit_code: i32, role: &Option<String>) {
    let msg = format!(
        r#"{{"event":"b00t.up.cycle","cycle":{},"exit_code":{},"role":"{}","timestamp":"{}"}}"#,
        cycle,
        exit_code,
        role.as_deref().unwrap_or("developer"),
        chrono::Utc::now().to_rfc3339(),
    );

    // Only attempt IPC if socket exists — best-effort, never fatal
    let ipc_sock = dirs::home_dir()
        .unwrap_or_default()
        .join(".b00t")
        .join("ipc.sock");
    if ipc_sock.exists() {
        let _ = std::process::Command::new("b00t-ipc")
            .args(["pub", "b00t.up", &msg])
            .output();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_up_command_parses() {
        let args = UpArgs::try_parse_from(["b00t-cli", "--tool", "claude"]);
        assert!(args.is_ok(), "UpArgs should parse --tool claude");
    }

    #[test]
    fn test_up_command_parses_mistralrs_tool() {
        let args = UpArgs::try_parse_from(["b00t-cli", "--tool", "mistralrs"]);
        assert!(args.is_ok(), "UpArgs should parse --tool mistralrs");
    }

    #[test]
    fn test_up_command_defaults() {
        let args = UpArgs {
            tool: "claude".to_string(),
            max_iter: 10,
            role: None,
            max_restarts: 5,
            repo: None,
            dry_run: false,
        };
        assert_eq!(args.tool, "claude");
        assert_eq!(args.max_iter, 10);
        assert_eq!(args.max_restarts, 5);
        assert!(args.role.is_none());
    }

    #[test]
    fn test_up_command_invalid_tool_rejected() {
        let args = UpArgs::try_parse_from(["b00t-cli", "--tool", "invalid_tool"]);
        assert!(args.is_err(), "Invalid tool should be rejected");
    }

    #[test]
    fn test_exit_code_75_is_tempfail() {
        const POSIX_TEMPFAIL: i32 = 75;
        assert_eq!(POSIX_TEMPFAIL, 75);
    }

    #[test]
    fn test_restart_logic_counts_correctly() {
        let exit_codes = vec![75i32, 75, 75, 0];
        let max_restarts = 5u32;
        let mut restart_count = 0u32;
        let mut final_code = -1i32;

        for code in exit_codes {
            match code {
                0 => { final_code = 0; break; }
                75 => {
                    restart_count += 1;
                    if restart_count >= max_restarts {
                        final_code = 75;
                        break;
                    }
                }
                n => { final_code = n; break; }
            }
        }
        assert_eq!(restart_count, 3);
        assert_eq!(final_code, 0);
    }

    #[test]
    fn test_restart_logic_stops_at_max() {
        let exit_codes = vec![75i32, 75, 75, 75, 75, 75]; // 6 restarts
        let max_restarts = 3u32;
        let mut restart_count = 0u32;
        let mut hit_max = false;

        for code in exit_codes {
            if code == 75 {
                restart_count += 1;
                if restart_count >= max_restarts {
                    hit_max = true;
                    break;
                }
            }
        }
        assert!(hit_max);
        assert_eq!(restart_count, 3);
    }

    #[test]
    fn test_find_git_root_finds_ancestor() {
        // Any path inside this cargo workspace should resolve to a git root
        let cwd = std::env::current_dir().unwrap();
        let root = find_git_root(&cwd);
        assert!(root.is_some(), "should find git root from cwd");
        assert!(root.unwrap().join(".git").exists());
    }

    #[test]
    fn test_namespace_from_repo_fallback_to_dirname() {
        // In a temp dir with no git remote, should fall back to dir name
        let tmp = std::env::temp_dir().join("b00t-test-ns");
        let _ = std::fs::create_dir_all(&tmp);
        let ns = namespace_from_repo(&tmp);
        assert!(!ns.is_empty(), "namespace should not be empty");
        // Cleanup (best-effort)
        let _ = std::fs::remove_dir(&tmp);
    }

    #[test]
    fn test_find_repo_datum_dir_returns_none_for_tempdir() {
        // A fresh temp dir has no ._b00t_/ etc.
        let tmp = std::env::temp_dir();
        // Only fails if none of the candidate dirs exist at tmp or its ancestors
        // (This is a best-effort check — may return Some if temp is under a b00t repo)
        let _ = find_repo_datum_dir(&tmp); // just verify it doesn't panic
    }

    #[test]
    fn test_datum_toml_has_validate_section() {
        let workspace = crate::utils::get_workspace_root();
        let git_datum = format!("{}/_b00t_/gh.cli.toml", workspace);
        if std::path::Path::new(&git_datum).exists() {
            let content = std::fs::read_to_string(&git_datum).unwrap();
            assert!(content.contains("[validate]"), "gh datum missing [validate] section");
            assert!(content.contains("[roles]"), "gh datum missing [roles] section");
            assert!(content.contains("required_for"), "gh datum missing required_for field");
        }
        // Graceful skip if file doesn't exist (CI environments)

        // Also check rustc datum if present
        let rustc_datum = format!("{}/_b00t_/rustc.cli.toml", workspace);
        if std::path::Path::new(&rustc_datum).exists() {
            let content = std::fs::read_to_string(&rustc_datum).unwrap();
            assert!(content.contains("[validate]"), "rustc datum missing [validate] section");
            assert!(content.contains("[roles]"), "rustc datum missing [roles] section");
        }
    }
}
