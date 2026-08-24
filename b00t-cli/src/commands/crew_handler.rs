//! Handlers for crew subcommands — recruit, hire, roster
//!
//! Uses A2A AgentStore as the persistent agent backing store.
//! Each agent is stored as an AgentCard (A2A format) with an accompanying
//! crew metadata file for hierarchy-specific fields (role, manager, cake, etc.).

use std::collections::HashMap;
use std::path::PathBuf;

use b00t_c0re_a2a::agent_card::{AgentCard, Skill};
use b00t_c0re_a2a::agent_store::AgentStore;
use b00t_c0re_hierarchy::recruitment::*;
use b00t_c0re_hierarchy::roles::*;
use b00t_c0re_role::KnownRole;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::cake_ledger::CakeLedger;
use crate::commands::crew::CrewCommand;

/// Real, ledger-backed cake balance for `agent`, falling back to the
/// static `CrewMeta` placeholder value only if the ledger can't be opened
/// (e.g. no writable `$HOME` in a constrained environment) or has no
/// record for this agent yet.
fn live_cake_balance(ledger: Option<&CakeLedger>, agent: &Agent) -> i64 {
    let Some(ledger) = ledger else {
        return agent.cake_balance as i64;
    };
    match ledger.has_record(&agent.id) {
        Ok(true) => ledger.balance(&agent.id).unwrap_or(agent.cake_balance as i64),
        _ => agent.cake_balance as i64,
    }
}

/// Metadata for crew-specific fields not present in AgentCard.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CrewMeta {
    role: KnownRole,
    manager_id: Option<String>,
    cake_balance: f64,
    is_alive: bool,
    is_player: bool,
}

impl Default for CrewMeta {
    fn default() -> Self {
        Self {
            role: KnownRole::worker(),
            manager_id: None,
            cake_balance: 100.0,
            is_alive: true,
            is_player: false,
        }
    }
}

/// Convert an AgentCard + CrewMeta into a hierarchy Agent.
fn to_agent(card: &AgentCard, meta: &CrewMeta) -> Agent {
    Agent {
        id: card.name.clone(),
        role: meta.role.clone(),
        skills: card.skills.iter().map(|s| s.name.clone()).collect(),
        cake_balance: meta.cake_balance,
        is_alive: meta.is_alive,
        manager_id: meta.manager_id.clone(),
        is_player: meta.is_player,
    }
}

/// Build an AgentCard from a hierarchy Agent (inverse of to_agent).
fn card_from_agent(agent: &Agent) -> AgentCard {
    let url = Url::parse("stdio://local").unwrap();
    let mut card = AgentCard::new(&agent.id, &format!("{} agent", agent.id), url);
    for skill_name in &agent.skills {
        card = card.with_skill(Skill::new(
            skill_name,
            skill_name,
            &format!("{} skill", skill_name),
            serde_json::json!({}),
            serde_json::json!({}),
        ));
    }
    card
}

/// Default data directory: ~/.local/share/b00t/agents/
fn default_store_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".local");
        p.push("share");
        p.push("b00t");
        p.push("agents");
        p
    } else {
        PathBuf::from("/tmp/b00t/agents")
    }
}

/// Path to the crew metadata file (stored alongside AgentCards).
fn meta_path(store_dir: &PathBuf) -> PathBuf {
    store_dir.join("_crew_meta.json")
}

/// Load crew metadata from disk.
fn load_meta(path: &PathBuf) -> HashMap<String, CrewMeta> {
    if path.exists() {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    } else {
        HashMap::new()
    }
}

/// Save crew metadata to disk.
fn save_meta(path: &PathBuf, meta: &HashMap<String, CrewMeta>) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(meta) {
        let _ = std::fs::write(path, json);
    }
}

/// Seed 3 initial demo agents if the store is empty.
fn seed_if_empty(store: &AgentStore) {
    let count = store.count().unwrap_or(0);
    if count > 0 {
        return;
    }

    let url = Url::parse("stdio://local").unwrap();

    // RustCoder
    let rc_card = AgentCard::new("RustCoder", "Systems-level Rust developer", url.clone())
        .with_skill(Skill::new(
            "rust",
            "rust",
            "Rust programming language",
            serde_json::json!({}),
            serde_json::json!({}),
        ))
        .with_skill(Skill::new(
            "typescript",
            "typescript",
            "TypeScript/JavaScript",
            serde_json::json!({}),
            serde_json::json!({}),
        ));
    let rc_meta = CrewMeta {
        role: KnownRole::worker(),
        manager_id: None,
        cake_balance: 100.0,
        is_alive: true,
        is_player: false,
    };

    // DataEngineer
    let de_card = AgentCard::new("DataEngineer", "Data pipeline engineer", url.clone())
        .with_skill(Skill::new(
            "python",
            "python",
            "Python programming",
            serde_json::json!({}),
            serde_json::json!({}),
        ))
        .with_skill(Skill::new(
            "sql",
            "sql",
            "SQL queries",
            serde_json::json!({}),
            serde_json::json!({}),
        ))
        .with_skill(Skill::new(
            "data-engineering",
            "data-engineering",
            "Data pipeline engineering",
            serde_json::json!({}),
            serde_json::json!({}),
        ));
    let de_meta = CrewMeta {
        role: KnownRole::worker(),
        manager_id: None,
        cake_balance: 150.0,
        is_alive: true,
        is_player: false,
    };

    // DevOpsBot
    let db_card = AgentCard::new("DevOpsBot", "DevOps automation bot", url.clone())
        .with_skill(Skill::new(
            "docker",
            "docker",
            "Container management",
            serde_json::json!({}),
            serde_json::json!({}),
        ))
        .with_skill(Skill::new(
            "k8s",
            "k8s",
            "Kubernetes orchestration",
            serde_json::json!({}),
            serde_json::json!({}),
        ))
        .with_skill(Skill::new(
            "ci/cd",
            "ci/cd",
            "CI/CD pipelines",
            serde_json::json!({}),
            serde_json::json!({}),
        ));
    let db_meta = CrewMeta {
        role: KnownRole::worker(),
        manager_id: None,
        cake_balance: 80.0,
        is_alive: true,
        is_player: false,
    };

    // Save cards to AgentStore
    if let Err(e) = store.save(&rc_card) {
        eprintln!("Warning: failed to seed RustCoder: {e}");
    }
    if let Err(e) = store.save(&de_card) {
        eprintln!("Warning: failed to seed DataEngineer: {e}");
    }
    if let Err(e) = store.save(&db_card) {
        eprintln!("Warning: failed to seed DevOpsBot: {e}");
    }

    // Save metadata
    let mut m = HashMap::new();
    m.insert("RustCoder".to_string(), rc_meta);
    m.insert("DataEngineer".to_string(), de_meta);
    m.insert("DevOpsBot".to_string(), db_meta);
    let mp = meta_path(store.dir());
    save_meta(&mp, &m);
}

/// Collect all agents from the store with their crew metadata.
fn all_agents(store: &AgentStore) -> Vec<Agent> {
    let mp = meta_path(store.dir());
    let meta = load_meta(&mp);

    let cards = match store.list() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: failed to list agents: {e}");
            return Vec::new();
        }
    };

    cards
        .into_iter()
        .map(|card| {
            let m = meta.get(&card.name).cloned().unwrap_or_default();
            to_agent(&card, &m)
        })
        .collect()
}

/// Update crew metadata for an agent.
fn update_meta(store: &AgentStore, name: &str, f: impl FnOnce(&mut CrewMeta)) {
    let mp = meta_path(store.dir());
    let mut meta = load_meta(&mp);
    let mut entry = meta.remove(name).unwrap_or_default();
    f(&mut entry);
    meta.insert(name.to_string(), entry);
    save_meta(&mp, &meta);
}

/// Look up whether `agent_id` is recorded as a human player in the crew
/// roster (`Agent::is_player`, via `_crew_meta.json`). Returns `false` for
/// unknown agent ids — i.e. "assume software agent" absent other evidence.
/// Used by `b00t-mcp` to tag outgoing messages/votes with sender identity.
pub fn is_player(agent_id: &str) -> bool {
    let store = AgentStore::with_path(default_store_dir());
    all_agents(&store)
        .into_iter()
        .find(|a| a.id == agent_id)
        .map(|a| a.is_player)
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn handle_crew_command(cmd: &CrewCommand) {
    // Initialize the AgentStore
    let dir = default_store_dir();
    let store = AgentStore::with_path(dir.clone());
    let _ = std::fs::create_dir_all(&dir);

    // Seed demo agents if this is a first run
    seed_if_empty(&store);

    match cmd {
        CrewCommand::Recruit { skills, limit } => handle_recruit(&store, skills, *limit),
        CrewCommand::Hire { agent_id, role } => handle_hire(&store, agent_id, role.as_deref()),
        CrewCommand::Roster => handle_roster(&store),
    }
}

fn handle_recruit(store: &AgentStore, skills: &str, limit: usize) {
    let required: Vec<String> = skills.split(',').map(|s| s.trim().to_string()).collect();

    // Gather all agents
    let available = all_agents(store);

    let request = RecruitRequest {
        captain_id: "captain".into(),
        required_skills: required,
        max_players: limit,
        bounty_share: 0.1,
    };

    let response = process_recruit_request(&request, &available, "crew-cli");

    if response.candidates.is_empty() {
        println!("No suitable candidates found.");
        return;
    }

    println!(
        "Top candidates (operator fee: {}%):",
        (response.operator_fee_pct * 100.0) as u32
    );
    let ledger = CakeLedger::open().ok();
    for (i, agent) in response.candidates.iter().enumerate() {
        println!(
            "  {}. {} — skills: {:?}, cake: {}",
            i + 1,
            agent.id,
            agent.skills,
            live_cake_balance(ledger.as_ref(), agent)
        );
    }
}

fn handle_hire(store: &AgentStore, agent_id: &str, role: Option<&str>) {
    let target_role = match role {
        // "executor" is a deliberate CLI-input backward-compat alias for the old
        // b00t-c0re-hierarchy::Role::Executor, now KnownRole::worker() -- keep it.
        Some("worker") | Some("executor") => KnownRole::worker(),
        Some("specialist") => KnownRole::specialist(),
        _ => KnownRole::worker(),
    };

    // Update the agent's role and manager in the metadata
    update_meta(store, agent_id, |meta| {
        meta.role = target_role.clone();
        meta.manager_id = Some("captain".to_string());
    });

    println!("Hired {} as {}", agent_id, target_role);
}

fn handle_roster(store: &AgentStore) {
    let agents = all_agents(store);
    println!("Current roster ({} agents):", agents.len());
    let ledger = CakeLedger::open().ok();

    // Separate by role
    let mut executives = Vec::new();
    let mut workers = Vec::new();
    let mut operators = Vec::new();
    let mut specialists = Vec::new();

    for agent in &agents {
        match &agent.role {
            KnownRole::Executive(_) => executives.push(agent),
            KnownRole::Worker(_) => workers.push(agent),
            KnownRole::Operator(_) => operators.push(agent),
            KnownRole::Specialist(_) => specialists.push(agent),
        }
    }

    println!("  Executive:");
    if executives.is_empty() {
        println!("    you");
    } else {
        for a in &executives {
            println!("    {} (cake: {})", a.id, live_cake_balance(ledger.as_ref(), a));
        }
    }

    println!("  Workers:");
    if workers.is_empty() {
        println!("    (none)");
    } else {
        for a in &workers {
            let mgr = a.manager_id.as_deref().unwrap_or("none");
            println!(
                "    {} (manager: {}, cake: {})",
                a.id, mgr, live_cake_balance(ledger.as_ref(), a)
            );
        }
    }

    println!("  Operators:");
    if operators.is_empty() {
        println!("    (none)");
    } else {
        for a in &operators {
            let mgr = a.manager_id.as_deref().unwrap_or("none");
            println!(
                "    {} (manager: {}, cake: {})",
                a.id, mgr, live_cake_balance(ledger.as_ref(), a)
            );
        }
    }

    println!("  Specialists:");
    if specialists.is_empty() {
        println!("    (none)");
    } else {
        for a in &specialists {
            let mgr = a.manager_id.as_deref().unwrap_or("none");
            println!(
                "    {} (manager: {}, cake: {})",
                a.id, mgr, live_cake_balance(ledger.as_ref(), a)
            );
        }
    }
}
