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

use crate::commands::blessing::collect_role_unlocks;
use crate::datum_agent_profile::{AgentProfileSpec, ModelTier, ShardMode, SoulShardGrant};
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
        other => Err(anyhow!("unknown tier '{other}' (want sm0l | ch0nky | frontier)")),
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
    let skills = manifest.required_skills();
    let tool_allowlist = manifest.all_unlocks();

    let mut soul_shard_grants: Vec<SoulShardGrant> = skills
        .iter()
        .map(|s| SoulShardGrant {
            kind: ShardKind::Skill,
            id: s.clone(),
            mode: ShardMode::R,
        })
        .collect();
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

            let toml = toml::to_string_pretty(&spec)?;
            match out {
                Some(p) => {
                    std::fs::write(p, &toml)?;
                    println!("wrote {}", p.display());
                }
                None => print!("{toml}"),
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(spec.soul_shard_grants.iter().any(|g| g.kind == ShardKind::Agent
            && g.id == "worker"
            && g.mode == ShardMode::Rw));
        assert!(spec.soul_shard_grants.iter().any(|g| g.kind == ShardKind::Skill
            && g.id == "rust.skill"
            && g.mode == ShardMode::R));
        assert!(spec.signature.is_none());

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
}
