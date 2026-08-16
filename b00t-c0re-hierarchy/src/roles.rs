use b00t_c0re_role::KnownRole;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub role: KnownRole,
    pub skills: Vec<String>,
    pub cake_balance: f64,
    pub is_alive: bool,
    pub manager_id: Option<String>, // Executive or Operator who recruited them
    pub is_player: bool,            // true if this Agent represents a human user
}

/// First behavioral use of `is_player` in this codebase — previously set at
/// construction but never read. Lets hive messaging (`b00t-council`) tag
/// traffic as human- vs. software-originated.
impl b00t_council::Player for Agent {
    fn player_id(&self) -> &str {
        &self.id
    }

    fn is_human(&self) -> bool {
        self.is_player
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub executive_id: String,
    pub worker_ids: Vec<String>,
    pub operator_ids: Vec<String>,
    pub specialist_ids: Vec<String>,
    #[serde(default)]
    pub player_ids: Vec<String>, // human users (not agent software roles)
}

impl Team {
    /// Create a new Team with the given executive.
    pub fn new(executive_id: &str) -> Self {
        Self {
            executive_id: executive_id.to_string(),
            worker_ids: Vec::new(),
            operator_ids: Vec::new(),
            specialist_ids: Vec::new(),
            player_ids: Vec::new(),
        }
    }

    /// Add an agent ID to the worker roster.
    pub fn add_worker(&mut self, id: &str) {
        if !self.worker_ids.iter().any(|m| m == id) {
            self.worker_ids.push(id.to_string());
        }
    }

    /// Remove an agent ID from the worker roster.
    pub fn remove_worker(&mut self, id: &str) {
        self.worker_ids.retain(|m| m != id);
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
    pub fn new(id: &str, bounty: f64, description: &str, required_skills: Vec<String>) -> Self {
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
