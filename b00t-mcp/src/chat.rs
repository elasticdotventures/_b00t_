use once_cell::sync::OnceCell;
use tracing::{error, info};

use b00t_chat::{ChatInbox, spawn_local_server};

#[derive(Clone, Debug)]
pub struct ChatRuntime {
    inbox: ChatInbox,
}

impl ChatRuntime {
    pub fn global() -> Self {
        static INSTANCE: OnceCell<ChatRuntime> = OnceCell::new();
        INSTANCE
            .get_or_init(|| {
                let inbox = ChatInbox::new();
                let inbox_clone = inbox.clone();

                tokio::spawn(async move {
                    if let Err(err) = spawn_local_server(inbox_clone).await {
                        error!("chat server failed: {}", err);
                    } else {
                        info!("local chat server ready");
                    }
                });

                ChatRuntime { inbox }
            })
            .clone()
    }

    pub fn inbox(&self) -> ChatInbox {
        self.inbox.clone()
    }

    pub async fn drain_indicator(&self) -> String {
        let messages = self.inbox.drain().await;
        let count = messages.len();
        if messages.is_empty() {
            return String::new();
        }

        for msg in &messages {
            info!(
                channel = %msg.channel,
                sender = %msg.sender,
                body = %msg.body,
                "chat message queued"
            );
        }

        let displayed_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|msg| {
                serde_json::json!({
                    "channel": msg.channel,
                    "sender": msg.sender,
                    "body": msg.body,
                })
            })
            .collect();
        let payload = serde_json::json!({
            "chat": {
                "msgs": count,
                "messages": displayed_messages,
            }
        });

        format!("<🥾>{payload}</🥾>")
    }

    pub async fn unread_count(&self) -> usize {
        self.inbox.unread_count().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use b00t_chat::ChatMessage;

    #[tokio::test]
    async fn empty_inbox_emits_no_indicator() {
        let runtime = ChatRuntime {
            inbox: ChatInbox::new(),
        };

        assert_eq!(runtime.drain_indicator().await, "");
    }

    #[tokio::test]
    async fn nonempty_inbox_displays_message_content() {
        let inbox = ChatInbox::new();
        inbox
            .push(ChatMessage::new(
                "a2a.review",
                "worker-1",
                "ready for review",
            ))
            .await;
        let runtime = ChatRuntime { inbox };

        let output = runtime.drain_indicator().await;
        let payload = output
            .strip_prefix("<🥾>")
            .and_then(|value| value.strip_suffix("</🥾>"))
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(payload).unwrap();

        assert_eq!(value["chat"]["msgs"], 1);
        assert_eq!(value["chat"]["messages"][0]["channel"], "a2a.review");
        assert_eq!(value["chat"]["messages"][0]["sender"], "worker-1");
        assert_eq!(value["chat"]["messages"][0]["body"], "ready for review");
    }
}
