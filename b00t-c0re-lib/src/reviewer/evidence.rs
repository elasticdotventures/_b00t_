// b00t-c0re-lib/src/reviewer/evidence.rs
// 🤓 Evidence types for the reviewer verdict pipeline.
//    Pure Rust — no git commands. Uses SHA-256 for content-addressed evidence.
//    Without evidence → command DID NOT HAPPEN → rollback.
//
//    Josh-project (josh-project/josh) has learn content in _b00t_/learn/git.md
//    for git history filtering at scale, but evidence must NOT depend on git.

use super::governance::{EvidenceNode, VerdictDisposition};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

// ── Command Evidence ────────────────────────────────────────────────────────

/// Evidence that a reviewer command was executed with a valid verdict.
/// Content-addressed via SHA-256 — no dependency on git.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEvidence {
    /// Content-addressed hash of the change being reviewed
    pub content_hash: String,
    /// The verdict that was rendered
    pub verdict: String,
    /// The command that was triggered
    pub command: String,
    /// ISO 8601 timestamp of evidence creation
    pub timestamp: String,
}

impl CommandEvidence {
    /// Create a new evidence record from content and verdict.
    /// The content_hash is a SHA-256 digest of the diff/commit/review target.
    pub fn new(
        content: impl AsRef<[u8]>,
        verdict: &VerdictDisposition,
        command: impl Into<String>,
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(content.as_ref());
        let content_hash = format!("{:x}", hasher.finalize());

        Self {
            content_hash,
            verdict: verdict.to_string(),
            command: command.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Create an evidence node from the content hash and verdict.
    /// The evidence node binds verdict to content without git dependency.
    pub fn to_evidence_node(&self) -> EvidenceNode {
        let mut hasher = Sha256::new();
        hasher.update(self.content_hash.as_bytes());
        hasher.update(self.verdict.as_bytes());
        let node_hash = format!("{:x}", hasher.finalize());

        EvidenceNode {
            commit_hash: node_hash,
            verdict: self.verdict.clone(),
            notes_ref: "review-verdict".into(),
        }
    }

    /// Verify that this evidence is internally consistent.
    /// Pure Rust — no shelling out to git.
    ///
    /// 🤓 Verification ensures the content_hash + verdict produce a
    ///    deterministic evidence node. External verification (e.g.,
    ///    checking against a ledgerr receipt) is a separate concern.
    pub fn verify(&self) -> bool {
        // Self-consistency check: the evidence node should be deterministic
        let node = self.to_evidence_node();
        !node.commit_hash.is_empty()
            && node.commit_hash.len() == 64  // SHA-256 hex = 64 chars
            && !self.content_hash.is_empty()
            && !self.verdict.is_empty()
    }
}

/// Trait for objects that can produce an evidence node.
///
/// 🤓 This is the evidence production side of the Satisfies<Constraint> pattern.
///    After a verdict satisfies its constraint, it produces an evidence node.
///    Without evidence → command DID NOT HAPPEN.
pub trait ProduceEvidence {
    /// Produce an evidence record from this object and its content.
    fn produce_evidence(&self, content: &[u8]) -> CommandEvidence;
}

// ── Convenience constructors ────────────────────────────────────────────────

/// Create an evidence record for an APPROVE verdict.
pub fn evidence_approve(content: impl AsRef<[u8]>) -> CommandEvidence {
    CommandEvidence::new(content, &VerdictDisposition::Approve, "proceed")
}

/// Create an evidence record for a REQUEST_CHANGES verdict.
pub fn evidence_request_changes(
    content: impl AsRef<[u8]>,
    reasons: &[String],
) -> CommandEvidence {
    let command = if reasons.is_empty() {
        "revise".to_string()
    } else {
        format!("revise: {}", reasons.join("; "))
    };
    CommandEvidence::new(
        content,
        &VerdictDisposition::RequestChanges {
            reasons: reasons.to_vec(),
        },
        command,
    )
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evidence_approve() {
        let ev = evidence_approve(b"diff content here");
        assert_eq!(ev.verdict, "APPROVE");
        assert_eq!(ev.command, "proceed");
        assert!(!ev.content_hash.is_empty());
        assert_eq!(ev.content_hash.len(), 64); // SHA-256 hex
        assert!(!ev.timestamp.is_empty());
    }

    #[test]
    fn test_evidence_deterministic() {
        let content = b"same diff content";
        let ev1 = evidence_approve(content);
        let ev2 = evidence_approve(content);
        assert_eq!(ev1.content_hash, ev2.content_hash);
        assert_eq!(ev1.to_evidence_node(), ev2.to_evidence_node());
    }

    #[test]
    fn test_evidence_content_sensitive() {
        let ev1 = evidence_approve(b"diff A");
        let ev2 = evidence_approve(b"diff B");
        assert_ne!(ev1.content_hash, ev2.content_hash);
    }

    #[test]
    fn test_evidence_request_changes() {
        let reasons = vec!["missing error handling".into(), "unsafe unwrap".into()];
        let ev = evidence_request_changes(b"problematic diff", &reasons);
        assert_eq!(ev.verdict, "REQUEST_CHANGES");
        assert!(ev.command.contains("missing error handling"));
        assert!(ev.command.contains("unsafe unwrap"));
        assert_eq!(ev.content_hash.len(), 64);
    }

    #[test]
    fn test_evidence_verify() {
        let ev = evidence_approve(b"valid content");
        assert!(ev.verify());
    }

    #[test]
    fn test_evidence_roundtrip() {
        let ev = evidence_approve(b"test content");
        let serialized = serde_json::to_string(&ev).unwrap();
        let deserialized: CommandEvidence = serde_json::from_str(&serialized).unwrap();
        assert_eq!(ev, deserialized);
        assert_eq!(ev.content_hash, deserialized.content_hash);
        assert_eq!(ev.verdict, deserialized.verdict);
    }

    #[test]
    fn test_evidence_node_deterministic() {
        let ev = evidence_approve(b"content");
        let node1 = ev.to_evidence_node();
        let node2 = ev.to_evidence_node();
        assert_eq!(node1, node2);
        assert_eq!(node1.commit_hash, node2.commit_hash);
    }

    #[test]
    fn test_evidence_node_hash_format() {
        let ev = evidence_approve(b"test");
        let node = ev.to_evidence_node();
        // commit_hash is SHA-256 of content_hash + verdict
        assert_eq!(node.commit_hash.len(), 64); // SHA-256 hex
        // All chars should be hex
        assert!(node.commit_hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // ── UFO grounding test ────────────────────────────────────────────────

    #[test]
    fn test_evidence_is_content_addressed() {
        // Content-addressed = same input → same output, different input → different output
        let ev1 = evidence_approve(b"same");
        let ev2 = evidence_approve(b"same");
        let ev3 = evidence_approve(b"different");
        assert_eq!(ev1.content_hash, ev2.content_hash);
        assert_ne!(ev1.content_hash, ev3.content_hash);
    }
}
