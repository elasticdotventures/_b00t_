//! Zellij interaction types — shared by CLI and WASM targets.
//!
//! Provides the type-safe representation of the 5 Zellij interaction modes
//! plus Eisenhower quadrant routing, menu items, and agent actions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// The 5 interaction modes available in Zellij floating panes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionMode {
    /// Y/N confirm dialog (whiptail or fzf)
    Menu,
    /// Y/N confirm dialog (yes/no prompt)
    Confirm,
    /// Free-text input with optional default value
    Input,
    /// Sub-agent report modal (read-only display)
    Subagent,
    /// Multi-step wizard with sequential prompts
    Wizard,
}

impl InteractionMode {
    /// Human-readable label for CLI display.
    pub fn as_label(&self) -> &'static str {
        match self {
            InteractionMode::Menu => "Menu",
            InteractionMode::Confirm => "Confirm",
            InteractionMode::Input => "Input",
            InteractionMode::Subagent => "Subagent",
            InteractionMode::Wizard => "Wizard",
        }
    }
}

impl std::fmt::Display for InteractionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_label())
    }
}

/// Parse an interaction mode from a CLI argument string.
///
/// Accepts kebab-case, snake_case, and case-insensitive variants.
impl FromStr for InteractionMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "menu" | "fzf-menu" | "fzf" => Ok(InteractionMode::Menu),
            "confirm" | "yesno" | "yes-no" | "y/n" => Ok(InteractionMode::Confirm),
            "input" | "text-input" | "text" | "prompt" => Ok(InteractionMode::Input),
            "subagent" | "sub-agent" | "report" | "subagent-report" => {
                Ok(InteractionMode::Subagent)
            }
            "wizard" | "multi-step" | "multistep" => Ok(InteractionMode::Wizard),
            other => Err(format!(
                "unknown interaction mode: '{other}'. Valid modes: menu, confirm, input, subagent, wizard"
            )),
        }
    }
}

/// Request for user input via a Zellij floating pane.
///
/// Sent by the agent when it needs human input before proceeding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputRequest {
    /// The prompt text shown to the user.
    pub prompt: String,
    /// Optional default value pre-filled in the input.
    #[serde(default)]
    pub default: Option<String>,
    /// Timeout in milliseconds for user response (None = no timeout).
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Whether a response is mandatory (blocks agent until provided).
    #[serde(default)]
    pub required: bool,
}

/// Response from a user interaction in Zellij.
///
/// Captured after the user dismisses the floating pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResponse {
    /// The interaction mode that generated this response.
    pub mode: InteractionMode,
    /// The raw value returned by the user (JSON string from fzf, text, or confirm).
    pub value: String,
    /// When the response was received.
    pub timestamp: DateTime<Utc>,
    /// Snapshot of relevant KV keys at response time.
    #[serde(default)]
    pub kv_snapshot: serde_json::Value,
}

/// A single item in an fzf-style interactive menu.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuItem {
    /// Short key for the item (e.g., "build", "test").
    pub key: String,
    /// Human-readable label shown in the menu.
    pub label: String,
    /// Eisenhower quadrant classification for routing.
    pub quadrant: EisenhowerQuadrant,
    /// The agent action triggered when this item is selected.
    pub action: AgentAction,
    /// Optional description shown in preview/lower pane.
    #[serde(default)]
    pub description: Option<String>,
}

/// Eisenhower Matrix quadrant for task prioritization.
///
/// Determines how the governance gate routes an interaction:
/// - UrgentImportant → Confirm (Allow with user review)
/// - NotUrgentImportant → FzfMenu (Hook with scheduled prompt)
/// - UrgentNotImportant → SubagentReport (Hook with delegation)
/// - NotUrgentNotImportant → Deny (lowest priority, eliminated)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EisenhowerQuadrant {
    /// Urgent + Important → Allow via confirm dialog
    UrgentImportant,
    /// Not Urgent + Important → Hook via fzf menu
    NotUrgentImportant,
    /// Urgent + Not Important → Hook via subagent report
    UrgentNotImportant,
    /// Not Urgent + Not Important → Deny automatically
    NotUrgentNotImportant,
}

impl EisenhowerQuadrant {
    /// Classify based on boolean urgency and importance flags.
    pub fn classify(urgent: bool, important: bool) -> Self {
        match (urgent, important) {
            (true, true) => EisenhowerQuadrant::UrgentImportant,
            (false, true) => EisenhowerQuadrant::NotUrgentImportant,
            (true, false) => EisenhowerQuadrant::UrgentNotImportant,
            (false, false) => EisenhowerQuadrant::NotUrgentNotImportant,
        }
    }

    /// Human-readable label for audit/log display.
    pub fn as_label(&self) -> &'static str {
        match self {
            EisenhowerQuadrant::UrgentImportant => "Urgent & Important",
            EisenhowerQuadrant::NotUrgentImportant => "Not Urgent & Important",
            EisenhowerQuadrant::UrgentNotImportant => "Urgent & Not Important",
            EisenhowerQuadrant::NotUrgentNotImportant => "Not Urgent & Not Important",
        }
    }
}

impl std::fmt::Display for EisenhowerQuadrant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_label())
    }
}

/// Action triggered by a menu selection or gate decision.
///
/// Maps to the gate result: Approve→Allow, Reject→Deny, Delegate→Hook, Audit→Hook.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentAction {
    /// Approve the action (maps to Allow).
    Approve,
    /// Reject the action (maps to Deny).
    Reject,
    /// Delegate to another agent/process (maps to Hook).
    Delegate,
    /// Audit-only — log and proceed (maps to Hook).
    Audit,
}

impl AgentAction {
    /// Human-readable label for audit display.
    pub fn as_label(&self) -> &'static str {
        match self {
            AgentAction::Approve => "Approve",
            AgentAction::Reject => "Reject",
            AgentAction::Delegate => "Delegate",
            AgentAction::Audit => "Audit",
        }
    }
}

impl std::fmt::Display for AgentAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interaction_mode_from_str_valid() {
        assert_eq!("menu".parse::<InteractionMode>().unwrap(), InteractionMode::Menu);
        assert_eq!("confirm".parse::<InteractionMode>().unwrap(), InteractionMode::Confirm);
        assert_eq!("input".parse::<InteractionMode>().unwrap(), InteractionMode::Input);
        assert_eq!("subagent".parse::<InteractionMode>().unwrap(), InteractionMode::Subagent);
        assert_eq!("wizard".parse::<InteractionMode>().unwrap(), InteractionMode::Wizard);
    }

    #[test]
    fn test_interaction_mode_from_str_aliases() {
        assert_eq!("fzf".parse::<InteractionMode>().unwrap(), InteractionMode::Menu);
        assert_eq!("text".parse::<InteractionMode>().unwrap(), InteractionMode::Input);
        assert_eq!("report".parse::<InteractionMode>().unwrap(), InteractionMode::Subagent);
        assert_eq!("multi-step".parse::<InteractionMode>().unwrap(), InteractionMode::Wizard);
    }

    #[test]
    fn test_interaction_mode_from_str_case_insensitive() {
        assert_eq!("MENU".parse::<InteractionMode>().unwrap(), InteractionMode::Menu);
        assert_eq!("Confirm".parse::<InteractionMode>().unwrap(), InteractionMode::Confirm);
    }

    #[test]
    fn test_interaction_mode_from_str_invalid() {
        assert!("unknown".parse::<InteractionMode>().is_err());
    }

    #[test]
    fn test_eisenhower_quadrant_classify() {
        assert_eq!(
            EisenhowerQuadrant::classify(true, true),
            EisenhowerQuadrant::UrgentImportant
        );
        assert_eq!(
            EisenhowerQuadrant::classify(false, true),
            EisenhowerQuadrant::NotUrgentImportant
        );
        assert_eq!(
            EisenhowerQuadrant::classify(true, false),
            EisenhowerQuadrant::UrgentNotImportant
        );
        assert_eq!(
            EisenhowerQuadrant::classify(false, false),
            EisenhowerQuadrant::NotUrgentNotImportant
        );
    }

    #[test]
    fn test_serialize_roundtrip_menu_item() {
        let item = MenuItem {
            key: "build".to_string(),
            label: "Build Project".to_string(),
            quadrant: EisenhowerQuadrant::UrgentImportant,
            action: AgentAction::Approve,
            description: Some("Run cargo build".to_string()),
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: MenuItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.key, "build");
        assert_eq!(parsed.quadrant, EisenhowerQuadrant::UrgentImportant);
        assert_eq!(parsed.action, AgentAction::Approve);
    }

    #[test]
    fn test_serialize_roundtrip_input_request() {
        let req = InputRequest {
            prompt: "Enter name".to_string(),
            default: Some("world".to_string()),
            timeout: Some(30000),
            required: true,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: InputRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.prompt, "Enter name");
        assert_eq!(parsed.default, Some("world".to_string()));
        assert_eq!(parsed.timeout, Some(30000));
        assert!(parsed.required);
    }

    #[test]
    fn test_agent_action_display() {
        assert_eq!(AgentAction::Approve.to_string(), "Approve");
        assert_eq!(AgentAction::Reject.to_string(), "Reject");
        assert_eq!(AgentAction::Delegate.to_string(), "Delegate");
        assert_eq!(AgentAction::Audit.to_string(), "Audit");
    }
}
