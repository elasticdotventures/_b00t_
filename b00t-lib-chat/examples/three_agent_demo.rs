//! Three-agent chat demo using the local inbox.
//!
//! Two producer tasks post chat messages; a coordinator drains the inbox
//! showing how unread messages surface inside the b00t MCP runtime.

use b00t_chat::{ChatInbox, ChatMessage};
use rand::{thread_rng, Rng};
use tokio::time::{sleep, Duration};
use tracing::{info, Level};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let inbox = ChatInbox::new();

    let producers = ["frontend", "backend"];
    let mut joins = Vec::new();

    for agent in producers {
        let inbox = inbox.clone();
        let agent_name = agent.to_string();
        joins.push(tokio::spawn(async move {
            for step in 1..=3 {
                let value: u32 = thread_rng().gen_range(10..50);
                let body = format!("step {} update → {}", step, value);
                let msg = ChatMessage::new("mission.gamma", format!("{agent_name}.agent"), body);
                inbox.push(msg).await;
                sleep(Duration::from_millis(50)).await;
            }
        }));
    }

    for handle in joins {
        handle.await?;
    }

    let unread = inbox.drain().await;
    for msg in unread {
        info!(
            channel = %msg.channel,
            sender = %msg.sender,
            body = %msg.body,
            "📨 chat message"
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inbox_flow() {
        let inbox = ChatInbox::new();
        let msg = ChatMessage::new("mission.test", "tester", "ping");
        inbox.push(msg).await;

        let unread = inbox.drain().await;
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].channel, "mission.test");
    }
}
