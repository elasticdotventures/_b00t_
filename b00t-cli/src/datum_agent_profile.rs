//! `AgentProfileSpec` — the payload of a `DatumType::AgentProfile` (`.agentprofile.toml`).
//!
//! A signed, versioned bundle assigned to an agent: the tool allow-list, the
//! skills it may learn, its soul/memory shard grants, model tier, budget
//! ceiling and coarse permissions. Issued by b00t.promptexecution.com's product
//! plane (SP1) and enforced by the b00t-mcp proxy (SP3). Signing is SP2-04.

use serde::{Deserialize, Serialize};

use crate::soul_scope::ShardKind;

/// Cognitive tier the r0le is allowed to run at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelTier {
    Sm0l,
    Ch0nky,
    Frontier,
}

impl Default for ModelTier {
    fn default() -> Self {
        ModelTier::Ch0nky
    }
}

/// Read vs read-write access to a soul/memory shard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShardMode {
    R,
    Rw,
}

/// One soul/memory shard grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoulShardGrant {
    pub kind: ShardKind,
    pub id: String,
    pub mode: ShardMode,
}

/// Detached signature over the canonical projection of an `AgentProfileSpec`
/// (see SP2-04 for `canonical_bytes()` / `sign()` / `verify()`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatumSignature {
    pub kid: String,
    #[serde(default = "default_alg")]
    pub alg: String,
    pub sig_b64: String,
    pub signed_fields_hash: String,
}

fn default_alg() -> String {
    "RS256".to_string()
}

/// The body of a `DatumType::AgentProfile` datum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct AgentProfileSpec {
    /// Tool-name globs the r0le may call (`*` = all).
    #[serde(default)]
    pub tool_allowlist: Vec<String>,
    /// Skill keys the r0le may learn.
    #[serde(default)]
    pub skills: Vec<String>,
    /// Soul/memory shard grants.
    #[serde(default)]
    pub soul_shard_grants: Vec<SoulShardGrant>,
    /// Cognitive tier ceiling.
    #[serde(default)]
    pub model_tier: ModelTier,
    /// Per-issuance budget ceiling, in cake.
    #[serde(default)]
    pub budget_ceiling: u64,
    /// Coarse permission strings (free-form, enforced by the proxy).
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Detached signature (absent on freshly-built, unsigned specs).
    #[serde(default)]
    pub signature: Option<DatumSignature>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"
tool_allowlist = ["cargo.*", "b00t_status"]
skills = ["rust", "worker"]
model_tier = "ch0nky"
budget_ceiling = 5000
permissions = ["net:egress"]

[[soul_shard_grants]]
kind = "skill"
id = "rust"
mode = "r"

[[soul_shard_grants]]
kind = "agent"
id = "worker"
mode = "rw"
"#;

    #[test]
    fn fixture_deserializes() {
        let spec: AgentProfileSpec = toml::from_str(FIXTURE).unwrap();
        assert_eq!(spec.tool_allowlist, ["cargo.*", "b00t_status"]);
        assert_eq!(spec.model_tier, ModelTier::Ch0nky);
        assert_eq!(spec.budget_ceiling, 5000);
        assert_eq!(spec.soul_shard_grants.len(), 2);
        assert_eq!(spec.soul_shard_grants[0].kind, ShardKind::Skill);
        assert_eq!(spec.soul_shard_grants[1].mode, ShardMode::Rw);
        assert!(spec.signature.is_none());
    }

    #[test]
    fn empty_spec_defaults() {
        let spec: AgentProfileSpec = toml::from_str("").unwrap();
        assert!(spec.tool_allowlist.is_empty());
        assert_eq!(spec.model_tier, ModelTier::Ch0nky);
        assert!(spec.signature.is_none());
    }

    #[test]
    fn bad_model_tier_errors() {
        assert!(toml::from_str::<AgentProfileSpec>("model_tier = \"turbo\"").is_err());
    }
}
