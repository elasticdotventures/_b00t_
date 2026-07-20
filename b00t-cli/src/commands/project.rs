use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use std::process::Command;

/// Node-local overlay enclave — isolated changeset management for b00t datums.
#[derive(Parser)]
pub enum ProjectCommands {
    #[clap(about = "Initialize enclave branch + boundary tag for this node")]
    Init {
        #[clap(long, help = "Override detected node name (defaults to hostname)")]
        node: Option<String>,
        #[clap(long, help = "Start from this ref instead of current HEAD")]
        base: Option<String>,
    },
    #[clap(about = "Show enclave state (branch, tag, commit count, dirty files)")]
    Status {
        #[clap(long, help = "Emit JSON output")]
        json: bool,
    },
    #[clap(about = "Rebase enclave onto upstream HEAD; move boundary tag forward")]
    Sync,
    #[clap(about = "Stage overlay changes and commit to enclave branch")]
    Commit {
        #[clap(short, long, help = "Commit message")]
        message: Option<String>,
        #[clap(help = "Files to stage (default: all modified *.overlay.toml)")]
        files: Vec<String>,
    },
    #[clap(about = "Return working tree to clean baseline (drop overlay changes)")]
    Reset {
        #[clap(long, help = "Reset to this commit instead of boundary tag")]
        to: Option<String>,
        #[clap(long, short = 'y', help = "Skip confirmation prompt")]
        yes: bool,
    },
    #[clap(about = "Show enclave commit history (changesets since boundary tag)")]
    Log,
    #[clap(about = "Diff working tree against the boundary tag (overlay delta)")]
    Diff,
    #[clap(about = "Show project config from .git/🥾.tomllmd")]
    Show,
    #[clap(about = "Set a project override (pin tool version)")]
    Override {
        #[clap(help = "Tool=version pair, e.g. rustc=1.85.0")]
        tool_version: String,
    },
    #[clap(about = "Remove a project override")]
    Unset {
        #[clap(help = "Tool name to unpin")]
        tool: String,
    },
}

impl ProjectCommands {
    pub fn execute(&self) -> Result<()> {
        match self {
            Self::Init { node, base } => cmd_init(node.clone(), base.clone()),
            Self::Status { json } => cmd_status(*json),
            Self::Sync => cmd_sync(),
            Self::Commit { message, files } => cmd_commit(message.clone(), files.clone()),
            Self::Reset { to, yes } => cmd_reset(to.clone(), *yes),
            Self::Log => cmd_log(),
            Self::Diff => cmd_diff(),
            Self::Show => cmd_show(),
            Self::Override { tool_version } => cmd_override(tool_version),
            Self::Unset { tool } => cmd_unset(tool),
        }
    }
}

// ── project config (.git/🥾.tomllmd) ────────────────────────────────────

fn boot_path() -> Result<std::path::PathBuf> {
    let root = git_root()?;
    Ok(root.join(".git").join("🥾.tomllmd"))
}

fn read_boot() -> Result<String> {
    let path = boot_path()?;
    std::fs::read_to_string(&path).context("no 🥾.tomllmd found — run b00t init project first")
}

fn cmd_show() -> Result<()> {
    let content = read_boot()?;
    let overrides = crate::load_project_overrides();
    println!("{}", content);
    if !overrides.is_empty() {
        println!("\n📌 Active overrides:");
        for (k, v) in &overrides {
            println!("  {} = {}", k, v);
        }
    }
    Ok(())
}

fn cmd_override(tool_version: &str) -> Result<()> {
    let (tool, version) = tool_version
        .split_once('=')
        .ok_or_else(|| anyhow!("expected format: tool=version (e.g. rustc=1.85.0)"))?;
    let path = boot_path()?;
    let content = read_boot()?;

    let updated = if content.contains(&format!("# {}", tool)) {
        // Replace existing commented override
        content.replace(&format!("# {} = \"", tool), &format!("{} = \"", tool))
    } else {
        // Add to overrides section
        content.replace(
            "[overrides]",
            &format!("[overrides]\n{} = \"{}\"", tool, version),
        )
    };
    std::fs::write(&path, &updated)?;
    println!("✅ {} pinned to {}", tool, version);
    Ok(())
}

fn cmd_unset(tool: &str) -> Result<()> {
    let path = boot_path()?;
    let content = read_boot()?;
    // Comment out the override line
    let updated = content.replace(&format!("{} = \"", tool), &format!("# {} = \"", tool));
    std::fs::write(&path, &updated)?;
    println!("✅ {} unpinned", tool);
    Ok(())
}

fn current_hostname() -> Result<String> {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|h| !h.is_empty())
        .or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .ok_or_else(|| anyhow!("cannot determine hostname — set $HOSTNAME or use --node"))
}

fn git_root() -> Result<std::path::PathBuf> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("failed to run git rev-parse")?;
    if !out.status.success() {
        bail!("not inside a git repository");
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(std::path::PathBuf::from(path))
}

fn tag_name(host: &str) -> String {
    format!("b00t/node/{host}/base")
}

fn branch_name(host: &str) -> String {
    format!("b00t/node/{host}/overlay")
}

fn git(args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .args(args)
        .output()
        .with_context(|| format!("failed to run git {:?}", args))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        bail!("{err}");
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn git_optional(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn tag_exists(tag: &str) -> bool {
    git_optional(&["rev-parse", "--verify", &format!("refs/tags/{tag}")]).is_some()
}

fn branch_exists(branch: &str) -> bool {
    git_optional(&["rev-parse", "--verify", &format!("refs/heads/{branch}")]).is_some()
}

fn current_branch() -> Result<String> {
    git(&["symbolic-ref", "--short", "HEAD"])
}

// ── commands ─────────────────────────────────────────────────────────

fn cmd_init(node: Option<String>, base: Option<String>) -> Result<()> {
    let host = node.unwrap_or_else(|| current_hostname().unwrap_or("localhost".into()));
    let tag = tag_name(&host);
    let branch = branch_name(&host);

    if tag_exists(&tag) {
        bail!("enclave tag already exists: {tag}");
    }
    if branch_exists(&branch) {
        bail!("enclave branch already exists: {branch}");
    }

    let base_ref = base.as_deref().unwrap_or("HEAD");
    let base_sha = git(&["rev-parse", base_ref])?;

    // create boundary tag
    git(&["tag", &tag, &base_sha])?;
    // create overlay branch at the same point
    git(&["branch", &branch, &base_sha])?;
    // switch to enclave
    git(&["checkout", &branch])?;

    println!("✓ enclave initialized for node '{host}'");
    println!("  tag:     {tag} @ {base_sha:.12}");
    println!("  branch:  {branch}");
    println!();
    println!("overlay changes now commit to the enclave branch.");
    println!("push is blocked — use 'b00t project sync' to rebase onto upstream.");

    Ok(())
}

fn cmd_status(json: bool) -> Result<()> {
    let host = current_hostname().unwrap_or("localhost".into());
    let tag = tag_name(&host);
    let branch = branch_name(&host);

    let tag_ok = tag_exists(&tag);
    let branch_ok = branch_exists(&branch);

    if !tag_ok && !branch_ok {
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "initialized": false,
                    "node": host
                })
            );
        } else {
            println!("✗ no enclave found for node '{host}'");
            println!("  run: b00t project init");
        }
        return Ok(());
    }

    let commits_ahead = if tag_ok && branch_ok {
        git(&["rev-list", "--count", &format!("{tag}..{branch}")])
            .unwrap_or_else(|_| "0".into())
            .parse::<u32>()
            .unwrap_or(0)
    } else {
        0
    };

    let dirty = git(&["status", "--porcelain"]).unwrap_or_default();
    let dirty_files: Vec<&str> = dirty.lines().filter(|l| !l.is_empty()).collect();

    let current = current_branch().unwrap_or_default();
    let on_enclave = current == branch;

    if json {
        println!(
            "{}",
            serde_json::json!({
                "initialized": true,
                "node": host,
                "tag": tag,
                "branch": branch,
                "on_enclave": on_enclave,
                "commits_ahead": commits_ahead,
                "dirty_files": dirty_files,
            })
        );
    } else {
        println!("enclave: {branch}");
        println!("  tag:      {tag}");
        println!("  on branch: {on_enclave}");
        println!("  commits:   {commits_ahead} ahead of base");
        if dirty_files.is_empty() {
            println!("  dirty:     (clean)");
        } else {
            println!("  dirty:     {} file(s)", dirty_files.len());
            for f in &dirty_files {
                println!("    {f}");
            }
        }
    }

    Ok(())
}

fn cmd_sync() -> Result<()> {
    let host = current_hostname().unwrap_or("localhost".into());
    let tag = tag_name(&host);
    let branch = branch_name(&host);

    if !tag_exists(&tag) || !branch_exists(&branch) {
        bail!("enclave not initialized — run: b00t project init");
    }

    // fetch upstream
    println!("fetching upstream…");
    git(&["fetch", "origin"])?;

    // find upstream HEAD (prefer main, fallback to master)
    let upstream = git_optional(&["rev-parse", "origin/main"])
        .or_else(|| git_optional(&["rev-parse", "origin/master"]))
        .ok_or_else(|| anyhow!("cannot find origin/main or origin/master"))?;

    let old_tag = git(&["rev-parse", &tag])?;

    // rebase enclave onto upstream
    println!("rebasing enclave onto {upstream:.12}…");
    let rebase = Command::new("git")
        .args(["rebase", "--onto", &upstream, &old_tag, &branch])
        .status()
        .context("failed to run git rebase")?;

    if !rebase.success() {
        bail!("rebase failed — resolve conflicts then 'git rebase --continue'");
    }

    // move tag forward
    git(&["tag", "-f", &tag, &upstream])?;

    let new_count =
        git(&["rev-list", "--count", &format!("{tag}..{branch}")]).unwrap_or_else(|_| "0".into());

    println!("✓ enclave synced");
    println!("  tag moved: {old_tag:.12} → {upstream:.12}");
    println!("  {new_count} overlay commit(s) rebased");

    Ok(())
}

fn cmd_commit(message: Option<String>, files: Vec<String>) -> Result<()> {
    let host = current_hostname().unwrap_or("localhost".into());
    let branch = branch_name(&host);

    if !branch_exists(&branch) {
        bail!("enclave not initialized — run: b00t project init");
    }

    let current = current_branch()?;
    if current != branch {
        bail!("not on enclave branch (on '{current}') — run: git checkout {branch}");
    }

    // determine files to stage
    let to_stage: Vec<String> = if files.is_empty() {
        // auto-stage *.overlay.toml files
        let status = git(&["status", "--porcelain"])?;
        status
            .lines()
            .filter(|l| l.ends_with(".overlay.toml") || l.contains(".overlay.toml"))
            .map(|l| l[3..].trim().to_string())
            .collect()
    } else {
        files
    };

    if to_stage.is_empty() {
        println!("no overlay files to commit");
        return Ok(());
    }

    // stage
    for f in &to_stage {
        git(&["add", f])?;
    }

    let msg = message.unwrap_or_else(|| format!("overlay: {}", to_stage.join(", ")));

    git(&["commit", "-m", &msg])?;

    println!("✓ committed {msg}");
    for f in &to_stage {
        println!("  {f}");
    }

    Ok(())
}

fn cmd_reset(to: Option<String>, yes: bool) -> Result<()> {
    let host = current_hostname().unwrap_or("localhost".into());
    let tag = tag_name(&host);
    let branch = branch_name(&host);

    if !branch_exists(&branch) {
        bail!("enclave not initialized — run: b00t project init");
    }

    let target = to.unwrap_or_else(|| tag.clone());

    if !yes {
        println!("⚠️  this will discard all overlay changes back to {target}");
        println!("   press Enter to continue, Ctrl-C to abort…");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
    }

    git(&["reset", "--hard", &target])?;

    println!("✓ reset to {target}");

    Ok(())
}

fn cmd_log() -> Result<()> {
    let host = current_hostname().unwrap_or("localhost".into());
    let tag = tag_name(&host);
    let branch = branch_name(&host);

    if !tag_exists(&tag) || !branch_exists(&branch) {
        bail!("enclave not initialized — run: b00t project init");
    }

    let _log = Command::new("git")
        .args(["log", "--oneline", &format!("{tag}..{branch}")])
        .status()
        .context("failed to run git log")?;

    Ok(())
}

fn cmd_diff() -> Result<()> {
    let host = current_hostname().unwrap_or("localhost".into());
    let tag = tag_name(&host);

    if !tag_exists(&tag) {
        bail!("enclave not initialized — run: b00t project init");
    }

    let _diff = Command::new("git")
        .args(["diff", &format!("{tag}...HEAD")])
        .status()
        .context("failed to run git diff")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_and_branch_naming() {
        assert_eq!(tag_name("rock-5c"), "b00t/node/rock-5c/base");
        assert_eq!(branch_name("rock-5c"), "b00t/node/rock-5c/overlay");
    }

    #[test]
    fn test_hostname_fallback() {
        // current_hostname falls back to /etc/hostname
        let host = current_hostname();
        assert!(host.is_ok(), "hostname detection should not fail");
        assert!(!host.unwrap().is_empty());
    }
}
