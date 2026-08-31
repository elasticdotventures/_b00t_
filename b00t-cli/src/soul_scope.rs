//! #1102: explicit scoped shards for the soul memory system.
//!
//! Replaces the old binary local-vs-global split (a directory-presence
//! check, see `memory_provider::active_soul_path`) with a 6-way categorical
//! taxonomy — project/system/agent/skill/tool/datum — each independently
//! addressable by kind + identifier. Deliberately additive: calls that don't
//! pass a scope keep resolving to exactly the same path as before
//! (`memory_provider::soul_path_for(None)` == `active_soul_path()`), so
//! existing unscoped data is the de facto "legacy" shard with no migration
//! required.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShardKind {
    Project,
    System,
    Agent,
    Skill,
    Tool,
    Datum,
}

impl ShardKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ShardKind::Project => "project",
            ShardKind::System => "system",
            ShardKind::Agent => "agent",
            ShardKind::Skill => "skill",
            ShardKind::Tool => "tool",
            ShardKind::Datum => "datum",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "project" => Some(ShardKind::Project),
            "system" => Some(ShardKind::System),
            "agent" => Some(ShardKind::Agent),
            "skill" => Some(ShardKind::Skill),
            "tool" => Some(ShardKind::Tool),
            "datum" => Some(ShardKind::Datum),
            _ => None,
        }
    }

    pub fn all() -> [ShardKind; 6] {
        [
            ShardKind::Project,
            ShardKind::System,
            ShardKind::Agent,
            ShardKind::Skill,
            ShardKind::Tool,
            ShardKind::Datum,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SoulScope {
    pub kind: ShardKind,
    pub id: String,
}

impl SoulScope {
    pub fn new(kind: ShardKind, id: impl Into<String>) -> Self {
        Self { kind, id: id.into() }
    }

    /// Parse a `<kind>:<id>` CLI/MCP flag value, e.g. `"agent:pi"`.
    pub fn parse_flag(s: &str) -> anyhow::Result<Self> {
        let (kind_str, id) = s
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("scope must be '<kind>:<id>', e.g. 'agent:pi' — got '{s}'"))?;
        let kind = ShardKind::parse(kind_str).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown shard kind '{kind_str}' — expected one of: project system agent skill tool datum"
            )
        })?;
        if id.is_empty() {
            anyhow::bail!("scope identifier must not be empty (got '{s}')");
        }
        Ok(Self::new(kind, id))
    }

    /// Default project scope inferred from the current directory: sha256 of
    /// `git remote get-url origin` (stable across clones/worktrees of the
    /// same repo), falling back to the git toplevel path when there's no
    /// remote (e.g. a fresh local-only repo). `None` if cwd isn't inside a
    /// git repo at all — callers fall back to the legacy shard in that case.
    pub fn infer_project() -> Option<Self> {
        repo_identity().map(|id| Self::new(ShardKind::Project, id))
    }

    /// Filesystem-safe directory for this shard, rooted under `<root>/shards/`.
    pub fn shard_dir(&self, root: &Path) -> PathBuf {
        root.join("shards")
            .join(self.kind.as_str())
            .join(sanitize_path_segment(&self.id))
    }
}

impl std::fmt::Display for SoulScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.kind.as_str(), self.id)
    }
}

fn git_toplevel() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn git_remote_url() -> Option<String> {
    let out = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

/// Stable repo identity for the "project" shard kind's default inference:
/// sha256 hex prefix of the origin remote URL, or the toplevel path when
/// there's no remote configured.
pub fn repo_identity() -> Option<String> {
    if let Some(url) = git_remote_url() {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let digest = hasher.finalize();
        return Some(hex_prefix(&digest, 16));
    }
    git_toplevel()
}

fn hex_prefix(bytes: &[u8], n: usize) -> String {
    bytes.iter().take(n).map(|b| format!("{b:02x}")).collect()
}

/// Renders an arbitrary scope identifier safe for use as one path segment.
fn sanitize_path_segment(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shard_kind_round_trips_through_as_str_and_parse() {
        for kind in ShardKind::all() {
            assert_eq!(ShardKind::parse(kind.as_str()), Some(kind));
        }
    }

    #[test]
    fn shard_kind_parse_is_case_insensitive_and_rejects_unknown() {
        assert_eq!(ShardKind::parse("Agent"), Some(ShardKind::Agent));
        assert_eq!(ShardKind::parse("AGENT"), Some(ShardKind::Agent));
        assert_eq!(ShardKind::parse("nonsense"), None);
    }

    #[test]
    fn parse_flag_accepts_kind_colon_id() {
        let scope = SoulScope::parse_flag("agent:pi").unwrap();
        assert_eq!(scope.kind, ShardKind::Agent);
        assert_eq!(scope.id, "pi");
    }

    #[test]
    fn parse_flag_rejects_missing_colon_unknown_kind_or_empty_id() {
        assert!(SoulScope::parse_flag("agent-pi").is_err());
        assert!(SoulScope::parse_flag("nonsense:pi").is_err());
        assert!(SoulScope::parse_flag("agent:").is_err());
    }

    #[test]
    fn display_round_trips_through_parse_flag() {
        let scope = SoulScope::new(ShardKind::Skill, "b00t-learn");
        let rendered = scope.to_string();
        let reparsed = SoulScope::parse_flag(&rendered).unwrap();
        assert_eq!(scope, reparsed);
    }

    #[test]
    fn sanitize_path_segment_neutralizes_path_traversal_and_separators() {
        assert_eq!(sanitize_path_segment("../../etc/passwd"), ".._.._etc_passwd");
        assert_eq!(sanitize_path_segment("weird id/with spaces"), "weird_id_with_spaces");
        assert_eq!(sanitize_path_segment("safe-id_1.2"), "safe-id_1.2");
    }

    #[test]
    fn shard_dir_nests_under_shards_kind_id() {
        let scope = SoulScope::new(ShardKind::Tool, "grok");
        let dir = scope.shard_dir(Path::new("/home/x/._b00t_"));
        assert_eq!(dir, PathBuf::from("/home/x/._b00t_/shards/tool/grok"));
    }

    #[test]
    fn repo_identity_is_stable_across_repeated_calls() {
        // Whatever this returns (Some or None, depending on the test
        // sandbox's git state), it must be deterministic within one process.
        assert_eq!(repo_identity(), repo_identity());
    }
}
