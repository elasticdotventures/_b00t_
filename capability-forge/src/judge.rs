use async_openai::{types::CreateChatCompletionRequestArgs, Client};
use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JudgeOutcome {
    Granted,
    Denied { reason: String },
}

#[async_trait]
pub trait EscalationJudge: Send + Sync {
    async fn judge(
        &self,
        agent_id: &str,
        skill: &str,
        skill_description: &str,
        justification: &str,
    ) -> JudgeOutcome;
}

pub struct FakeJudge {
    pub decision: JudgeOutcome,
}

impl FakeJudge {
    pub fn always_grant() -> Self {
        Self { decision: JudgeOutcome::Granted }
    }

    pub fn always_deny(reason: &str) -> Self {
        Self { decision: JudgeOutcome::Denied { reason: reason.to_string() } }
    }
}

#[async_trait]
impl EscalationJudge for FakeJudge {
    async fn judge(&self, _agent_id: &str, _skill: &str, _skill_description: &str, _justification: &str) -> JudgeOutcome {
        self.decision.clone()
    }
}

#[derive(Deserialize)]
struct JudgeResponse {
    granted: bool,
    reason: String,
}

pub struct OpenAiJudge {
    client: Client<async_openai::config::OpenAIConfig>,
    model: String,
    timeout: Duration,
}

impl OpenAiJudge {
    pub fn new(model: impl Into<String>) -> Self {
        Self { client: Client::new(), model: model.into(), timeout: Duration::from_secs(15) }
    }
}

#[async_trait]
impl EscalationJudge for OpenAiJudge {
    async fn judge(&self, agent_id: &str, skill: &str, skill_description: &str, justification: &str) -> JudgeOutcome {
        let prompt = format!(
            "Agent '{agent_id}' requests skill '{skill}' ({skill_description}). \
             Justification: {justification}. \
             Reply with ONLY a JSON object: {{\"granted\": bool, \"reason\": string}}."
        );

        let request = match CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages([async_openai::types::ChatCompletionRequestUserMessageArgs::default()
                .content(prompt)
                .build()
                .expect("static message construction cannot fail")
                .into()])
            .build()
        {
            Ok(r) => r,
            Err(e) => return JudgeOutcome::Denied { reason: format!("request build failed: {e}") },
        };

        // `async-openai`'s `OpenAIConfig::headers()` builds the Authorization header
        // with `.parse().unwrap()` on the raw API key — a malformed OPENAI_API_KEY
        // (stray newline/non-ASCII byte from a bad secrets-injection path) panics
        // *inside* the client call, not as a returned Err. Running the call in a
        // spawned task turns that into a catchable JoinError instead of an unwind
        // through this function, preserving fail-closed even against that panic.
        let chat_client = self.client.clone();
        let timeout = self.timeout;
        let handle = tokio::spawn(async move {
            let chat = chat_client.chat();
            let call = chat.create(request);
            tokio::time::timeout(timeout, call).await
        });

        let response = match handle.await {
            Ok(Ok(Ok(r))) => r,
            Ok(Ok(Err(e))) => return JudgeOutcome::Denied { reason: format!("llm call failed: {e}") },
            Ok(Err(_)) => return JudgeOutcome::Denied { reason: "llm call timed out".into() },
            Err(join_err) => {
                return JudgeOutcome::Denied {
                    reason: format!("llm call panicked or was cancelled: {join_err}"),
                }
            }
        };

        let content = response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_default();

        match serde_json::from_str::<JudgeResponse>(&content) {
            Ok(parsed) if parsed.granted => JudgeOutcome::Granted,
            Ok(parsed) => JudgeOutcome::Denied { reason: parsed.reason },
            Err(e) => JudgeOutcome::Denied { reason: format!("malformed llm response: {e}") },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fake_judge_always_grant_grants() {
        let j = FakeJudge::always_grant();
        assert_eq!(j.judge("a", "s", "d", "j").await, JudgeOutcome::Granted);
    }

    #[tokio::test]
    async fn fake_judge_always_deny_denies_with_reason() {
        let j = FakeJudge::always_deny("no");
        assert_eq!(j.judge("a", "s", "d", "j").await, JudgeOutcome::Denied { reason: "no".into() });
    }
}
