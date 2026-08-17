//! Recruitment flow: Captain requests → Operator searches → Captain hires

use serde::{Deserialize, Serialize};

use crate::roles::*;
use b00t_c0re_role::KnownRole;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecruitRequest {
    pub captain_id: String,
    pub required_skills: Vec<String>,
    pub max_players: usize,
    pub bounty_share: f64, // % of mission bounty offered to each player
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecruitResponse {
    pub candidates: Vec<Agent>,
    pub operator_id: String,
    pub operator_fee_pct: f64, // 20%
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HireAction {
    pub captain_id: String,
    pub agent_id: String,
    /// Expected hireable role for the recruited agent.
    /// Recruitment normally assigns operational roles such as
    /// `KnownRole::worker()` or `KnownRole::specialist()`; callers should
    /// avoid passing unrelated roles unless the processing path explicitly
    /// supports them.
    pub role: KnownRole,
}

/// Score an agent's skill match against required skills.
/// Returns the number of matching skills.
fn skill_match_score(agent: &Agent, required_skills: &[String]) -> usize {
    agent
        .skills
        .iter()
        .filter(|s| required_skills.contains(s))
        .count()
}

/// Process a recruit request:
/// 1. Search available agents by skill match
/// 2. Rank by relevance
/// 3. Return top candidates
pub fn process_recruit_request(
    request: &RecruitRequest,
    available_agents: &[Agent],
    operator_id: &str,
) -> RecruitResponse {
    // Score and collect candidates that have at least one matching skill
    let mut scored: Vec<(usize, &Agent)> = available_agents
        .iter()
        .filter(|a| a.is_alive)
        .map(|a| {
            let score = skill_match_score(a, &request.required_skills);
            (score, a)
        })
        .filter(|(score, _)| *score > 0)
        .collect();

    // Sort by score descending (highest match first)
    scored.sort_by(|a, b| b.0.cmp(&a.0));

    // Take top candidates up to max_players
    let candidates: Vec<Agent> = scored
        .into_iter()
        .take(request.max_players)
        .map(|(_, agent)| agent.clone())
        .collect();

    RecruitResponse {
        candidates,
        operator_id: operator_id.to_string(),
        operator_fee_pct: 20.0,
    }
}

/// Hire an agent, updating their role and manager.
pub fn hire_agent(agent: &mut Agent, action: &HireAction) {
    agent.role = action.role.clone();
    agent.manager_id = Some(action.captain_id.clone());
}

/// Check if an agent is dead.
pub fn is_dead(agent: &Agent) -> bool {
    !agent.is_alive
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent(id: &str, role: KnownRole, skills: &[&str], alive: bool) -> Agent {
        Agent {
            id: id.to_string(),
            role,
            skills: skills.iter().map(|s| s.to_string()).collect(),
            cake_balance: 100.0,
            is_alive: alive,
            manager_id: None,
            is_player: false,
        }
    }

    #[test]
    fn test_skill_match_score() {
        let agent = make_agent("a1", KnownRole::worker(), &["rust", "python", "docker"], true);
        let required = vec!["rust".to_string(), "kubernetes".to_string()];
        assert_eq!(skill_match_score(&agent, &required), 1);
    }

    #[test]
    fn test_process_recruit_request_ranks_by_skill() {
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
            make_agent("a1", KnownRole::worker(), &["rust"], true),
            make_agent("a2", KnownRole::worker(), &["rust", "python", "docker"], true),
            make_agent("a3", KnownRole::worker(), &["python", "docker"], true),
        ];

        let response = process_recruit_request(&request, &agents, "op1");

        assert_eq!(response.candidates.len(), 2);
        assert_eq!(response.candidates[0].id, "a2");
        assert_eq!(response.candidates[1].id, "a3");
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
            make_agent("a1", KnownRole::worker(), &["rust"], true),
            make_agent("a2", KnownRole::worker(), &["python"], true),
        ];

        let response = process_recruit_request(&request, &agents, "op1");
        assert!(response.candidates.is_empty());
    }

    #[test]
    fn test_hire_updates_role_and_manager() {
        let mut agent = make_agent("a1", KnownRole::worker(), &["rust"], true);
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
    fn test_dead_agents_not_recruited() {
        let request = RecruitRequest {
            captain_id: "cap1".to_string(),
            required_skills: vec!["rust".to_string()],
            max_players: 5,
            bounty_share: 10.0,
        };

        let agents = vec![
            make_agent("a1", KnownRole::worker(), &["rust"], false), // dead
            make_agent("a2", KnownRole::worker(), &["rust"], true),  // alive
        ];

        let response = process_recruit_request(&request, &agents, "op1");
        assert_eq!(response.candidates.len(), 1);
        assert_eq!(response.candidates[0].id, "a2");
    }

    #[test]
    fn test_operator_id_is_set_on_response() {
        let request = RecruitRequest {
            captain_id: "cap1".to_string(),
            required_skills: vec!["rust".to_string()],
            max_players: 2,
            bounty_share: 10.0,
        };

        let agents = vec![make_agent("a1", KnownRole::specialist(), &["rust"], true)];

        let response = process_recruit_request(&request, &agents, "operator-42");
        assert_eq!(response.operator_id, "operator-42");
    }
}
