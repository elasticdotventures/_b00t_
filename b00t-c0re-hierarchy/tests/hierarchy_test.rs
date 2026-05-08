use b00t_c0re_hierarchy::recruitment::*;
use b00t_c0re_hierarchy::roles::*;

fn make_agent(id: &str, role: Role, skills: &[&str], alive: bool) -> Agent {
    Agent {
        id: id.to_string(),
        role,
        skills: skills.iter().map(|s| s.to_string()).collect(),
        cake_balance: 100.0,
        is_alive: alive,
        manager_id: None,
    }
}

#[test]
fn test_captain_creates_team() {
    let captain = make_agent("cap1", Role::Captain, &["leadership"], true);
    let specialist = make_agent("spec1", Role::Specialist, &["navigation"], true);
    let executor = make_agent("ex1", Role::Executor, &["rust"], true);

    let team = Team {
        captain_id: captain.id.clone(),
        executor_ids: vec![executor.id.clone()],
        specialist_ids: vec![specialist.id.clone()],
        operator_ids: vec![],
        bouncer_ids: vec![],
    };

    assert_eq!(team.captain_id, "cap1");
    assert_eq!(team.executor_ids.len(), 1);
    assert_eq!(team.specialist_ids.len(), 1);
}

#[test]
fn test_recruit_request_ranks_by_skill() {
    let request = RecruitRequest {
        captain_id: "cap1".to_string(),
        required_skills: vec!["rust".to_string(), "python".to_string(), "docker".to_string()],
        max_players: 2,
        bounty_share: 10.0,
    };

    let agents = vec![
        make_agent("a1", Role::Executor, &["rust"], true),
        make_agent("a2", Role::Executor, &["rust", "python", "docker"], true),
        make_agent("a3", Role::Executor, &["python", "docker"], true),
    ];

    let response = process_recruit_request(&request, &agents);

    assert_eq!(response.candidates.len(), 2);
    // a2 has 3 matching skills, a3 has 2 — a2 should be first
    assert_eq!(response.candidates[0].id, "a2");
    assert_eq!(response.candidates[1].id, "a3");
    assert_eq!(response.operator_fee_pct, 20.0);
}

#[test]
fn test_hire_updates_role() {
    let mut agent = make_agent("a1", Role::Executor, &["rust"], true);
    let action = HireAction {
        captain_id: "cap1".to_string(),
        agent_id: "a1".to_string(),
        role: Role::Specialist,
    };

    hire_agent(&mut agent, &action);
    assert_eq!(agent.role, Role::Specialist);
    assert_eq!(agent.manager_id, Some("cap1".to_string()));
}

#[test]
fn test_agent_death_detection() {
    let alive = make_agent("a1", Role::Executor, &["rust"], true);
    let dead = make_agent("a2", Role::Executor, &["rust"], false);

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
        make_agent("a1", Role::Executor, &["rust"], true),
        make_agent("a2", Role::Executor, &["python"], true),
    ];

    let response = process_recruit_request(&request, &agents);
    assert!(response.candidates.is_empty());
}
