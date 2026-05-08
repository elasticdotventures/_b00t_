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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role {
    Captain,
    Executor,
    Operator,
    Specialist,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Team {
    pub captain_id: String,
    pub executor_ids: Vec<String>,
    pub operator_ids: Vec<String>,
    pub specialist_ids: Vec<String>,
    pub bouncer_ids: Vec<String>,
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
