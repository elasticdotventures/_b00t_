//! Integration tests for reputation tracking and recruitment scoring.
//!
//! Tests cover:
//! - AgentReputation lifecycle (completions, abandons, ballast, endorsements)
//! - Recruitment scoring with 70% skill + 30% reputation blend
//! - Ranking of agents by combined score

use b00t_c0re_a2a::agent_card::{AgentCard, AgentReputation, Skill};
use b00t_c0re_a2a::recruitment::{rank_agents, score_agent_for_skills};
use url::Url;

fn make_agent(name: &str, skill_ids: &[&str], reputation: AgentReputation) -> AgentCard {
    let url = Url::parse(&format!("stdio://{}", name)).unwrap();
    let mut card = AgentCard::new_with_reputation(name, "", url, reputation);
    for skill_id in skill_ids {
        card = card.with_skill(Skill::new(
            skill_id,
            skill_id,
            "",
            serde_json::json!({}),
            serde_json::json!({}),
        ));
    }
    card
}

// ---------------------------------------------------------------------------
// Reputation lifecycle tests
// ---------------------------------------------------------------------------

#[test]
fn test_reputation_from_completions() {
    let mut rep = AgentReputation::new();
    assert_eq!(rep.score, 0.0);
    assert_eq!(rep.missions_completed, 0);

    rep.record_completion();
    assert_eq!(rep.missions_completed, 1);
    assert!((rep.score - 1.0).abs() < 1e-6);

    // Push to the limit
    for _ in 0..9 {
        rep.record_completion();
    }
    assert_eq!(rep.missions_completed, 10);
    assert!((rep.score - 10.0).abs() < 1e-6, "score should cap at 10.0");

    // Verify clamping
    rep.record_completion();
    assert!((rep.score - 10.0).abs() < 1e-6, "score should not exceed 10.0");
}

#[test]
fn test_reputation_from_abandons() {
    let mut rep = AgentReputation::new();

    rep.record_abandon();
    assert_eq!(rep.missions_abandoned, 1);
    assert!((rep.score - (-2.0)).abs() < 1e-6);

    // Push to the floor
    for _ in 0..4 {
        rep.record_abandon();
    }
    assert_eq!(rep.missions_abandoned, 5);
    assert!((rep.score - (-10.0)).abs() < 1e-6, "score should floor at -10.0");

    // Verify clamping
    rep.record_abandon();
    assert!((rep.score - (-10.0)).abs() < 1e-6, "score should not go below -10.0");
}

#[test]
fn test_ballast_penalty() {
    let mut rep = AgentReputation::new();
    rep.record_ballast();
    assert_eq!(rep.times_ballast, 1);
    assert!((rep.score - (-3.0)).abs() < 1e-6);

    rep.record_ballast();
    assert_eq!(rep.times_ballast, 2);
    assert!((rep.score - (-6.0)).abs() < 1e-6);
}

#[test]
fn test_captain_endorsement() {
    let mut rep = AgentReputation::new();
    rep.endorse();
    assert_eq!(rep.captain_endorsements, 1);
    assert!((rep.score - 1.5).abs() < 1e-6);

    rep.endorse();
    assert_eq!(rep.captain_endorsements, 2);
    assert!((rep.score - 3.0).abs() < 1e-6);
}

#[test]
fn test_mixed_reputation_events() {
    let mut rep = AgentReputation::new();

    // Complete a few missions
    rep.record_completion(); // +1.0 => 1.0
    rep.record_completion(); // +1.0 => 2.0
    rep.record_abandon();    // -2.0 => 0.0
    rep.endorse();           // +1.5 => 1.5
    rep.record_ballast();    // -3.0 => -1.5

    assert_eq!(rep.missions_completed, 2);
    assert_eq!(rep.missions_abandoned, 1);
    assert_eq!(rep.times_ballast, 1);
    assert_eq!(rep.captain_endorsements, 1);
    assert!((rep.score - (-1.5)).abs() < 1e-6);

    // Recruitment weight: (-1.5 + 10.0) / 20.0 = 8.5 / 20.0 = 0.425
    assert!((rep.recruitment_weight() - 0.425).abs() < 1e-6);
}

// ---------------------------------------------------------------------------
// Recruitment scoring tests
// ---------------------------------------------------------------------------

#[test]
fn test_recruitment_weights_skill_above_reputation() {
    // Agent with all skills matched but low reputation (0.7 skill * 0.7 + 0.0 * 0.3 = 0.49... wait)
    // Let me recalculate more carefully

    // Agent A: perfect skill match (2/2), -10 reputation => weight = 0.0
    // Score: 1.0 * 0.7 + 0.0 * 0.3 = 0.7
    let mut bad_rep = AgentReputation::new();
    bad_rep.score = -10.0;
    let agent_skilled = make_agent("skilled", &["ping", "pong"], bad_rep);

    // Agent B: no skill match (0/2), +10 reputation => weight = 1.0
    // Score: 0.0 * 0.7 + 1.0 * 0.3 = 0.3
    let mut good_rep = AgentReputation::new();
    good_rep.score = 10.0;
    let agent_reputed = make_agent("reputed", &[], good_rep);

    let required = vec!["ping".to_string(), "pong".to_string()];

    let score_skilled = score_agent_for_skills(&agent_skilled, &required);
    let score_reputed = score_agent_for_skills(&agent_reputed, &required);

    // Skill match dominates (0.7 vs 0.3 blend)
    assert!(
        score_skilled > score_reputed,
        "Skill match should outweigh reputation: skilled={score_skilled}, reputed={score_reputed}"
    );
    assert!((score_skilled - 0.7).abs() < 1e-6);
    assert!((score_reputed - 0.3).abs() < 1e-6);
}

#[test]
fn test_high_reputation_outranks_partial_skill_match() {
    // Agent A: 2/3 skills matched, neutral reputation (0.5)
    // Score: 0.6667 * 0.7 + 0.5 * 0.3 = 0.4667 + 0.15 = 0.6167
    let agent_a = make_agent("agent-a", &["ping", "pong"], AgentReputation::new());

    // Agent B: 1/3 skills matched, max reputation (1.0)
    // Score: 0.3333 * 0.7 + 1.0 * 0.3 = 0.2333 + 0.3 = 0.5333
    let mut rep_b = AgentReputation::new();
    rep_b.score = 10.0;
    let agent_b = make_agent("agent-b", &["ping"], rep_b);

    // Agent C: 3/3 skills matched, min reputation (0.0)
    // Score: 1.0 * 0.7 + 0.0 * 0.3 = 0.7
    let mut rep_c = AgentReputation::new();
    rep_c.score = -10.0;
    let agent_c = make_agent("agent-c", &["ping", "pong", "code-gen"], rep_c);

    let required = vec![
        "ping".to_string(),
        "pong".to_string(),
        "code-gen".to_string(),
    ];

    let agents = vec![agent_a.clone(), agent_b.clone(), agent_c.clone()];
    let ranked = rank_agents(&agents, &required);

    // Should be sorted descending
    assert_eq!(ranked.len(), 3);
    for i in 0..ranked.len() - 1 {
        assert!(
            ranked[i].1 >= ranked[i + 1].1,
            "Ranking should be descending at position {i}: {:?} vs {:?}",
            ranked[i].1,
            ranked[i + 1].1
        );
    }

    // Agent C (3/3 skills matched) should rank highest despite terrible reputation
    assert_eq!(ranked[0].0.name, "agent-c", "Full skill match should win");
    assert!((ranked[0].1 - 0.7).abs() < 1e-6);

    // Agent A (2/3 skills, neutral) should rank above Agent B (1/3 skills, max rep)
    // A: 0.6167 > B: 0.5333
    let pos_a = ranked.iter().position(|(a, _)| a.name == "agent-a").unwrap();
    let pos_b = ranked.iter().position(|(a, _)| a.name == "agent-b").unwrap();
    assert!(
        pos_a < pos_b,
        "Agent A (2/3 skills, neutral rep) should outrank Agent B (1/3 skills, max rep)"
    );
}

#[test]
fn test_rank_agents_empty_list() {
    let agents: Vec<AgentCard> = vec![];
    let required = vec!["ping".to_string()];
    let ranked = rank_agents(&agents, &required);
    assert!(ranked.is_empty());
}

#[test]
fn test_rank_agents_no_required_skills() {
    let agent = make_agent("agent-x", &["ping"], AgentReputation::new());
    let required: Vec<String> = vec![];
    let agent_list = vec![agent];
    let ranked = rank_agents(&agent_list, &required);
    assert_eq!(ranked.len(), 1);
    // score = 0.0 * 0.7 + 0.5 * 0.3 = 0.15
    assert!((ranked[0].1 - 0.15).abs() < 1e-6);
}

#[test]
fn test_agent_card_with_reputation() {
    let mut rep = AgentReputation::new();
    rep.record_completion();

    let url = Url::parse("stdio://rep-agent").unwrap();
    let card = AgentCard::new_with_reputation("rep-agent", "Has reputation", url, rep.clone());

    assert_eq!(card.reputation.missions_completed, 1);
    assert_eq!(card.reputation.score, 1.0);
}

#[test]
fn test_default_reputation_in_agent_card() {
    let url = Url::parse("stdio://default-agent").unwrap();
    let card = AgentCard::new("default-agent", "Default reputation", url);

    assert_eq!(card.reputation.score, 0.0);
    assert_eq!(card.reputation.missions_completed, 0);
    assert_eq!(card.reputation.recruitment_weight(), 0.5);
}
