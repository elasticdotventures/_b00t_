//! Library surface for b00t-forge-kv, so integration tests (and any future
//! embedder — e.g. running ForgeKV in-process rather than as a subprocess)
//! can drive the server without shelling out to the binary.

pub mod commands;
pub mod resp;
pub mod server;
pub mod store;

use std::sync::Arc;

use tokio::net::TcpListener;

use store::Store;

/// Runs the accept loop against an already-bound listener. Exists
/// separately from `main()` so tests can bind an ephemeral port
/// (`127.0.0.1:0`) and drive the real server in-process.
pub async fn serve(listener: TcpListener, store: Arc<Store>) {
    loop {
        let (socket, peer) = match listener.accept().await {
            Ok(pair) => pair,
            Err(e) => {
                tracing::warn!(error = %e, "accept() failed, continuing");
                continue;
            }
        };
        tracing::debug!(%peer, "connection accepted");
        let store = Arc::clone(&store);
        tokio::spawn(async move {
            server::handle_connection(socket, store).await;
        });
    }
}
