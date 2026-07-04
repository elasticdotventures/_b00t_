// b00t-c0re-lib/src/sudo_operator/governance.rs
// 🤓 Governance types for the sudo-grant adversarial-review pipeline.
//    Sibling of reviewer::governance — reuses the SAME evidence mechanism
//    (SHA-256 content-addressed, no external crate dependency) rather than
//    inventing a parallel Blake3/NodeId system. See PRD-SUDO-OPERATOR-GOVERNANCE
//    for the corrected-after-reading-the-real-code rationale.
//
//    UFO grounding: SudoDisposition = Moment::Mode (same as VerdictDisposition),
//    SudoGrantEvidence = Endurant::SubKind (same as CommandEvidence).
//
//    Without evidence → command DID NOT HAPPEN. Grant additionally requires
//    a checkpoint reference before the command executes (see checkpoint.rs).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ── Sudo Disposition ─────────────────────────────────────────────────────────
// UFO: Moment::Mode — intrinsic property of the review event.

/// The disposition of an adversarial sudo-grant review.
/// Three variants (vs VerdictDisposition's two) because a sudo grant needs
/// a distinct "can't decide, ask a human" state, and Grant carries a TTL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SudoDisposition {
    /// Command may execute; grant expires after ttl_seconds.
    Grant { ttl_seconds: u64 },
    /// Command must not execute.
    Deny { reason: String },
    /// The model can't confidently decide — escalate to a human, deny for now.
    Escalate { reason: String },
}

impl std::fmt::Display for SudoDisposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SudoDisposition::Grant { ttl_seconds } => write!(f, "GRANT(ttl={ttl_seconds}s)"),
            SudoDisposition::Deny { reason } => write!(f, "DENY({reason})"),
            SudoDisposition::Escalate { reason } => write!(f, "ESCALATE({reason})"),
        }
    }
}

impl SudoDisposition {
    /// Whether this disposition permits the command to execute.
    pub fn permits_execution(&self) -> bool {
        matches!(self, SudoDisposition::Grant { .. })
    }
}

// ── Sudo Review Event ────────────────────────────────────────────────────────
// UFO: Perdurant::Event — the review itself, unfolding: command received →
// justification parsed → cited commits inspected → model queried → verdict.

/// The full context of a single adversarial sudo-grant review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SudoReviewEvent {
    pub command: String,
    pub justification: String,
    pub cited_commits: Vec<String>,
    /// `git show --stat` output for each cited commit that resolved
    /// successfully — grounds the review in a real diff, not just trusted
    /// free text. Commits that fail to resolve are simply omitted (treated
    /// as unverifiable, lowering confidence in the eventual verdict).
    pub cited_commit_evidence: Vec<String>,
}

impl SudoReviewEvent {
    /// The content that gets hashed for evidence — deterministic ordering.
    pub fn content_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(self.command.as_bytes());
        buf.extend_from_slice(self.justification.as_bytes());
        for c in &self.cited_commits {
            buf.extend_from_slice(c.as_bytes());
        }
        for e in &self.cited_commit_evidence {
            buf.extend_from_slice(e.as_bytes());
        }
        buf
    }
}

// ── Sudo Grant Evidence ──────────────────────────────────────────────────────
// UFO: Endurant::SubKind — persists beyond the review event, same subkind as
// reviewer::evidence::CommandEvidence.

/// Evidence that a sudo-grant review was performed with a given disposition.
/// Content-addressed via SHA-256 — identical mechanism to CommandEvidence,
/// distinct type because the hashed content and disposition shape differ.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SudoGrantEvidence {
    pub content_hash: String,
    pub disposition: String,
    pub command: String,
    /// Set by checkpoint_system_state() after a Grant; None until then.
    pub checkpoint_ref: Option<String>,
    pub timestamp: String,
}

impl SudoGrantEvidence {
    pub fn new(event: &SudoReviewEvent, disposition: &SudoDisposition) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(event.content_bytes());
        hasher.update(disposition.to_string().as_bytes());
        let content_hash = format!("{:x}", hasher.finalize());

        Self {
            content_hash,
            disposition: disposition.to_string(),
            command: event.command.clone(),
            checkpoint_ref: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn with_checkpoint(mut self, checkpoint_ref: impl Into<String>) -> Self {
        self.checkpoint_ref = Some(checkpoint_ref.into());
        self
    }

    /// Self-consistency check — same shape as CommandEvidence::verify().
    pub fn verify(&self) -> bool {
        self.content_hash.len() == 64
            && self.content_hash.chars().all(|c| c.is_ascii_hexdigit())
            && !self.disposition.is_empty()
            && !self.command.is_empty()
    }

    /// Governance invariant: a Grant's evidence MUST carry a checkpoint
    /// reference before the command it authorizes may execute.
    pub fn grant_is_execution_ready(&self) -> bool {
        self.disposition.starts_with("GRANT") && self.checkpoint_ref.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event() -> SudoReviewEvent {
        SudoReviewEvent {
            command: "sudo systemctl restart k0scontroller".into(),
            justification: "kubelet device-plugin registration wedged".into(),
            cited_commits: vec!["deadbeef".into()],
            cited_commit_evidence: vec!["1 file changed".into()],
        }
    }

    #[test]
    fn test_evidence_deterministic() {
        let event = sample_event();
        let disposition = SudoDisposition::Grant { ttl_seconds: 300 };
        let a = SudoGrantEvidence::new(&event, &disposition);
        let b = SudoGrantEvidence::new(&event, &disposition);
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_evidence_differs_by_disposition() {
        let event = sample_event();
        let grant = SudoGrantEvidence::new(&event, &SudoDisposition::Grant { ttl_seconds: 300 });
        let deny = SudoGrantEvidence::new(
            &event,
            &SudoDisposition::Deny { reason: "no".into() },
        );
        assert_ne!(grant.content_hash, deny.content_hash);
    }

    #[test]
    fn test_evidence_valid_hash() {
        let event = sample_event();
        let evidence = SudoGrantEvidence::new(&event, &SudoDisposition::Grant { ttl_seconds: 60 });
        assert!(evidence.verify());
    }

    #[test]
    fn test_grant_not_execution_ready_without_checkpoint() {
        let event = sample_event();
        let evidence = SudoGrantEvidence::new(&event, &SudoDisposition::Grant { ttl_seconds: 60 });
        assert!(!evidence.grant_is_execution_ready());
        let with_cp = evidence.with_checkpoint("checkpoint/sudo/abc123");
        assert!(with_cp.grant_is_execution_ready());
    }

    #[test]
    fn test_deny_never_execution_ready() {
        let event = sample_event();
        let evidence = SudoGrantEvidence::new(
            &event,
            &SudoDisposition::Deny { reason: "insufficient justification".into() },
        )
        .with_checkpoint("checkpoint/sudo/should-not-matter");
        assert!(!evidence.grant_is_execution_ready());
    }

    #[test]
    fn test_permits_execution() {
        assert!(SudoDisposition::Grant { ttl_seconds: 1 }.permits_execution());
        assert!(!SudoDisposition::Deny { reason: "x".into() }.permits_execution());
        assert!(!SudoDisposition::Escalate { reason: "x".into() }.permits_execution());
    }
}
