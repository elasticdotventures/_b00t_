//! SP5-02 — compile the tenant / r0le / tool / grant / budget graph as
//! `b00t:`-namespace SPO triples. Lives in `b00t-cli` (with `BootDatum`,
//! `datum_utils`, `AgentProfileSpec`, `ShardKind`); the composed triple set
//! reaches the `store-oxigraph` substrate via `b00t graph emit-triples`.

use crate::datum_agent_profile::ShardMode;
use crate::datum_types::DatumType;
use crate::datum_utils::get_all_datums_for_tenant;
use anyhow::Result;

/// Compile the identity/authz overlay as `b00t:` SPO triples:
/// `tenant -> hasR0le -> r0le`, `r0le -> {allowsTool, grantsShard, modelTier,
/// budgetCeiling}`, `shard -> shardMode`, and for SP4 MCP-server datums
/// `svc -> {image, budgetCeiling}`. `tenant == None` scans the base tree and
/// tags everything `_base`.
pub fn compile_identity_triples(
    b00t_path: &str,
    tenant: Option<&str>,
) -> Result<Vec<(String, String, String)>> {
    let t = tenant.unwrap_or("_base");
    let datums = get_all_datums_for_tenant(b00t_path, tenant, Some(6))?;
    let mut out: Vec<(String, String, String)> = Vec::new();

    let mut keys: Vec<&String> = datums.keys().collect();
    keys.sort();

    for key in keys {
        let (datum, _path) = &datums[key];

        // ── r0le / AgentProfile ───────────────────────────────────────────
        if datum.datum_type == Some(DatumType::AgentProfile) {
            if let Some(spec) = &datum.agent_profile {
                let role = key.strip_suffix(".agentprofile").unwrap_or(key);
                let r0le_iri = format!("b00t:r0le/{t}/{role}");
                out.push((
                    format!("b00t:tenant/{t}"),
                    "b00t:hasR0le".into(),
                    r0le_iri.clone(),
                ));
                for tool in &spec.tool_allowlist {
                    out.push((r0le_iri.clone(), "b00t:allowsTool".into(), tool.clone()));
                }
                for g in &spec.soul_shard_grants {
                    let shard = format!("b00t:shard/{}/{}", g.kind.as_str(), g.id);
                    out.push((r0le_iri.clone(), "b00t:grantsShard".into(), shard.clone()));
                    let mode = match g.mode {
                        ShardMode::Rw => "rw",
                        ShardMode::R => "r",
                    };
                    out.push((shard, "b00t:shardMode".into(), mode.to_string()));
                }
                out.push((
                    r0le_iri.clone(),
                    "b00t:modelTier".into(),
                    format!("{:?}", spec.model_tier).to_lowercase(),
                ));
                out.push((
                    r0le_iri,
                    "b00t:budgetCeiling".into(),
                    spec.budget_ceiling.to_string(),
                ));
            }
        }

        // ── MCP service (SP4 [b00t.mcp_server]) ───────────────────────────
        if let Some(mcp) = &datum.mcp_server {
            let svc = key.strip_suffix(".mcp_server").unwrap_or(key);
            let svc_iri = format!("b00t:svc/{svc}");
            out.push((svc_iri.clone(), "b00t:image".into(), mcp.image.clone()));
            if mcp.budget_ceiling > 0 {
                out.push((
                    svc_iri,
                    "b00t:budgetCeiling".into(),
                    mcp.budget_ceiling.to_string(),
                ));
            }
        }
    }

    out.sort();
    out.dedup();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("worker.agentprofile.toml");
        let mut f = std::fs::File::create(&p).unwrap();
        writeln!(
            f,
            r#"
[b00t]
name = "worker"
type = "r0le"

[b00t.agent_profile]
tool_allowlist = ["cargo.*", "b00t_status"]
skills = ["rust"]
model_tier = "ch0nky"
budget_ceiling = 1000
permissions = []

[[b00t.agent_profile.soul_shard_grants]]
kind = "datum"
id = "*"
mode = "rw"

[[b00t.agent_profile.soul_shard_grants]]
kind = "project"
id = "*"
mode = "rw"
"#
        )
        .unwrap();
        dir
    }

    #[test]
    fn emits_tenant_role_tool_and_grant_triples() {
        let dir = fixture();
        let triples = compile_identity_triples(dir.path().to_str().unwrap(), None).unwrap();
        let has = |s: &str, p: &str, o: &str| {
            triples.iter().any(|(a, b, c)| a == s && b == p && c == o)
        };
        assert!(has("b00t:tenant/_base", "b00t:hasR0le", "b00t:r0le/_base/worker"));
        assert!(has("b00t:r0le/_base/worker", "b00t:allowsTool", "cargo.*"));
        assert!(has(
            "b00t:r0le/_base/worker",
            "b00t:grantsShard",
            "b00t:shard/datum/*"
        ));
        assert!(has("b00t:shard/datum/*", "b00t:shardMode", "rw"));
        assert!(has("b00t:r0le/_base/worker", "b00t:modelTier", "ch0nky"));
        assert!(has("b00t:r0le/_base/worker", "b00t:budgetCeiling", "1000"));
    }
}
