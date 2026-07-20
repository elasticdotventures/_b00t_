/// A2A HTTP transport — sends and receives A2A messages over HTTP.
///
/// Provides:
/// - An HTTP server exposing agent cards and task endpoints
/// - An HTTP client for sending tasks to remote agents
/// - Remote agent card discovery
use crate::agent_card::AgentCard;
use crate::error::A2AError;
use crate::skill_registry::SkillRegistry;
use crate::task::{Artifact, Task, TaskState};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use url::Url;

/// A2A HTTP transport — exposes local agents via HTTP and can discover/call
/// remote agents via their card URL.
pub struct A2aHttpTransport {
    registry: Arc<SkillRegistry>,
    port: u16,
    /// In-memory task store for status queries
    tasks: Arc<Mutex<HashMap<String, Task>>>,
}

impl A2aHttpTransport {
    /// Create a new HTTP transport bound to the given port.
    pub fn new(registry: Arc<SkillRegistry>, port: u16) -> Self {
        Self {
            registry,
            port,
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Return the port this transport is configured for.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Start the HTTP server. Binds to `0.0.0.0:{port}` and spawns a tokio
    /// task that runs until the returned handle is dropped or the server
    /// encounters a fatal error.
    ///
    /// Routes:
    ///   GET  /.well-known/agent-cards  -> list all agent cards
    ///   POST /task                     -> receive and execute a task
    ///   GET  /task/{id}/status         -> check task status
    pub async fn serve(&self) -> Result<tokio::task::JoinHandle<()>, Box<dyn std::error::Error>> {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        let registry = Arc::clone(&self.registry);
        let tasks = Arc::clone(&self.tasks);

        let handle = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer)) => {
                        let registry = Arc::clone(&registry);
                        let tasks = Arc::clone(&tasks);
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(stream, registry, tasks).await {
                                eprintln!("[a2a-http] handler error from {peer}: {e}");
                            }
                        });
                    }
                    Err(e) => {
                        eprintln!("[a2a-http] accept error: {e}");
                    }
                }
            }
        });

        Ok(handle)
    }

    /// Send a task to a remote agent via HTTP.
    ///
    /// Uses the agent card's URL to determine the endpoint.
    /// Sends a POST /task with the task JSON body.
    pub async fn send_task(
        agent_url: &Url,
        task: &Task,
    ) -> Result<Task, Box<dyn std::error::Error>> {
        let body = serde_json::to_string(task)?;
        let host = agent_url
            .host_str()
            .ok_or_else(|| A2AError::RuntimeError("Agent URL has no host".to_string()))?;
        let port = agent_url.port().unwrap_or(80);
        let path = agent_url.path();
        let task_path = format!("{}/task", path.trim_end_matches('/'));

        let response = http_request(host, port, "POST", &task_path, Some(&body)).await?;

        let (status, _, resp_body) = response;
        if status != 202 && status != 200 {
            return Err(Box::new(A2AError::RuntimeError(format!(
                "Remote agent returned HTTP {status}: {resp_body}"
            ))));
        }

        let result_task: Task = serde_json::from_str(&resp_body)?;
        Ok(result_task)
    }

    /// Discover agent cards from a remote hive's well-known endpoint.
    pub async fn discover_remote(
        remote_url: &Url,
    ) -> Result<Vec<AgentCard>, Box<dyn std::error::Error>> {
        let host = remote_url
            .host_str()
            .ok_or_else(|| A2AError::RuntimeError("Remote URL has no host".to_string()))?;
        let port = remote_url.port().unwrap_or(80);
        let base = remote_url.path().trim_end_matches('/');
        let cards_path = format!("{base}/.well-known/agent-cards");

        let response = http_request(host, port, "GET", &cards_path, None).await?;

        let (status, _, resp_body) = response;
        if status != 200 {
            return Err(Box::new(A2AError::RuntimeError(format!(
                "Remote hive returned HTTP {status}: {resp_body}"
            ))));
        }

        let cards: Vec<AgentCard> = serde_json::from_str(&resp_body)?;
        Ok(cards)
    }
}

// ---------------------------------------------------------------------------
// Internal: per-connection handler
// ---------------------------------------------------------------------------

async fn handle_connection(
    stream: TcpStream,
    registry: Arc<SkillRegistry>,
    tasks: Arc<Mutex<HashMap<String, Task>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut request_line = String::new();
    buf_reader.read_line(&mut request_line).await?;

    let request_line = request_line.trim().to_string();
    if request_line.is_empty() {
        return Ok(());
    }

    // Parse request line: METHOD PATH HTTP/1.1
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        send_response(&mut writer, 400, "Bad Request", "Invalid request line").await?;
        return Ok(());
    }
    let method = parts[0];
    let path = parts[1];

    // Read headers until empty line
    let mut content_length: usize = 0;
    loop {
        let mut header_line = String::new();
        buf_reader.read_line(&mut header_line).await?;
        let trimmed = header_line.trim().to_string();
        if trimmed.is_empty() {
            break;
        }
        if let Some(val) = trimmed.to_lowercase().strip_prefix("content-length:") {
            content_length = val.trim().parse().unwrap_or(0);
        }
    }

    // Read body if Content-Length is set
    let mut body = String::new();
    if content_length > 0 {
        let mut buf = vec![0u8; content_length];
        buf_reader.read_exact(&mut buf).await?;
        body = String::from_utf8_lossy(&buf).to_string();
    }

    // Route
    match (method, path) {
        ("GET", "/.well-known/agent-cards") | ("GET", "/a2a/.well-known/agent-cards") => {
            handle_get_cards(&mut writer, &registry).await?;
        }
        ("POST", "/task") | ("POST", "/a2a/task") => {
            handle_post_task(&mut writer, &registry, &tasks, &body).await?;
        }
        (_, path)
            if path.starts_with("/task/") && path.ends_with("/status")
                || path.starts_with("/a2a/task/") && path.ends_with("/status") =>
        {
            let id = extract_task_id(path);
            handle_get_status(&mut writer, &tasks, &id).await?;
        }
        _ => {
            send_response(&mut writer, 404, "Not Found", "Unknown endpoint").await?;
        }
    }

    Ok(())
}

fn extract_task_id(path: &str) -> String {
    let stripped = path
        .strip_prefix("/a2a/task/")
        .or_else(|| path.strip_prefix("/task/"))
        .unwrap_or(path);
    stripped
        .strip_suffix("/status")
        .unwrap_or(stripped)
        .to_string()
}

async fn handle_get_cards(
    writer: &mut OwnedWriteHalf,
    registry: &SkillRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    let skills = registry.list_skills();
    let card = AgentCard::new(
        "a2a-agent",
        "A2A HTTP Agent",
        Url::parse("http://localhost").unwrap(),
    );
    // Return actual skills from the registry
    let cards = vec![AgentCard { skills, ..card }];
    let json = serde_json::to_string(&cards)?;
    send_json_response(writer, 200, &json).await
}

async fn handle_post_task(
    writer: &mut OwnedWriteHalf,
    registry: &SkillRegistry,
    tasks: &Arc<Mutex<HashMap<String, Task>>>,
    body: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut task: Task = match serde_json::from_str(body) {
        Ok(t) => t,
        Err(e) => {
            let err_body = serde_json::json!({"error": format!("Invalid task JSON: {e}")});
            send_json_response(writer, 400, &err_body.to_string()).await?;
            return Ok(());
        }
    };

    // Store the task before execution
    {
        let mut store = tasks.lock().await;
        store.insert(task.id.to_string(), task.clone());
    }

    // Execute via registry
    match registry.execute(&task) {
        Ok(result) => {
            let mut store = tasks.lock().await;
            store.insert(result.id.to_string(), result.clone());
            let json = serde_json::to_string(&result)?;
            send_json_response(writer, 202, &json).await
        }
        Err(e) => {
            task.transition_to(TaskState::Failed);
            task.add_artifact(Artifact::text(
                "error",
                &format!("Skill execution failed: {e}"),
            ));
            let mut store = tasks.lock().await;
            store.insert(task.id.to_string(), task.clone());
            let json = serde_json::to_string(&task)?;
            send_json_response(writer, 202, &json).await
        }
    }
}

async fn handle_get_status(
    writer: &mut OwnedWriteHalf,
    tasks: &Arc<Mutex<HashMap<String, Task>>>,
    task_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let store = tasks.lock().await;
    match store.get(task_id) {
        Some(task) => {
            let json = serde_json::to_string(task)?;
            send_json_response(writer, 200, &json).await
        }
        None => {
            let err_body = serde_json::json!({"error": format!("Task not found: {task_id}")});
            send_json_response(writer, 404, &err_body.to_string()).await
        }
    }
}

// ---------------------------------------------------------------------------
// Internal: HTTP request helper (client side)
// ---------------------------------------------------------------------------

async fn http_request(
    host: &str,
    port: u16,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<(u16, Vec<String>, String), Box<dyn std::error::Error>> {
    let addr = format!("{host}:{port}");
    let mut stream = TcpStream::connect(&addr).await?;

    let body_bytes = body.unwrap_or("");
    let content_length = body_bytes.len();

    let request = format!(
        "{method} {path} HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {content_length}\r\n\
         Connection: close\r\n\
         \r\n\
         {body_bytes}"
    );

    stream.write_all(request.as_bytes()).await?;
    stream.flush().await?;

    // Read response
    let (reader, _) = stream.into_split();
    let mut buf_reader = BufReader::new(reader);
    let mut status_line = String::new();
    buf_reader.read_line(&mut status_line).await?;

    // Parse status line: HTTP/1.1 200 OK
    let status_parts: Vec<&str> = status_line.trim().splitn(3, ' ').collect();
    let status: u16 = status_parts.get(1).unwrap_or(&"500").parse().unwrap_or(500);

    // Read headers
    let mut headers = Vec::new();
    let mut content_length: usize = 0;
    loop {
        let mut header_line = String::new();
        buf_reader.read_line(&mut header_line).await?;
        let trimmed = header_line.trim().to_string();
        if trimmed.is_empty() {
            break;
        }
        if let Some(val) = trimmed.to_lowercase().strip_prefix("content-length:") {
            content_length = val.trim().parse().unwrap_or(0);
        }
        headers.push(trimmed);
    }

    // Read body
    let mut body = String::new();
    if content_length > 0 {
        let mut buf = vec![0u8; content_length];
        buf_reader.read_exact(&mut buf).await?;
        body = String::from_utf8_lossy(&buf).to_string();
    } else {
        buf_reader.read_to_string(&mut body).await?;
    }

    Ok((status, headers, body))
}

// ---------------------------------------------------------------------------
// Internal: response helpers (server side)
// ---------------------------------------------------------------------------

async fn send_response(
    writer: &mut OwnedWriteHalf,
    status: u16,
    reason: &str,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: text/plain\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {text}",
        text.len()
    );
    writer.write_all(response.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}

async fn send_json_response(
    writer: &mut OwnedWriteHalf,
    status: u16,
    json: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {json}",
        json.len()
    );
    writer.write_all(response.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}
