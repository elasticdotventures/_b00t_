// orchestrator/agent_messaging.rs
// Secure message passing between agents with k0mmand3r authorization

use crate::k0mmand3r::{EvaluationContext, GuardCondition};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Agent message for inter-agent communication
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentMessage {
    pub id: String,
    pub from: String, // Agent ID
    pub to: String,   // Agent ID or broadcast
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    pub timestamp: String,                 // ISO8601
    pub requires_blessing: Option<String>, // k0mmand3r blessing needed to deliver
    #[serde(default)]
    pub audit_trail: Vec<AuditEntry>,
}

/// Audit entry for message processing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditEntry {
    pub timestamp: String,
    pub actor: String,
    pub action: String,
    pub result: String,
}

/// Message delivery result with audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryResult {
    pub message_id: String,
    pub delivered: bool,
    pub recipient: String,
    pub reason: String,
    pub timestamp: String,
}

/// Agent message router with authorization
pub struct MessageRouter {
    mailboxes: BTreeMap<String, Vec<AgentMessage>>,
    audit_log: Vec<AuditEntry>,
}

impl MessageRouter {
    /// Create a new message router
    pub fn new() -> Self {
        MessageRouter {
            mailboxes: BTreeMap::new(),
            audit_log: Vec::new(),
        }
    }

    /// Create a message from one agent to another
    pub fn create_message(from: &str, to: &str, subject: &str, body: &str) -> AgentMessage {
        AgentMessage {
            id: format!("msg:{}", uuid::Uuid::new_v4()),
            from: from.to_string(),
            to: to.to_string(),
            subject: subject.to_string(),
            body: body.to_string(),
            metadata: BTreeMap::new(),
            timestamp: Utc::now().to_rfc3339(),
            requires_blessing: None,
            audit_trail: vec![],
        }
    }

    /// Send a message with authorization check
    pub fn send(
        &mut self,
        mut message: AgentMessage,
        guard: Option<&GuardCondition>,
        auth_context: &EvaluationContext,
    ) -> Result<DeliveryResult, String> {
        let message_id = message.id.clone();

        // Check authorization if required
        if let Some(guard_condition) = guard {
            let authorized = guard_condition
                .evaluate(auth_context)
                .map_err(|e| format!("Authorization evaluation failed: {}", e))?;

            if !authorized {
                self.audit_log.push(AuditEntry {
                    timestamp: Utc::now().to_rfc3339(),
                    actor: message.from.clone(),
                    action: format!("send_message:{}", message_id),
                    result: "DENIED_AUTHORIZATION".to_string(),
                });

                return Ok(DeliveryResult {
                    message_id,
                    delivered: false,
                    recipient: message.to.clone(),
                    reason: "Authorization failed".to_string(),
                    timestamp: Utc::now().to_rfc3339(),
                });
            }
        }

        // Add delivery audit entry
        message.audit_trail.push(AuditEntry {
            timestamp: Utc::now().to_rfc3339(),
            actor: message.from.clone(),
            action: "SENT".to_string(),
            result: format!("to {}", message.to),
        });

        self.audit_log.push(AuditEntry {
            timestamp: Utc::now().to_rfc3339(),
            actor: message.from.clone(),
            action: format!("send_message:{}", message_id),
            result: "SUCCESS".to_string(),
        });

        // Route to mailbox(es)
        if message.to == "*" {
            // Broadcast
            for (agent_id, mailbox) in self.mailboxes.iter_mut() {
                if agent_id != &message.from {
                    mailbox.push(message.clone());
                }
            }
        } else {
            // Direct delivery
            self.mailboxes
                .entry(message.to.clone())
                .or_insert_with(Vec::new)
                .push(message.clone());
        }

        Ok(DeliveryResult {
            message_id,
            delivered: true,
            recipient: message.to,
            reason: "Delivered".to_string(),
            timestamp: Utc::now().to_rfc3339(),
        })
    }

    /// Receive messages for an agent
    pub fn receive(&mut self, agent_id: &str) -> Vec<AgentMessage> {
        self.mailboxes.remove(agent_id).unwrap_or_default()
    }

    /// Peek at messages without consuming them
    pub fn peek(&self, agent_id: &str) -> Vec<&AgentMessage> {
        self.mailboxes
            .get(agent_id)
            .map(|msgs| msgs.iter().collect())
            .unwrap_or_default()
    }

    /// Count pending messages for an agent
    pub fn count_pending(&self, agent_id: &str) -> usize {
        self.mailboxes
            .get(agent_id)
            .map(|msgs| msgs.len())
            .unwrap_or(0)
    }

    /// Get audit log entries for a message
    pub fn get_audit_trail(&self, message_id: &str) -> Vec<&AuditEntry> {
        self.audit_log
            .iter()
            .filter(|e| e.action.contains(message_id))
            .collect()
    }

    /// Get all audit logs for an agent
    pub fn get_agent_audit(&self, agent_id: &str) -> Vec<&AuditEntry> {
        self.audit_log
            .iter()
            .filter(|e| e.actor == agent_id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_message() {
        let msg = MessageRouter::create_message(
            "agent:executive",
            "agent:executor",
            "execute-step",
            "Please execute the deployment step",
        );

        assert_eq!(msg.from, "agent:executive");
        assert_eq!(msg.to, "agent:executor");
        assert_eq!(msg.subject, "execute-step");
    }

    #[test]
    fn test_send_message_without_auth() {
        let mut router = MessageRouter::new();

        let msg =
            MessageRouter::create_message("agent:executive", "agent:executor", "test", "test body");

        let context = EvaluationContext {
            agent_blessings: vec!["blessing:execute".to_string()],
            available_budget: 1000,
            votes: vec![],
            authorized: true,
        };

        let result = router.send(msg, None, &context).expect("Should send");

        assert!(result.delivered);
        assert_eq!(router.count_pending("agent:executor"), 1);
    }

    #[test]
    fn test_send_message_with_successful_auth() {
        let mut router = MessageRouter::new();

        let msg = MessageRouter::create_message(
            "agent:observer",
            "agent:auditor",
            "audit-request",
            "Please audit the system",
        );

        let guard = GuardCondition {
            requires: vec![],
            expression: "has_blessing(blessing:audit-access)".to_string(),
        };

        let context = EvaluationContext {
            agent_blessings: vec!["blessing:audit-access".to_string()],
            available_budget: 1000,
            votes: vec![],
            authorized: true,
        };

        let result = router
            .send(msg, Some(&guard), &context)
            .expect("Should send");

        assert!(result.delivered);
    }

    #[test]
    fn test_send_message_with_failed_auth() {
        let mut router = MessageRouter::new();

        let msg = MessageRouter::create_message(
            "agent:observer",
            "agent:auditor",
            "privileged-request",
            "Execute dangerous action",
        );

        let guard = GuardCondition {
            requires: vec![],
            expression: "has_blessing(blessing:execute-dangerous)".to_string(),
        };

        let context = EvaluationContext {
            agent_blessings: vec![], // Missing required blessing
            available_budget: 1000,
            votes: vec![],
            authorized: false,
        };

        let result = router
            .send(msg, Some(&guard), &context)
            .expect("Should return result");

        assert!(!result.delivered);
        assert!(result.reason.contains("Authorization failed"));
    }

    #[test]
    fn test_broadcast_message() {
        let mut router = MessageRouter::new();

        // Pre-create mailboxes for agents
        router.mailboxes.insert("agent:1".to_string(), Vec::new());
        router.mailboxes.insert("agent:2".to_string(), Vec::new());

        let msg = MessageRouter::create_message(
            "agent:orchestrator",
            "*",
            "system-alert",
            "All agents attend",
        );

        let context = EvaluationContext::default();
        router.send(msg, None, &context).expect("Should broadcast");

        assert_eq!(router.count_pending("agent:1"), 1);
        assert_eq!(router.count_pending("agent:2"), 1);
    }

    #[test]
    fn test_receive_consumes_messages() {
        let mut router = MessageRouter::new();

        let msg = MessageRouter::create_message("agent:sender", "agent:receiver", "test", "test");

        let context = EvaluationContext::default();
        router.send(msg, None, &context).expect("Should send");

        assert_eq!(router.count_pending("agent:receiver"), 1);

        let received = router.receive("agent:receiver");
        assert_eq!(received.len(), 1);
        assert_eq!(router.count_pending("agent:receiver"), 0);
    }

    #[test]
    fn test_peek_does_not_consume() {
        let mut router = MessageRouter::new();

        let msg = MessageRouter::create_message("agent:sender", "agent:receiver", "test", "test");

        let context = EvaluationContext::default();
        router.send(msg, None, &context).expect("Should send");

        let peeked = router.peek("agent:receiver");
        assert_eq!(peeked.len(), 1);
        assert_eq!(router.count_pending("agent:receiver"), 1);

        let peeked_again = router.peek("agent:receiver");
        assert_eq!(peeked_again.len(), 1);
    }

    #[test]
    fn test_audit_trail_on_send() {
        let mut router = MessageRouter::new();

        let msg = MessageRouter::create_message("agent:sender", "agent:receiver", "test", "test");

        let msg_id = msg.id.clone();
        let context = EvaluationContext::default();
        router.send(msg, None, &context).expect("Should send");

        let audit = router.get_audit_trail(&msg_id);
        assert!(!audit.is_empty());
        assert!(audit.iter().any(|e| e.result == "SUCCESS"));
    }

    #[test]
    fn test_agent_audit_log() {
        let mut router = MessageRouter::new();

        let msg1 = MessageRouter::create_message("agent:sender", "agent:receiver1", "msg1", "body");

        let msg2 = MessageRouter::create_message("agent:sender", "agent:receiver2", "msg2", "body");

        let context = EvaluationContext::default();
        router.send(msg1, None, &context).expect("Should send msg1");
        router.send(msg2, None, &context).expect("Should send msg2");

        let agent_audit = router.get_agent_audit("agent:sender");
        assert!(agent_audit.len() >= 2);
    }
}
