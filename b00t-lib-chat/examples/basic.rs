//! Basic example of b00t chat usage.

use b00t_chat::{ChatClient, ChatMessage};
use tracing::{info, Level};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting b00t-chat basic example");

    // Use the NATS transport stub so the example runs without extra setup.
    let client = ChatClient::nats(None)?;

    let message = ChatMessage::new("mission.example", "basic-demo", "hello from b00t-chat");
    client.send(&message).await?;

    info!("Sent chat message to mission.example");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_message_creation() {
        let msg = ChatMessage::new("mission.alpha", "tester", "ping");
        assert_eq!(msg.channel, "mission.alpha");
        assert_eq!(msg.sender, "tester");
        assert_eq!(msg.body, "ping");
    }
}
