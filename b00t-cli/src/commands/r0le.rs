//! `b00t r0le` — build / (later) show / verify r0le packages
//! (`DatumType::AgentProfile`, `.agentprofile.toml`).
//!
//! `build` composes an [`AgentProfileSpec`] from a role datum's `depends_on`
//! blessing chain: the union of every discovered skill's `unlocks` becomes the
//! tool allow-list, the discovered skill keys become `skills`, and each gets a
//! read grant on its soul shard (plus a read-write grant on the role's own
//! agent shard). Emitted TOML is unsigned unless `--sign`.

use std::path::PathBuf;

use anyhow::{Result, anyhow};

use crate::DatumType;
use crate::boot_datum::BootDatum;
use crate::commands::blessing::collect_role_unlocks;
use crate::config_types::UnifiedConfig;
use crate::datum_agent_profile::{AgentProfileSpec, ModelTier, ShardMode, SoulShardGrant};
use crate::datum_utils::get_all_datums_for_tenant;
use crate::soul_scope::ShardKind;
use crate::whoami::load_role_datum;

#[derive(clap::Parser, Clone)]
pub struct R0leArgs {
    #[command(subcommand)]
    pub cmd: R0leCmd,
}

#[derive(clap::Subcommand, Clone)]
pub enum R0leCmd {
    /// Compose an r0le package from a role datum's blessing chain.
    Build {
        /// Role datum name (resolves `<role>.role` first).
        #[clap(long)]
        role: String,
        /// Tenant namespace (reserved — resolution is base-only for now).
        #[clap(long)]
        tenant: Option<String>,
        /// Write the TOML here instead of stdout.
        #[clap(long)]
        out: Option<PathBuf>,
        /// Cognitive tier ceiling: sm0l | ch0nky | frontier.
        #[clap(long, default_value = "ch0nky")]
        tier: String,
        /// Per-issuance budget ceiling, in cake.
        #[clap(long, default_value_t = 0)]
        budget_ceiling: u64,
        /// Sign the spec with `$B00T_DATUM_SIGNING_KEY_PEM`
        /// (kid from `$B00T_DATUM_SIGNING_KID`, default `datum-1`).
        #[clap(long)]
        sign: bool,
        /// Override the `_b00t_` directory (default `$_B00T_Path` or `~/.b00t/_b00t_`).
        #[clap(long)]
        b00t_path: Option<String>,
    },
    /// Print an r0le package (`b00t datum` of type AgentProfile).
    Show {
        /// Datum key (e.g. `worker`).
        id: String,
        #[clap(long)]
        tenant: Option<String>,
        #[clap(long)]
        json: bool,
        #[clap(long)]
        b00t_path: Option<String>,
    },
    /// Verify an r0le package's signature against a JWKS.
    Verify {
        id: String,
        #[clap(long)]
        tenant: Option<String>,
        #[clap(long)]
        b00t_path: Option<String>,
        /// JWKS JSON file (default: `$B00T_IDENTITY_JWKS` inline).
        #[clap(long)]
        jwks: Option<PathBuf>,
    },
}

fn default_b00t_path() -> String {
    std::env::var("_B00T_Path")
        .ok()
        .unwrap_or_else(|| shellexpand::tilde("~/.b00t/_b00t_").into_owned())
}

fn parse_tier(s: &str) -> Result<ModelTier> {
    match s.to_ascii_lowercase().as_str() {
        "sm0l" => Ok(ModelTier::Sm0l),
        "ch0nky" => Ok(ModelTier::Ch0nky),
        "frontier" => Ok(ModelTier::Frontier),
        other => Err(anyhow!(
            "unknown tier '{other}' (want sm0l | ch0nky | frontier)"
        )),
    }
}

/// Compose an [`AgentProfileSpec`] for `role` from the `_b00t_` tree at `b00t_path`.
pub fn compose_agent_profile(
    b00t_path: &str,
    role: &str,
    tier: ModelTier,
    budget_ceiling: u64,
) -> Result<AgentProfileSpec> {
    // Presence check — a real role datum must exist.
    let _role_details = load_role_datum(role, b00t_path)
        .ok_or_else(|| anyhow!("no role datum for '{role}' under {b00t_path}"))?;

    let manifest = collect_role_unlocks(b00t_path, role)?;
    let mut skills: Vec<String> = manifest
        .required
        .iter()
        .chain(&manifest.optional)
        .map(|(skill, _)| skill.clone())
        .collect();
    skills.sort();
    skills.dedup();
    let mut tool_allowlist = manifest.all_unlocks();
    if !tool_allowlist.iter().any(|tool| tool == "b00t_learn") {
        tool_allowlist.push("b00t_learn".into());
    }

    // SP5-02b (operator directive 2026-09-11): every r0le gets read/write on
    // both soulscopes AND datums. Blanket `{kind}:*:rw` for all six ShardKinds.
    let mut soul_shard_grants: Vec<SoulShardGrant> = [
        ShardKind::Datum,
        ShardKind::Project,
        ShardKind::System,
        ShardKind::Agent,
        ShardKind::Skill,
        ShardKind::Tool,
    ]
    .into_iter()
    .map(|kind| SoulShardGrant {
        kind,
        id: "*".to_string(),
        mode: ShardMode::Rw,
    })
    .collect();
    // narrower per-skill read grants, kept for provenance / discovery
    soul_shard_grants.extend(skills.iter().map(|s| SoulShardGrant {
        kind: ShardKind::Skill,
        id: s.clone(),
        mode: ShardMode::R,
    }));
    soul_shard_grants.push(SoulShardGrant {
        kind: ShardKind::Agent,
        id: role.to_string(),
        mode: ShardMode::Rw,
    });

    Ok(AgentProfileSpec {
        tool_allowlist,
        skills,
        soul_shard_grants,
        model_tier: tier,
        budget_ceiling,
        permissions: Vec::new(),
        signature: None,
    })
}

/// Wrap an `AgentProfileSpec` as a `DatumType::AgentProfile` BootDatum and
/// render it to `.agentprofile.toml` text (`[b00t]` + `[b00t.agent_profile]`).
pub fn render_agent_profile_datum(role: &str, spec: AgentProfileSpec) -> Result<String> {
    let datum = BootDatum {
        name: role.to_string(),
        datum_type: Some(DatumType::AgentProfile),
        hint: format!("r0le package for {role}"),
        agent_profile: Some(spec),
        ..Default::default()
    };
    let cfg = UnifiedConfig {
        b00t: datum,
        service_contract: Vec::new(),
        env: None,
        sections: None,
    };
    Ok(toml::to_string_pretty(&cfg)?)
}

/// Load the `AgentProfileSpec` of the r0le datum keyed `id`.
fn load_r0le_spec(b00t_path: &str, tenant: Option<&str>, id: &str) -> Result<AgentProfileSpec> {
    let datums = get_all_datums_for_tenant(b00t_path, tenant, Some(6))?;
    // Datum keys are filename-minus-`.toml`, so `worker.agentprofile.toml` keys
    // as `worker.agentprofile` — try the bare id and the suffixed forms.
    let (datum, _path) = datums
        .get(id)
        .or_else(|| datums.get(&format!("{id}.agentprofile")))
        .or_else(|| datums.get(&format!("{id}.agent_profile")))
        .or_else(|| datums.get(&format!("{id}.r0le")))
        .ok_or_else(|| anyhow!("no r0le datum keyed '{id}' under {b00t_path}"))?;
    if datum.datum_type != Some(DatumType::AgentProfile) {
        return Err(anyhow!("datum '{id}' is not an AgentProfile (r0le) datum"));
    }
    datum
        .agent_profile
        .clone()
        .ok_or_else(|| anyhow!("datum '{id}' has no [b00t.agent_profile] payload"))
}

fn read_jwks(jwks: Option<&std::path::Path>) -> Result<String> {
    if let Some(p) = jwks {
        return std::fs::read_to_string(p).map_err(|e| anyhow!("read {}: {e}", p.display()));
    }
    std::env::var("B00T_IDENTITY_JWKS")
        .map_err(|_| anyhow!("no --jwks and $B00T_IDENTITY_JWKS is unset"))
}

pub fn handle_r0le(args: &R0leArgs) -> Result<()> {
    match &args.cmd {
        R0leCmd::Build {
            role,
            tenant,
            out,
            tier,
            budget_ceiling,
            sign,
            b00t_path,
        } => {
            if tenant.is_some() {
                eprintln!("⚠️  --tenant is accepted but datum resolution is base-only for now");
            }
            let path = b00t_path.clone().unwrap_or_else(default_b00t_path);
            let tier = parse_tier(tier)?;
            let mut spec = compose_agent_profile(&path, role, tier, *budget_ceiling)?;

            if *sign {
                let pem = crate::datum_agent_profile::datum_signing_key_pem()?;
                let kid = std::env::var("B00T_DATUM_SIGNING_KID")
                    .unwrap_or_else(|_| "datum-1".to_string());
                spec.sign(&kid, &pem)?;
            }

            let toml = render_agent_profile_datum(role, spec)?;
            match out {
                Some(p) => {
                    std::fs::write(p, &toml)?;
                    println!("wrote {}", p.display());
                }
                None => print!("{toml}"),
            }
            Ok(())
        }
        R0leCmd::Show {
            id,
            tenant,
            json,
            b00t_path,
        } => {
            let path = b00t_path.clone().unwrap_or_else(default_b00t_path);
            let spec = load_r0le_spec(&path, tenant.as_deref(), id)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&spec)?);
            } else {
                println!("r0le: {id}");
                println!("  model_tier      : {:?}", spec.model_tier);
                println!("  budget_ceiling  : {}", spec.budget_ceiling);
                println!("  skills          : {}", spec.skills.join(", "));
                println!("  tool_allowlist  : {}", spec.tool_allowlist.join(", "));
                println!("  permissions     : {}", spec.permissions.join(", "));
                println!("  shard grants    : {}", spec.soul_shard_grants.len());
                match &spec.signature {
                    Some(s) => println!("  signature       : {} (kid={})", s.alg, s.kid),
                    None => println!("  signature       : <unsigned>"),
                }
            }
            Ok(())
        }
        R0leCmd::Verify {
            id,
            tenant,
            b00t_path,
            jwks,
        } => {
            let path = b00t_path.clone().unwrap_or_else(default_b00t_path);
            let spec = load_r0le_spec(&path, tenant.as_deref(), id)?;
            let jwks_json = read_jwks(jwks.as_deref())?;
            match spec.verify(&jwks_json) {
                Ok(()) => {
                    let kid = spec
                        .signature
                        .as_ref()
                        .map(|s| s.kid.as_str())
                        .unwrap_or("?");
                    println!("✅ signature valid (kid={kid})");
                    Ok(())
                }
                Err(e) => Err(anyhow!("❌ {e}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_profile_retains_optional_skills_and_learning_bootstrap() {
        let fixture: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_str(include_str!("../../tests/fixtures/identity-role.json")).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        for (name, datum) in fixture {
            std::fs::write(tmp.path().join(name), toml::to_string(&datum).unwrap()).unwrap();
        }
        let profile = compose_agent_profile(
            tmp.path().to_str().unwrap(),
            "worker",
            ModelTier::Ch0nky,
            42,
        )
        .unwrap();
        assert!(profile.skills.contains(&"audit.skill".into()));
        assert!(profile.tool_allowlist.contains(&"audit_*".into()));
        assert!(profile.tool_allowlist.contains(&"b00t_learn".into()));
    }

    #[test]
    fn build_composes_allowlist_from_skill_unlocks() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path();
        std::fs::write(
            p.join("worker.role.toml"),
            "[b00t]\nname = \"worker\"\ntype = \"role\"\ndepends_on = [\"rust.skill\"]\n",
        )
        .unwrap();
        std::fs::write(
            p.join("rust.skill.toml"),
            "[b00t]\nname = \"rust\"\ntype = \"skill\"\nunlocks = [\"cargo.*\", \"rustfmt\"]\n",
        )
        .unwrap();

        let spec =
            compose_agent_profile(p.to_str().unwrap(), "worker", ModelTier::Ch0nky, 42).unwrap();

        assert!(spec.tool_allowlist.contains(&"cargo.*".to_string()));
        assert!(spec.skills.contains(&"rust.skill".to_string()));
        assert_eq!(spec.model_tier, ModelTier::Ch0nky);
        assert_eq!(spec.budget_ceiling, 42);
        assert!(
            spec.soul_shard_grants
                .iter()
                .any(|g| g.kind == ShardKind::Agent && g.id == "worker" && g.mode == ShardMode::Rw)
        );
        assert!(
            spec.soul_shard_grants
                .iter()
                .any(|g| g.kind == ShardKind::Skill
                    && g.id == "rust.skill"
                    && g.mode == ShardMode::R)
        );
        assert!(spec.signature.is_none());

        // SP5-02b: rw:* on all six soulscope + datum ShardKinds
        for kind in [
            ShardKind::Datum,
            ShardKind::Project,
            ShardKind::System,
            ShardKind::Agent,
            ShardKind::Skill,
            ShardKind::Tool,
        ] {
            assert!(
                spec.soul_shard_grants.iter().any(|g| {
                    g.kind == kind && g.id == "*" && g.mode == ShardMode::Rw
                }),
                "missing rw:* grant for {kind:?}"
            );
        }

        // round-trips to valid TOML
        let toml = toml::to_string_pretty(&spec).unwrap();
        let back: AgentProfileSpec = toml::from_str(&toml).unwrap();
        assert_eq!(back.tool_allowlist, spec.tool_allowlist);
    }

    #[test]
    fn missing_role_datum_errors() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            compose_agent_profile(tmp.path().to_str().unwrap(), "ghost", ModelTier::Sm0l, 0)
                .is_err()
        );
    }

    #[test]
    fn parse_tier_accepts_the_three_tiers() {
        assert_eq!(parse_tier("SM0L").unwrap(), ModelTier::Sm0l);
        assert_eq!(parse_tier("ch0nky").unwrap(), ModelTier::Ch0nky);
        assert_eq!(parse_tier("frontier").unwrap(), ModelTier::Frontier);
        assert!(parse_tier("turbo").is_err());
    }

    #[test]
    fn build_output_is_a_parseable_agent_profile_datum() {
        let spec = AgentProfileSpec {
            tool_allowlist: vec!["cargo.*".into()],
            skills: vec!["rust.skill".into()],
            budget_ceiling: 7,
            ..Default::default()
        };
        let toml = render_agent_profile_datum("worker", spec).unwrap();
        assert!(toml.contains("[b00t]"));
        assert!(toml.contains("[b00t.agent_profile]"));
        let cfg: crate::config_types::UnifiedConfig = toml::from_str(&toml).unwrap();
        assert_eq!(cfg.b00t.datum_type, Some(DatumType::AgentProfile));
        let ap = cfg.b00t.agent_profile.unwrap();
        assert_eq!(ap.tool_allowlist, ["cargo.*"]);
        assert_eq!(ap.budget_ceiling, 7);
    }

    #[test]
    fn show_and_verify_resolve_a_built_datum() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path();
        std::fs::write(
            p.join("worker.role.toml"),
            "[b00t]\nname = \"worker\"\ntype = \"role\"\ndepends_on = [\"rust.skill\"]\n",
        )
        .unwrap();
        std::fs::write(
            p.join("rust.skill.toml"),
            "[b00t]\nname = \"rust\"\ntype = \"skill\"\nunlocks = [\"cargo.*\"]\n",
        )
        .unwrap();

        let spec =
            compose_agent_profile(p.to_str().unwrap(), "worker", ModelTier::Ch0nky, 5).unwrap();
        let datum_toml = render_agent_profile_datum("worker", spec).unwrap();
        std::fs::write(p.join("worker.agentprofile.toml"), datum_toml).unwrap();

        // show
        let loaded = load_r0le_spec(p.to_str().unwrap(), None, "worker").unwrap();
        assert!(loaded.tool_allowlist.contains(&"cargo.*".to_string()));
        assert!(loaded.signature.is_none());

        // verify of an unsigned spec must error
        assert!(loaded.verify("{\"keys\":[]}").is_err());
    }
}
