//! b00t-admin — Internal admin dashboard server.
//!
//! Serves:
//! - `/` → Admin dashboard SPA (from b00t-ui/dist/spa/)
//! - `/v1/*` → Reverse proxy to LLM backend
//! - `/api/admin/*` → JSON API for pipeline state/type introspection/viz
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
use axum::http::header;
use tower_http::services::ServeDir;
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

/// GET `/` or `/admin` — Admin dashboard SPA
async fn dashboard_handler() -> impl IntoResponse {
    // Serve the SPA index.html — the frontend handles routing client-side
    let spa_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("b00t-ui/dist/spa/index.html");

    match tokio::fs::read_to_string(&spa_path).await {
        Ok(html) => Html(html).into_response(),
        Err(_) => {
            // Fallback: try relative from cwd
            let cwd_path = std::path::Path::new("b00t-ui/dist/spa/index.html");
            match std::fs::read_to_string(cwd_path) {
                Ok(html) => Html(html).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("SPA not built. Run: cd b00t-ui && quasar build\nError: {e}"),
                ).into_response(),
            }
        }
    }
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

/// GET `/api/admin/health` — System health metrics
async fn health_metrics_handler() -> impl IntoResponse {
    let uptime = std::process::Command::new("uptime")
        .output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_string();

    let memory = std::process::Command::new("free")
        .arg("-h")
        .output().ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    let cpu_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);

    let load_avg = std::fs::read_to_string("/proc/loadavg")
        .unwrap_or_default();

    axum::Json(serde_json::json!({
        "status": "operational",
        "service": "b00t-admin",
        "version": env!("CARGO_PKG_VERSION"),
        "uptime": uptime,
        "cpu": {
            "logical_cores": cpu_count,
            "load_avg": load_avg.trim(),
        },
        "memory": memory,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// GET `/api/admin/processes` — Hive process NodeGraph (SysMLv2/KerML visual)
async fn processes_handler() -> impl IntoResponse {
    use b00t_c0re_lib::pipeline_nodes::{
        build_graph_from_pipeline, ChunkNode, EvidenceNode, FetchNode,
        LegislationChunker, RequirementsNode,
    };

    // Document evidence pipeline (generic)
    let fetch_graph = build_graph_from_pipeline(&FetchNode);
    let chunk_graph = build_graph_from_pipeline(&ChunkNode);
    let evidence_graph = build_graph_from_pipeline(&EvidenceNode);
    let req_graph = build_graph_from_pipeline(&RequirementsNode);

    // ATO legislation pipeline
    let legis_graph = build_graph_from_pipeline(&LegislationChunker);

    axum::Json(serde_json::json!({
        "pipeline": "hive-document-evidence",
        "version": env!("CARGO_PKG_VERSION"),
        "nodes": [
            fetch_graph,
            chunk_graph,
            evidence_graph,
            req_graph,
        ],
        "mermaid": format!(
            "{}\n{}\n{}\n{}",
            fetch_graph.to_mermaid(),
            chunk_graph.to_mermaid(),
            evidence_graph.to_mermaid(),
            req_graph.to_mermaid(),
        ),
        "pipelines": {
            "ato-legislation": {
                "description": "ATO Legislation ingestion: AtoClient → LegislationChunker → EvidenceNode → RequirementsNode",
                "jurisdiction": "AU",
                "acts": ["ITAA 1997", "ITAA 1936", "GST Act 1999", "FBT Act 1986"],
                "nodes": [legis_graph, evidence_graph, req_graph],
                "mermaid": format!(
                    "{}\n{}\n{}",
                    legis_graph.to_mermaid(),
                    evidence_graph.to_mermaid(),
                    req_graph.to_mermaid(),
                ),
                "health": {
                    "source": "https://www.legislation.gov.au",
                    "rate_limit_secs": 3,
                    "datum": "ato-legislation.cli.toml",
                },
            }
        },
        "export_formats": ["mermaid", "svg", "comfyui", "json"],
    }))
}

// ═══════════════════════════════════════════════════════════════════════════
// ── Viz endpoints ──────────────────────────────────────────────────────────

/// GET `/api/admin/viz/entangle` — Datum entanglement graph (Mermaid + SVG)
async fn viz_entangle_handler() -> impl IntoResponse {
    viz_output("entangle")
}

/// GET `/api/admin/viz/task` — Task dependency graph (Mermaid + SVG)
async fn viz_task_handler() -> impl IntoResponse {
    viz_output("task")
}

fn viz_output(subcommand: &str) -> impl IntoResponse {
    let mermaid = std::process::Command::new("b00t")
        .args(["viz", subcommand, "--format", "mermaid"])
        .output()
        .ok()
        .and_then(|o| o.status.success().then(|| {
            let raw = String::from_utf8_lossy(&o.stdout).to_string();
            raw.replace("```mermaid\n", "").replace("\n```", "").trim().to_string()
        }))
        .unwrap_or_default();

    let svg = std::process::Command::new("b00t")
        .args(["viz", subcommand, "--format", "svg"])
        .output()
        .ok()
        .and_then(|o| o.status.success().then(|| String::from_utf8_lossy(&o.stdout).to_string()))
        .unwrap_or_default();

    axum::Json(serde_json::json!({
        "viz_type": subcommand,
        "mermaid": mermaid,
        "svg": svg,
    }))
}

// Dashboard HTML (embedded)
// ═══════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════
// Server setup

// ═══════════════════════════════════════════════════════════════════════════
// Server setup
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() {
    let config = AdminConfig::default();

    let state = Arc::new(Mutex::new(AppState::new(config.clone())));

    let app = Router::new()
        // Dashboard SPA
        .route("/", get(dashboard_handler))
        .route("/admin", get(dashboard_handler))
        // SPA static assets (JS, CSS, fonts)
        .nest_service("/assets", ServeDir::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent().unwrap_or(std::path::Path::new("."))
                .join("b00t-ui/dist/spa/assets")
        ))
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
        // API — health and processes
        .route("/api/admin/health", get(health_metrics_handler))
        .route("/api/admin/processes", get(processes_handler))
        // API — visualizations
        .route("/api/admin/viz/entangle", get(viz_entangle_handler))
        .route("/api/admin/viz/task", get(viz_task_handler))
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
