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

/// Load the vetted-script registry from `<repo_root>/_b00t_/vetted-scripts.toml`.
/// A missing file is not an error — it just means no scripts are registered yet.
pub fn load_vetted_registry(repo_root: &Path) -> Vec<VettedScriptEntry> {
    let registry_path = repo_root.join(VETTED_REGISTRY_PATH);
    let Ok(text) = std::fs::read_to_string(&registry_path) else {
        return Vec::new();
    };
    toml::from_str::<VettedRegistryFile>(&text)
        .map(|f| f.vetted)
        .unwrap_or_default()
}

/// `git fetch origin main` — failure is non-fatal to the caller; it just means
/// the local `origin/main` ref may be stale, which check_vetted treats as grounds
/// for NotVetted via the subsequent rev-parse failing or a genuine mismatch.
fn fetch_origin_main(repo_root: &Path) -> Result<()> {
    cmd!("git", "fetch", "origin", "main", "--quiet")
        .dir(repo_root)
        .stdout_to_stderr()
        .run()?;
    Ok(())
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
    let registry = load_vetted_registry(repo_root);
    if !registry.iter().any(|e| e.path == script_path) {
        return VettedResult::NotVetted {
            reason: format!("'{script_path}' is not a registered vetted script"),
        };
    }

    if let Err(e) = fetch_origin_main(repo_root) {
        return VettedResult::NotVetted {
            reason: format!("git fetch origin main failed: {e}"),
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
}
