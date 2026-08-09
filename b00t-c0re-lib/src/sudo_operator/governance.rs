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

use crate::reviewer::governance::SatisfiesResult;
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
    /// Deterministic grant: script_path's on-disk content matched
    /// origin/main's blob hash for that path. No adversarial review, no
    /// checkpoint. See PRD-SUDO-OPERATOR-GOVERNANCE's vetted-script extension.
    VettedGrant { script_path: String, blob_hash: String },
}

impl std::fmt::Display for SudoDisposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SudoDisposition::Grant { ttl_seconds } => write!(f, "GRANT(ttl={ttl_seconds}s)"),
            SudoDisposition::Deny { reason } => write!(f, "DENY({reason})"),
            SudoDisposition::Escalate { reason } => write!(f, "ESCALATE({reason})"),
            SudoDisposition::VettedGrant { script_path, blob_hash } => {
                write!(f, "VETTED_GRANT(path={script_path}, blob={blob_hash})")
            }
        }
    }
}

impl SudoDisposition {
    /// Whether this disposition permits the command to execute.
    pub fn permits_execution(&self) -> bool {
        matches!(
            self,
            SudoDisposition::Grant { .. } | SudoDisposition::VettedGrant { .. }
        )
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

    /// Evidence for a deterministic vetted-script grant. Distinct from
    /// `new()` because a vetted grant has no SudoReviewEvent (no
    /// justification, no cited commits, no adversarial review) — the
    /// content being hashed is just the script path + blob hash + a fixed
    /// disposition tag, which is sufficient to make the evidence
    /// deterministic and content-addressed the same way new() is.
    pub fn new_vetted(script_path: &str, blob_hash: &str) -> Self {
        let disposition = SudoDisposition::VettedGrant {
            script_path: script_path.to_string(),
            blob_hash: blob_hash.to_string(),
        };
        let mut hasher = Sha256::new();
        hasher.update(script_path.as_bytes());
        hasher.update(blob_hash.as_bytes());
        hasher.update(disposition.to_string().as_bytes());
        let content_hash = format!("{:x}", hasher.finalize());

        Self {
            content_hash,
            disposition: disposition.to_string(),
            command: script_path.to_string(),
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

    /// Governance invariant: an adversarially-reviewed Grant's evidence
    /// MUST carry a checkpoint reference before the command it authorizes
    /// may execute. A VettedGrant is execution-ready with evidence alone —
    /// by design it never has a checkpoint (see design spec's "Why no
    /// checkpoint" section) — its safety instead comes from the content
    /// having already gone through normal PR review on origin/main.
    pub fn grant_is_execution_ready(&self) -> bool {
        (self.disposition.starts_with("GRANT(") && self.checkpoint_ref.is_some())
            || self.disposition.starts_with("VETTED_GRANT(")
    }
}

// ── Satisfies<SudoGrantConstraint> (Tax-Lawyer pattern) ─────────────────────
// UFO: Abstract — constraints are formal rules governing the grant moment,
// independent of any particular grant instance (see PRD [ufo_grounding]).
// 🤓 No free-standing Satisfies<T> trait exists in the workspace yet; this
//    mirrors reviewer::governance's constraint-enum + SatisfiesResult idiom
//    and reuses the reviewer's SatisfiesResult type directly.

/// A constraint a sudo-grant's evidence must satisfy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SudoGrantConstraint {
    /// The governance invariant gating execution: evidence verifies
    /// (content_hash is a real SHA-256 digest) AND the Grant carries a
    /// checkpoint reference. Delegates to grant_is_execution_ready().
    ExecutionReady,
}

impl SudoGrantConstraint {
    /// FOL: constraint(evidence) → bool
    pub fn evaluate(&self, evidence: &SudoGrantEvidence) -> bool {
        match self {
            SudoGrantConstraint::ExecutionReady => {
                evidence.verify() && evidence.grant_is_execution_ready()
            }
        }
    }
}

impl SudoGrantEvidence {
    /// Satisfies<SudoGrantConstraint>: a Grant satisfies ExecutionReady only
    /// with BOTH verified evidence and a checkpoint ref; Deny/Escalate never do.
    pub fn satisfies(&self, constraint: &SudoGrantConstraint) -> SatisfiesResult {
        if constraint.evaluate(self) {
            SatisfiesResult::yes("grant has verified evidence and a checkpoint ref")
        } else if !self.verify() {
            SatisfiesResult::no("evidence failed self-consistency check")
        } else {
            SatisfiesResult::no(
                "grant is not execution-ready (non-Grant disposition or missing checkpoint ref)",
            )
        }
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

    #[test]
    fn test_satisfies_execution_ready_grant_with_checkpoint() {
        let evidence =
            SudoGrantEvidence::new(&sample_event(), &SudoDisposition::Grant { ttl_seconds: 60 })
                .with_checkpoint("git_tag=checkpoint/sudo/abc123");
        let result = evidence.satisfies(&SudoGrantConstraint::ExecutionReady);
        assert!(result.satisfied);
        assert!(!result.is_violated());
    }

    #[test]
    fn test_satisfies_unsatisfied_without_checkpoint() {
        let evidence =
            SudoGrantEvidence::new(&sample_event(), &SudoDisposition::Grant { ttl_seconds: 60 });
        assert!(evidence.satisfies(&SudoGrantConstraint::ExecutionReady).is_violated());
    }

    #[test]
    fn test_satisfies_unsatisfied_for_deny_even_with_checkpoint() {
        let evidence = SudoGrantEvidence::new(
            &sample_event(),
            &SudoDisposition::Deny { reason: "no".into() },
        )
        .with_checkpoint("git_tag=checkpoint/sudo/abc123");
        assert!(evidence.satisfies(&SudoGrantConstraint::ExecutionReady).is_violated());
    }

    #[test]
    fn test_satisfies_unsatisfied_for_tampered_hash() {
        let mut evidence =
            SudoGrantEvidence::new(&sample_event(), &SudoDisposition::Grant { ttl_seconds: 60 })
                .with_checkpoint("git_tag=checkpoint/sudo/abc123");
        evidence.content_hash = "not-a-hash".into();
        assert!(evidence.satisfies(&SudoGrantConstraint::ExecutionReady).is_violated());
    }

    #[test]
    fn test_vetted_grant_display() {
        let d = SudoDisposition::VettedGrant {
            script_path: "_b00t_/ci/vetted/pg-setup.sh".into(),
            blob_hash: "abc123".into(),
        };
        assert_eq!(
            d.to_string(),
            "VETTED_GRANT(path=_b00t_/ci/vetted/pg-setup.sh, blob=abc123)"
        );
    }

    #[test]
    fn test_vetted_grant_permits_execution() {
        let d = SudoDisposition::VettedGrant {
            script_path: "x.sh".into(),
            blob_hash: "abc".into(),
        };
        assert!(d.permits_execution());
    }

    #[test]
    fn test_new_vetted_evidence_is_execution_ready_without_checkpoint() {
        let evidence = SudoGrantEvidence::new_vetted("_b00t_/ci/vetted/pg-setup.sh", "abc123");
        assert!(evidence.checkpoint_ref.is_none());
        assert!(evidence.verify());
        assert!(evidence.grant_is_execution_ready());
    }

    #[test]
    fn test_new_vetted_evidence_deterministic_hash() {
        let a = SudoGrantEvidence::new_vetted("x.sh", "abc");
        let b = SudoGrantEvidence::new_vetted("x.sh", "abc");
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_new_vetted_evidence_differs_by_blob_hash() {
        let a = SudoGrantEvidence::new_vetted("x.sh", "abc");
        let b = SudoGrantEvidence::new_vetted("x.sh", "def");
        assert_ne!(a.content_hash, b.content_hash);
    }

    #[test]
    fn test_satisfies_execution_ready_for_vetted_grant_without_checkpoint() {
        let evidence = SudoGrantEvidence::new_vetted("x.sh", "abc");
        let result = evidence.satisfies(&SudoGrantConstraint::ExecutionReady);
        assert!(result.satisfied);
    }
}
