use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role {
    Captain,
    Mate,
    Player,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub captain_id: String,
    pub mate_ids: Vec<String>,
    pub player_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionTopic {
    pub id: String,
    pub bounty: f64,
    pub description: String,
    pub required_skills: Vec<String>,
    pub status: TopicStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TopicStatus {
    Open,
    InProgress,
    Completed,
    Abandoned,
}
