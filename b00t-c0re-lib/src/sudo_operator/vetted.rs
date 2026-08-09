// b00t-c0re-lib/src/sudo_operator/vetted.rs
// 🤓 Deterministic sudo-grant path: NOPASSWD execution authorized by a
//    git-content-hash match against origin/main, not human/LLM judgment.
//    See PRD-SUDO-OPERATOR-GOVERNANCE's vetted-script extension and the
//    design spec at app4dog/workspace's
//    docs/superpowers/specs/2026-08-08-vetted-sudo-mechanism-design.md.
//
//    KNOWN LIMITATION — no trust-anchor pinning, tracked as
//    elasticdotventures/_b00t_#991: this mechanism verifies a repo's
//    internal self-consistency, never that the repo itself is one anyone
//    should trust, and "matches origin/main" alone doesn't distinguish a
//    human-approved release from an agent's own merged PR. Safe today only
//    because the sudoers grant targets the operator's own login account
//    (see check_vetted()'s doc comment for the full explanation). Do not
//    extend the sudoers grant to a more-restricted principal (e.g. a
//    dedicated service/CI account) without implementing #991 first.

use anyhow::Result;
use duct::cmd;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum VettedResult {
    Vetted { blob_hash: String },
    NotVetted { reason: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct VettedScriptEntry {
    pub path: String,
    pub description: String,
}

#[derive(Debug, Default, Deserialize)]
struct VettedRegistryFile {
    #[serde(default)]
    vetted: Vec<VettedScriptEntry>,
}

const VETTED_REGISTRY_PATH: &str = "_b00t_/vetted-scripts.toml";

/// Load the vetted-script registry from `origin/main`'s
/// `_b00t_/vetted-scripts.toml` — deliberately NOT the local working tree.
/// The registry is itself part of the trust boundary (it's the allowlist),
/// so it must come from the same origin/main-reviewed source as the
/// scripts it names; reading it off local disk would let an uncommitted
/// local edit expand the allowlist without going through PR review.
/// Caller must fetch first (`check_vetted` does this before calling); a
/// missing/unreadable path on origin/main resolves to an empty registry
/// (fail-closed — nothing is vetted, not "trust everything").
pub fn load_vetted_registry(repo_root: &Path) -> Vec<VettedScriptEntry> {
    // Fully-qualified `refs/remotes/origin/main`, NOT the short `origin/main`
    // — git resolves an ambiguous short name against `refs/heads/` first, so
    // a local branch literally named `origin/main` (created by anyone with
    // write access to this working tree, no push required) would silently
    // shadow the real remote-tracking ref and reopen the exact "local edit
    // expands the allowlist" hole this function exists to close.
    let Some(text) = cmd!(
        "git",
        "show",
        format!("refs/remotes/origin/main:{VETTED_REGISTRY_PATH}")
    )
    .dir(repo_root)
    .stderr_capture()
    .read()
    .ok()
    else {
        return Vec::new();
    };
    toml::from_str::<VettedRegistryFile>(&text)
        .map(|f| f.vetted)
        .unwrap_or_default()
}

/// Bounded `git fetch origin main` — never hangs indefinitely. Failure is
/// fail-closed and immediate: `check_vetted` returns `NotVetted` as soon as
/// this errors, before any registry lookup or rev-parse is attempted.
fn fetch_origin_main(repo_root: &Path) -> Result<()> {
    let handle = cmd!("git", "fetch", "origin", "main", "--quiet")
        .dir(repo_root)
        .stdout_to_stderr()
        .start()?;

    match handle.wait_timeout(std::time::Duration::from_secs(5))? {
        Some(_) => Ok(()),
        None => {
            let _ = handle.kill();
            Err(anyhow::anyhow!("git fetch timed out after 5 seconds"))
        }
    }
}

fn git_blob_hash(repo_root: &Path, rev_and_path: &str) -> Option<String> {
    if let Some(path) = rev_and_path.strip_prefix("WORKTREE:") {
        cmd!("git", "hash-object", path).dir(repo_root).read().ok()
    } else {
        cmd!("git", "rev-parse", rev_and_path).dir(repo_root).read().ok()
    }
}

/// The core deterministic check: is `script_path` (relative to `repo_root`)
/// both registered in `_b00t_/vetted-scripts.toml` AND byte-identical (via
/// git blob hash) to what `origin/main` currently has at that path?
///
/// Fail-closed: any git operation failing (missing repo, network error,
/// fetch timeout, path not present on origin/main) resolves to NotVetted.
/// Never resolves to Vetted on an error path.
///
/// # Known limitation: no trust-anchor pinning (tracked: elasticdotventures/_b00t_#991)
///
/// `repo_root` is resolved from the caller's cwd (see `resolve_repo_toplevel`
/// in `exec.rs`) with no check on WHICH repos are actually trusted — this
/// function only verifies internal self-consistency ("does this repo's
/// working tree match that same repo's own origin/main"), never that the
/// repo itself is one anyone should trust. A git working tree with a
/// self-authored `origin/main` (any repo, any origin, self-created is fine)
/// satisfies this check for its own content.
///
/// This is safe today ONLY because the sudoers NOPASSWD grant currently
/// targets the operator's own login account — that principal already has
/// root-equivalent access via the same grant regardless of what repo it
/// points at, so "create your own repo" isn't a new privilege escalation
/// in the current deployment. It stops being safe the moment this grant
/// is extended to any more-restricted principal (e.g. a dedicated
/// service/CI account) without first adding an origin-URL trust-anchor
/// allowlist checked BEFORE this function's registry/hash logic runs.
///
/// A deeper, related gap (also tracked in #991): even with trust-anchor
/// pinning, "content matches a trusted repo's own origin/main" is a check
/// an agent with ordinary commit/PR-merge access can already satisfy for
/// its own changes — closing that requires a human-signed release gate
/// (e.g. a signed git tag verified via `git verify-tag` against an
/// allowed-signers file that lives outside any git repo), not just a
/// wider allowlist. Do not extend this mechanism's sudoers grant to a
/// more-restricted principal without implementing #991 first.
pub fn check_vetted(repo_root: &Path, script_path: &str) -> VettedResult {
    if let Err(e) = fetch_origin_main(repo_root) {
        return VettedResult::NotVetted {
            reason: format!("git fetch origin main failed: {e}"),
        };
    }

    let registry = load_vetted_registry(repo_root);
    if !registry.iter().any(|e| e.path == script_path) {
        return VettedResult::NotVetted {
            reason: format!("'{script_path}' is not a registered vetted script on origin/main"),
        };
    }

    let local_hash = match git_blob_hash(repo_root, &format!("WORKTREE:{script_path}")) {
        Some(h) => h,
        None => {
            return VettedResult::NotVetted {
                reason: format!("could not hash local file '{script_path}'"),
            };
        }
    };

    // Same ambiguous-refname concern as load_vetted_registry above —
    // fully-qualified so a local `origin/main` branch can't shadow it.
    let remote_hash = match git_blob_hash(repo_root, &format!("refs/remotes/origin/main:{script_path}")) {
        Some(h) => h,
        None => {
            return VettedResult::NotVetted {
                reason: format!("'{script_path}' not found on origin/main"),
            };
        }
    };

    if local_hash == remote_hash {
        VettedResult::Vetted { blob_hash: local_hash }
    } else {
        VettedResult::NotVetted {
            reason: "local content does not match origin/main".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    /// Builds a throwaway git repo with a committed `origin` remote
    /// pointing at an equally throwaway bare repo, and one file
    /// registered in `_b00t_/vetted-scripts.toml`. Returns the tempdir
    /// (kept alive for the test's duration) and its path.
    fn fixture_repo() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let bare = tmp.path().join("origin.git");
        let work = tmp.path().join("work");
        fs::create_dir_all(&bare).unwrap();
        fs::create_dir_all(&work).unwrap();

        Command::new("git").args(["init", "--bare"]).arg(&bare).status().unwrap();
        Command::new("git").arg("init").current_dir(&work).status().unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&work)
            .status()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "test"])
            .current_dir(&work)
            .status()
            .unwrap();
        Command::new("git")
            .args(["remote", "add", "origin"])
            .arg(&bare)
            .current_dir(&work)
            .status()
            .unwrap();

        fs::create_dir_all(work.join("_b00t_")).unwrap();
        fs::write(
            work.join("_b00t_/vetted-scripts.toml"),
            "[[vetted]]\npath = \"_b00t_/hello.sh\"\ndescription = \"test script\"\n",
        )
        .unwrap();
        fs::write(work.join("_b00t_/hello.sh"), "#!/bin/sh\necho hello\n").unwrap();

        Command::new("git").args(["add", "-A"]).current_dir(&work).status().unwrap();
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(&work)
            .status()
            .unwrap();
        Command::new("git")
            .args(["push", "-u", "origin", "HEAD:main"])
            .current_dir(&work)
            .status()
            .unwrap();

        (tmp, work)
    }

    #[test]
    fn test_matching_content_is_vetted() {
        let (_tmp, work) = fixture_repo();
        let result = check_vetted(&work, "_b00t_/hello.sh");
        assert!(matches!(result, VettedResult::Vetted { .. }));
    }

    #[test]
    fn test_tampered_local_content_is_not_vetted() {
        let (_tmp, work) = fixture_repo();
        std::fs::write(work.join("_b00t_/hello.sh"), "#!/bin/sh\necho PWNED\n").unwrap();
        let result = check_vetted(&work, "_b00t_/hello.sh");
        assert!(matches!(result, VettedResult::NotVetted { .. }));
    }

    #[test]
    fn test_unregistered_path_is_not_vetted() {
        let (_tmp, work) = fixture_repo();
        std::fs::write(work.join("_b00t_/other.sh"), "#!/bin/sh\necho other\n").unwrap();
        let result = check_vetted(&work, "_b00t_/other.sh");
        assert!(matches!(result, VettedResult::NotVetted { .. }));
    }

    #[test]
    fn test_missing_registry_file_is_not_vetted() {
        let tmp = tempfile::tempdir().unwrap();
        let result = check_vetted(tmp.path(), "_b00t_/whatever.sh");
        assert!(matches!(result, VettedResult::NotVetted { .. }));
    }

    #[test]
    fn test_load_vetted_registry_returns_entries() {
        let (_tmp, work) = fixture_repo();
        let entries = load_vetted_registry(&work);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "_b00t_/hello.sh");
    }

    #[test]
    fn test_load_vetted_registry_missing_file_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let entries = load_vetted_registry(tmp.path());
        assert!(entries.is_empty());
    }

    #[test]
    fn test_locally_edited_registry_entry_is_not_vetted() {
        let (_tmp, work) = fixture_repo();
        // Simulate the attack: add a NEW script and register it LOCALLY only
        // (write to disk, never commit/push) — origin/main's registry doesn't
        // know about it.
        fs::write(work.join("_b00t_/local-only.sh"), "#!/bin/sh\necho local-only\n").unwrap();
        let mut registry_text =
            fs::read_to_string(work.join("_b00t_/vetted-scripts.toml")).unwrap();
        registry_text.push_str(
            "\n[[vetted]]\npath = \"_b00t_/local-only.sh\"\ndescription = \"not actually reviewed\"\n",
        );
        fs::write(work.join("_b00t_/vetted-scripts.toml"), registry_text).unwrap();

        let result = check_vetted(&work, "_b00t_/local-only.sh");
        assert!(
            matches!(result, VettedResult::NotVetted { .. }),
            "a locally-added, never-committed registry entry must NOT be trusted: {result:?}"
        );
    }

    #[test]
    fn test_fetch_timeout_resolves_to_not_vetted() {
        let (_tmp, work) = fixture_repo();
        // Point origin at an address that will stall rather than fail-fast, so
        // this exercises the bounded-timeout branch specifically (not just
        // "any fetch error").
        Command::new("git")
            .args(["remote", "set-url", "origin", "http://10.255.255.1/unreachable.git"])
            .current_dir(&work)
            .status()
            .unwrap();

        let start = std::time::Instant::now();
        let result = check_vetted(&work, "_b00t_/hello.sh");
        let elapsed = start.elapsed();

        assert!(matches!(result, VettedResult::NotVetted { .. }));
        assert!(
            elapsed < std::time::Duration::from_secs(10),
            "fetch should have been bounded to ~5s by fetch_origin_main's timeout, took {elapsed:?}"
        );
    }

    #[test]
    fn test_local_branch_named_origin_slash_main_does_not_shadow_remote_tracking_ref() {
        let (_tmp, work) = fixture_repo();

        // Add an "evil" script and register it, but only in a LOCAL commit
        // — never pushed to the real origin. Then create a local branch
        // literally named `origin/main` pointing at this commit: git
        // resolves an ambiguous short name against refs/heads/ before
        // refs/remotes/, so before the refs/remotes/origin/main fix this
        // local branch would have silently shadowed the real
        // remote-tracking ref.
        fs::write(work.join("_b00t_/evil.sh"), "#!/bin/sh\necho PWNED\n").unwrap();
        let mut registry_text =
            fs::read_to_string(work.join("_b00t_/vetted-scripts.toml")).unwrap();
        registry_text.push_str(
            "\n[[vetted]]\npath = \"_b00t_/evil.sh\"\ndescription = \"never reviewed\"\n",
        );
        fs::write(work.join("_b00t_/vetted-scripts.toml"), registry_text).unwrap();
        Command::new("git").args(["add", "-A"]).current_dir(&work).status().unwrap();
        Command::new("git")
            .args(["commit", "-m", "local-only: add evil.sh"])
            .current_dir(&work)
            .status()
            .unwrap();
        // NOT pushed — this commit never reaches the real origin/main.
        Command::new("git")
            .args(["branch", "-f", "origin/main", "HEAD"])
            .current_dir(&work)
            .status()
            .unwrap();

        let result = check_vetted(&work, "_b00t_/evil.sh");
        assert!(
            matches!(result, VettedResult::NotVetted { .. }),
            "a local branch shadowing the short name 'origin/main' must not be trusted: {result:?}"
        );
    }
}
