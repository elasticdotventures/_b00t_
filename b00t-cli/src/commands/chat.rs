use anyhow::{Context, Result};
use b00t_chat::{ChatClient, ChatError, ChatMessage, ChatTransportConfig, ChatTransportKind};
use clap::Subcommand;
use clap::ValueEnum;
use serde_json::Value;

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
            nats_user: std::env::var("B00T_HIVE_NATS_USER").ok(),
            nats_password: std::env::var("B00T_HIVE_NATS_PASSWORD").ok(),
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

        match client.send(&chat_message).await {
            Ok(()) => {
                println!(
                    "🥾 Sent chat message via {} → {}",
                    client.transport_kind(),
                    resolved_channel
                );
            }
            Err(ChatError::NotConnected) => {
                // ACP fire-and-forget: no listener on socket is not an error
                eprintln!(
                    "⚠️  No listener on chat socket (stale or b00t-mcp not running). \
                     Message dropped — start b00t-mcp to receive chat."
                );
            }
            Err(e) => {
                return Err(e).context("failed to deliver chat message");
            }
        }

        Ok(())
    }

    async fn show_info(&self) -> Result<()> {
        let socket = b00t_chat::default_socket_path()?;
        println!("🥾 Local chat socket: {}", socket.display());
        println!("📡 Available transports: local, nats (stub)");
        // Check if socket exists and whether a listener is active
        if socket.exists() {
            // Attempt a probe connect (0.2s timeout) to distinguish live vs stale
            let probe = tokio::time::timeout(
                std::time::Duration::from_millis(200),
                tokio::net::UnixStream::connect(&socket),
            )
            .await;
            match probe {
                Ok(Ok(_)) => println!("✅ Socket active — listener is running"),
                Ok(Err(_)) | Err(_) => {
                    println!(
                        "⚠️  Socket file exists but no listener responds. \
                         Run `b00t-mcp` or `b00t hive activate` to start one."
                    );
                }
            }
        } else {
            println!("ℹ️  Socket does not exist yet (no b00t-mcp process started)");
        }
        Ok(())
    }
}
