use b00t_c0re_hierarchy::recruitment::*;
use b00t_c0re_hierarchy::roles::*;

fn make_agent(id: &str, role: Role, skills: &[&str], alive: bool) -> Agent {
    let is_player = matches!(role, Role::Player);
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
fn test_captain_creates_team() {
    let captain = make_agent("cap1", Role::Captain, &["leadership"], true);
    let mate = make_agent("mate1", Role::Mate, &["navigation"], true);
    let player = make_agent("p1", Role::Player, &["rust"], true);

    let mut team = Team::new(&captain.id);
    team.add_mate(&mate.id);
    team.add_player(&player.id);

    assert_eq!(team.captain_id, "cap1");
    assert_eq!(team.mate_ids.len(), 1);
    assert_eq!(team.player_ids.len(), 1);
    assert!(team.mate_ids.contains(&"mate1".to_string()));
    assert!(team.player_ids.contains(&"p1".to_string()));
}

#[test]
fn test_team_add_remove_mate() {
    let mut team = Team::new("cap1");

    team.add_mate("mate1");
    team.add_mate("mate2");
    assert_eq!(team.mate_ids.len(), 2);

    // Duplicate add should not increase count
    team.add_mate("mate1");
    assert_eq!(team.mate_ids.len(), 2);

    team.remove_mate("mate1");
    assert_eq!(team.mate_ids.len(), 1);
    assert_eq!(team.mate_ids[0], "mate2");
}

#[test]
fn test_team_add_remove_player() {
    let mut team = Team::new("cap1");

    team.add_player("p1");
    team.add_player("p2");
    assert_eq!(team.player_ids.len(), 2);

    team.remove_player("p1");
    assert_eq!(team.player_ids.len(), 1);
    assert_eq!(team.player_ids[0], "p2");
}

#[test]
fn test_mission_topic_lifecycle() {
    let mut mission = MissionTopic::new(
        "m1",
        500.0,
        "Build the navigation module",
        vec!["rust".to_string(), "navigation".to_string()],
    );

    assert_eq!(mission.status, TopicStatus::Open);
    assert_eq!(mission.bounty, 500.0);
    assert_eq!(mission.description, "Build the navigation module");
    assert_eq!(mission.required_skills.len(), 2);

    mission.start();
    assert_eq!(mission.status, TopicStatus::InProgress);

    mission.complete();
    assert_eq!(mission.status, TopicStatus::Completed);
}

#[test]
fn test_mission_abandon() {
    let mut mission = MissionTopic::new("m2", 250.0, "Scout the perimeter", vec!["stealth".to_string()]);
    mission.start();
    mission.abandon();
    assert_eq!(mission.status, TopicStatus::Abandoned);
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
        make_agent("a1", Role::Player, &["rust"], true),
        make_agent("a2", Role::Player, &["rust", "python", "docker"], true),
        make_agent("a3", Role::Player, &["python", "docker"], true),
    ];

    let response = process_recruit_request(&request, &agents, "op1");

    assert_eq!(response.candidates.len(), 2);
    // a2 has 3 matching skills, a3 has 2 — a2 should be first
    assert_eq!(response.candidates[0].id, "a2");
    assert_eq!(response.candidates[1].id, "a3");
    assert_eq!(response.operator_fee_pct, 20.0);
}

#[test]
fn test_hire_updates_role() {
    let mut agent = make_agent("a1", Role::Player, &["rust"], true);
    let action = HireAction {
        captain_id: "cap1".to_string(),
        agent_id: "a1".to_string(),
        role: Role::Mate,
    };

    hire_agent(&mut agent, &action);
    assert_eq!(agent.role, Role::Mate);
    assert_eq!(agent.manager_id, Some("cap1".to_string()));
}

#[test]
fn test_agent_death_detection() {
    let alive = make_agent("a1", Role::Player, &["rust"], true);
    let dead = make_agent("a2", Role::Player, &["rust"], false);

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
        make_agent("a1", Role::Player, &["rust"], true),
        make_agent("a2", Role::Player, &["python"], true),
    ];

    let response = process_recruit_request(&request, &agents, "op1");
    assert!(response.candidates.is_empty());
    assert_eq!(response.operator_id, "op1");
}
