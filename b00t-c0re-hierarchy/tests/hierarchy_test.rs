use b00t_c0re_hierarchy::recruitment::*;
use b00t_c0re_hierarchy::roles::*;
use b00t_c0re_role::KnownRole;
use serde_json::json;

fn make_agent(id: &str, role: KnownRole, skills: &[&str], alive: bool, is_player: bool) -> Agent {
    Agent {
        id: id.to_string(),
        role,
        skills: skills.iter().map(|s| s.to_string()).collect(),
        cake_balance: 100.0,
        is_alive: alive,
        manager_id: None,
        is_player,
    }
}

#[test]
fn test_executive_creates_team() {
    let executive = make_agent("exec1", KnownRole::executive(), &["leadership"], true, false);
    let specialist = make_agent("spec1", KnownRole::specialist(), &["navigation"], true, false);
    let worker = make_agent("w1", KnownRole::worker(), &["rust"], true, false);

    let mut team = Team::new(&executive.id);
    team.add_specialist(&specialist.id);
    team.add_worker(&worker.id);

    assert_eq!(team.executive_id, "exec1");
    assert_eq!(team.specialist_ids.len(), 1);
    assert_eq!(team.worker_ids.len(), 1);
}

#[test]
fn test_recruit_request_ranks_by_skill() {
    let request = RecruitRequest {
        captain_id: "cap1".to_string(),
        required_skills: vec![
            "rust".to_string(),
            "python".to_string(),
            "docker".to_string(),
        ],
        max_players: 2,
        bounty_share: 10.0,
    };

    let agents = vec![
        make_agent("a1", KnownRole::worker(), &["rust"], true, false),
        make_agent(
            "a2",
            KnownRole::worker(),
            &["rust", "python", "docker"],
            true,
            false,
        ),
        make_agent("a3", KnownRole::worker(), &["python", "docker"], true, false),
    ];

    let response = process_recruit_request(&request, &agents, "op1");

    assert_eq!(response.candidates.len(), 2);
    assert_eq!(response.candidates[0].id, "a2");
    assert_eq!(response.candidates[1].id, "a3");
    assert_eq!(response.operator_fee_pct, 20.0);
}

#[test]
fn test_hire_updates_role() {
    let mut agent = make_agent("a1", KnownRole::worker(), &["rust"], true, false);
    let action = HireAction {
        captain_id: "cap1".to_string(),
        agent_id: "a1".to_string(),
        role: KnownRole::specialist(),
    };

    hire_agent(&mut agent, &action);
    assert_eq!(agent.role, KnownRole::specialist());
    assert_eq!(agent.manager_id, Some("cap1".to_string()));
}

#[test]
fn test_agent_death_detection() {
    let alive = make_agent("a1", KnownRole::worker(), &["rust"], true, false);
    let dead = make_agent("a2", KnownRole::worker(), &["rust"], false, false);

    assert!(!is_dead(&alive));
    assert!(is_dead(&dead));
}

#[test]
fn test_recruit_no_candidates_returns_empty() {
    let request = RecruitRequest {
        captain_id: "cap1".to_string(),
        required_skills: vec!["java".to_string(), "scala".to_string()],
        max_players: 3,
        bounty_share: 10.0,
    };

    let agents = vec![
        make_agent("a1", KnownRole::worker(), &["rust"], true, false),
        make_agent("a2", KnownRole::worker(), &["python"], true, false),
    ];

    let response = process_recruit_request(&request, &agents, "op1");
    assert!(response.candidates.is_empty());
}

#[test]
fn test_team_deserializes_without_player_ids() {
    let legacy_team = json!({
        "executive_id": "exec1",
        "worker_ids": [],
        "operator_ids": [],
        "specialist_ids": []
    });

    let team: Team = serde_json::from_value(legacy_team).expect("legacy payload must deserialize");
    assert!(team.player_ids.is_empty());
}
