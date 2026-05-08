use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role {
    /// Captain — leader of a team, holds command authority
    Captain,
    /// Mate = any non-captain agent (assistant, executive, specialist)
    Mate,
    /// Player = human/user (not an agent software role)
    Player,
    /// Operator — system operator with administrative privileges
    Operator,
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
    pub mate_ids: Vec<String>,
    pub player_ids: Vec<String>,
}

impl Team {
    /// Create a new Team with the given captain.
    /// Mate and player lists start empty.
    pub fn new(captain_id: &str) -> Self {
        Self {
            captain_id: captain_id.to_string(),
            mate_ids: Vec::new(),
            player_ids: Vec::new(),
        }
    }

    /// Add an agent ID to the mate roster.
    pub fn add_mate(&mut self, id: &str) {
        if !self.mate_ids.iter().any(|m| m == id) {
            self.mate_ids.push(id.to_string());
        }
    }

    /// Remove an agent ID from the mate roster.
    pub fn remove_mate(&mut self, id: &str) {
        self.mate_ids.retain(|m| m != id);
    }

    /// Add a player ID to the player roster.
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
