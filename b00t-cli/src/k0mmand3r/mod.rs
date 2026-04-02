// b00t-cli/src/k0mmand3r/mod.rs
// k0mmand3r: agent coordination protocol for step guards + authorization

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// k0mmand3r command: slash command for agent coordination
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct K0mmand {
    pub verb: String,       // negotiate, vote, delegate, status, etc.
    pub object: String,     // blessing:name, step:name, agent:name
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

/// Guard condition: evaluates to true/false based on agent blessings, budget, votes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GuardCondition {
    pub requires: Vec<String>,      // k0mmand3r commands to negotiate
    pub expression: String,         // boolean expression to evaluate
}

/// Evaluation context: agent's current state
#[derive(Debug, Clone, Default)]
pub struct EvaluationContext {
    pub agent_blessings: Vec<String>,
    pub available_budget: u32,
    pub votes: Vec<(String, String)>,   // (agent_id, vote: yes/no/abstain)
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
                let remainder = &self.expression[start + 13..];  // "has_blessing(" is 13 chars
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
                let remainder = &self.expression[start + 17..];  // "budget_available(" is 17 chars
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
