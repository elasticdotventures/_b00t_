// 🤓 b00t capability-forge — thin admin CLI over capability_forge::enroll.
//    Enroll/suspend/grant are operator-time actions against the same redb
//    ScopeStore the capability-forge NATS service reads at request time; this
//    subcommand opens that store directly rather than talking to a running
//    service, since there is no admin RPC surface (out of scope for this
//    task — see the capability-forge implementation plan).
use anyhow::{Context, Result};
use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
use b00t_c0re_gov::scope_store::ScopeId;
use capability_forge::enroll::{enroll_agent, grant_base_skill, suspend_agent};
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum CapabilityForgeCommands {
    #[clap(about = "Enroll a new agent: generate an nkey pair and grant base-tier skills")]
    Enroll {
        #[clap(long, help = "Agent ID")]
        agent_id: String,
        #[clap(long, help = "Base-tier skill to grant (repeatable)")]
        skill: Vec<String>,
        #[clap(long, help = "Path to the redb scope-store database file")]
        db_path: String,
    },
    #[clap(about = "Suspend an enrolled agent (blocks further authorization)")]
    Suspend {
        #[clap(long, help = "Agent ID")]
        agent_id: String,
        #[clap(long, help = "Path to the redb scope-store database file")]
        db_path: String,
    },
    #[clap(about = "Grant an additional base-tier skill to an already-enrolled agent")]
    Grant {
        #[clap(long, help = "Agent ID")]
        agent_id: String,
        #[clap(long, help = "Skill to grant")]
        skill: String,
        #[clap(long, help = "Path to the redb scope-store database file")]
        db_path: String,
    },
}

pub fn handle_capability_forge_command(cmd: &CapabilityForgeCommands) -> Result<()> {
    match cmd {
        CapabilityForgeCommands::Enroll {
            agent_id,
            skill,
            db_path,
        } => enroll(db_path, agent_id, skill),
        CapabilityForgeCommands::Suspend { agent_id, db_path } => suspend(db_path, agent_id),
        CapabilityForgeCommands::Grant {
            agent_id,
            skill,
            db_path,
        } => grant(db_path, agent_id, skill),
    }
}

pub fn enroll(db_path: &str, agent_id: &str, skills: &[String]) -> Result<()> {
    let mut store = RedbScopeStore::open(db_path, ScopeId::Global, None)
        .with_context(|| format!("opening redb at {db_path}"))?;
    let kp = enroll_agent(&mut store, agent_id, skills)?;
    println!("agent_id: {agent_id}");
    println!("pubkey: {}", kp.public_key());
    println!("seed (hand to agent, do not store here): {}", kp.seed()?);
    Ok(())
}

pub fn suspend(db_path: &str, agent_id: &str) -> Result<()> {
    let mut store = RedbScopeStore::open(db_path, ScopeId::Global, None)
        .with_context(|| format!("opening redb at {db_path}"))?;
    suspend_agent(&mut store, agent_id)?;
    println!("agent {agent_id} suspended");
    Ok(())
}

pub fn grant(db_path: &str, agent_id: &str, skill: &str) -> Result<()> {
    let mut store = RedbScopeStore::open(db_path, ScopeId::Global, None)
        .with_context(|| format!("opening redb at {db_path}"))?;
    grant_base_skill(&mut store, agent_id, skill)?;
    println!("agent {agent_id} granted base-tier skill {skill}");
    Ok(())
}
