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
    let Some(text) = cmd!("git", "show", format!("origin/main:{VETTED_REGISTRY_PATH}"))
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

    let remote_hash = match git_blob_hash(repo_root, &format!("origin/main:{script_path}")) {
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
}
