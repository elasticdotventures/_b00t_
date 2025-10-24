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
        for msg in &messages {
            info!(
                channel = %msg.channel,
                sender = %msg.sender,
                body = %msg.body,
                "chat message queued"
            );
        }
        format!("<🥾>{{ \"chat\": {{ \"msgs\": {} }} }}</🥾>", count)
    }

    pub async fn unread_count(&self) -> usize {
        self.inbox.unread_count().await
    }
}
