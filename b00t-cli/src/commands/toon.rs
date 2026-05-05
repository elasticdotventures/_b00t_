//! `b00t toon` — pgwire SQL adapter with mock mode for ledgrrr self-tests.
//!
//! Toon is a trait-implementation adapter that serves AbstractCloudProvider data
//! over pgwire, SQLite, or in-memory backends.  When `--mock` is set or the DSN
//! scheme is `mock://` or `postgres://`, the adapter generates random valid SQL
//! responses suitable for testing ledgrrr self-tests.
//!
//! # Usage
//! ```bash
//! b00t-cli toon serve --dsn mock://tomllmsdd --port 5432 --mock
//! b00t-cli toon query "SELECT * FROM plan" --dsn memory://tomllmsdd
//! b00t-cli toon export plan --format jsonl
//! ```
//!
//! # PostgreSQL Wire Protocol Implementation
//!
//! This module implements a minimal PostgreSQL wire protocol server using tokio's
//! TCP listener.  It handles the simplest query protocol needed for `psql -c`
//! and ledgrrr self-tests:
//!
//! - StartupMessage  → AuthenticationOk + ParameterStatus + ReadyForQuery
//! - SimpleQuery (Q) → RowDescription + DataRow(s) + CommandComplete + ReadyForQuery
//! - Terminate (X)   → close connection
//! - SSLRequest      → 'N' (refuse SSL)
//!
//! References:
//!   https://www.postgresql.org/docs/current/protocol.html
//!   https://www.postgresql.org/docs/current/protocol-message-formats.html

use anyhow::{Context, Result};
use clap::Parser;
use rand::Rng;
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::runtime::Handle;
use tokio::task;

use b00t_c0re_lib::cloud::AbstractCloudProvider;

// ---------------------------------------------------------------------------
// PostgreSQL wire protocol constants
// ---------------------------------------------------------------------------

/// Protocol version 3.0 encoded as int32 (big-endian: major << 16 | minor).
#[allow(dead_code)]
const PROTOCOL_VERSION_3: i32 = 196_608; // 3 << 16 | 0

/// SSL request code (sent as protocol field in startup message).
const SSL_REQUEST_CODE: i32 = 808_771_03; // 0x04D2_162F — wait no, it's 1234 5679 in decimal

// The SSL request code is actually the int32 0x04D2162F = 80877103 decimal.
// But postgres docs describe it as two int16's: 1234, 5679.
// As one int32 big-endian: 1234 << 16 | 5679 = 80877103. Yes.

/// pgwire message type byte for Authentication* responses.
const MSG_AUTHENTICATION: u8 = b'R';
/// pgwire message type byte for BackendKeyData.
const MSG_BACKEND_KEYDATA: u8 = b'K';
/// pgwire message type byte for ParameterStatus.
const MSG_PARAMETER_STATUS: u8 = b'S';
/// pgwire message type byte for ReadyForQuery.
const MSG_READY_FOR_QUERY: u8 = b'Z';
/// pgwire message type byte for RowDescription.
const MSG_ROW_DESCRIPTION: u8 = b'T';
/// pgwire message type byte for DataRow.
const MSG_DATA_ROW: u8 = b'D';
/// pgwire message type byte for CommandComplete.
const MSG_COMMAND_COMPLETE: u8 = b'C';
/// pgwire message type byte for ErrorResponse.
const MSG_ERROR_RESPONSE: u8 = b'E';
/// pgwire message type byte for client Query.
const MSG_QUERY: u8 = b'Q';
/// pgwire message type byte for client Terminate.
const MSG_TERMINATE: u8 = b'X';

// PostgreSQL type OIDs
const OID_INT4: i32 = 23;
const OID_INT8: i32 = 20;
const OID_TEXT: i32 = 25;
const OID_BOOL: i32 = 16;

// ---------------------------------------------------------------------------
// ToonCommands — CLI subcommands
// ---------------------------------------------------------------------------

#[derive(Parser, Debug, Clone)]
pub enum ToonCommands {
    /// Serve the toon pgwire adapter — listen for SQL connections
    #[clap(about = "Serve the toon pgwire adapter — listen for SQL connections (use --mock for mock mode)")]
    Serve {
        /// DSN: postgres://host:port/db, sqlite://path, memory://, mock://
        #[arg(long, default_value = "mock://tomllmsdd")]
        dsn: String,
        /// Listen port (for postgres:// DSN)
        #[arg(long, default_value_t = 5432)]
        port: u16,
        /// Mock mode: generate random valid responses for self-testing
        #[arg(long)]
        mock: bool,
    },
    /// Query the toon adapter directly (CLI mode, no server needed)
    Query {
        /// SQL query to execute
        query: String,
        /// DSN for query routing
        #[arg(long, default_value = "memory://tomllmsdd")]
        dsn: String,
    },
    /// Export tomllmsdd data as DataFrame (parquet or JSONL)
    Export {
        /// Table or query to export
        table: String,
        /// Output format: parquet, jsonl, csv
        #[arg(long, default_value = "jsonl")]
        format: String,
        /// Output path
        #[arg(long)]
        output: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Dispatch a single ToonCommand.
pub fn handle_toon_command(cmd: &ToonCommands) -> Result<()> {
    match cmd {
        ToonCommands::Serve { dsn, port, mock } => handle_toon_serve(dsn, *port, *mock),
        ToonCommands::Query { query, dsn } => handle_toon_query(query, dsn),
        ToonCommands::Export { table, format, output } => {
            handle_toon_export(table, format, output.as_deref())
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn handle_toon_serve(dsn: &str, port: u16, mock: bool) -> Result<()> {
    println!("  b00t toon: pgwire adapter");
    println!("  DSN:  {dsn}");
    println!("  Mock: {mock}");

    let use_mock = mock || dsn.starts_with("mock://") || dsn.starts_with("postgres://");

    if use_mock {
        println!("  Running mock pgwire server on 127.0.0.1:{port}...");
        println!("  Connect: PGPASSWORD=toon psql -h 127.0.0.1 -p {port} -U toon -d tomllmsdd");
        println!("  Press Ctrl+C to stop.");

        // We are inside #[tokio::main] — use block_in_place + Handle to run async.
        task::block_in_place(move || {
            let handle = Handle::current();
            handle.block_on(run_mock_pgwire_server(port))
        })
        .context("pgwire mock server failed")?;
    } else if dsn.starts_with("sqlite://") || dsn.starts_with("memory://") {
        println!("  Local backend: {dsn}");
        println!("  (SQLite backend stub — use --mock for pgwire mode)");
    }

    Ok(())
}

fn handle_toon_query(query: &str, dsn: &str) -> Result<()> {
    use b00t_c0re_lib::sql::SqlProvider;
    println!("  b00t toon: query");
    println!("  DSN:   {dsn}");
    println!("  Query: {query}");

    if dsn.starts_with("mock://") || dsn.starts_with("memory://") || dsn.starts_with("postgres://") {
        let provider = b00t_c0re_lib::sql::DuckDbProvider::new(":memory:")?;
        let result = provider.query(query)?;
        println!("  Columns: {:?}", result.columns);
        println!("  Rows: {}", result.rows.len());
        for row in &result.rows {
            let vals: Vec<String> = row.iter().map(|(c, v)| format!("{}={}", c, v)).collect();
            println!("    {}", vals.join(", "));
        }
    }
    Ok(())
}

fn handle_toon_export(table: &str, format: &str, output: Option<&str>) -> Result<()> {
    let destination = output.unwrap_or("stdout");
    println!("  b00t toon: export");
    println!("  Table:  {table}");
    println!("  Format: {format}");
    println!("  To:     {destination}");
    println!("  (export stub — use --output to specify file path)");
    Ok(())
}

// ---------------------------------------------------------------------------
// pgwire mock server
// ---------------------------------------------------------------------------

/// Run the pgwire mock server — accepts connections and handles them.
async fn run_mock_pgwire_server(port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind to {addr}"))?;

    println!("  Listening on {addr}");

    loop {
        let (mut socket, peer) = listener.accept().await?;
        println!("  Connection from {peer}");

        tokio::spawn(async move {
            if let Err(e) = handle_pgwire_connection(&mut socket).await {
                eprintln!("  Connection error from {peer}: {e}");
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Per-connection handler
// ---------------------------------------------------------------------------

async fn handle_pgwire_connection(socket: &mut tokio::net::TcpStream) -> Result<()> {
    // ---- Step 1: Read startup message ----
    let startup = read_startup_message(socket).await?;

    // Handle SSL request
    if startup.protocol == SSL_REQUEST_CODE {
        // 'N' byte = refuse SSL
        socket.write_all(&[b'N']).await?;
        // Client should now send a regular startup message
        // Read again — the next 4 bytes are the length
        let _ssl_startup = read_startup_message(socket).await?;
    }

    // ---- Step 2: Send AuthenticationOk ----
    send_authentication_ok(socket).await?;

    // ---- Step 3: Send ParameterStatus ----
    send_parameter_status(socket, "server_version", "16.0 (mock-toon)").await?;
    send_parameter_status(socket, "server_encoding", "UTF8").await?;
    send_parameter_status(socket, "client_encoding", "UTF8").await?;
    send_parameter_status(socket, "DateStyle", "ISO, MDY").await?;
    send_parameter_status(socket, "TimeZone", "UTC").await?;
    send_parameter_status(socket, "integer_datetimes", "on").await?;

    // ---- Step 4: Send BackendKeyData ----
    let pid: i32 = rand::thread_rng().gen_range(1000..9999);
    let secret_key: i32 = rand::thread_rng().gen_range(100_000..999_999);
    send_backend_key_data(socket, pid, secret_key).await?;

    // ---- Step 5: Send ReadyForQuery (initial) ----
    send_ready_for_query(socket).await?;

    // ---- Step 6: Process query/terminate loop ----
    loop {
        let msg_type = match read_message_header(socket).await {
            Ok(t) => t,
            Err(_) => break, // Connection closed
        };

        match msg_type {
            MSG_QUERY => {
                let query = read_query_string(socket).await?;
                println!("  SQL: {query}");
                handle_query(socket, &query).await?;
                send_ready_for_query(socket).await?;
            }
            MSG_TERMINATE => {
                println!("  Terminate received");
                break;
            }
            // Extended Query — we don't support it; send error and continue
            b'P' | b'B' | b'E' | b'D' | b'H' | b'F' | b'p' | b'C' | b'd' | b'c' | b'f'
            | b'A' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' | b'9' | b'0' => {
                // Unknown/unsupported message type — drain payload and skip
                let len = read_int32(socket).await?;
                if len > 4 {
                    let mut buf = vec![0u8; len as usize - 4];
                    let _ = socket.read_exact(&mut buf).await;
                }
                // Send error
                send_error_response(socket, "ERROR", "XX000",
                    &format!("unsupported message type: 0x{:02x}", msg_type)).await?;
                send_ready_for_query(socket).await?;
            }
            _ => {
                // Unknown message type — drain and continue
                let len = read_int32(socket).await?;
                if len > 4 {
                    let mut buf = vec![0u8; len as usize - 4];
                    let _ = socket.read_exact(&mut buf).await;
                }
                send_error_response(socket, "ERROR", "XX000",
                    &format!("unknown message type: 0x{:02x}", msg_type)).await?;
                send_ready_for_query(socket).await?;
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Startup message parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct StartupMessage {
    protocol: i32,
    params: HashMap<String, String>,
}

async fn read_startup_message(socket: &mut tokio::net::TcpStream) -> Result<StartupMessage> {
    // Read length (int32, network byte order)
    let len = read_int32(socket).await?;

    if len < 8 {
        anyhow::bail!("startup message too short: {len}");
    }

    // Read protocol (int32)
    let protocol = read_int32(socket).await?;

    let mut msg = StartupMessage {
        protocol,
        params: HashMap::new(),
    };

    // Read key-value pairs: null-terminated strings, ends with double null
    let payload_len = len as usize - 8;
    if payload_len > 0 {
        let mut buf = vec![0u8; payload_len];
        socket.read_exact(&mut buf).await?;

        // Parse the key=value\0key=value\0\0 sequence
        let mut pos = 0;
        while pos < buf.len() {
            // Find null terminator
            let null_pos = match buf[pos..].iter().position(|&b| b == 0) {
                Some(p) => p,
                None => break,
            };
            if null_pos == 0 {
                // Double null — end of params
                break;
            }
            let kv = String::from_utf8_lossy(&buf[pos..pos + null_pos]).to_string();
            if let Some(eq_pos) = kv.find('=') {
                let key = kv[..eq_pos].to_string();
                let value = kv[eq_pos + 1..].to_string();
                msg.params.insert(key, value);
            }
            pos += null_pos + 1;
        }
    }

    Ok(msg)
}

// ---------------------------------------------------------------------------
// Message sending helpers
// ---------------------------------------------------------------------------

/// Send AuthenticationOk (type 'R', payload int32 0).
async fn send_authentication_ok(socket: &mut tokio::net::TcpStream) -> Result<()> {
    let payload = 0i32.to_be_bytes(); // AuthenticationOk = 0
    let len = 4 + 4; // length(int32) + payload
    let mut msg = Vec::with_capacity(1 + len);
    msg.push(MSG_AUTHENTICATION);
    msg.extend_from_slice(&(len as i32).to_be_bytes());
    msg.extend_from_slice(&payload);
    socket.write_all(&msg).await?;
    Ok(())
}

/// Send ParameterStatus (type 'S').
async fn send_parameter_status(socket: &mut tokio::net::TcpStream, key: &str, value: &str) -> Result<()> {
    let key_bytes = key.as_bytes();
    let val_bytes = value.as_bytes();
    // length: int32 self + key string (null-term) + value string (null-term)
    let len = 4 + key_bytes.len() as i32 + 1 + val_bytes.len() as i32 + 1;
    let mut msg = Vec::with_capacity(1 + len as usize);
    msg.push(MSG_PARAMETER_STATUS);
    msg.extend_from_slice(&len.to_be_bytes());
    msg.extend_from_slice(key_bytes);
    msg.push(0);
    msg.extend_from_slice(val_bytes);
    msg.push(0);
    socket.write_all(&msg).await?;
    Ok(())
}

/// Send BackendKeyData (type 'K').
async fn send_backend_key_data(socket: &mut tokio::net::TcpStream, pid: i32, secret_key: i32) -> Result<()> {
    let len: i32 = 4 + 4 + 4; // int32 self + pid + secret_key
    let mut msg = Vec::with_capacity(1 + len as usize);
    msg.push(MSG_BACKEND_KEYDATA);
    msg.extend_from_slice(&len.to_be_bytes());
    msg.extend_from_slice(&pid.to_be_bytes());
    msg.extend_from_slice(&secret_key.to_be_bytes());
    socket.write_all(&msg).await?;
    Ok(())
}

/// Send ReadyForQuery (type 'Z') with idle status.
async fn send_ready_for_query(socket: &mut tokio::net::TcpStream) -> Result<()> {
    let len: i32 = 4 + 1; // int32 self + status byte
    let mut msg = Vec::with_capacity(1 + len as usize);
    msg.push(MSG_READY_FOR_QUERY);
    msg.extend_from_slice(&len.to_be_bytes());
    msg.push(b'I'); // Idle
    socket.write_all(&msg).await?;
    Ok(())
}

/// Send an ErrorResponse (type 'E').
async fn send_error_response(
    socket: &mut tokio::net::TcpStream,
    severity: &str,
    code: &str,
    message: &str,
) -> Result<()> {
    // Build payload as a series of field-type + string pairs, terminated by \0
    // Field types: 'S' = severity, 'C' = code, 'M' = message
    let mut payload = Vec::new();
    payload.push(b'S'); // Severity
    payload.extend_from_slice(severity.as_bytes());
    payload.push(0);
    payload.push(b'C'); // Code
    payload.extend_from_slice(code.as_bytes());
    payload.push(0);
    payload.push(b'M'); // Message
    payload.extend_from_slice(message.as_bytes());
    payload.push(0);
    payload.push(0); // Terminator

    let len = 4 + payload.len() as i32;
    let mut msg = Vec::with_capacity(1 + len as usize);
    msg.push(MSG_ERROR_RESPONSE);
    msg.extend_from_slice(&len.to_be_bytes());
    msg.extend_from_slice(&payload);
    socket.write_all(&msg).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Query handling
// ---------------------------------------------------------------------------

/// Handle a simple query ('Q') message.
async fn handle_query(socket: &mut tokio::net::TcpStream, query: &str) -> Result<()> {
    let upper = query.trim().to_uppercase();

    // Normalize whitespace for pattern matching
    let normalized: String = upper.split_whitespace().collect::<Vec<_>>().join(" ");

    // ---- SELECT * FROM plan_phases ----
    if normalized.contains("SELECT") && normalized.contains("FROM PLAN_PHASES") {
        return send_plan_phases_result(socket).await;
    }

    // ---- SELECT * FROM env_vars ----
    if normalized.contains("SELECT") && normalized.contains("FROM ENV_VARS") {
        return send_env_vars_result(socket).await;
    }

    // ---- SELECT COUNT(*) ----
    if normalized.contains("SELECT") && normalized.contains("COUNT") {
        return send_count_result(socket).await;
    }

    // ---- Generic SELECT fallback: return a single row with "result" column ----
    if normalized.starts_with("SELECT") {
        return send_generic_select_result(socket, query).await;
    }

    // ---- Non-SELECT: just return CommandComplete "OK" ----
    send_command_complete(socket, "OK 0").await
}

/// Send result for "SELECT * FROM plan_phases".
async fn send_plan_phases_result(socket: &mut tokio::net::TcpStream) -> Result<()> {
    // RowDescription
    send_row_description(socket, &[
        ("id", OID_INT4),
        ("name", OID_TEXT),
        ("description", OID_TEXT),
        ("status", OID_TEXT),
    ]).await?;

    // Data rows
    send_data_row(socket, &["1", "research", "Research phase - initial discovery", "done"]).await?;
    send_data_row(socket, &["2", "implement", "Implementation phase - build features", "in-progress"]).await?;
    send_data_row(socket, &["3", "test", "Testing phase - verify correctness", "pending"]).await?;
    send_data_row(socket, &["4", "deploy", "Deployment phase - release to production", "pending"]).await?;
    send_data_row(socket, &["5", "monitor", "Monitoring phase - observe and alert", "pending"]).await?;

    // CommandComplete
    send_command_complete(socket, "SELECT 5").await
}

/// Send result for "SELECT * FROM env_vars".
async fn send_env_vars_result(socket: &mut tokio::net::TcpStream) -> Result<()> {
    use b00t_c0re_lib::cloud::CloudflareProvider;

    let provider = CloudflareProvider::new();
    let vars = provider.env_health();

    // RowDescription
    send_row_description(socket, &[
        ("name", OID_TEXT),
        ("detected", OID_BOOL),
        ("hint", OID_TEXT),
    ]).await?;

    // Data rows
    for v in &vars {
        let detected_str = if v.detected { "t" } else { "f" };
        send_data_row(socket, &[&v.name, detected_str, &v.hint]).await?;
    }

    send_command_complete(socket, &format!("SELECT {}", vars.len())).await
}

/// Send result for "SELECT COUNT(*) FROM ...".
async fn send_count_result(socket: &mut tokio::net::TcpStream) -> Result<()> {
    let count: u32 = rand::thread_rng().gen_range(1..101);

    send_row_description(socket, &[("count", OID_INT8)]).await?;
    send_data_row(socket, &[&count.to_string()]).await?;
    send_command_complete(socket, "SELECT 1").await
}

/// Send a generic SELECT result: one column "result" with one row containing the query text.
async fn send_generic_select_result(socket: &mut tokio::net::TcpStream, query: &str) -> Result<()> {
    // Extract first column name from query, or use "result"
    let col_name = extract_first_column(query).unwrap_or("result");

    send_row_description(socket, &[(col_name, OID_TEXT)]).await?;
    send_data_row(socket, &["mock response"]).await?;
    send_command_complete(socket, "SELECT 1").await
}

/// Extract the first column name from a SELECT query.
fn extract_first_column(query: &str) -> Option<&str> {
    let upper = query.to_uppercase();
    let after_select = upper.find("SELECT ")?;
    let after_select = &query[after_select + 7..];
    let from_pos = after_select.to_uppercase().find(" FROM")?;
    let cols = after_select[..from_pos].trim();
    // If it's "col1, col2, ..." return the first one
    let first_col = cols.split(',').next()?.trim();
    if first_col == "*" {
        return None;
    }
    Some(first_col)
}

// ---------------------------------------------------------------------------
// Wire protocol message builders
// ---------------------------------------------------------------------------

/// Send a RowDescription ('T') message.
async fn send_row_description(
    socket: &mut tokio::net::TcpStream,
    fields: &[(&str, i32)],
) -> Result<()> {
    let num_fields = fields.len() as i16;
    let mut payload = Vec::new();
    payload.extend_from_slice(&num_fields.to_be_bytes());

    for &(name, type_oid) in fields {
        let name_bytes = name.as_bytes();
        payload.extend_from_slice(name_bytes);
        payload.push(0); // null-terminated string
        payload.extend_from_slice(&0i32.to_be_bytes()); // table_oid
        payload.extend_from_slice(&0i16.to_be_bytes()); // attribute_number
        payload.extend_from_slice(&type_oid.to_be_bytes()); // type_oid
        payload.extend_from_slice(&(-1i16).to_be_bytes()); // type_size (-1 = variable)
        payload.extend_from_slice(&(-1i32).to_be_bytes()); // type_modifier
        payload.extend_from_slice(&0i16.to_be_bytes()); // format (0 = text)
    }

    let len = 4 + payload.len() as i32;
    let mut msg = Vec::with_capacity(1 + len as usize);
    msg.push(MSG_ROW_DESCRIPTION);
    msg.extend_from_slice(&len.to_be_bytes());
    msg.extend_from_slice(&payload);
    socket.write_all(&msg).await?;
    Ok(())
}

/// Send a DataRow ('D') message. Values must be text representations.
async fn send_data_row(
    socket: &mut tokio::net::TcpStream,
    values: &[&str],
) -> Result<()> {
    let num_cols = values.len() as i16;
    let mut payload = Vec::new();
    payload.extend_from_slice(&num_cols.to_be_bytes());

    for val in values {
        let val_bytes = val.as_bytes();
        let col_len = val_bytes.len() as i32;
        payload.extend_from_slice(&col_len.to_be_bytes());
        payload.extend_from_slice(val_bytes);
    }

    let len = 4 + payload.len() as i32;
    let mut msg = Vec::with_capacity(1 + len as usize);
    msg.push(MSG_DATA_ROW);
    msg.extend_from_slice(&len.to_be_bytes());
    msg.extend_from_slice(&payload);
    socket.write_all(&msg).await?;
    Ok(())
}

/// Send a CommandComplete ('C') message.
async fn send_command_complete(socket: &mut tokio::net::TcpStream, tag: &str) -> Result<()> {
    let tag_bytes = tag.as_bytes();
    let len = 4 + tag_bytes.len() as i32 + 1; // +1 for null terminator
    let mut msg = Vec::with_capacity(1 + len as usize);
    msg.push(MSG_COMMAND_COMPLETE);
    msg.extend_from_slice(&len.to_be_bytes());
    msg.extend_from_slice(tag_bytes);
    msg.push(0); // null-terminated string
    socket.write_all(&msg).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Message reading helpers
// ---------------------------------------------------------------------------

/// Read a single message type byte from the stream.
async fn read_message_header(socket: &mut tokio::net::TcpStream) -> Result<u8> {
    let mut buf = [0u8; 1];
    socket.read_exact(&mut buf).await?;
    Ok(buf[0])
}

/// Read a 4-byte big-endian int32.
async fn read_int32(socket: &mut tokio::net::TcpStream) -> Result<i32> {
    let mut buf = [0u8; 4];
    socket.read_exact(&mut buf).await?;
    Ok(i32::from_be_bytes(buf))
}

/// Read a 2-byte big-endian int16.
#[allow(dead_code)]
async fn read_int16(socket: &mut tokio::net::TcpStream) -> Result<i16> {
    let mut buf = [0u8; 2];
    socket.read_exact(&mut buf).await?;
    Ok(i16::from_be_bytes(buf))
}

/// Read a query string from a 'Q' message (after consuming the type byte and length).
async fn read_query_string(socket: &mut tokio::net::TcpStream) -> Result<String> {
    let len = read_int32(socket).await?; // includes self (4 bytes)
    let payload_len = len as usize - 4;
    if payload_len == 0 {
        return Ok(String::new());
    }
    let mut buf = vec![0u8; payload_len];
    socket.read_exact(&mut buf).await?;

    // Remove trailing null byte if present
    let trimmed = if buf.last() == Some(&0) {
        &buf[..buf.len() - 1]
    } else {
        &buf[..]
    };

    Ok(String::from_utf8_lossy(trimmed).to_string())
}
