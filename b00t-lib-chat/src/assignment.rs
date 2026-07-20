use crate::{
    error::ChatResult,
    message::{NotificationMessage, TaskMessage},
    transport::ChatTransport,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TriggerKind {
    Timer,
    Event,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TimerSpec {
    Interval { seconds: u64 },
    Cron { expr: String },
}

impl TimerSpec {
    pub fn interval_secs(seconds: u64) -> Self {
        Self::Interval { seconds }
    }

    pub fn cron(expr: impl Into<String>) -> Self {
        Self::Cron { expr: expr.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConditionOp {
    Eq,
    Neq,
    Contains,
    Regex,
}

impl ConditionOp {
    pub fn evaluate(&self, actual: &str, expected: &str) -> bool {
        match self {
            Self::Eq => actual == expected,
            Self::Neq => actual != expected,
            Self::Contains => actual.contains(expected),
            Self::Regex => regex::Regex::new(expected)
                .map(|re| re.is_match(actual))
                .unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub operator: ConditionOp,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTemplate {
    pub to_agent: String,
    pub action: String,
    pub payload_template: serde_json::Value,
}

impl TaskTemplate {
    pub fn render(&self, notification: &NotificationMessage) -> serde_json::Value {
        interpolate_value(&self.payload_template, notification)
    }
}

fn interpolate_value(
    value: &serde_json::Value,
    notification: &NotificationMessage,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            let rendered = s
                .replace("{event.source}", &notification.source)
                .replace("{event.type}", &notification.event_type)
                .replace("{event.timestamp}", &notification.timestamp.to_rfc3339())
                .replace(
                    "{event.payload}",
                    &serde_json::to_string(&notification.payload).unwrap_or_default(),
                );
            serde_json::Value::String(rendered)
        }
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|v| interpolate_value(v, notification))
                .collect(),
        ),
        serde_json::Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (k, v) in map {
                new_map.insert(k.clone(), interpolate_value(v, notification));
            }
            serde_json::Value::Object(new_map)
        }
        other => other.clone(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentRule {
    pub id: String,
    pub name: String,
    pub trigger: TriggerKind,
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timer_spec: Option<TimerSpec>,
    pub condition: Option<Condition>,
    pub action: TaskTemplate,
    pub enabled: bool,
    pub repeat: bool,
    pub created_at: DateTime<Utc>,
    pub last_triggered: Option<DateTime<Utc>>,
}

impl AssignmentRule {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        trigger: TriggerKind,
        subject: impl Into<String>,
        action: TaskTemplate,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            trigger,
            subject: subject.into(),
            timer_spec: None,
            condition: None,
            action,
            enabled: true,
            repeat: true,
            created_at: Utc::now(),
            last_triggered: None,
        }
    }

    pub fn with_timer(mut self, spec: TimerSpec) -> Self {
        self.trigger = TriggerKind::Timer;
        self.timer_spec = Some(spec);
        self
    }

    pub fn with_condition(mut self, condition: Condition) -> Self {
        self.condition = Some(condition);
        self
    }

    pub fn one_shot(mut self) -> Self {
        self.repeat = false;
        self
    }

    pub fn matches(&self, notification: &NotificationMessage) -> bool {
        if !self.enabled {
            return false;
        }
        if self.trigger != TriggerKind::Event {
            return false;
        }
        let notif_subject = notification.subject();
        if !matches_subject(&self.subject, &notif_subject) {
            return false;
        }
        if let Some(ref condition) = self.condition {
            let actual_value = get_payload_field(&notification.payload, &condition.field);
            return condition.operator.evaluate(&actual_value, &condition.value);
        }
        true
    }

    pub fn build_task(&self, notification: &NotificationMessage) -> TaskMessage {
        TaskMessage {
            task_id: uuid::Uuid::new_v4().to_string(),
            action: self.action.action.clone(),
            payload: self.action.render(notification),
            from_agent: "assignment-engine".to_string(),
            to_agent: self.action.to_agent.clone(),
            deadline: None,
            priority: "normal".to_string(),
            timestamp: Utc::now(),
        }
    }
}

fn matches_subject(pattern: &str, subject: &str) -> bool {
    if pattern.contains('*') || pattern.contains('>') {
        let re_str = pattern
            .replace('.', "\\.")
            .replace('*', "[^.]+")
            .replace('>', ".+");
        regex::Regex::new(&format!("^{}$", re_str))
            .map(|re| re.is_match(subject))
            .unwrap_or(false)
    } else {
        pattern == subject
    }
}

fn get_payload_field(payload: &serde_json::Value, field: &str) -> String {
    let parts: Vec<&str> = field.split('/').filter(|s| !s.is_empty()).collect();
    let mut current = payload;
    for part in parts {
        match current {
            serde_json::Value::Object(map) => {
                current = map.get(part).unwrap_or(&serde_json::Value::Null);
            }
            serde_json::Value::Array(arr) => {
                if let Ok(idx) = part.parse::<usize>() {
                    current = arr.get(idx).unwrap_or(&serde_json::Value::Null);
                } else {
                    return String::new();
                }
            }
            _ => return String::new(),
        }
    }
    match current {
        serde_json::Value::String(s) => s.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

pub struct AssignmentEngine {
    rules: Arc<RwLock<Vec<AssignmentRule>>>,
    transport: ChatTransport,
    active_subscription: Mutex<Option<tokio::task::JoinHandle<()>>>,
    timer_handles: Mutex<HashMap<String, tokio::task::JoinHandle<()>>>,
}

impl AssignmentEngine {
    pub fn new(transport: ChatTransport) -> Self {
        Self {
            rules: Arc::new(RwLock::new(Vec::new())),
            transport,
            active_subscription: Mutex::new(None),
            timer_handles: Mutex::new(HashMap::new()),
        }
    }

    pub async fn add_rule(&self, rule: AssignmentRule) -> ChatResult<()> {
        let mut rules = self.rules.write().await;
        rules.push(rule);
        Ok(())
    }

    pub async fn remove_rule(&self, id: &str) -> ChatResult<Option<AssignmentRule>> {
        let mut rules = self.rules.write().await;
        if let Some(pos) = rules.iter().position(|r| r.id == id) {
            Ok(Some(rules.remove(pos)))
        } else {
            Ok(None)
        }
    }

    pub async fn list_rules(&self) -> Vec<AssignmentRule> {
        self.rules.read().await.clone()
    }

    pub async fn get_rule(&self, id: &str) -> Option<AssignmentRule> {
        self.rules.read().await.iter().find(|r| r.id == id).cloned()
    }

    pub async fn evaluate(&self, notification: &NotificationMessage) -> Vec<TaskMessage> {
        let rules = self.rules.read().await;
        rules
            .iter()
            .filter(|r| r.matches(notification))
            .map(|r| r.build_task(notification))
            .collect()
    }

    pub async fn start(&self) -> ChatResult<()> {
        self.start_event_loop().await?;
        self.start_timer_loops().await?;
        Ok(())
    }

    async fn start_event_loop(&self) -> ChatResult<()> {
        let mut rx = self
            .transport
            .subscribe_notifications("b00t.notify.>")
            .await?;
        let rules = self.rules.clone();
        let transport = self.transport.clone();

        let handle = tokio::spawn(async move {
            info!("AssignmentEngine event loop started on b00t.notify.>");
            while let Some(notification) = rx.recv().await {
                debug!(
                    "AssignmentEngine event: {}.{}",
                    notification.source, notification.event_type
                );
                let mut rules = rules.write().await;
                let matching: Vec<&mut AssignmentRule> = rules
                    .iter_mut()
                    .filter(|r| {
                        r.trigger == TriggerKind::Event && r.matches(&notification) && r.enabled
                    })
                    .collect();

                for rule in matching {
                    let task = rule.build_task(&notification);
                    info!(
                        "AssignmentEngine: event rule '{}' matched, dispatching task {}",
                        rule.name, task.task_id
                    );
                    if let Err(e) = transport.send_task(&task).await {
                        warn!("AssignmentEngine: task dispatch failed: {}", e);
                        continue;
                    }
                    rule.last_triggered = Some(Utc::now());
                    if !rule.repeat {
                        rule.enabled = false;
                    }
                }
            }
            warn!("AssignmentEngine event loop ended");
        });

        let mut sub = self.active_subscription.lock().await;
        *sub = Some(handle);
        Ok(())
    }

    async fn start_timer_loops(&self) -> ChatResult<()> {
        let rules = self.rules.read().await;
        let timer_rules: Vec<AssignmentRule> = rules
            .iter()
            .filter(|r| r.trigger == TriggerKind::Timer && r.timer_spec.is_some())
            .cloned()
            .collect();

        let transport = self.transport.clone();
        let rules_arc = self.rules.clone();
        let mut timer_handles = self.timer_handles.lock().await;

        for rule in timer_rules {
            let timer_spec = match &rule.timer_spec {
                Some(spec) => spec.clone(),
                None => continue,
            };

            let rule_id = rule.id.clone();
            let rule_name = rule.name.clone();
            let action = rule.action.clone();
            let repeat = rule.repeat;
            let transport = transport.clone();
            let rules = rules_arc.clone();

            let handle = tokio::spawn(async move {
                let synthetic_notification = NotificationMessage::new(
                    "timer",
                    rule_id.clone(),
                    serde_json::json!({"rule": rule_name.clone()}),
                );

                match timer_spec {
                    TimerSpec::Interval { seconds } => {
                        info!("Timer '{}' started (interval: {}s)", rule_name, seconds);
                        loop {
                            tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
                            let task = TaskMessage {
                                task_id: uuid::Uuid::new_v4().to_string(),
                                action: action.action.clone(),
                                payload: action.render(&synthetic_notification),
                                from_agent: "assignment-engine-timer".to_string(),
                                to_agent: action.to_agent.clone(),
                                deadline: None,
                                priority: "normal".to_string(),
                                timestamp: Utc::now(),
                            };
                            if let Err(e) = transport.send_task(&task).await {
                                warn!("Timer task dispatch failed: {}", e);
                                continue;
                            }
                            info!("Timer '{}' fired, task {}", rule_name, task.task_id);
                            {
                                let mut rules = rules.write().await;
                                if let Some(r) = rules.iter_mut().find(|r| r.id == rule_id) {
                                    r.last_triggered = Some(Utc::now());
                                }
                            }
                            if !repeat {
                                let mut rules = rules.write().await;
                                if let Some(r) = rules.iter_mut().find(|r| r.id == rule_id) {
                                    r.enabled = false;
                                }
                                break;
                            }
                        }
                    }
                    TimerSpec::Cron { expr } => {
                        let schedule = match expr.parse::<cron::Schedule>() {
                            Ok(s) => s,
                            Err(e) => {
                                warn!(
                                    "Invalid cron expression '{}' for rule '{}': {}",
                                    expr, rule_name, e
                                );
                                return;
                            }
                        };
                        info!("Timer '{}' started (cron: {})", rule_name, expr);
                        loop {
                            let next = match schedule.upcoming(Utc).next() {
                                Some(t) => t,
                                None => {
                                    warn!("Cron schedule exhausted for rule '{}'", rule_name);
                                    break;
                                }
                            };
                            let delay = (next - Utc::now())
                                .to_std()
                                .unwrap_or(std::time::Duration::from_secs(1));
                            tokio::time::sleep(delay).await;

                            let task = TaskMessage {
                                task_id: uuid::Uuid::new_v4().to_string(),
                                action: action.action.clone(),
                                payload: action.render(&synthetic_notification),
                                from_agent: "assignment-engine-timer".to_string(),
                                to_agent: action.to_agent.clone(),
                                deadline: None,
                                priority: "normal".to_string(),
                                timestamp: Utc::now(),
                            };
                            if let Err(e) = transport.send_task(&task).await {
                                warn!("Cron timer task dispatch failed: {}", e);
                                continue;
                            }
                            info!("Cron timer '{}' fired, task {}", rule_name, task.task_id);
                            {
                                let mut rules = rules.write().await;
                                if let Some(r) = rules.iter_mut().find(|r| r.id == rule_id) {
                                    r.last_triggered = Some(Utc::now());
                                }
                            }
                            if !repeat {
                                let mut rules = rules.write().await;
                                if let Some(r) = rules.iter_mut().find(|r| r.id == rule_id) {
                                    r.enabled = false;
                                }
                                break;
                            }
                        }
                    }
                }
            });

            timer_handles.insert(rule.id.clone(), handle);
        }

        Ok(())
    }

    pub async fn stop(&self) {
        let mut sub = self.active_subscription.lock().await;
        if let Some(handle) = sub.take() {
            handle.abort();
            info!("AssignmentEngine stopped");
        }

        let mut timers = self.timer_handles.lock().await;
        for (_, handle) in timers.drain() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod assignment_tests {
    use super::*;

    #[test]
    fn test_condition_operator_eq() {
        assert!(ConditionOp::Eq.evaluate("hello", "hello"));
        assert!(!ConditionOp::Eq.evaluate("hello", "world"));
    }

    #[test]
    fn test_condition_operator_contains() {
        assert!(ConditionOp::Contains.evaluate("hello world", "world"));
        assert!(!ConditionOp::Contains.evaluate("hello world", "moon"));
    }

    #[test]
    fn test_condition_operator_regex() {
        assert!(ConditionOp::Regex.evaluate("CI failed", "CI.*"));
        assert!(!ConditionOp::Regex.evaluate("build ok", "CI.*"));
    }

    #[test]
    fn test_matches_subject_exact() {
        let n = NotificationMessage::new("gmail", "new_email", serde_json::json!({}));
        let rule = AssignmentRule::new(
            "r1",
            "email alerts",
            TriggerKind::Event,
            "b00t.notify.gmail.new_email",
            TaskTemplate {
                to_agent: "bot".into(),
                action: "alert".into(),
                payload_template: serde_json::json!({}),
            },
        );
        assert!(rule.matches(&n));
    }

    #[test]
    fn test_matches_subject_wildcard() {
        let n = NotificationMessage::new("gmail", "new_email", serde_json::json!({}));
        let rule = AssignmentRule::new(
            "r1",
            "all gmail",
            TriggerKind::Event,
            "b00t.notify.gmail.>",
            TaskTemplate {
                to_agent: "bot".into(),
                action: "alert".into(),
                payload_template: serde_json::json!({}),
            },
        );
        assert!(rule.matches(&n));
    }

    #[test]
    fn test_matches_subject_no_match() {
        let n = NotificationMessage::new("slack", "new_message", serde_json::json!({}));
        let rule = AssignmentRule::new(
            "r1",
            "gmail only",
            TriggerKind::Event,
            "b00t.notify.gmail.>",
            TaskTemplate {
                to_agent: "bot".into(),
                action: "alert".into(),
                payload_template: serde_json::json!({}),
            },
        );
        assert!(!rule.matches(&n));
    }

    #[test]
    fn test_matches_with_condition() {
        let n = NotificationMessage::new(
            "gmail",
            "new_email",
            serde_json::json!({"from": "alert@ci.com", "subject": "CI failed"}),
        );
        let rule = AssignmentRule::new(
            "r1",
            "CI alerts",
            TriggerKind::Event,
            "b00t.notify.gmail.>",
            TaskTemplate {
                to_agent: "bot".into(),
                action: "alert".into(),
                payload_template: serde_json::json!({}),
            },
        )
        .with_condition(Condition {
            field: "from".to_string(),
            operator: ConditionOp::Contains,
            value: "alert@".to_string(),
        });
        assert!(rule.matches(&n));
    }

    #[test]
    fn test_matches_with_condition_no_match() {
        let n = NotificationMessage::new(
            "gmail",
            "new_email",
            serde_json::json!({"from": "friend@personal.com"}),
        );
        let rule = AssignmentRule::new(
            "r1",
            "CI alerts only",
            TriggerKind::Event,
            "b00t.notify.gmail.>",
            TaskTemplate {
                to_agent: "bot".into(),
                action: "alert".into(),
                payload_template: serde_json::json!({}),
            },
        )
        .with_condition(Condition {
            field: "from".to_string(),
            operator: ConditionOp::Contains,
            value: "alert@".to_string(),
        });
        assert!(!rule.matches(&n));
    }

    #[test]
    fn test_disabled_rule_does_not_match() {
        let n = NotificationMessage::new("gmail", "new_email", serde_json::json!({}));
        let mut rule = AssignmentRule::new(
            "r1",
            "disabled",
            TriggerKind::Event,
            "b00t.notify.gmail.>",
            TaskTemplate {
                to_agent: "bot".into(),
                action: "alert".into(),
                payload_template: serde_json::json!({}),
            },
        );
        rule.enabled = false;
        assert!(!rule.matches(&n));
    }

    #[test]
    fn test_task_template_interpolation() {
        let n = NotificationMessage::new(
            "files",
            "new_file",
            serde_json::json!({"path": "/data/report.pdf", "size": 1024}),
        );
        let template = TaskTemplate {
            to_agent: "doc-processor".into(),
            action: "review".into(),
            payload_template: serde_json::json!({
                "source": "{event.source}",
                "type": "{event.type}",
                "event_data": "{event.payload}"
            }),
        };
        let rendered = template.render(&n);
        assert_eq!(rendered["source"], "files");
        assert_eq!(rendered["type"], "new_file");
    }

    #[tokio::test]
    async fn test_engine_evaluate_multiple_rules() {
        use crate::transport::ChatTransportConfig;
        let config = ChatTransportConfig::default();
        let transport = ChatTransport::from_config(config).expect("default transport");
        let engine = AssignmentEngine::new(transport);

        engine
            .add_rule(AssignmentRule::new(
                "r1",
                "all gmail",
                TriggerKind::Event,
                "b00t.notify.gmail.>",
                TaskTemplate {
                    to_agent: "handler1".into(),
                    action: "process".into(),
                    payload_template: serde_json::json!({}),
                },
            ))
            .await
            .unwrap();

        engine
            .add_rule(AssignmentRule::new(
                "r2",
                "slack only",
                TriggerKind::Event,
                "b00t.notify.slack.>",
                TaskTemplate {
                    to_agent: "handler2".into(),
                    action: "process".into(),
                    payload_template: serde_json::json!({}),
                },
            ))
            .await
            .unwrap();

        let n = NotificationMessage::new("gmail", "new_email", serde_json::json!({}));
        let tasks = engine.evaluate(&n).await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].to_agent, "handler1");

        let n2 = NotificationMessage::new("slack", "new_dm", serde_json::json!({}));
        let tasks2 = engine.evaluate(&n2).await;
        assert_eq!(tasks2.len(), 1);
        assert_eq!(tasks2[0].to_agent, "handler2");
    }

    #[test]
    fn test_get_payload_field_nested() {
        let payload = serde_json::json!({"email": {"from": "a@b.com", "subject": "hi"}});
        assert_eq!(get_payload_field(&payload, "email/from"), "a@b.com");
    }

    #[test]
    fn test_timer_spec_interval() {
        let spec = TimerSpec::interval_secs(300);
        match spec {
            TimerSpec::Interval { seconds } => assert_eq!(seconds, 300),
            _ => panic!("expected Interval"),
        }
    }

    #[test]
    fn test_timer_spec_cron() {
        let spec = TimerSpec::cron("0 9 * * 1-5");
        match spec {
            TimerSpec::Cron { ref expr } => assert_eq!(expr, "0 9 * * 1-5"),
            _ => panic!("expected Cron"),
        }
    }

    #[test]
    fn test_rule_with_timer_builder() {
        let rule = AssignmentRule::new(
            "r1",
            "daily summary",
            TriggerKind::Event,
            "unused",
            TaskTemplate {
                to_agent: "reporter".into(),
                action: "summarize".into(),
                payload_template: serde_json::json!({}),
            },
        )
        .with_timer(TimerSpec::interval_secs(3600));

        assert_eq!(rule.trigger, TriggerKind::Timer);
        assert!(rule.timer_spec.is_some());
    }

    #[test]
    fn test_timer_rule_matches_event_filter() {
        let n = NotificationMessage::new("timer", "anything", serde_json::json!({}));
        let rule = AssignmentRule::new(
            "r1",
            "timer rule",
            TriggerKind::Timer,
            "b00t.notify.>",
            TaskTemplate {
                to_agent: "bot".into(),
                action: "act".into(),
                payload_template: serde_json::json!({}),
            },
        )
        .with_timer(TimerSpec::interval_secs(60));
        assert!(!rule.matches(&n));
    }
}
