// b00t-c0re-lib/src/sudo_operator/checkpoint.rs
// 🤓 Pre-grant system-state checkpoint. Generalizes job_executor.rs's
//    git-tag checkpoint pattern (b00t-cli/src/job_executor.rs::create_checkpoint,
//    lines ~213-238) into a standalone function usable outside job execution.
//
//    v1 scope: gives an operator what they need to MANUALLY revert (a git
//    tag + a text dump of relevant systemd/k8s state). Automated rollback
//    execution is explicit follow-up (see PRD-SUDO-OPERATOR-GOVERNANCE).

use anyhow::Result;
use duct::cmd;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Where the checkpoint's evidence actually lives — a git tag for repo
/// state, and/or a text artifact path for systemd/k8s state that isn't
/// captured by git.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointRef {
    pub git_tag: Option<String>,
    pub state_dump_path: Option<String>,
}

impl CheckpointRef {
    pub fn is_empty(&self) -> bool {
        self.git_tag.is_none() && self.state_dump_path.is_none()
    }

    /// A single-string reference for embedding in SudoGrantEvidence.checkpoint_ref.
    pub fn as_evidence_string(&self) -> String {
        match (&self.git_tag, &self.state_dump_path) {
            (Some(tag), Some(dump)) => format!("git_tag={tag};state_dump={dump}"),
            (Some(tag), None) => format!("git_tag={tag}"),
            (None, Some(dump)) => format!("state_dump={dump}"),
            (None, None) => "none".to_string(),
        }
    }
}

/// Capture a pre-command checkpoint before a Granted privileged command runs.
///
/// `project_root`: git repo to tag, if `Some` and it's actually a git repo
///   (tagging failure is non-fatal — same "don't block on checkpoint failure"
///   behavior as job_executor.rs::create_checkpoint).
/// `checkpoint_id`: unique-ish id for this grant (e.g. the SudoGrantEvidence
///   content_hash, truncated) — becomes `checkpoint/sudo/<checkpoint_id>`.
/// `state_dump_cmd`: an optional command (e.g. `["systemctl", "show", "k0scontroller"]`
///   or `["kubectl", "get", "deployment/ch0nky", "-n", "b00t-inference", "-o", "yaml"]`)
///   whose output gets written to `~/.b00t/checkpoints/<checkpoint_id>.txt` so a
///   human has concrete pre-state to compare against if they need to revert.
pub fn checkpoint_system_state(
    project_root: Option<&Path>,
    checkpoint_id: &str,
    state_dump_cmd: Option<&[String]>,
) -> Result<CheckpointRef> {
    let git_tag = project_root.and_then(|root| {
        let tag_name = format!("checkpoint/sudo/{checkpoint_id}");
        let message = format!("Sudo-grant checkpoint: {checkpoint_id}");
        match cmd!("git", "tag", "-a", &tag_name, "-m", &message)
            .dir(root)
            .stdout_to_stderr()
            .run()
        {
            Ok(_) => Some(tag_name),
            Err(e) => {
                // Non-fatal — don't block the grant on checkpoint failure,
                // same policy as job_executor.rs's create_checkpoint.
                eprintln!("⚠️  sudo-operator checkpoint: git tag failed: {e}");
                None
            }
        }
    });

    let state_dump_path = state_dump_cmd.and_then(|argv| {
        if argv.is_empty() {
            return None;
        }
        let output = cmd(&argv[0], &argv[1..]).stderr_to_stdout().read();
        match output {
            Ok(text) => {
                let dir = shellexpand::tilde("~/.b00t/checkpoints").to_string();
                if std::fs::create_dir_all(&dir).is_err() {
                    return None;
                }
                let path = format!("{dir}/{checkpoint_id}.txt");
                match std::fs::write(&path, text) {
                    Ok(_) => Some(path),
                    Err(e) => {
                        eprintln!("⚠️  sudo-operator checkpoint: failed to write state dump: {e}");
                        None
                    }
                }
            }
            Err(e) => {
                eprintln!("⚠️  sudo-operator checkpoint: state dump command failed: {e}");
                None
            }
        }
    });

    Ok(CheckpointRef {
        git_tag,
        state_dump_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_ref_empty() {
        let r = CheckpointRef {
            git_tag: None,
            state_dump_path: None,
        };
        assert!(r.is_empty());
        assert_eq!(r.as_evidence_string(), "none");
    }

    #[test]
    fn test_checkpoint_ref_evidence_string_both() {
        let r = CheckpointRef {
            git_tag: Some("checkpoint/sudo/abc".into()),
            state_dump_path: Some("/tmp/abc.txt".into()),
        };
        assert!(!r.is_empty());
        assert_eq!(
            r.as_evidence_string(),
            "git_tag=checkpoint/sudo/abc;state_dump=/tmp/abc.txt"
        );
    }

    #[test]
    fn test_checkpoint_no_project_root_no_dump_cmd() {
        // With nothing to checkpoint, should return an empty-but-Ok ref,
        // never fail the grant.
        let result = checkpoint_system_state(None, "test-id", None).unwrap();
        assert!(result.is_empty());
    }
}
