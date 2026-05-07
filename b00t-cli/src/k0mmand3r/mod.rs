// b00t-cli/src/k0mmand3r/mod.rs
// k0mmand3r: agent coordination protocol for step guards + authorization

use b00t_ipc::VoteChoice;
use opentelemetry::trace::{Span, Tracer};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// Re-export emoji registry from k0mmand3r crate for compile-time datum embedding
pub use k0mmand3r::emoji_registry;

/// Legacy k0mmand3r command: slash command for agent coordination
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct K0mmand {
    pub verb: String,   // negotiate, vote, delegate, status, etc.
    pub object: String, // blessing:name, step:name, agent:name
    pub modifiers: BTreeMap<String, String>,
}

impl K0mmand {
    /// Parse k0mmand3r command string
    /// Examples:
    ///   /negotiate blessing:observe-infrastructure
    ///   /vote on blessing:execute-transition-safely
    ///   /delegate step:apply-transitions to agent:b00t-sandbox
    ///   /status from agent:executive
    pub fn parse(cmd: &str) -> Result<Self, String> {
        let cmd = cmd.trim();

        if !cmd.starts_with('/') {
            return Err("k0mmand3r command must start with /".to_string());
        }

        let parts: Vec<&str> = cmd[1..].split_whitespace().collect();

        if parts.is_empty() {
            return Err("Empty command".to_string());
        }

        let verb = parts[0].to_lowercase();
        let mut modifiers = BTreeMap::new();
        let mut object = String::new();

        let mut i = 1;

        // Locate object as the first token that looks like type:id (e.g. blessing:name)
        for token in parts.iter().skip(1) {
            if token.contains(':') {
                object = (*token).to_string();
                break;
            }
        }

        // Parse modifiers (key value, on <target>, to <target>, from <source>)
        while i < parts.len() {
            let key = parts[i];

            match key {
                "on" | "to" | "from" | "with" => {
                    if i + 1 < parts.len() {
                        let value = parts[i + 1..].join(" ");
                        modifiers.insert(key.to_string(), value);
                        break;
                    }
                }
                _ => {
                    // Might be key=value or key:value
                    if let Some((k, v)) = key.split_once('=') {
                        modifiers.insert(k.to_string(), v.to_string());
                    } else if let Some((k, v)) = key.split_once(':') {
                        modifiers.insert(k.to_string(), v.to_string());
                    }
                }
            }

            i += 1;
        }

        Ok(K0mmand {
            verb,
            object,
            modifiers,
        })
    }

    /// Validate k0mmand3r syntax
    pub fn validate(&self) -> Result<(), String> {
        match self.verb.as_str() {
            "negotiate" | "vote" | "delegate" | "status" | "handshake" | "crew" => Ok(()),
            _ => Err(format!("Unknown k0mmand3r verb: {}", self.verb)),
        }
    }
}

// ============== Typed Structural Commands ==============

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum K0mmand3rCmd {
    Negotiate {
        resource: String,
        id: String,
        modifiers: BTreeMap<String, String>,
    },
    Vote {
        proposal: String,
        choice: VoteChoice,
        reason: Option<String>,
    },
    Delegate {
        agent: String,
        budget: u64,
    },
    Loop {
        spec: LoopSpec,
    },
    Handshake {
        agent: String,
        proposal: Option<String>,
    },
    Crew {
        action: CrewAction,
        members: Vec<String>,
    },
    Status,
    Propose {
        description: String,
    },
    Ahoy {
        role: String,
        budget: u64,
        skills: Vec<String>,
        description: String,
    },
    Apply {
        ahoy_id: String,
        pitch: String,
    },
    Award {
        ahoy_id: String,
        winner: String,
    },
    Unknown {
        raw: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrewAction {
    Form,
    Join,
    Leave,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoopSpec {
    pub goal: String,
    pub metric: String,
    pub verify: String,
    pub guard: Option<String>,
    pub max: Option<u32>,
    pub scope: Option<String>,
    pub direction: Option<String>,
}

impl LoopSpec {
    pub fn from_tokens(
        positional: &[&str],
        modifiers: &BTreeMap<String, String>,
    ) -> Result<Self, String> {
        if let Some(raw) = positional.first() {
            let mut spec = LoopSpec {
                goal: String::new(),
                metric: String::new(),
                verify: String::new(),
                guard: None,
                max: None,
                scope: None,
                direction: None,
            };
            for segment in raw.split('|') {
                if let Some((k, v)) = segment.split_once(':') {
                    match k {
                        "goal" => spec.goal = v.to_string(),
                        "metric" => spec.metric = v.to_string(),
                        "verify" => spec.verify = v.to_string(),
                        "guard" => spec.guard = Some(v.to_string()),
                        "max" => spec.max = v.parse().ok(),
                        "scope" => spec.scope = Some(v.to_string()),
                        "direction" => spec.direction = Some(v.to_string()),
                        _ => {}
                    }
                }
            }
            if spec.goal.is_empty() {
                return Err("Loop spec requires 'goal'".to_string());
            }
            Ok(spec)
        } else {
            let goal = modifiers
                .get("goal")
                .cloned()
                .ok_or("Loop spec requires 'goal'")?;
            Ok(LoopSpec {
                goal,
                metric: modifiers.get("metric").cloned().unwrap_or_default(),
                verify: modifiers.get("verify").cloned().unwrap_or_default(),
                guard: modifiers.get("guard").cloned(),
                max: modifiers.get("max").and_then(|m| m.parse().ok()),
                scope: modifiers.get("scope").cloned(),
                direction: modifiers.get("direction").cloned(),
            })
        }
    }
}

impl K0mmand3rCmd {
    pub fn parse(cmd: &str) -> Result<Self, String> {
        let cmd = cmd.trim();
        if !cmd.starts_with('/') {
            return Err("k0mmand3r command must start with /".to_string());
        }
        let parts: Vec<&str> = cmd[1..].split_whitespace().collect();
        if parts.is_empty() {
            return Err("Empty command".to_string());
        }
        let verb = parts[0].to_lowercase();
        let mut modifiers = BTreeMap::new();
        let mut positional = Vec::new();
        let mut i = 1;
        while i < parts.len() {
            let token = parts[i];
            match token {
                "on" | "to" | "from" | "with" => {
                    if i + 1 < parts.len() {
                        let value = parts[i + 1..].join(" ");
                        modifiers.insert(token.to_string(), value);
                        break;
                    } else {
                        return Err(format!("Expected value after {}", token));
                    }
                }
                _ => {
                    if let Some((k, v)) = token.split_once(':') {
                        modifiers.insert(k.to_string(), v.to_string());
                    } else if let Some((k, v)) = token.split_once('=') {
                        modifiers.insert(k.to_string(), v.to_string());
                    } else {
                        positional.push(token);
                    }
                }
            }
            i += 1;
        }

        // Expand certain keyword modifiers back into individual k=v pairs
        // for typed commands that need granular access.
        // e.g. "with goal=deploy metric=uptime" → {"goal": "deploy", "metric": "uptime"}
        if let Some(with_val) = modifiers.remove("with") {
            for segment in with_val.split_whitespace() {
                if let Some((k, v)) = segment.split_once('=') {
                    modifiers.insert(k.to_string(), v.to_string());
                } else if let Some((k, v)) = segment.split_once(':') {
                    modifiers.insert(k.to_string(), v.to_string());
                }
            }
        }

        match verb.as_str() {
            "negotiate" => {
                if positional.len() >= 2 {
                    Ok(K0mmand3rCmd::Negotiate {
                        resource: positional[0].to_string(),
                        id: positional[1].to_string(),
                        modifiers,
                    })
                } else if let Some(id) = modifiers.remove("blessing") {
                    Ok(K0mmand3rCmd::Negotiate {
                        resource: "blessing".to_string(),
                        id,
                        modifiers,
                    })
                } else if let Some(id) = modifiers.remove("resource") {
                    Ok(K0mmand3rCmd::Negotiate {
                        resource: "resource".to_string(),
                        id,
                        modifiers,
                    })
                } else {
                    Err("Usage: /negotiate <resource> <id> or /negotiate blessing:<id>".to_string())
                }
            }
            "vote" => {
                let on_value = modifiers.remove("on");
                let proposal = positional
                    .first()
                    .map(|s| s.to_string())
                    .or_else(|| modifiers.remove("proposal"))
                    .or_else(|| {
                        on_value.as_ref().and_then(|v| v.split_whitespace().next().map(|s| s.to_string()))
                    })
                    .ok_or("Usage: /vote <proposal> <yes|no|abstain>")?;
                let choice_str = positional
                    .get(1)
                    .map(|s| s.to_string())
                    .or_else(|| modifiers.remove("choice"))
                    .or_else(|| {
                        on_value.as_ref().and_then(|v| {
                            let parts: Vec<&str> = v.split_whitespace().collect();
                            // Look for "choice" keyword in the on-value: "proposal-456 choice abstain"
                            let choice_idx = parts.iter().position(|&p| p == "choice");
                            if let Some(idx) = choice_idx {
                                parts.get(idx + 1).map(|s| s.to_string())
                            } else if parts.len() >= 2 {
                                // No "choice" keyword, try second token as choice
                                Some(parts[1].to_string())
                            } else {
                                None
                            }
                        })
                    })
                    .ok_or("Missing vote choice")?;
                let choice = match choice_str.to_lowercase().as_str() {
                    "yes" | "y" => VoteChoice::Yes,
                    "no" | "n" => VoteChoice::No,
                    "abstain" | "a" => VoteChoice::Abstain,
                    _ => return Err("Vote must be yes, no, or abstain".to_string()),
                };
                let reason = if positional.len() > 2 {
                    Some(positional[2..].join(" "))
                } else {
                    modifiers.remove("reason").or_else(|| {
                        on_value.as_ref().and_then(|v| {
                            let parts: Vec<&str> = v.split_whitespace().collect();
                            let choice_idx = parts.iter().position(|&p| p == "choice");
                            if let Some(idx) = choice_idx {
                                if idx + 2 < parts.len() {
                                    Some(parts[idx + 2..].join(" "))
                                } else {
                                    None
                                }
                            } else if parts.len() >= 3 {
                                Some(parts[2..].join(" "))
                            } else {
                                None
                            }
                        })
                    })
                };
                Ok(K0mmand3rCmd::Vote {
                    proposal,
                    choice,
                    reason,
                })
            }
            "delegate" => {
                let agent = positional
                    .first()
                    .map(|s| s.to_string())
                    .or_else(|| modifiers.remove("agent"))
                    .or_else(|| modifiers.remove("to"))
                    .ok_or("Usage: /delegate <agent> <budget>")?;
                let budget_str = positional
                    .get(1)
                    .map(|s| s.to_string())
                    .or_else(|| modifiers.remove("budget"))
                    .ok_or("Missing budget")?;
                let budget = budget_str
                    .parse::<u64>()
                    .map_err(|_| "Budget must be a number".to_string())?;
                Ok(K0mmand3rCmd::Delegate {
                    agent,
                    budget,
                })
            }
            "loop" => {
                let spec = LoopSpec::from_tokens(&positional, &modifiers)?;
                Ok(K0mmand3rCmd::Loop { spec })
            }
            "handshake" => {
                let agent = positional
                    .first()
                    .map(|s| s.to_string())
                    .or_else(|| modifiers.remove("agent"))
                    .or_else(|| {
                        modifiers.remove("to").map(|v| {
                            // `to` modifier value is the full target identifier
                            v
                        })
                    })
                    .ok_or("Usage: /handshake <agent>")?;
                let proposal = if positional.len() > 1 {
                    Some(positional[1..].join(" "))
                } else {
                    modifiers.remove("proposal")
                };
                Ok(K0mmand3rCmd::Handshake {
                    agent: agent.to_string(),
                    proposal,
                })
            }
            "crew" => {
                let action_str = positional
                    .first()
                    .map(|s| s.to_string())
                    .or_else(|| modifiers.remove("action"))
                    .ok_or("Usage: /crew <form|join|leave>")?;
                let action = match action_str.to_lowercase().as_str() {
                    "form" => CrewAction::Form,
                    "join" => CrewAction::Join,
                    "leave" => CrewAction::Leave,
                    _ => return Err("Unknown crew action".to_string()),
                };
                let members = if positional.len() > 1 {
                    positional[1..].iter().map(|s| s.to_string()).collect()
                } else {
                    modifiers
                        .get("members")
                        .map(|m| m.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default()
                };
                Ok(K0mmand3rCmd::Crew { action, members })
            }
            "status" => Ok(K0mmand3rCmd::Status),
            "propose" => {
                let description = if !positional.is_empty() {
                    positional.join(" ")
                } else {
                    modifiers.remove("description").unwrap_or_default()
                };
                Ok(K0mmand3rCmd::Propose { description })
            }
            "ahoy" => {
                let role = positional
                    .first()
                    .map(|s| s.to_string())
                    .or_else(|| modifiers.remove("role"))
                    .ok_or("Usage: /ahoy <role> <budget> <skills> <description>")?;
                let budget_str = positional
                    .get(1)
                    .map(|s| s.to_string())
                    .or_else(|| modifiers.remove("budget"))
                    .ok_or("Missing budget")?;
                let budget = budget_str
                    .parse::<u64>()
                    .map_err(|_| "Budget must be a number".to_string())?;
                let skills = positional
                    .get(2)
                    .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
                    .or_else(|| {
                        modifiers
                            .get("skills")
                            .map(|s| s.split(',').map(|t| t.trim().to_string()).collect())
                    })
                    .unwrap_or_default();
                let description = if positional.len() > 3 {
                    positional[3..].join(" ")
                } else {
                    modifiers.remove("description").unwrap_or_default()
                };
                Ok(K0mmand3rCmd::Ahoy {
                    role: role.to_string(),
                    budget,
                    skills,
                    description,
                })
            }
            "apply" => {
                let ahoy_id = positional
                    .first()
                    .map(|s| s.to_string())
                    .or_else(|| modifiers.remove("ahoy_id"))
                    .ok_or("Usage: /apply <ahoy_id> <pitch>")?;
                let pitch = if positional.len() > 1 {
                    positional[1..].join(" ")
                } else {
                    modifiers.remove("pitch").unwrap_or_default()
                };
                Ok(K0mmand3rCmd::Apply {
                    ahoy_id: ahoy_id.to_string(),
                    pitch,
                })
            }
            "award" => {
                let ahoy_id = positional
                    .first()
                    .map(|s| s.to_string())
                    .or_else(|| modifiers.remove("ahoy_id"))
                    .ok_or("Usage: /award <ahoy_id> <winner>")?;
                let winner = positional
                    .get(1)
                    .map(|s| s.to_string())
                    .or_else(|| modifiers.remove("winner"))
                    .ok_or("Missing winner")?;
                Ok(K0mmand3rCmd::Award {
                    ahoy_id: ahoy_id.to_string(),
                    winner: winner.to_string(),
                })
            }
            _ => Ok(K0mmand3rCmd::Unknown {
                raw: cmd.to_string(),
            }),
        }
    }

    pub fn verb(&self) -> String {
        match self {
            K0mmand3rCmd::Negotiate { .. } => "negotiate".to_string(),
            K0mmand3rCmd::Vote { .. } => "vote".to_string(),
            K0mmand3rCmd::Delegate { .. } => "delegate".to_string(),
            K0mmand3rCmd::Loop { .. } => "loop".to_string(),
            K0mmand3rCmd::Handshake { .. } => "handshake".to_string(),
            K0mmand3rCmd::Crew { .. } => "crew".to_string(),
            K0mmand3rCmd::Status => "status".to_string(),
            K0mmand3rCmd::Propose { .. } => "propose".to_string(),
            K0mmand3rCmd::Ahoy { .. } => "ahoy".to_string(),
            K0mmand3rCmd::Apply { .. } => "apply".to_string(),
            K0mmand3rCmd::Award { .. } => "award".to_string(),
            K0mmand3rCmd::Unknown { raw } => raw
                .split_whitespace()
                .next()
                .unwrap_or("unknown")
                .trim_start_matches('/')
                .to_string(),
        }
    }

    pub fn object(&self) -> String {
        match self {
            K0mmand3rCmd::Negotiate { resource, id, .. } => format!("{}:{}", resource, id),
            K0mmand3rCmd::Vote { proposal, .. } => format!("proposal:{}", proposal),
            K0mmand3rCmd::Delegate { agent, .. } => format!("agent:{}", agent),
            K0mmand3rCmd::Loop { .. } => "loop".to_string(),
            K0mmand3rCmd::Handshake { agent, .. } => format!("agent:{}", agent),
            K0mmand3rCmd::Crew { action, .. } => {
                format!("crew:{:?}", action).to_lowercase()
            }
            K0mmand3rCmd::Status => "status".to_string(),
            K0mmand3rCmd::Propose { .. } => "proposal".to_string(),
            K0mmand3rCmd::Ahoy { role, .. } => format!("role:{}", role),
            K0mmand3rCmd::Apply { ahoy_id, .. } => format!("ahoy:{}", ahoy_id),
            K0mmand3rCmd::Award { ahoy_id, .. } => format!("ahoy:{}", ahoy_id),
            K0mmand3rCmd::Unknown { raw } => raw.to_string(),
        }
    }
}

// ============== OpenTelemetry Integration ==============

pub struct K0mmand3rTelemetry {
    enabled: bool,
}

impl K0mmand3rTelemetry {
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    pub fn with_span<F, T>(&self, verb: &str, object: &str, agent_id: &str, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        if !self.enabled {
            return f();
        }
        let tracer = opentelemetry::global::tracer("k0mmand3r");
        let mut span = tracer.start(format!("k0mmand3r.{}", verb));
        span.set_attribute(opentelemetry::KeyValue::new("k0mmand3r.verb", verb.to_string()));
        span.set_attribute(opentelemetry::KeyValue::new("k0mmand3r.object", object.to_string()));
        span.set_attribute(opentelemetry::KeyValue::new("k0mmand3r.agent_id", agent_id.to_string()));
        let result = f();
        span.end();
        result
    }
}

/// Guard condition: evaluates to true/false based on agent blessings, budget, votes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GuardCondition {
    pub requires: Vec<String>, // k0mmand3r commands to negotiate
    pub expression: String,    // boolean expression to evaluate
}

/// Evaluation context: agent's current state
#[derive(Debug, Clone, Default)]
pub struct EvaluationContext {
    pub agent_blessings: Vec<String>,
    pub available_budget: u32,
    pub votes: Vec<(String, String)>, // (agent_id, vote: yes/no/abstain)
    pub authorized: bool,
}

impl GuardCondition {
    /// Evaluate guard condition in context
    pub fn evaluate(&self, context: &EvaluationContext) -> Result<bool, String> {
        // Parse and evaluate the expression
        // Simple implementation: pattern matching on common expressions

        if self.expression.contains("has_blessing") {
            // Extract blessing name from expression: has_blessing(blessing:name)
            if let Some(start) = self.expression.find("has_blessing(") {
                let remainder = &self.expression[start + 13..]; // "has_blessing(" is 13 chars
                if let Some(end) = remainder.find(')') {
                    let blessing_str = remainder[..end].trim();
                    let has_it = context.agent_blessings.iter().any(|b| b == blessing_str);
                    return Ok(has_it);
                }
            }
        }

        if self.expression.contains("budget_available") {
            // Extract budget amount: budget_available(amount)
            if let Some(start) = self.expression.find("budget_available(") {
                let remainder = &self.expression[start + 17..]; // "budget_available(" is 17 chars
                if let Some(end) = remainder.find(')') {
                    let amount_str = remainder[..end].trim();
                    if let Ok(amount) = amount_str.parse::<u32>() {
                        return Ok(context.available_budget >= amount);
                    }
                }
            }
        }

        if self.expression.contains("voted_yes") {
            // Check if quorum voted yes
            let yes_count = context
                .votes
                .iter()
                .filter(|(_, vote)| vote == "yes")
                .count();

            // Require at least 2 yes votes (configurable)
            return Ok(yes_count >= 2);
        }

        // Default: expression evaluated to true if no recognizable pattern
        Ok(true)
    }

    /// Extract required blessings from guard
    pub fn required_blessings(&self) -> Vec<String> {
        self.requires
            .iter()
            .filter_map(|cmd| {
                if let Ok(k0mmand) = K0mmand::parse(cmd) {
                    if k0mmand.verb == "negotiate" && k0mmand.object.starts_with("blessing:") {
                        Some(k0mmand.object)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    /// Extract required budget from guard
    pub fn required_budget(&self) -> u32 {
        self.requires
            .iter()
            .filter_map(|cmd| {
                if let Ok(k0mmand) = K0mmand::parse(cmd) {
                    k0mmand
                        .modifiers
                        .get("budget")
                        .and_then(|b| b.parse::<u32>().ok())
                } else {
                    None
                }
            })
            .sum()
    }
}

#[cfg(test)]
mod tests;
