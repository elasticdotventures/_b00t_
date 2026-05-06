//! Handlers for crew subcommands — recruit, hire, roster

use b00t_c0re_hierarchy::recruitment::*;
use b00t_c0re_hierarchy::roles::*;

use crate::commands::crew::CrewCommand;

pub fn handle_crew_command(cmd: &CrewCommand) {
    match cmd {
        CrewCommand::Recruit { skills, limit } => handle_recruit(skills, *limit),
        CrewCommand::Hire { agent_id, role } => handle_hire(agent_id, role.as_deref()),
        CrewCommand::Roster => handle_roster(),
    }
}

fn handle_recruit(skills: &str, limit: usize) {
    let required: Vec<String> = skills.split(',').map(|s| s.trim().to_string()).collect();

    // For now, create some demo agents (in production, read from a store)
    let available = vec![
        Agent { id: "RustCoder".into(), role: Role::Player, skills: vec!["rust".into(), "typescript".into()], cake_balance: 100.0, is_alive: true, manager_id: None },
        Agent { id: "DataEngineer".into(), role: Role::Player, skills: vec!["python".into(), "sql".into(), "data-engineering".into()], cake_balance: 150.0, is_alive: true, manager_id: None },
        Agent { id: "DevOpsBot".into(), role: Role::Player, skills: vec!["docker".into(), "k8s".into(), "ci/cd".into()], cake_balance: 80.0, is_alive: true, manager_id: None },
    ];

    let request = RecruitRequest {
        captain_id: "captain".into(),
        required_skills: required,
        max_players: limit,
        bounty_share: 0.1,
    };

    let response = process_recruit_request(&request, &available);

    if response.candidates.is_empty() {
        println!("No suitable candidates found.");
        return;
    }

    println!("Top candidates (operator fee: {}%):", (response.operator_fee_pct * 100.0) as u32);
    for (i, agent) in response.candidates.iter().enumerate() {
        println!("  {}. {} — skills: {:?}, cake: {:.1}", i + 1, agent.id, agent.skills, agent.cake_balance);
    }
}

fn handle_hire(agent_id: &str, role: Option<&str>) {
    let target_role = match role {
        Some("mate") => Role::Mate,
        _ => Role::Player,
    };
    println!("Hired {} as {:?}", agent_id, target_role);
    // In production: update the agent store
}

fn handle_roster() {
    // In production: read from agent store
    println!("Current roster:");
    println!("  Captain: you");
    println!("  Mates: (none)");
    println!("  Players: RustCoder, DataEngineer, DevOpsBot");
}
