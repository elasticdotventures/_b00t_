//! Per-connection handling: request/reply dispatch, plus SUBSCRIBE mode
//! (RESP2 pub/sub — once a connection subscribes, it stops accepting
//! regular commands and instead receives pushed `message` frames, matching
//! how the `redis` crate's `aio::PubSub` actually drives a connection).

use std::sync::Arc;

use tokio::io::BufReader;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

use crate::commands::dispatch;
use crate::resp::{read_command, write_reply, RespValue};
use crate::store::Store;

pub async fn handle_connection(socket: TcpStream, store: Arc<Store>) {
    let peer = socket.peer_addr().map(|a| a.to_string()).unwrap_or_default();
    let (read_half, mut write_half) = socket.into_split();
    let mut reader = BufReader::new(read_half);

    loop {
        let args = match read_command(&mut reader).await {
            Ok(Some(args)) if !args.is_empty() => args,
            Ok(Some(_)) => continue, // empty multibulk (e.g. "*0\r\n") — ignore
            Ok(None) => {
                tracing::debug!(%peer, "connection closed");
                return;
            }
            Err(e) => {
                tracing::warn!(%peer, error = %e, "malformed request, closing connection");
                let _ = write_reply(&mut write_half, &RespValue::Error(format!("ERR Protocol error: {e}"))).await;
                return;
            }
        };

        let cmd_name = String::from_utf8_lossy(&args[0]).to_ascii_uppercase();
        if cmd_name == "SUBSCRIBE" {
            let channels: Vec<String> = args[1..]
                .iter()
                .map(|c| String::from_utf8_lossy(c).to_string())
                .collect();
            if channels.is_empty() {
                let _ = write_reply(&mut write_half, &RespValue::Error("ERR wrong number of arguments for 'subscribe' command".into())).await;
                continue;
            }
            if run_subscribe_loop(&mut reader, &mut write_half, &store, channels).await.is_err() {
                tracing::debug!(%peer, "subscriber connection closed");
            }
            return; // matches real Redis clients: pub/sub is a connection's terminal mode
        }

        let reply = dispatch(&store, &args).await;
        if write_reply(&mut write_half, &reply).await.is_err() {
            tracing::debug!(%peer, "write failed, closing connection");
            return;
        }
    }
}

/// Drives a connection once it has entered pub/sub mode: sends a
/// `subscribe` confirmation per channel, then relays published messages
/// until the client disconnects. Further SUBSCRIBE/UNSUBSCRIBE/PING on the
/// same connection are intentionally not supported — RedisComms's actual
/// usage (`subscriber.subscribe(&channels).await?` once, then a read loop)
/// never needs it, and real Redis clients open pub/sub as a dedicated
/// connection for exactly this lifetime.
async fn run_subscribe_loop(
    reader: &mut crate::resp::Reader,
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    store: &Arc<Store>,
    channels: Vec<String>,
) -> std::io::Result<()> {
    let (tx, mut rx) = mpsc::channel::<(String, Vec<u8>)>(256);

    for (i, channel) in channels.iter().enumerate() {
        let confirm = RespValue::Array(Some(vec![
            RespValue::bulk("subscribe"),
            RespValue::bulk(channel.as_str()),
            RespValue::Integer((i + 1) as i64),
        ]));
        write_reply(writer, &confirm).await?;

        let mut channel_rx = store.subscribe(channel).await;
        let channel_name = channel.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            loop {
                match channel_rx.recv().await {
                    Ok(payload) => {
                        if tx.send((channel_name.clone(), payload)).await.is_err() {
                            return; // connection gone
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                }
            }
        });
    }
    drop(tx);

    // Client sends no further commands once subscribed (see doc comment
    // above) — this loop only needs to watch for the client disconnecting
    // (to stop relaying) and for published messages to relay. Reusing
    // read_command as the disconnect watch (rather than a raw peek, which
    // OwnedReadHalf doesn't expose) means EOF/error handling stays in one
    // place instead of being reimplemented here.
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Some((channel, payload)) => {
                        let frame = RespValue::Array(Some(vec![
                            RespValue::bulk("message"),
                            RespValue::bulk(channel),
                            RespValue::BulkString(Some(payload)),
                        ]));
                        write_reply(writer, &frame).await?;
                    }
                    None => return Ok(()), // all channel forwarders ended
                }
            }
            next = read_command(reader) => {
                match next {
                    Ok(None) => return Ok(()), // client disconnected
                    Ok(Some(_)) => {} // extra commands unsupported in this mode — ignore, keep relaying
                    Err(_) => return Ok(()),
                }
            }
        }
    }
}
