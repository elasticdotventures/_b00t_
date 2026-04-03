// b00t-cli/src/step/mod.rs
// Moku type-state machines: serialized .step.toml files

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use crate::k0mmand3r::GuardCondition;

/// Moku step: type-state machine serialized in .tomllm format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MokuStep {
    pub name: String,
    pub states: StatesConfig,
    #[serde(default)]
    pub state_defs: Vec<StateDefinition>,
}

/// States configuration: enum variants + initial state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatesConfig {
    pub variants: Vec<String>,
    pub initial: String,
}

/// State definition: instructions + IO contract + transition
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StateDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<StateInstructions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub io: Option<IOContract>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<TransitionRule>,
}

/// OODA loop instructions for a state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateInstructions {
    pub observe: Option<String>,
    pub orient: Option<String>,
    pub decide: Option<String>,
    pub act: Option<String>,
}

/// Input/output contract for a state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IOContract {
    pub input: Option<BTreeMap<String, String>>,
    pub output: Option<BTreeMap<String, String>>,
}

/// Transition rule: guards + next state + contract
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransitionRule {
    pub to: String,
    #[serde(default)]
    pub requires: Vec<String>,  // k0mmand3r commands
    pub guard: Option<String>,  // Boolean expression
    pub output_contract: Option<BTreeMap<String, String>>,
}

impl MokuStep {
    /// Parse step from TOML
    pub fn from_toml(toml_str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let value: toml::Value = toml::from_str(toml_str)?;
        let b00t = value.get("b00t").ok_or("Missing [b00t] section")?;

        let name = b00t
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or("Missing name")?
            .to_string();

        let states = b00t
            .get("step")
            .and_then(|s| s.get("states"))
            .ok_or("Missing [b00t.step.states]")?;

        let variants: Vec<String> = states
            .get("variants")
            .and_then(|v| v.as_array())
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();

        let initial = states
            .get("initial")
            .and_then(|i| i.as_str())
            .ok_or("Missing initial state")?
            .to_string();

        // Parse state definitions
        let state_defs: Vec<StateDefinition> = b00t
            .get("step")
            .and_then(|s| s.get("state"))
            .and_then(|st| st.as_array())
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|s| {
                let name = s.get("name")?.as_str()?.to_string();
                Some(StateDefinition {
                    name,
                    instructions: None,
                    io: None,
                    transition: None,
                })
            })
            .collect();

        Ok(MokuStep {
            name,
            states: StatesConfig { variants, initial },
            state_defs,
        })
    }

    /// Get state definition by name
    pub fn get_state(&self, name: &str) -> Option<&StateDefinition> {
        self.state_defs.iter().find(|s| s.name == name)
    }

    /// Validate state machine
    pub fn validate(&self) -> Result<(), String> {
        // Check all transitions point to valid states
        let valid_states: std::collections::HashSet<&str> =
            self.states.variants.iter().map(|s| s.as_str()).collect();

        for state_def in &self.state_defs {
            if let Some(transition) = &state_def.transition {
                if !valid_states.contains(transition.to.as_str()) {
                    return Err(format!(
                        "State '{}' transitions to invalid state '{}'",
                        state_def.name, transition.to
                    ));
                }
            }
        }

        // Validate initial state
        self.validate_initial_state()?;

        Ok(())
    }

    /// Validate initial state exists
    pub fn validate_initial_state(&self) -> Result<(), String> {
        if !self.states.variants.contains(&self.states.initial) {
            return Err(format!(
                "Initial state '{}' not in variants",
                self.states.initial
            ));
        }
        Ok(())
    }

    /// Extract guards from all transitions
    pub fn extract_all_guards(&self) -> Vec<GuardCondition> {
        self.state_defs
            .iter()
            .filter_map(|s| s.transition.as_ref())
            .flat_map(|t| t.extract_guards())
            .collect()
    }

    /// Serialize to TOML
    pub fn to_toml(&self) -> Result<String, Box<dyn std::error::Error>> {
        let mut toml_map = toml::map::Map::new();

        let mut b00t_map = toml::map::Map::new();
        b00t_map.insert("name".to_string(), toml::Value::String(self.name.clone()));
        b00t_map.insert("type".to_string(), toml::Value::String("step".to_string()));

        let mut step_map = toml::map::Map::new();
        let mut states_map = toml::map::Map::new();

        let variants: Vec<toml::Value> = self
            .states
            .variants
            .iter()
            .map(|v| toml::Value::String(v.clone()))
            .collect();

        states_map.insert("variants".to_string(), toml::Value::Array(variants));
        states_map.insert(
            "initial".to_string(),
            toml::Value::String(self.states.initial.clone()),
        );

        step_map.insert("states".to_string(), toml::Value::Table(states_map));
        b00t_map.insert("step".to_string(), toml::Value::Table(step_map));

        toml_map.insert("b00t".to_string(), toml::Value::Table(b00t_map));

        Ok(toml::to_string_pretty(&toml_map)?)
    }

    /// Generate Mermaid state diagram
    pub fn to_mermaid(&self) -> Result<String, String> {
        let mut diagram = String::from("stateDiagram-v2\n");
        diagram.push_str(&format!("    [*] --> {}\n", self.states.initial));

        for state_def in &self.state_defs {
            if let Some(transition) = &state_def.transition {
                if let Some(guard) = &transition.guard {
                    diagram.push_str(&format!(
                        "    {} --> {} : {}\n",
                        state_def.name, transition.to, guard
                    ));
                } else {
                    diagram.push_str(&format!("    {} --> {}\n", state_def.name, transition.to));
                }
            }
        }

        Ok(diagram)
    }
}

impl TransitionRule {
    /// Extract guard conditions from transition
    pub fn extract_guards(&self) -> Vec<GuardCondition> {
        if let Some(guard_expr) = &self.guard {
            vec![GuardCondition {
                requires: self.requires.clone(),
                expression: guard_expr.clone(),
            }]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests;
