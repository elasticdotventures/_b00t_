//! b00t-forge-kv — a RESP2-speaking key-value server that's a real drop-in
//! for `b00t-c0re-lib::redis::RedisComms` (the `redis` crate client used by
//! agent coordination). Gives `KvBackend::ForgeKV` (b00t-c0re-lib/src/kv_store.rs)
//! an actual server to detect, instead of just being a fallback label.
//!
//! Deployment model matches the existing NATS-on-Vultr pattern
//! (nats/vultr-node-setup.sh): bind 127.0.0.1 only, reach it over an SSH
//! tunnel — never expose the KV port publicly. Nothing outside this
//! process talks RESP2 to the outside world; agents never connect to it
//! directly (they go through NATS; the historian is the one thing that
//! may use this store).

use std::sync::Arc;

use clap::Parser;
use tokio::net::TcpListener;

use b00t_forge_kv::{serve, store::Store};

#[derive(Parser)]
#[command(name = "b00t-forge-kv", about = "RESP2-compatible KV server — b00t-native Redis/Valkey replacement for hive nodes")]
struct Args {
    /// Bind address. Per the vultr-node-setup.sh convention, this should
    /// stay 127.0.0.1 on any internet-facing host — reach it via an SSH
    /// tunnel, never expose it publicly.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 6379)]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let addr = format!("{}:{}", args.host, args.port);
    let listener = TcpListener::bind(&addr).await.map_err(|e| {
        anyhow::anyhow!(
            "b00t-forge-kv: failed to bind {addr}: {e} — is another process (redis-server, valkey-server, or a previous b00t-forge-kv) already listening on this port?"
        )
    })?;
    tracing::info!(%addr, "b00t-forge-kv listening (RESP2)");

    serve(listener, Arc::new(Store::new())).await;
    Ok(())
}
