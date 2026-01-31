use anyhow::{Context, Result};
use clap::Subcommand;
use clap::ValueEnum;
use serde_json::Value;

use b00t_chat::{ChatClient, ChatMessage, ChatTransportConfig, ChatTransportKind};

#[derive(Debug, Clone, ValueEnum)]
pub enum TransportArg {
    Local,
    Nats,
}

impl From<TransportArg> for ChatTransportKind {
    fn from(value: TransportArg) -> ChatTransportKind {
        match value {
            TransportArg::Local => ChatTransportKind::LocalSocket,
            TransportArg::Nats => ChatTransportKind::Nats,
        }
    }
}

/// Chat-centric commands for coordinating with other agents.
#[derive(Subcommand, Debug)]
pub enum ChatCommands {
    /// Send a chat message to the coordination socket or NATS stub.
    Send {
        /// Target chat channel (defaults to user's namespace).
        #[arg(short, long)]
        channel: Option<String>,
        /// Body of the chat message.
        #[arg(short, long)]
        message: String,
        /// Transport backend selection (local socket or NATS).
        #[arg(short, long, default_value = "local")]
        transport: TransportArg,
        /// Optional sender override (defaults to current username).
        #[arg(long)]
        sender: Option<String>,
        /// Optional JSON metadata payload appended to the message.
        #[arg(long)]
        metadata: Option<String>,
    },

    /// Display chat transport information.
    Info,
}

impl ChatCommands {
    pub async fn execute(&self) -> Result<()> {
        match self {
            ChatCommands::Send {
                channel,
                message,
                transport,
                sender,
                metadata,
            } => {
                self.send_message(channel, message, transport, sender, metadata)
                    .await
            }
            ChatCommands::Info => self.show_info().await,
        }
    }

    async fn send_message(
        &self,
        channel: &Option<String>,
        message: &String,
        transport: &TransportArg,
        sender: &Option<String>,
        metadata: &Option<String>,
    ) -> Result<()> {
        let transport_kind: ChatTransportKind = (*transport).clone().into();

        let nats_url = if matches!(transport_kind, ChatTransportKind::Nats) {
            std::env::var("NATS_URL").ok()
        } else {
            None
        };

        let config: ChatTransportConfig = ChatTransportConfig {
            kind: transport_kind,
            socket_path: None,
            nats_url,
        };

        let client = ChatClient::new(config).context("failed to initialize chat client")?;

        let resolved_sender = sender.clone().unwrap_or_else(|| whoami::username());
        let resolved_channel = channel
            .clone()
            .unwrap_or_else(|| format!("account.{}", whoami::username()));

        let mut chat_message = ChatMessage::new(&resolved_channel, resolved_sender, message);

        if let Some(meta_raw) = metadata {
            let meta: Value =
                serde_json::from_str(meta_raw).context("metadata must be valid JSON")?;
            chat_message.metadata = meta;
        }

        client
            .send(&chat_message)
            .await
            .context("failed to deliver chat message")?;

        println!(
            "🥾 Sent chat message via {} → {}",
            client.transport_kind(),
            resolved_channel
        );

        Ok(())
    }

    async fn show_info(&self) -> Result<()> {
        let socket = b00t_chat::default_socket_path()?;
        println!("🥾 Local chat socket: {}", socket.display());
        println!("📡 Available transports: local, nats (stub)");
        Ok(())
    }
}
