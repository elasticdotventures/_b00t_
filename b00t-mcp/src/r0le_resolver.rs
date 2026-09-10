//! SP3-03 — resolve a `(tenant, r0le)` to the tool allow-list + skills the
//! proxy enforces. `FixtureR0leResolver` for tests / pre-SP2; `DatumR0leResolver`
//! reads a signed `DatumType::AgentProfile` datum (`b00t r0le build` output).

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use b00t_cli::DatumType;
use b00t_cli::datum_agent_profile::{AgentProfileSpec, ModelTier};
use b00t_cli::datum_utils::get_all_datums_for_tenant;

/// The proxy-relevant projection of an r0le package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedR0le {
    pub tool_allowlist: Vec<String>,
    pub skills: Vec<String>,
    pub budget_ceiling: u64,
    pub model_tier: ModelTier,
}

impl From<AgentProfileSpec> for ResolvedR0le {
    fn from(s: AgentProfileSpec) -> Self {
        Self {
            tool_allowlist: s.tool_allowlist,
            skills: s.skills,
            budget_ceiling: s.budget_ceiling,
            model_tier: s.model_tier,
        }
    }
}

/// Resolve `(tenant, r0le)` to a [`ResolvedR0le`].
pub trait R0leResolver: Send + Sync {
    fn resolve(&self, tenant: Option<&str>, r0le: &str) -> Result<ResolvedR0le>;
}

/// In-memory map keyed by r0le name. Tests and the pre-SP2 path.
pub struct FixtureR0leResolver {
    map: HashMap<String, ResolvedR0le>,
}

impl FixtureR0leResolver {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }

    pub fn with(mut self, r0le: impl Into<String>, resolved: ResolvedR0le) -> Self {
        self.map.insert(r0le.into(), resolved);
        self
    }
}

impl Default for FixtureR0leResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl R0leResolver for FixtureR0leResolver {
    fn resolve(&self, _tenant: Option<&str>, r0le: &str) -> Result<ResolvedR0le> {
        self.map
            .get(r0le)
            .cloned()
            .ok_or_else(|| anyhow!("no fixture r0le '{r0le}'"))
    }
}

/// Resolves against `DatumType::AgentProfile` datums on disk. When
/// `require_signature` is set, the datum's `[b00t.agent_profile].signature` must
/// verify against `jwks_json` — otherwise the r0le is rejected.
pub struct DatumR0leResolver {
    pub b00t_path: String,
    pub jwks_json: Option<String>,
    pub require_signature: bool,
}

impl DatumR0leResolver {
    pub fn new(b00t_path: impl Into<String>) -> Self {
        Self {
            b00t_path: b00t_path.into(),
            jwks_json: None,
            require_signature: false,
        }
    }

    pub fn require_signature(mut self, jwks_json: impl Into<String>) -> Self {
        self.jwks_json = Some(jwks_json.into());
        self.require_signature = true;
        self
    }
}

impl R0leResolver for DatumR0leResolver {
    fn resolve(&self, tenant: Option<&str>, r0le: &str) -> Result<ResolvedR0le> {
        let datums = get_all_datums_for_tenant(&self.b00t_path, tenant, Some(6))?;
        let (datum, _path) = datums
            .get(r0le)
            .or_else(|| datums.get(&format!("{r0le}.agentprofile")))
            .or_else(|| datums.get(&format!("{r0le}.r0le")))
            .ok_or_else(|| anyhow!("no r0le datum '{r0le}' under {}", self.b00t_path))?;
        if datum.datum_type != Some(DatumType::AgentProfile) {
            return Err(anyhow!("datum '{r0le}' is not an AgentProfile"));
        }
        let spec = datum
            .agent_profile
            .clone()
            .ok_or_else(|| anyhow!("datum '{r0le}' has no [b00t.agent_profile] payload"))?;

        if self.require_signature {
            let jwks = self
                .jwks_json
                .as_deref()
                .ok_or_else(|| anyhow!("require_signature set but no JWKS configured"))?;
            spec.verify(jwks)
                .map_err(|e| anyhow!("r0le '{r0le}' signature rejected: {e}"))?;
        }

        Ok(ResolvedR0le::from(spec))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_resolved() -> ResolvedR0le {
        ResolvedR0le {
            tool_allowlist: vec!["cargo.*".into(), "b00t_status".into()],
            skills: vec!["rust.skill".into()],
            budget_ceiling: 100,
            model_tier: ModelTier::Ch0nky,
        }
    }

    #[test]
    fn fixture_resolver_hits_and_misses() {
        let r = FixtureR0leResolver::new().with("worker", sample_resolved());
        let got = r.resolve(None, "worker").unwrap();
        assert_eq!(got.tool_allowlist, ["cargo.*", "b00t_status"]);
        assert!(r.resolve(None, "nobody").is_err());
    }

    #[test]
    fn datum_resolver_reads_an_unsigned_agent_profile() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path();
        // `b00t r0le build` output shape.
        std::fs::write(
            p.join("worker.agentprofile.toml"),
            "[b00t]\nname = \"worker\"\ntype = \"agent_profile\"\nhint = \"r0le\"\n\n\
             [b00t.agent_profile]\ntool_allowlist = [\"cargo.*\"]\nskills = [\"rust.skill\"]\n\
             model_tier = \"ch0nky\"\nbudget_ceiling = 42\npermissions = []\n",
        )
        .unwrap();

        let resolver = DatumR0leResolver::new(p.to_str().unwrap());
        let got = resolver.resolve(None, "worker").unwrap();
        assert_eq!(got.tool_allowlist, ["cargo.*"]);
        assert_eq!(got.budget_ceiling, 42);
        assert_eq!(got.model_tier, ModelTier::Ch0nky);

        // same datum, but now demand a signature it doesn't have
        let strict = DatumR0leResolver::new(p.to_str().unwrap())
            .require_signature("{\"keys\":[]}");
        assert!(strict.resolve(None, "worker").is_err());
    }

    #[test]
    fn datum_resolver_missing_role_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let resolver = DatumR0leResolver::new(tmp.path().to_str().unwrap());
        assert!(resolver.resolve(None, "ghost").is_err());
    }
}
