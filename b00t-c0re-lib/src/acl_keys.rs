// 🤓 Shared ACL types for b00t API key management
//    Used by both b00t-cli (server.rs) and b00t-mcp (server_llm.rs)
//    to guarantee identical JSON serialization format.

use serde::{Deserialize, Serialize};

/// Ontology-scoped action: what operation a permission grants.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Read,
    Write,
    Execute,
}

/// A single permission entry: access to an ontology class with an action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClassPermission {
    pub class: String,
    pub action: Action,
}

impl ClassPermission {
    /// Parse "b00t:EmbeddingModel:execute" → ClassPermission { class, action }
    /// Uses `rsplitn(2, ':')` so the class can contain colons (e.g. "b00t:ChatModel").
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.rsplitn(2, ':').collect();
        if parts.len() != 2 {
            return None;
        }
        let action = match parts[0] {
            "read" => Action::Read,
            "write" => Action::Write,
            "execute" => Action::Execute,
            _ => return None,
        };
        Some(ClassPermission {
            class: parts[1].to_string(),
            action,
        })
    }
}

/// A registered API key entry stored in server-keys.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEntry {
    pub consumer: String,
    /// RFC 3339 timestamp of key creation (via chrono serde feature).
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Access permissions — empty means deny-all (v0.9+ behavior).
    #[serde(default)]
    pub access: Vec<ClassPermission>,
}
