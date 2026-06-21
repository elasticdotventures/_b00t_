//! b00t-admin — Internal admin dashboard server.
//!
//! Serves:
//! - `/` → Admin dashboard HTML
//! - `/v1/*` → Reverse proxy to LLM backend
//! - `/api/admin/*` → JSON API for pipeline state/type introspection
//! - `/ws` → WebSocket for live twin simulation updates

use axum::{
    body::Body,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::{HeaderMap, Method, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use b00t_admin::{
    DigitalTwin, PipelineStateSnapshot, TypeSchema, WasmCodegen,
    registered_type_names,
};
use b00t_c0re_lib::doc_pipeline::FullPipelineResult;
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use reqwest::Client as ReqwestClient;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// ═══════════════════════════════════════════════════════════════════════════
// App State
// ═══════════════════════════════════════════════════════════════════════════

/// Server configuration from environment variables.
#[derive(Debug, Clone)]
struct AdminConfig {
    llm_backend_url: String,
    admin_port: u16,
    admin_host: String,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            llm_backend_url: std::env::var("LLM_BACKEND_URL")
                .unwrap_or_else(|_| "http://localhost:5273".to_string()),
            admin_port: std::env::var("ADMIN_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(31337),
            admin_host: std::env::var("ADMIN_HOST")
                .unwrap_or_else(|_| "0.0.0.0".to_string()),
        }
    }
}

/// Shared application state.
struct AppState {
    config: AdminConfig,
    pipeline: PipelineStateSnapshot,
    twin: DigitalTwin<FullPipelineResult>,
    type_schemas: HashMap<String, TypeSchema>,
    reqwest_client: ReqwestClient,
}

impl AppState {
    fn new(config: AdminConfig) -> Self {
        // Create a default pipeline result for the twin's initial state
        let default_pipeline = FullPipelineResult {
            source: b00t_c0re_lib::doc_pipeline::DocumentSource {
                source_id: "arxiv:demo-0001".into(),
                title: "Demo Pipeline — Awaiting Ingestion".into(),
                authors: vec!["b00t-admin".into()],
                abstract_text: "Pipeline has not yet ingested a document.".into(),
                url: None,
                pdf_url: None,
                fetched_at: Utc::now(),
                content_hash: None,
                format: b00t_c0re_lib::doc_pipeline::DocumentFormat::Markdown,
                metadata: HashMap::new(),
            },
            chunks: vec![],
            evidences: vec![],
            requirements: vec![],
            fol_formulas: vec![],
            pipeline_version: env!("CARGO_PKG_VERSION").to_string(),
            executed_at: Utc::now(),
            total_duration_ms: 0,
        };

        let pipeline = PipelineStateSnapshot::default();
        let twin = DigitalTwin::new("doc-pipeline", default_pipeline);

        // Build type schema registry for all doc_pipeline types
        let mut type_schemas = HashMap::new();
        for name in registered_type_names() {
            if let Some(schema) = build_type_schema(name) {
                type_schemas.insert(name.to_string(), schema);
            }
        }

        Self {
            config,
            pipeline,
            twin,
            type_schemas,
            reqwest_client: ReqwestClient::new(),
        }
    }
}

/// Build a TypeSchema manually for a known doc_pipeline type name.
///
/// Since the upstream types in b00t-c0re-lib may not derive `JsonSchema`,
/// we build schemas manually with field knowledge.
fn build_type_schema(name: &str) -> Option<TypeSchema> {
    let (fields, mermaid, ufo, has_codegen) = match name {
        "DocumentSource" => (
            vec![
                field("source_id", "String", false, false, "Unique identifier (e.g., arxiv:2404.17842)"),
                field("title", "String", false, false, "Human-readable title"),
                field("authors", "Vec<String>", false, true, "Author list"),
                field("abstract_text", "String", false, false, "Full abstract or summary"),
                field("url", "Option<String>", true, false, "Canonical URL to the source"),
                field("pdf_url", "Option<String>", true, false, "Direct PDF/download URL"),
                field("fetched_at", "DateTime", false, false, "When the document was fetched"),
                field("content_hash", "Option<String>", true, false, "SHA-256 content hash"),
                field("format", "DocumentFormat", false, false, "Original format"),
                field("metadata", "HashMap<String,String>", false, true, "Additional metadata"),
            ],
            "classDiagram\n    class DocumentSource {\n        +String source_id\n        +String title\n        +Vec~String~ authors\n        +String abstract_text\n        +Option~String~ url\n        +Option~String~ pdf_url\n        +DateTime fetched_at\n        +Option~String~ content_hash\n        +DocumentFormat format\n        +HashMap~String,String~ metadata\n    }",
            "Endurant",
            false,
        ),
        "Evidence" => (
            vec![
                field("evidence_id", "String", false, false, "Unique evidence identifier"),
                field("chunk_id", "String", false, false, "Back-reference to source chunk"),
                field("source_id", "String", false, false, "Back-reference to source document"),
                field("statement", "String", false, false, "The extracted statement/claim/fact"),
                field("evidence_type", "EvidenceType", false, false, "Classification of evidence"),
                field("confidence", "f32", false, false, "Confidence in extraction [0.0, 1.0]"),
                field("extraction_method", "String", false, false, "Method used for extraction"),
                field("source_quote", "String", false, false, "Verbatim quote from source"),
                field("line_range", "Option<(usize,usize)>", true, false, "Line range in source"),
                field("provenance", "ProvenancePointer", false, false, "Proxy-pointer for RAG"),
                field("extracted_at", "DateTime", false, false, "When the evidence was extracted"),
            ],
            "classDiagram\n    class Evidence {\n        +String evidence_id\n        +String chunk_id\n        +String source_id\n        +String statement\n        +EvidenceType evidence_type\n        +f32 confidence\n        +String extraction_method\n        +String source_quote\n        +Option~Pair~ line_range\n        +ProvenancePointer provenance\n        +DateTime extracted_at\n    }",
            "Relator",
            true,
        ),
        "Requirement" => (
            vec![
                field("req_id", "String", false, false, "Unique requirement identifier"),
                field("text", "String", false, false, "Human-readable requirement text"),
                field("req_type", "RequirementType", false, false, "SysMLv2 / ReqIF type"),
                field("priority", "u8", false, false, "Priority (1 = highest, 5 = lowest)"),
                field("rationale", "Option<String>", true, false, "Why this requirement exists"),
                field("derived_from", "Vec<String>", false, true, "Evidence IDs supporting this req"),
                field("satisfies", "Vec<String>", false, true, "Requirements this one traces to"),
                field("verified_by", "Option<String>", true, false, "Verification method"),
                field("status", "RequirementStatus", false, false, "Lifecycle status"),
                field("source_id", "String", false, false, "Source document this was derived from"),
                field("reqif", "Option<ReqIFMetadata>", true, false, "ReqIF interchange metadata"),
                field("sysml_stereotype", "Option<SysMLv2Stereotype>", true, false, "SysMLv2 stereotype"),
                field("created_at", "DateTime", false, false, "When the requirement was created"),
            ],
            "classDiagram\n    class Requirement {\n        +String req_id\n        +String text\n        +RequirementType req_type\n        +u8 priority\n        +Option~String~ rationale\n        +Vec~String~ derived_from\n        +Vec~String~ satisfies\n        +Option~String~ verified_by\n        +RequirementStatus status\n        +String source_id\n        +Option~ReqIFMetadata~ reqif\n        +Option~SysMLv2Stereotype~ sysml_stereotype\n        +DateTime created_at\n    }",
            "Endurant+Role",
            true,
        ),
        "SemanticChunk" => (
            vec![
                field("chunk_id", "String", false, false, "Unique chunk identifier"),
                field("source_id", "String", false, false, "Back-reference to parent document"),
                field("chunk_index", "usize", false, false, "0-based index in chunk sequence"),
                field("content", "String", false, false, "Chunk text content"),
                field("topic_tags", "Vec<String>", false, true, "Topic tags for filtering"),
                field("embedding", "Vec<f32>", false, true, "Semantic embedding vector"),
                field("embedding_model", "Option<String>", true, false, "Model used for embedding"),
                field("confidence", "f32", false, false, "Chunk quality confidence [0.0, 1.0]"),
                field("created_at", "DateTime", false, false, "When the chunk was created"),
                field("metadata", "ChunkMetadata", false, false, "Additional chunk metadata"),
            ],
            "classDiagram\n    class SemanticChunk {\n        +String chunk_id\n        +String source_id\n        +usize chunk_index\n        +String content\n        +Vec~String~ topic_tags\n        +Vec~f32~ embedding\n        +Option~String~ embedding_model\n        +f32 confidence\n        +DateTime created_at\n        +ChunkMetadata metadata\n    }",
            "Perdurant",
            false,
        ),
        "FullPipelineResult" => (
            vec![
                field("source", "DocumentSource", false, false, "Original document source"),
                field("chunks", "Vec<SemanticChunk>", false, true, "Semantic chunks extracted"),
                field("evidences", "Vec<Evidence>", false, true, "Evidence items extracted"),
                field("requirements", "Vec<Requirement>", false, true, "Requirements derived"),
                field("fol_formulas", "Vec<SerializableFOLFormula>", false, true, "FOL formulas"),
                field("pipeline_version", "String", false, false, "Pipeline version"),
                field("executed_at", "DateTime", false, false, "When the pipeline executed"),
                field("total_duration_ms", "u64", false, false, "Total execution time ms"),
            ],
            "classDiagram\n    class FullPipelineResult {\n        +DocumentSource source\n        +Vec~SemanticChunk~ chunks\n        +Vec~Evidence~ evidences\n        +Vec~Requirement~ requirements\n        +Vec~SerializableFOLFormula~ fol_formulas\n        +String pipeline_version\n        +DateTime executed_at\n        +u64 total_duration_ms\n    }",
            "Endurant",
            false,
        ),
        // For other types, return a minimal schema
        _ => {
            return Some(TypeSchema {
                name: name.to_string(),
                module_path: format!("b00t_c0re_lib::doc_pipeline::{name}"),
                json_schema: serde_json::json!({"type": "object", "title": name}),
                mermaid_diagram: format!("classDiagram\n    class {name} {{\n        +... fields\n    }}"),
                fields: vec![],
                ufo_stereotype: None,
                has_wasm_codegen: false,
            });
        }
    };

    Some(TypeSchema {
        name: name.to_string(),
        module_path: format!("b00t_c0re_lib::doc_pipeline::{name}"),
        json_schema: serde_json::json!({
            "type": "object",
            "title": name,
            "properties": fields.iter().map(|f| {
                (f.name.clone(), serde_json::json!({
                    "type": rust_to_json_type(&f.rust_type),
                    "description": f.description,
                }))
            }).collect::<HashMap<_,_>>()
        }),
        mermaid_diagram: mermaid.to_string(),
        fields,
        ufo_stereotype: Some(ufo.to_string()),
        has_wasm_codegen: has_codegen,
    })
}

fn field(
    name: &str,
    rust_type: &str,
    is_optional: bool,
    is_collection: bool,
    description: &str,
) -> b00t_admin::FieldSchema {
    b00t_admin::FieldSchema {
        name: name.to_string(),
        rust_type: rust_type.to_string(),
        is_optional,
        is_collection,
        schema: serde_json::json!({"type": rust_to_json_type(rust_type)}),
        description: Some(description.to_string()),
    }
}

fn rust_to_json_type(rust_type: &str) -> &str {
    match rust_type {
        s if s.starts_with("String") => "string",
        s if s.starts_with("u8") || s.starts_with("u16") || s.starts_with("u32")
            || s.starts_with("u64") || s.starts_with("usize") || s.starts_with("i64")
            || s.starts_with("i32") => "integer",
        s if s.starts_with("f32") || s.starts_with("f64") => "number",
        s if s.starts_with("bool") => "boolean",
        s if s.starts_with("Vec") || s.starts_with("HashMap") => "array",
        s if s.starts_with("Option") => {
            // Extract inner type
            let inner = s.trim_start_matches("Option<").trim_end_matches('>');
            rust_to_json_type(inner)
        }
        _ => "object",
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Route handlers
// ═══════════════════════════════════════════════════════════════════════════

/// GET `/` or `/admin` — Admin dashboard HTML
async fn dashboard_handler(State(state): State<Arc<Mutex<AppState>>>) -> Html<String> {
    let app = state.lock().await;
    let pipeline_json = serde_json::to_string(&app.pipeline).unwrap_or_default();
    let types_json = serde_json::json!(app.type_schemas.keys().collect::<Vec<_>>()).to_string();
    drop(app);
    Html(dashboard_html(&pipeline_json, &types_json))
}

/// GET `/health` or `/healthz` — Health check (for container probes)
async fn health_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "status": "ok",
        "service": "b00t-admin",
        "version": b00t_c0re_lib::version::VERSION,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// GET `/api/admin/pipeline` — Current pipeline state as JSON
async fn pipeline_handler(State(state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let app = state.lock().await;
    axum::Json(app.pipeline.clone())
}

/// GET `/api/admin/types` — List all reflected types
async fn types_list_handler(State(state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let app = state.lock().await;
    let names: Vec<&String> = app.type_schemas.keys().collect();
    let type_list: Vec<serde_json::Value> = names
        .iter()
        .map(|name| {
            let schema = &app.type_schemas[*name];
            serde_json::json!({
                "name": schema.name,
                "ufo_stereotype": schema.ufo_stereotype,
                "has_wasm_codegen": schema.has_wasm_codegen,
                "field_count": schema.fields.len(),
            })
        })
        .collect();
    axum::Json(serde_json::json!({ "types": type_list }))
}

/// GET `/api/admin/types/:name` — Single type schema + WASM codegen output
async fn type_detail_handler(
    State(state): State<Arc<Mutex<AppState>>>,
    Path(name): Path<String>,
) -> Result<axum::Json<serde_json::Value>, (StatusCode, String)> {
    let app = state.lock().await;
    let schema = app.type_schemas.get(&name).cloned();
    drop(app);

    match schema {
        Some(s) => {
            // Generate WASM codegen samples for Evidence and Requirement
            let wasm_sample = match name.as_str() {
                "Evidence" => {
                    let ev = b00t_c0re_lib::doc_pipeline::Evidence {
                        evidence_id: "ev:example".into(),
                        chunk_id: "chunk:0".into(),
                        source_id: "arxiv:demo".into(),
                        statement: "Example evidence statement.".into(),
                        evidence_type: b00t_c0re_lib::doc_pipeline::EvidenceType::Claim,
                        confidence: 0.95,
                        extraction_method: "llm".into(),
                        source_quote: "Example quote".into(),
                        line_range: Some((1, 3)),
                        provenance: b00t_c0re_lib::doc_pipeline::ProvenancePointer {
                            source_id: "arxiv:demo".into(),
                            chunk_id: "chunk:0".into(),
                            line_start: 1,
                            line_end: 3,
                            quote_snippet: "Example quote".into(),
                        },
                        extracted_at: Utc::now(),
                    };
                    Some(serde_json::json!({
                        "wasm": ev.to_wasm_module(),
                        "cython": ev.to_cython(),
                        "diagram": ev.to_type_diagram(),
                    }))
                }
                "Requirement" => {
                    let req = b00t_c0re_lib::doc_pipeline::Requirement {
                        req_id: "REQ-EXAMPLE".into(),
                        text: "Example requirement text.".into(),
                        req_type: b00t_c0re_lib::doc_pipeline::RequirementType::Functional,
                        priority: 1,
                        rationale: Some("Derived from example evidence".into()),
                        derived_from: vec!["ev:example".into()],
                        satisfies: vec![],
                        verified_by: None,
                        status: b00t_c0re_lib::doc_pipeline::RequirementStatus::Proposed,
                        source_id: "arxiv:demo".into(),
                        reqif: None,
                        sysml_stereotype: Some(
                            b00t_c0re_lib::doc_pipeline::SysMLv2Stereotype::FunctionalRequirement,
                        ),
                        created_at: Utc::now(),
                    };
                    Some(serde_json::json!({
                        "wasm": req.to_wasm_module(),
                        "cython": req.to_cython(),
                        "diagram": req.to_type_diagram(),
                    }))
                }
                _ => None,
            };

            Ok(axum::Json(serde_json::json!({
                "schema": s,
                "codegen": wasm_sample,
            })))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            format!("Type '{name}' not found"),
        )),
    }
}

/// ALL `/v1/*` — Reverse proxy to LLM backend
async fn proxy_handler(
    State(state): State<Arc<Mutex<AppState>>>,
    req_method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Response, (StatusCode, String)> {
    let app = state.lock().await;
    let backend_url = app.config.llm_backend_url.clone();
    let client = app.reqwest_client.clone();
    drop(app);

    // Build the target URL
    let path_and_query = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");
    let target_url = format!("{}{}", backend_url.trim_end_matches('/'), path_and_query);

    // Build the proxied request
    let mut req_builder = client.request(req_method.clone(), &target_url);

    // Forward headers (skip host and connection-specific headers)
    for (key, value) in headers.iter() {
        let key_str = key.as_str().to_lowercase();
        if key_str == "host" || key_str == "connection" || key_str == "transfer-encoding" {
            continue;
        }
        if let Ok(v) = value.to_str() {
            req_builder = req_builder.header(key_str, v);
        }
    }

    // Forward body
    if !body.is_empty() {
        req_builder = req_builder.body(body.to_vec());
    }

    // Execute the request
    match req_builder.send().await {
        Ok(resp) => {
            let status = resp.status();
            let resp_headers = resp.headers().clone();
            let resp_body = resp.bytes().await.unwrap_or_default();

            let mut response = Response::builder().status(status);
            for (key, value) in resp_headers.iter() {
                response = response.header(key, value);
            }
            Ok(response
                .body(Body::from(resp_body))
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?)
        }
        Err(e) => Err((
            StatusCode::BAD_GATEWAY,
            format!("Proxy error: {e}"),
        )),
    }
}

/// GET `/ws` — WebSocket upgrade for twin simulation live feed
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<Mutex<AppState>>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: Arc<Mutex<AppState>>) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to twin updates
    let mut rx = {
        let app = state.lock().await;
        app.twin.subscribe()
    };

    // Spawn task to forward twin updates to the WebSocket
    let send_task = tokio::spawn(async move {
        while let Ok(update) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&update) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming messages (e.g., control commands)
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                let mut app = state.lock().await;
                match text.as_str() {
                    "tick" => {
                        app.twin.tick::<fn(&FullPipelineResult) -> FullPipelineResult>(None);
                    }
                    "rollback" => {
                        let idx = app.twin.history_len().saturating_sub(1);
                        let _ = app.twin.rollback(idx);
                    }
                    cmd if cmd.starts_with("rollback:") => {
                        if let Ok(idx) = cmd.trim_start_matches("rollback:").parse::<usize>() {
                            let _ = app.twin.rollback(idx);
                        }
                    }
                    cmd if cmd.starts_with("delta:") => {
                        let json_str = cmd.trim_start_matches("delta:");
                        if let Ok(delta) = serde_json::from_str::<serde_json::Value>(json_str) {
                            let _ = app.twin.apply_delta(delta);
                        }
                    }
                    _ => {}
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    send_task.abort();
}

/// GET `/api/admin/simulate/tick` — Advance simulation one step
async fn simulate_tick_handler(State(state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let mut app = state.lock().await;
    app.twin.tick::<fn(&FullPipelineResult) -> FullPipelineResult>(None);
    axum::Json(serde_json::json!({
        "tick": app.twin.tick_count(),
        "history_len": app.twin.history_len(),
    }))
}

/// GET `/api/admin/simulate/state` — Current simulation state
async fn simulate_state_handler(State(state): State<Arc<Mutex<AppState>>>) -> impl IntoResponse {
    let app = state.lock().await;
    axum::Json(app.twin.snapshot())
}

// ═══════════════════════════════════════════════════════════════════════════
// Dashboard HTML (embedded)
// ═══════════════════════════════════════════════════════════════════════════

fn dashboard_html(pipeline_json: &str, types_json: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>b00t Admin Dashboard</title>
<style>
  @import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;700&display=swap');

  * {{ margin: 0; padding: 0; box-sizing: border-box; }}

  body {{
    background: #020617;
    color: #e2e8f0;
    font-family: 'JetBrains Mono', monospace;
    display: grid;
    grid-template-columns: 1fr 1fr;
    grid-template-rows: auto 1fr 1fr;
    gap: 16px;
    padding: 20px;
    min-height: 100vh;
  }}

  .header {{
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 16px 24px;
    background: #0f172a;
    border: 1px solid #1e293b;
    border-radius: 12px;
  }}

  .header h1 {{
    font-size: 24px;
    color: #38bdf8;
  }}

  .status-dot {{
    width: 10px; height: 10px;
    border-radius: 50%;
    background: #34d399;
    animation: pulse 2s infinite;
  }}

  @keyframes pulse {{
    0%, 100% {{ opacity: 1; box-shadow: 0 0 0 0 rgba(52,211,153,0.4); }}
    50% {{ opacity: 0.6; box-shadow: 0 0 0 8px rgba(52,211,153,0); }}
  }}

  .header-info {{
    margin-left: auto;
    font-size: 11px;
    color: #64748b;
  }}

  /* Four panels grid */
  .panel {{
    background: #0f172a;
    border: 1px solid #1e293b;
    border-radius: 12px;
    padding: 20px;
    overflow: auto;
  }}

  .panel h2 {{
    font-size: 14px;
    color: #38bdf8;
    margin-bottom: 16px;
    display: flex;
    align-items: center;
    gap: 8px;
  }}

  .panel h2 .icon {{
    font-size: 18px;
  }}

  /* Pipeline Status panel */
  .pipeline-stats {{
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }}

  .stat {{
    background: rgba(56,189,248,0.06);
    border: 1px solid rgba(56,189,248,0.15);
    border-radius: 8px;
    padding: 12px;
    text-align: center;
  }}

  .stat-value {{
    font-size: 24px;
    font-weight: 700;
    color: #38bdf8;
  }}

  .stat-label {{
    font-size: 10px;
    color: #64748b;
    margin-top: 4px;
  }}

  .pipeline-source {{
    margin-top: 16px;
    padding: 12px;
    background: rgba(34,211,238,0.05);
    border: 1px solid rgba(34,211,238,0.2);
    border-radius: 8px;
    font-size: 11px;
    color: #94a3b8;
  }}

  .pipeline-source strong {{
    color: #22d3ee;
  }}

  /* Type Explorer panel */
  .type-list {{
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 240px;
    overflow-y: auto;
  }}

  .type-item {{
    padding: 8px 12px;
    background: rgba(167,139,250,0.05);
    border: 1px solid rgba(167,139,250,0.1);
    border-radius: 6px;
    cursor: pointer;
    font-size: 11px;
    color: #c4b5fd;
    transition: all 0.2s;
  }}

  .type-item:hover {{
    background: rgba(167,139,250,0.12);
    border-color: rgba(167,139,250,0.3);
  }}

  .type-item.active {{
    background: rgba(167,139,250,0.15);
    border-color: #a78bfa;
  }}

  .type-tag {{
    display: inline-block;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 9px;
    margin-left: 6px;
    background: rgba(167,139,250,0.2);
    color: #a78bfa;
  }}

  .type-detail {{
    margin-top: 12px;
    padding: 12px;
    background: #0a0f1e;
    border-radius: 8px;
    font-size: 10px;
    color: #94a3b8;
    max-height: 200px;
    overflow-y: auto;
    white-space: pre-wrap;
    font-family: 'JetBrains Mono', monospace;
  }}

  .type-detail .field {{
    color: #e2e8f0;
  }}

  .type-detail .field-name {{
    color: #38bdf8;
  }}

  .type-detail .field-type {{
    color: #a78bfa;
  }}

  .code-tabs {{
    display: flex;
    gap: 4px;
    margin-top: 8px;
  }}

  .code-tab {{
    padding: 4px 10px;
    border-radius: 4px;
    font-size: 10px;
    cursor: pointer;
    background: #1e293b;
    color: #64748b;
    border: 1px solid #334155;
  }}

  .code-tab.active {{
    background: #1e3a5f;
    color: #38bdf8;
    border-color: #38bdf8;
  }}

  /* Process Flow panel */
  .process-flow {{
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }}

  .flow-stages {{
    display: flex;
    align-items: center;
    gap: 0;
    flex-wrap: wrap;
    justify-content: center;
  }}

  .flow-stage {{
    background: #0f172a;
    border: 1.5px solid;
    border-radius: 10px;
    padding: 12px 16px;
    width: 130px;
    text-align: center;
    font-size: 10px;
    transition: transform 0.3s, box-shadow 0.3s;
  }}

  .flow-stage:hover {{
    transform: translateY(-3px);
    box-shadow: 0 6px 20px rgba(0,0,0,0.5);
  }}

  .flow-stage.fetch  {{ border-color: #22d3ee; }}
  .flow-stage.chunk  {{ border-color: #34d399; }}
  .flow-stage.evidence {{ border-color: #a78bfa; }}
  .flow-stage.require {{ border-color: #fbbf24; }}

  .flow-stage .stage-icon {{
    font-size: 22px;
    margin-bottom: 6px;
  }}

  .flow-stage .stage-title {{
    font-size: 11px;
    font-weight: 700;
  }}

  .flow-stage.fetch .stage-title  {{ color: #22d3ee; }}
  .flow-stage.chunk .stage-title  {{ color: #34d399; }}
  .flow-stage.evidence .stage-title {{ color: #a78bfa; }}
  .flow-stage.require .stage-title {{ color: #fbbf24; }}

  .flow-arrow {{
    display: flex;
    align-items: center;
    margin: 0 -4px;
  }}

  .flow-arrow svg {{
    width: 40px;
    height: 20px;
  }}

  .flow-arrow .line {{
    stroke: #334155;
    stroke-width: 2;
    stroke-dasharray: 4 3;
    animation: dash 1s linear infinite;
  }}

  .flow-arrow .head {{
    fill: #334155;
  }}

  @keyframes dash {{
    to {{ stroke-dashoffset: -14; }}
  }}

  .flow-legend {{
    margin-top: 16px;
    display: flex;
    gap: 16px;
    font-size: 10px;
    color: #64748b;
  }}

  .flow-legend .dot {{
    width: 6px; height: 6px;
    border-radius: 50%;
    display: inline-block;
    margin-right: 4px;
  }}

  .dot-cyan  {{ background: #22d3ee; }}
  .dot-green {{ background: #34d399; }}
  .dot-purp  {{ background: #a78bfa; }}
  .dot-amber {{ background: #fbbf24; }}

  /* Twin Simulation panel */
  .sim-controls {{
    display: flex;
    gap: 8px;
    margin-bottom: 16px;
  }}

  .sim-btn {{
    padding: 8px 16px;
    border-radius: 6px;
    border: 1px solid #334155;
    background: #1e293b;
    color: #e2e8f0;
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    cursor: pointer;
    transition: all 0.2s;
  }}

  .sim-btn:hover {{
    background: #334155;
    border-color: #38bdf8;
  }}

  .sim-btn.tick {{ border-color: #34d399; color: #34d399; }}
  .sim-btn.rollback {{ border-color: #fbbf24; color: #fbbf24; }}

  .sim-state {{
    padding: 12px;
    background: #0a0f1e;
    border-radius: 8px;
    font-size: 10px;
    color: #94a3b8;
    max-height: 240px;
    overflow-y: auto;
  }}

  .sim-state .sim-row {{
    display: flex;
    justify-content: space-between;
    padding: 4px 0;
    border-bottom: 1px solid #1e293b;
  }}

  .sim-state .sim-key {{ color: #64748b; }}
  .sim-state .sim-val {{ color: #e2e8f0; }}

  .ws-status {{
    margin-top: 12px;
    font-size: 10px;
    display: flex;
    align-items: center;
    gap: 6px;
  }}

  .ws-dot {{
    width: 8px; height: 8px;
    border-radius: 50%;
  }}

  .ws-dot.connected {{ background: #34d399; }}
  .ws-dot.disconnected {{ background: #ef4444; }}

  /* Scrollbar styling */
  ::-webkit-scrollbar {{ width: 6px; }}
  ::-webkit-scrollbar-track {{ background: #0f172a; }}
  ::-webkit-scrollbar-thumb {{ background: #334155; border-radius: 3px; }}
</style>
</head>
<body>

<div class="header">
  <div class="status-dot" id="status-dot"></div>
  <h1>b00t Admin Dashboard</h1>
  <div class="header-info" id="header-info">Pipeline v{} · Loading...</div>
</div>

<!-- Pipeline Status -->
<div class="panel" id="pipeline-panel">
  <h2><span class="icon">📊</span> Pipeline Status</h2>
  <div class="pipeline-stats" id="pipeline-stats">
    <div class="stat"><div class="stat-value" id="stat-chunks">0</div><div class="stat-label">Chunks</div></div>
    <div class="stat"><div class="stat-value" id="stat-evidence">0</div><div class="stat-label">Evidence</div></div>
    <div class="stat"><div class="stat-value" id="stat-reqs">0</div><div class="stat-label">Requirements</div></div>
    <div class="stat"><div class="stat-value" id="stat-fol">0</div><div class="stat-label">FOL Formulas</div></div>
  </div>
  <div class="pipeline-source" id="pipeline-source">
    <strong>No pipeline</strong> — Awaiting document ingestion
  </div>
</div>

<!-- Type Explorer -->
<div class="panel" id="type-panel">
  <h2><span class="icon">🔬</span> Type Explorer</h2>
  <div class="type-list" id="type-list"></div>
  <div class="code-tabs" id="code-tabs">
    <div class="code-tab active" data-tab="diagram">Diagram</div>
    <div class="code-tab" data-tab="wasm">WASM</div>
    <div class="code-tab" data-tab="cython">Cython</div>
    <div class="code-tab" data-tab="schema">Schema</div>
  </div>
  <div class="type-detail" id="type-detail">Select a type to inspect</div>
</div>

<!-- Process Flow -->
<div class="panel" id="flow-panel">
  <h2><span class="icon">🔄</span> Process Flow</h2>
  <div class="process-flow">
    <div class="flow-stages">
      <div class="flow-stage fetch">
        <div class="stage-icon">📄</div>
        <div class="stage-title">FETCH</div>
        <div style="font-size:9px;color:#94a3b8;margin-top:4px;">Arxiv/PDF → Markdown</div>
      </div>
      <div class="flow-arrow">
        <svg viewBox="0 0 40 20">
          <line class="line" x1="0" y1="10" x2="30" y2="10"/>
          <polygon class="head" points="30,10 24,6 24,14"/>
        </svg>
      </div>
      <div class="flow-stage chunk">
        <div class="stage-icon">🧩</div>
        <div class="stage-title">CHUNK</div>
        <div style="font-size:9px;color:#94a3b8;margin-top:4px;">Split + Embed</div>
      </div>
      <div class="flow-arrow">
        <svg viewBox="0 0 40 20">
          <line class="line" x1="0" y1="10" x2="30" y2="10"/>
          <polygon class="head" points="30,10 24,6 24,14"/>
        </svg>
      </div>
      <div class="flow-stage evidence">
        <div class="stage-icon">🔍</div>
        <div class="stage-title">EXTRACT</div>
        <div style="font-size:9px;color:#94a3b8;margin-top:4px;">Claims + Stats</div>
      </div>
      <div class="flow-arrow">
        <svg viewBox="0 0 40 20">
          <line class="line" x1="0" y1="10" x2="30" y2="10"/>
          <polygon class="head" points="30,10 24,6 24,14"/>
        </svg>
      </div>
      <div class="flow-stage require">
        <div class="stage-icon">📋</div>
        <div class="stage-title">DERIVE</div>
        <div style="font-size:9px;color:#94a3b8;margin-top:4px;">SysMLv2 ReqIF</div>
      </div>
    </div>
    <div class="flow-legend">
      <span><span class="dot dot-cyan"></span> DocumentSource</span>
      <span><span class="dot dot-green"></span> SemanticChunk</span>
      <span><span class="dot dot-purp"></span> Evidence</span>
      <span><span class="dot dot-amber"></span> Requirement</span>
    </div>
  </div>
</div>

<!-- Twin Simulation -->
<div class="panel" id="sim-panel">
  <h2><span class="icon">👥</span> Twin Simulation</h2>
  <div class="sim-controls">
    <button class="sim-btn tick" onclick="simTick()">▶ Tick</button>
    <button class="sim-btn rollback" onclick="simRollback()">↩ Rollback</button>
    <button class="sim-btn" onclick="simDeltas()">+ Delta</button>
  </div>
  <div class="sim-state" id="sim-state">
    <div class="sim-row"><span class="sim-key">Name</span><span class="sim-val">doc-pipeline</span></div>
    <div class="sim-row"><span class="sim-key">Tick</span><span class="sim-val" id="sim-tick">0</span></div>
    <div class="sim-row"><span class="sim-key">History</span><span class="sim-val" id="sim-history">0</span></div>
    <div class="sim-row"><span class="sim-key">Subscribers</span><span class="sim-val" id="sim-subs">0</span></div>
  </div>
  <div class="ws-status">
    <div class="ws-dot disconnected" id="ws-dot"></div>
    <span id="ws-text">WebSocket: disconnected</span>
  </div>
</div>

<script>
// ═══════════ Pipeline Data ═══════════
const PIPELINE = {pipeline_json};
const TYPES = {types_json};

let currentType = null;
let currentTab = 'diagram';
let typeData = {{}};

// ═══════════ Initialize Pipeline Panel ═══════════
function updatePipeline() {{
  const p = PIPELINE;
  document.getElementById('stat-chunks').textContent = p.chunk_count || 0;
  document.getElementById('stat-evidence').textContent = p.evidence_count || 0;
  document.getElementById('stat-reqs').textContent = p.requirement_count || 0;
  document.getElementById('stat-fol').textContent = p.fol_formula_count || 0;

  const src = document.getElementById('pipeline-source');
  if (p.has_pipeline) {{
    src.innerHTML = '<strong>' + (p.source_id || 'N/A') + '</strong> — ' +
      (p.source_title || 'Untitled') +
      (p.pipeline_version ? ' · v' + p.pipeline_version : '');
  }} else {{
    src.innerHTML = '<strong>No pipeline</strong> — Awaiting document ingestion';
  }}

  document.getElementById('header-info').textContent =
    'Pipeline v' + (p.pipeline_version || '—') + ' · ' +
    (p.executed_at ? new Date(p.executed_at).toLocaleString() : 'Not executed');
}}

// ═══════════ Initialize Type Explorer ═══════════
function initTypeExplorer() {{
  const list = document.getElementById('type-list');
  TYPES.forEach(function(name) {{
    const item = document.createElement('div');
    item.className = 'type-item';
    item.textContent = name;
    item.onclick = function() {{ selectType(name); }};
    list.appendChild(item);
  }});
}}

async function selectType(name) {{
  currentType = name;

  // Highlight active
  document.querySelectorAll('.type-item').forEach(function(el) {{
    el.classList.toggle('active', el.textContent === name);
  }});

  // Fetch type details
  try {{
    const resp = await fetch('/api/admin/types/' + encodeURIComponent(name));
    if (!resp.ok) throw new Error('Not found');
    typeData[name] = await resp.json();
    renderTypeDetail();
  }} catch(e) {{
    document.getElementById('type-detail').textContent = 'Error: ' + e.message;
  }}
}}

function renderTypeDetail() {{
  const data = typeData[currentType];
  if (!data) return;

  const detail = document.getElementById('type-detail');
  const tab = currentTab;

  if (tab === 'diagram' && data.schema) {{
    const fields = data.schema.fields || [];
    let html = '<div style="color:#38bdf8;font-size:11px;margin-bottom:8px;">' +
      data.schema.name;
    if (data.schema.ufo_stereotype) {{
      html += ' <span class="type-tag">' + data.schema.ufo_stereotype + '</span>';
    }}
    html += '</div>';
    fields.forEach(function(f) {{
      const opt = f.is_optional ? '?' : '';
      html += '<div style="padding:2px 0;">' +
        '<span class="field-name">' + f.name + opt + '</span>: ' +
        '<span class="field-type">' + f.rust_type + '</span>';
      if (f.description) {{
        html += ' <span style="color:#475569;font-size:9px;">// ' + f.description + '</span>';
      }}
      html += '</div>';
    }});
    detail.innerHTML = html;
  }} else if (tab === 'wasm' && data.codegen && data.codegen.wasm) {{
    detail.innerHTML = '<pre style="color:#94a3b8;font-size:10px;">' +
      escapeHtml(data.codegen.wasm.substring(0, 3000)) + '</pre>';
  }} else if (tab === 'cython' && data.codegen && data.codegen.cython) {{
    detail.innerHTML = '<pre style="color:#94a3b8;font-size:10px;">' +
      escapeHtml(data.codegen.cython.substring(0, 3000)) + '</pre>';
  }} else if (tab === 'schema' && data.schema) {{
    detail.innerHTML = '<pre style="color:#94a3b8;font-size:10px;">' +
      escapeHtml(JSON.stringify(data.schema.json_schema, null, 2).substring(0, 3000)) + '</pre>';
  }} else {{
    detail.innerHTML = '<span style="color:#64748b;">No ' + tab + ' output available</span>';
  }}
}}

function escapeHtml(text) {{
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;')
    .replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}}

// ═══════════ Code Tabs ═══════════
document.getElementById('code-tabs').addEventListener('click', function(e) {{
  if (e.target.classList.contains('code-tab')) {{
    document.querySelectorAll('.code-tab').forEach(function(t) {{
      t.classList.remove('active');
    }});
    e.target.classList.add('active');
    currentTab = e.target.dataset.tab;
    if (currentType) renderTypeDetail();
  }}
}});

// ═══════════ Simulation Controls ═══════════
async function simTick() {{
  try {{
    const resp = await fetch('/api/admin/simulate/tick');
    const data = await resp.json();
    document.getElementById('sim-tick').textContent = data.tick;
    document.getElementById('sim-history').textContent = data.history_len;
  }} catch(e) {{ console.error('Tick error:', e); }}
}}

async function simRollback() {{
  try {{
    const resp = await fetch('/api/admin/simulate/state');
    const data = await resp.json();
    if (data.history_len > 0) {{
      // Rollback via WebSocket
      if (ws && ws.readyState === WebSocket.OPEN) {{
        ws.send('rollback');
      }}
    }}
  }} catch(e) {{ console.error('Rollback error:', e); }}
}}

function simDeltas() {{
  const delta = prompt('Enter JSON delta to apply:');
  if (delta && ws && ws.readyState === WebSocket.OPEN) {{
    ws.send('delta:' + delta);
  }}
}}

// ═══════════ WebSocket ═══════════
let ws = null;

function connectWebSocket() {{
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const wsUrl = protocol + '//' + window.location.host + '/ws';

  ws = new WebSocket(wsUrl);

  ws.onopen = function() {{
    document.getElementById('ws-dot').className = 'ws-dot connected';
    document.getElementById('ws-text').textContent = 'WebSocket: connected';
    document.getElementById('status-dot').style.background = '#34d399';
  }};

  ws.onmessage = function(event) {{
    try {{
      const update = JSON.parse(event.data);
      document.getElementById('sim-tick').textContent = update.tick;
      if (update.event_type === 'tick' || update.event_type === 'rollback') {{
        fetchSimState();
      }}
    }} catch(e) {{}}
  }};

  ws.onclose = function() {{
    document.getElementById('ws-dot').className = 'ws-dot disconnected';
    document.getElementById('ws-text').textContent = 'WebSocket: disconnected (reconnecting...)';
    document.getElementById('status-dot').style.background = '#ef4444';
    setTimeout(connectWebSocket, 3000);
  }};

  ws.onerror = function() {{
    ws.close();
  }};
}}

async function fetchSimState() {{
  try {{
    const resp = await fetch('/api/admin/simulate/state');
    const data = await resp.json();
    document.getElementById('sim-tick').textContent = data.tick;
    document.getElementById('sim-history').textContent = data.history_len;
    document.getElementById('sim-subs').textContent = data.subscriber_count;
  }} catch(e) {{}}
}}

// ═══════════ Init ═══════════
updatePipeline();
initTypeExplorer();
connectWebSocket();
fetchSimState();

// Refresh pipeline state periodically
setInterval(async function() {{
  try {{
    const resp = await fetch('/api/admin/pipeline');
    const data = await resp.json();
    Object.assign(PIPELINE, data);
    updatePipeline();
  }} catch(e) {{}}
}}, 5000);
</script>

</body>
</html>"#,
        env!("CARGO_PKG_VERSION"),
    )
}

// ═══════════════════════════════════════════════════════════════════════════
// Server setup
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() {
    let config = AdminConfig::default();

    let state = Arc::new(Mutex::new(AppState::new(config.clone())));

    let app = Router::new()
        // Dashboard
        .route("/", get(dashboard_handler))
        .route("/admin", get(dashboard_handler))
        // Health check
        .route("/health", get(health_handler))
        .route("/healthz", get(health_handler))
        // API — pipeline state
        .route("/api/admin/pipeline", get(pipeline_handler))
        // API — type introspection
        .route("/api/admin/types", get(types_list_handler))
        .route("/api/admin/types/{name}", get(type_detail_handler))
        // API — simulation
        .route("/api/admin/simulate/tick", get(simulate_tick_handler))
        .route("/api/admin/simulate/state", get(simulate_state_handler))
        // WebSocket
        .route("/ws", get(ws_handler))
        // Reverse proxy — catch-all /v1/*
        .route("/v1/{*path}", get(proxy_handler).post(proxy_handler).put(proxy_handler)
            .patch(proxy_handler).delete(proxy_handler).head(proxy_handler)
            .options(proxy_handler))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("{}:{}", config.admin_host, config.admin_port);
    println!("b00t-admin starting on {addr}");
    println!("  Dashboard:    http://{addr}/");
    println!("  LLM backend:  {}", config.llm_backend_url);
    println!("  WebSocket:    ws://{addr}/ws");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
