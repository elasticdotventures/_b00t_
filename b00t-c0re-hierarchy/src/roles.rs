use serde::{Deserialize, Serialize};

/// Canonical crew roles matching the CREW-ROLES.tomllmd schema.
///
/// | Variant    | Authority | Description |
/// |------------|-----------|-------------|
/// | Captain    | Full command | Commands the crew, sets mission, delegates tasks |
/// | Executor   | Task execution | Runs autonomous task loops, executes backlog items |
/// | Operator   | Recruitment + training | Scouts/finds agents, enlists, executes training plans |
/// | Specialist | Domain expertise | Domain-specific work (coding, research, analysis) |
/// | Bouncer    | Quality gates | Validates inputs/outputs, enforces contracts, security |
///
/// ## Historical aliases
/// - **Mate** — any non-captain agent; conceptually maps to Executor or Specialist.
/// - **Player** — human/user; tracked via `Agent::is_player` / `Team::player_ids`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role {
    /// Captain — leader of a team, holds command authority
    Captain,
    /// Executor — runs autonomous task loops, executes backlog items
    Executor,
    /// Legacy alias kept for backward wire/storage compatibility.
    /// Prefer `Executor` or `Specialist` for new writes.
    Mate,
    /// Legacy human/user marker kept for backward wire/storage compatibility.
    /// Prefer `Agent::is_player` / `Team::player_ids` for new writes.
    Player,
    /// Operator — system operator with administrative privileges
    Operator,
    /// Specialist — domain-specific work (coding, research, analysis)
    Specialist,
    /// Bouncer — quality gates, security validation
    Bouncer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub role: Role,
    pub skills: Vec<String>,
    pub cake_balance: f64,
    pub is_alive: bool,
    pub manager_id: Option<String>, // Captain or Operator who recruited them
    pub is_player: bool,           // true if this Agent represents a human user
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub captain_id: String,
    pub executor_ids: Vec<String>,
    pub operator_ids: Vec<String>,
    pub specialist_ids: Vec<String>,
    pub bouncer_ids: Vec<String>,
    #[serde(default)]
    pub player_ids: Vec<String>, // human users (not agent software roles)
}

impl Team {
    /// Create a new Team with the given captain.
    pub fn new(captain_id: &str) -> Self {
        Self {
            captain_id: captain_id.to_string(),
            executor_ids: Vec::new(),
            operator_ids: Vec::new(),
            specialist_ids: Vec::new(),
            bouncer_ids: Vec::new(),
            player_ids: Vec::new(),
        }
    }

    /// Add an agent ID to the executor roster.
    pub fn add_executor(&mut self, id: &str) {
        if !self.executor_ids.iter().any(|m| m == id) {
            self.executor_ids.push(id.to_string());
        }
    }

    /// Remove an agent ID from the executor roster.
    pub fn remove_executor(&mut self, id: &str) {
        self.executor_ids.retain(|m| m != id);
    }

    /// Add an agent ID to the specialist roster.
    pub fn add_specialist(&mut self, id: &str) {
        if !self.specialist_ids.iter().any(|s| s == id) {
            self.specialist_ids.push(id.to_string());
        }
    }

    /// Remove an agent ID from the specialist roster.
    pub fn remove_specialist(&mut self, id: &str) {
        self.specialist_ids.retain(|s| s != id);
    }

    /// Add an agent ID to the operator roster.
    pub fn add_operator(&mut self, id: &str) {
        if !self.operator_ids.iter().any(|o| o == id) {
            self.operator_ids.push(id.to_string());
        }
    }

    /// Remove an agent ID from the operator roster.
    pub fn remove_operator(&mut self, id: &str) {
        self.operator_ids.retain(|o| o != id);
    }

    /// Add an agent ID to the bouncer roster.
    pub fn add_bouncer(&mut self, id: &str) {
        if !self.bouncer_ids.iter().any(|b| b == id) {
            self.bouncer_ids.push(id.to_string());
        }
    }

    /// Remove an agent ID from the bouncer roster.
    pub fn remove_bouncer(&mut self, id: &str) {
        self.bouncer_ids.retain(|b| b != id);
    }

    /// Add a player ID to the player roster (human user).
    pub fn add_player(&mut self, id: &str) {
        if !self.player_ids.iter().any(|p| p == id) {
            self.player_ids.push(id.to_string());
        }
    }

    /// Remove a player ID from the player roster.
    pub fn remove_player(&mut self, id: &str) {
        self.player_ids.retain(|p| p != id);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionTopic {
    pub id: String,
    pub bounty: f64,
    pub description: String,
    pub required_skills: Vec<String>,
    pub status: TopicStatus,
}

impl MissionTopic {
    /// Create a new MissionTopic with status `Open`.
    pub fn new(
        id: &str,
        bounty: f64,
        description: &str,
        required_skills: Vec<String>,
    ) -> Self {
        Self {
            id: id.to_string(),
            bounty,
            description: description.to_string(),
            required_skills,
            status: TopicStatus::Open,
        }
    }

    /// Transition the mission to `InProgress`.
    pub fn start(&mut self) {
        self.status = TopicStatus::InProgress;
    }

    /// Transition the mission to `Completed`.
    pub fn complete(&mut self) {
        self.status = TopicStatus::Completed;
    }

    /// Transition the mission to `Abandoned`.
    pub fn abandon(&mut self) {
        self.status = TopicStatus::Abandoned;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TopicStatus {
    Open,
    InProgress,
    Completed,
    Abandoned,
}
