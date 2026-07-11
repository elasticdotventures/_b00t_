//! `b00t-comms` mesh CLI — realizes `--agent=b00t-comms --skill=nats`.
//!
//! Run two terminals against a reachable NATS server:
//!
//! ```text
//! # terminal 1
//! cargo run -p b00t-chat --example mesh_cli -- node1 nats://localhost:4222 listen
//! # terminal 2
//! cargo run -p b00t-chat --example mesh_cli -- node2 nats://localhost:4222 discover
//! cargo run -p b00t-chat --example mesh_cli -- node2 nats://localhost:4222 send node1 "hello mesh"
//! ```
//!
//! `discover` publishes a query and prints live peers. `listen` joins the
//! default `b00t-comms` channel, announces presence, and prints every frame it
//! receives (including auto-replies to discovery queries from other nodes).

use b00t_chat::ledgrrr::{LocalLedgrrr, Ledgrrr};
use b00t_chat::mesh::{MeshFrame, MeshNodeConfig, NatsMeshNode};
use b00t_chat::message::ChatMessage;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let agent_id = args.next().unwrap_or_else(|| "b00t-comms".to_string());
    let nats_url = args.next().unwrap_or_else(|| "nats://localhost:4222".to_string());
    let command = args.next().unwrap_or_else(|| "discover".to_string());

    // Optional local finops ledger (collaborative-autonomy receipts).
    let ledger: Option<Arc<LocalLedgrrr>> =
        match std::env::var("B00T_LEDGER").ok().filter(|s| !s.is_empty()) {
            Some(path) => Some(Arc::new(LocalLedgrrr::file(&path)?)),
            None => None,
        };
    let project = std::env::var("B00T_PROJECT").unwrap_or_else(|_| "b00t-comms".to_string());

    let mut config = MeshNodeConfig::new(agent_id.clone(), nats_url)
        .with_role("b00t-comms")
        .with_skills(vec!["nats".to_string()])
        .with_project(project.clone());
    if let Some(l) = &ledger {
        config = config.with_ledgrrr(l.clone() as Arc<dyn Ledgrrr>);
    }

    let node = NatsMeshNode::new(config);
    node.connect().await?;
    node.announce().await?;

    match command.as_str() {
        "discover" => {
            let peers = node.discover().await?;
            println!("discovered {} peer(s):", peers.len());
            for p in &peers {
                println!("  - {} ({})", p.agent_id, p.endpoint_uri);
            }
        }
        "listen" => {
            node.join("b00t-comms").await?;
            node.start_presence().await;
            println!("{agent_id} listening on mesh channel 'b00t-comms' (ctrl-c to stop)");
            while let Some(frame) = node.recv().await? {
                match frame {
                    MeshFrame::Direct(m) => println!("[direct] {}: {}", m.sender, m.body),
                    MeshFrame::Broadcast { channel, message } => {
                        println!("[broadcast:{channel}] {}: {}", message.sender, message.body)
                    }
                    MeshFrame::Presence(p) => println!("[presence] {} role={}", p.agent_id, p.role),
                    MeshFrame::DiscoveryReply { endpoint, .. } => {
                        println!("[discovery-reply] {}", endpoint.agent_id)
                    }
                    MeshFrame::DiscoveryQuery { from, .. } => {
                        println!("[discovery-query] from {from}")
                    }
                }
            }
        }
        "send" => {
            let to = args.next().expect("send requires <to-agent>");
            let body = args.next().unwrap_or_else(|| "ping".to_string());
            let msg = ChatMessage::new("b00t-comms", &agent_id, body);
            node.send(&to, &msg).await?;
            println!("sent direct message to {to}");
        }
        "broadcast" => {
            let body = args.next().unwrap_or_else(|| "hello mesh".to_string());
            let msg = ChatMessage::new("b00t-comms", &agent_id, body);
            node.publish("b00t-comms", &msg).await?;
            println!("broadcast on channel 'b00t-comms'");
        }
        other => {
            eprintln!("unknown command: {other} (discover|listen|send|broadcast)");
            std::process::exit(2);
        }
    }

    if let Some(l) = &ledger {
        let codes = l.codes_for(&project);
        println!("finops: {project} -> {} receipt(s) minted", codes.len());
        for c in &codes {
            println!("  - {c}");
        }
    }

    node.close().await?;
    Ok(())
}
