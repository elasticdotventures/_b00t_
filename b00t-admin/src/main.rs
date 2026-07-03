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
use tower_http::services::ServeDir;
use b00t_admin::{
    DigitalTwin, PipelineStateSnapshot, TypeSchema, WasmCodegen,
    registered_type_names,
};
use b00t_l3dg3rr_viz::isometric::{parse_mermaid, graph_to_isometric_response, graph_to_container_response, render_mermaid_native, filter_orphans, filter_orphans_from_mermaid};
use b00t_l3dg3rr_viz::tax_lawyer_demo;
use b00t_c0re_lib::doc_pipeline::FullPipelineResult;
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use reqwest::Client as ReqwestClient;

include!(concat!(env!("OUT_DIR"), "/build_info.rs"));
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

/// GET `/wasm/` — Dioxus WASM SPA (next-gen dashboard)
async fn wasm_handler() -> impl IntoResponse {
    let wasm_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("b00t-ui-wasm/dist");

    let index_path = wasm_dir.join("index.html");
    match tokio::fs::read_to_string(&index_path).await {
        Ok(html) => Html(html).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "WASM SPA not built. Run: just build-wasm".to_string(),
        ).into_response(),
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

/// GET `/api/admin/datums` — Datum health dashboard (parse errors, install status, stale binaries)
async fn datum_health_handler() -> impl IntoResponse {
    let output = std::process::Command::new("b00t-cli")
        .args(["mcp", "list", "--json", "--all"])
        .output();
    match output {
        Ok(out) if out.status.success() => {
            let body = String::from_utf8_lossy(&out.stdout);
            match serde_json::from_str::<serde_json::Value>(&body) {
                Ok(json) => {
                    let servers = json.get("servers").cloned().unwrap_or_default();
                    let total = servers.as_array().map(|a| a.len()).unwrap_or(0);
                    let errors: Vec<_> = servers.as_array().map(|a| {
                        a.iter().filter(|s| s.get("error").and_then(|e| e.as_str()).is_some()).cloned().collect::<Vec<_>>()
                    }).unwrap_or_default();
                    let not_installed: Vec<_> = servers.as_array().map(|a| {
                        a.iter().filter(|s| !s.get("is_installed").unwrap_or(&serde_json::Value::Bool(false)).as_bool().unwrap_or(false)).cloned().collect::<Vec<_>>()
                    }).unwrap_or_default();
                    axum::Json(serde_json::json!({
                        "total": total,
                        "healthy": total.saturating_sub(errors.len()).saturating_sub(not_installed.len()),
                        "parse_errors": errors.len(),
                        "not_installed": not_installed.len(),
                        "servers": servers,
                    }))
                }
                Err(_) => axum::Json(serde_json::json!({"error": "parse failed"})),
            }
        }
        _ => axum::Json(serde_json::json!({"error": "b00t-cli unavailable"})),
    }
}

/// GET `/api/admin/graph` — Knowledge graph health (connectivity, isolates, hubs)
#[allow(dead_code)]
async fn graph_health_handler() -> impl IntoResponse {
    let project_name = std::env::current_dir()
        .map(|p| p.to_string_lossy().replace('/', "-").to_string())
        .unwrap_or_default();
    let output = std::process::Command::new("codebase-memory-mcp")
        .args(["cli", "get_graph_schema", &format!("{{\"project\":\"{project_name}\"}}")])
        .output();
    let kg = match output {
        Ok(out) if out.status.success() => {
            let body = String::from_utf8_lossy(&out.stdout);
            serde_json::from_str::<serde_json::Value>(&body).unwrap_or_default()
        }
        _ => serde_json::json!({"status": "offline"})
    };
    axum::Json(serde_json::json!({
        "knowledge_graph": kg,
        "mcp_health": "GET /api/admin/datums for MCP status",
    }))
}

/// Join multiple mermaid diagram strings into a single fenced block with --- separators.
fn join_mermaid(diagrams: &[String]) -> String {
    let blocks: Vec<_> = diagrams.iter()
        .map(|m| m.trim_start_matches("```mermaid\n").trim_end_matches("\n```").trim().to_string())
        .collect();
    format!("```mermaid\n{}\n```", blocks.join("\n\n---\n\n"))
}

/// GET `/api/admin/viz/isometric` — Isometric 3D graph view (SVG + glTF)
/// Uses the `kasuari` Cassowary constraint solver for deterministic layout.
/// Query params: `hide_orphans=true` — strip nodes with zero edges.
async fn viz_isometric_handler(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let hide_orphans = params.get("hide_orphans").map(|v| v == "true").unwrap_or(false);
    let output = std::process::Command::new("b00t-cli")
        .args(["viz", "entangle", "--format", "mermaid"])
        .output();
    let raw = output.ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .replace("```mermaid\n", "").replace("\n```", "")
        .replace("graph LR", "flowchart LR").replace("graph TD", "flowchart TD");

    match parse_mermaid(&raw) {
        Ok(mut graph) => {
            if hide_orphans {
                graph = filter_orphans(&graph);
            }
            if let Err(e) = graph.validate() {
                return axum::Json(serde_json::json!({
                    "svg": format!("<svg><text fill='red'>validation: {}</text></svg>", e),
                    "format": "isometric",
                    "error": e.to_string()
                }));
            }
            let response = graph_to_isometric_response(&graph);
            match response {
                Ok(r) => r.into(),
                Err(_) => {
                    // Direct layout failed (>40 nodes). Try container grouping.
                    graph_to_container_response(&graph)
                        .unwrap_or_else(|e| serde_json::json!({
                            "svg": format!("<svg><text fill='red'>{}</text></svg>", e),
                            "format": "isometric",
                            "error": e
                        }))
                        .into()
                }
            }
        }
        Err(e) => axum::Json(serde_json::json!({
            "svg": format!("<svg><text fill='red'>parse: {}</text></svg>", e),
            "format": "isometric",
            "error": e.to_string()
        })),
    }
}

/// GET `/api/admin/viz/isometric/demo` — Tax-Lawyer demonstration graph
async fn viz_isometric_demo_handler() -> impl IntoResponse {
    let graph = tax_lawyer_demo();
    axum::Json(graph_to_isometric_response(&graph).unwrap_or_else(|e| serde_json::json!({
        "svg": format!("<svg><text fill='red'>{}</text></svg>", e),
        "format": "isometric",
        "error": e
    })))
}

/// POST `/api/admin/viz/mermaid/render` — Server-side Mermaid SVG rendering
async fn viz_mermaid_render_handler(
    axum::extract::Json(body): axum::extract::Json<serde_json::Value>,
) -> impl IntoResponse {
    let text = body.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let hide_orphans = body.get("hide_orphans").and_then(|v| v.as_bool()).unwrap_or(false);
    if text.is_empty() {
        return axum::Json(serde_json::json!({"svg": "", "error": "missing text"}));
    }
    let to_render = if hide_orphans {
        let graph = filter_orphans_from_mermaid(text);
        graph.to_mermaid()
    } else {
        text.to_string()
    };
    match render_mermaid_native(&to_render) {
        Ok(svg) => axum::Json(serde_json::json!({"svg": svg})),
        Err(e) => axum::Json(serde_json::json!({
            "svg": format!("<svg><text fill='red'>{}</text></svg>", e),
            "error": e,
        })),
    }
}

/// GET `/api/admin/display` — DatumType visual display descriptors (shapes, colors, SVG)
async fn datum_display_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "displays": [
            {"datum_type":"K8s","label":"K8s","shape":"hexagon","color":"#326ce5","border_color":"#5b9cf5","icon":"☸","css_class":"dt-k8s"},
            {"datum_type":"Docker","label":"Docker","shape":"hexagon","color":"#1d63ed","border_color":"#4d8bf7","icon":"🐳","css_class":"dt-docker"},
            {"datum_type":"Hardware","label":"Hardware","shape":"hexagon","color":"#1a56db","border_color":"#3f83f8","icon":"💻","css_class":"dt-hardware"},
            {"datum_type":"Overlay","label":"Overlay","shape":"hexagon","color":"#1e429f","border_color":"#4789fa","icon":"📋","css_class":"dt-overlay"},
            {"datum_type":"Runtime","label":"Runtime","shape":"hexagon","color":"#233876","border_color":"#6094f7","icon":"⚡","css_class":"dt-runtime"},
            {"datum_type":"Nix","label":"Nix","shape":"hexagon","color":"#5271ff","border_color":"#7b93ff","icon":"❄️","css_class":"dt-nix"},
            {"datum_type":"Agent","label":"Agent","shape":"circle","color":"#059669","border_color":"#34d399","icon":"🤖","css_class":"dt-agent"},
            {"datum_type":"Role","label":"Role","shape":"circle","color":"#047857","border_color":"#2dd4bf","icon":"🎭","css_class":"dt-role"},
            {"datum_type":"Ai","label":"AI","shape":"circle","color":"#065f46","border_color":"#22c55e","icon":"🧠","css_class":"dt-ai"},
            {"datum_type":"Training","label":"Training","shape":"circle","color":"#064e3b","border_color":"#10b981","icon":"🎓","css_class":"dt-training"},
            {"datum_type":"Mcp","label":"MCP","shape":"diamond","color":"#7c3aed","border_color":"#a78bfa","icon":"🔌","css_class":"dt-mcp"},
            {"datum_type":"McpServer","label":"MCP Server","shape":"diamond","color":"#6d28d9","border_color":"#8b5cf6","icon":"🖥️","css_class":"dt-mcp-server"},
            {"datum_type":"Api","label":"API","shape":"diamond","color":"#5b21b6","border_color":"#7c3aed","icon":"🔗","css_class":"dt-api"},
            {"datum_type":"Schema","label":"Schema","shape":"diamond","color":"#4c1d95","border_color":"#6d28d9","icon":"📐","css_class":"dt-schema"},
            {"datum_type":"Skill","label":"Skill","shape":"triangle","color":"#d97706","border_color":"#fbbf24","icon":"🛠️","css_class":"dt-skill"},
            {"datum_type":"Job","label":"Job","shape":"triangle","color":"#b45309","border_color":"#f59e0b","icon":"⏱️","css_class":"dt-job"},
            {"datum_type":"Hook","label":"Hook","shape":"triangle","color":"#92400e","border_color":"#d97706","icon":"🪝","css_class":"dt-hook"},
            {"datum_type":"Gate","label":"Gate","shape":"triangle","color":"#78350f","border_color":"#c27803","icon":"🚧","css_class":"dt-gate"},
            {"datum_type":"Config","label":"Config","shape":"rectangle","color":"#0d9488","border_color":"#2dd4bf","icon":"⚙️","css_class":"dt-config"},
            {"datum_type":"Bash","label":"Bash","shape":"rectangle","color":"#0f766e","border_color":"#14b8a6","icon":"💻","css_class":"dt-bash"},
            {"datum_type":"Cli","label":"CLI","shape":"rectangle","color":"#115e59","border_color":"#0d9488","icon":"⌨️","css_class":"dt-cli"},
            {"datum_type":"Justfile","label":"Justfile","shape":"rectangle","color":"#134e4a","border_color":"#0f766e","icon":"📜","css_class":"dt-justfile"},
            {"datum_type":"Plan","label":"Plan","shape":"rectangle","color":"#0f766e","border_color":"#14b8a6","icon":"📋","css_class":"dt-plan"},
            {"datum_type":"Vendor","label":"Vendor","shape":"rectangle","color":"#115e59","border_color":"#0d9488","icon":"📦","css_class":"dt-vendor"},
            {"datum_type":"Stack","label":"Stack","shape":"vee","color":"#be123c","border_color":"#fb7185","icon":"📚","css_class":"dt-stack"},
            {"datum_type":"Repo","label":"Repo","shape":"vee","color":"#9f1239","border_color":"#f43f5e","icon":"📁","css_class":"dt-repo"},
            {"datum_type":"Vscode","label":"VSCode","shape":"vee","color":"#881337","border_color":"#e11d48","icon":"🆚","css_class":"dt-vscode"},
            {"datum_type":"Apt","label":"Apt","shape":"vee","color":"#4c0519","border_color":"#9f1239","icon":"📦","css_class":"dt-apt"},
            {"datum_type":"Database","label":"Database","shape":"rectangle","color":"#475569","border_color":"#94a3b8","icon":"🗄️","css_class":"dt-database"},
            {"datum_type":"HiveProfile","label":"Hive","shape":"rectangle","color":"#334155","border_color":"#64748b","icon":"🏗️","css_class":"dt-hive"},
            {"datum_type":"Polyseme","label":"Polyseme","shape":"circle","color":"#1e293b","border_color":"#475569","icon":"🔮","css_class":"dt-polyseme"},
            {"datum_type":"Credential","label":"Credential","shape":"circle","color":"#0f172a","border_color":"#334155","icon":"🔐","css_class":"dt-credential"},
            {"datum_type":"Unknown","label":"Unknown","shape":"rectangle","color":"#1e293b","border_color":"#475569","icon":"❓","css_class":"dt-unknown"}
        ]
    }))
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
        "version": VERSION,
        "built_at": BUILD_TIMESTAMP,
        "git": GIT_HASH,
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
        "mermaid": join_mermaid(&[fetch_graph.to_mermaid(), chunk_graph.to_mermaid(), evidence_graph.to_mermaid(), req_graph.to_mermaid()]),
        "pipelines": {
            "ato-legislation": {
                "description": "ATO Legislation ingestion: AtoClient → LegislationChunker → EvidenceNode → RequirementsNode",
                "jurisdiction": "AU",
                "acts": ["ITAA 1997", "ITAA 1936", "GST Act 1999", "FBT Act 1986"],
                "nodes": [&legis_graph, &evidence_graph, &req_graph],
                "mermaid": join_mermaid(&[legis_graph.to_mermaid(), evidence_graph.to_mermaid(), req_graph.to_mermaid()]),
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
            let cleaned = raw.replace("```mermaid\n", "").replace("\n```", "").trim().to_string();
            // Mermaid v11 dropped `graph` syntax — must use `flowchart`
            cleaned.replace("graph LR", "flowchart LR").replace("graph TD", "flowchart TD").replace("graph RL", "flowchart RL")
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

#[allow(unused_variables)]
pub fn dashboard_html(pipeline_json: &str, types_json: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<meta name="b00t-emoji" content="🥾">
<title>b00t Admin Dashboard</title>
<link rel="icon" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><text y='.9em' font-size='90'>🥾</text></svg>">
<script src="https://cdn.jsdelivr.net/npm/cytoscape@3/dist/cytoscape.min.js"></script>
<style>
  @import url('https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;700&display=swap');

  * {{ margin: 0; padding: 0; box-sizing: border-box; }}

  body {{
    background: #020617;
    color: #e2e8f0;
    font-family: 'JetBrains Mono', monospace;
    display: flex;
    min-height: 100vh;
    margin: 0;
    padding: 0;
  }}

  /* ── Sidebar ── */
  .sidebar {{
    width: 200px;
    min-width: 200px;
    background: #0f172a;
    border-right: 1px solid #1e293b;
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow-y: auto;
    position: sticky;
    top: 0;
  }}

  .sidebar-header {{
    padding: 16px;
    border-bottom: 1px solid #1e293b;
  }}

  .sidebar-header h1 {{
    font-size: 16px;
    color: #38bdf8;
    margin: 0;
  }}

  .sidebar-header .header-info {{
    font-size: 10px;
    color: #64748b;
    margin-top: 4px;
  }}

  .accordion-section {{
    border-bottom: 1px solid #1e293b;
  }}

  .accordion-header {{
    padding: 12px 16px;
    cursor: pointer;
    font-size: 12px;
    color: #94a3b8;
    display: flex;
    align-items: center;
    gap: 8px;
    user-select: none;
    transition: background 0.15s;
  }}

  .accordion-header:hover {{ background: #1e293b; }}
  .accordion-header.active {{ color: #38bdf8; background: rgba(56,189,248,0.08); }}

  .accordion-arrow {{
    margin-left: auto;
    font-size: 10px;
    transition: transform 0.2s;
    color: #475569;
  }}

  .accordion-header.active .accordion-arrow {{ transform: rotate(90deg); }}

  .accordion-body {{
    display: none;
    padding: 12px 16px;
  }}

  .accordion-body.open {{ display: block; }}

  /* ── Main content ── */
  .main-content {{
    flex: 1;
    padding: 20px;
    overflow-y: auto;
    height: 100vh;
  }}

  .main-content .panel {{
    background: #0f172a;
    border: 1px solid #1e293b;
    border-radius: 12px;
    padding: 20px;
    margin-bottom: 16px;
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
     display: none; /* hidden by default — shown when section opens */
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

/* ── Autopilot indicator ── */
#autopilot-badge {{
  display: none; position: fixed; bottom: 12px; right: 12px; z-index: 9999;
  padding: 6px 12px; border-radius: 20px; font-size: 10px; font-family: system-ui, sans-serif;
  background: rgba(251,191,36,0.15); border: 1px solid rgba(251,191,36,0.4); color: #fbbf24;
  align-items: center; gap: 6px; backdrop-filter: blur(4px);
}}
#autopilot-badge.show {{ display: flex; }}
.autopilot-dot {{ width: 6px; height: 6px; border-radius: 50%; background: #fbbf24; animation: pulse 1.5s infinite; }}

/* ── Progress bar ── */
.progress-bar {{
  width: 100%; height: 4px; background: #1e293b; border-radius: 2px; overflow: hidden; margin-top: 8px;
}}
.progress-fill {{
  height: 100%; background: linear-gradient(90deg, #38bdf8, #6366f1); border-radius: 2px;
  transition: width 0.3s ease; width: 0%;
}}
.progress-fill.indeterminate {{
  width: 30%; animation: progress-indeterminate 1.5s ease-in-out infinite;
}}
@keyframes progress-indeterminate {{
  0% {{ transform: translateX(-100%); }}
  100% {{ transform: translateX(400%); }}
}}

/* ── Status log ── */
.status-log {{
  margin-top: 8px; max-height: 120px; overflow-y: auto; font-size: 10px; color: #64748b;
  background: #1e293b; border-radius: 4px; padding: 4px 8px;
}}
.status-entry {{
  padding: 2px 0; border-bottom: 1px solid rgba(255,255,255,0.03);
}}
.status-entry .ts {{ color: #475569; margin-right: 6px; }}
.status-entry .msg {{ color: #94a3b8; }}
.status-entry.error .msg {{ color: #ef4444; }}
.status-entry.done .msg {{ color: #34d399; }}

/* ── Fade transitions ── */
.fade-in {{ animation: fadeIn 0.3s ease-in; }}
@keyframes fadeIn {{ from {{ opacity: 0; transform: translateY(4px); }} to {{ opacity: 1; transform: translateY(0); }} }}
@keyframes wasm-spin {{ to {{ transform: rotate(360deg); }} }}
.wasm-spinner {{ animation: wasm-spin 1s linear infinite; }}

  /* Scrollbar styling */
  ::-webkit-scrollbar {{ width: 6px; }}
  ::-webkit-scrollbar-track {{ background: #0f172a; }}
  ::-webkit-scrollbar-thumb {{ background: #334155; border-radius: 3px; }}
</style>
</head>
<body>
<!-- ── Sidebar ── -->
<div class="sidebar">
  <div class="sidebar-header">
    <div style="display:flex;align-items:center;gap:8px;margin-bottom:4px;">
      <div class="sidebar-header" id="status-dot" style="width:8px;height:8px;border-radius:50%;background:#34d399;animation:pulse 2s infinite;display:inline-block;"></div>
      <h1 style="font-size:16px;color:#38bdf8;margin:0;">b00t</h1>
    </div>
    <div class="header-info" id="header-info" style="font-size:10px;color:#64748b;margin-top:4px;display:flex;align-items:center;gap:6px;">
      <span id="heartbeat" style="display:inline-block;width:6px;height:6px;border-radius:50%;background:#34d399;flex-shrink:0;"></span>
      <span id="header-version">v0 ·</span>
      <span id="header-status">Loading...</span>
    </div>
    <div style="margin-top:8px;font-size:9px;color:#475569;border-top:1px solid #1e293b;padding-top:6px;">
      <span id="sidebar-version">🥾</span>
    </div>
  </div>
  <div class="accordion-section">
    <div class="accordion-header" onclick="toggleSection('pipeline')" data-b00t="section:pipeline" data-b00t-action="toggle" data-b00t-label="Pipeline Dashboard">📊 Pipeline <span class="accordion-arrow">▶</span></div>
    <div class="accordion-body" id="section-pipeline" style="padding:8px 16px;">
      <div style="display:grid;grid-template-columns:1fr 1fr;gap:4px;">
        <div style="background:rgba(56,189,248,0.06);border-radius:4px;padding:6px;text-align:center;"><div style="font-size:18px;font-weight:700;color:#38bdf8;" id="stat-chunks">0</div><div style="font-size:8px;color:#64748b;">Chunks</div></div>
        <div style="background:rgba(56,189,248,0.06);border-radius:4px;padding:6px;text-align:center;"><div style="font-size:18px;font-weight:700;color:#38bdf8;" id="stat-evidence">0</div><div style="font-size:8px;color:#64748b;">Evidence</div></div>
        <div style="background:rgba(56,189,248,0.06);border-radius:4px;padding:6px;text-align:center;"><div style="font-size:18px;font-weight:700;color:#38bdf8;" id="stat-reqs">0</div><div style="font-size:8px;color:#64748b;">Requirements</div></div>
        <div style="background:rgba(56,189,248,0.06);border-radius:4px;padding:6px;text-align:center;"><div style="font-size:18px;font-weight:700;color:#38bdf8;" id="stat-fol">0</div><div style="font-size:8px;color:#64748b;">FOL</div></div>
      </div>
      <div id="pipeline-source" style="margin-top:6px;font-size:9px;color:#64748b;"><strong style="color:#22d3ee;">No pipeline</strong></div>
    </div>
  </div>
  <div class="accordion-section">
    <div class="accordion-header" onclick="toggleSection('types')" data-b00t="section:types" data-b00t-action="toggle" data-b00t-label="Type Explorer">🔬 Types <span class="accordion-arrow">▶</span></div>
    <div class="accordion-body" id="section-types" style="padding:8px 16px;">
      <div class="type-list" id="type-list" style="max-height:150px;overflow-y:auto;"></div>
    </div>
  </div>
  <div class="accordion-section">
    <div class="accordion-header" onclick="toggleSection('sim')" data-b00t="section:sim" data-b00t-action="toggle" data-b00t-label="Twin Simulation">👥 Simulation <span class="accordion-arrow">▶</span></div>
    <div class="accordion-body" id="section-sim" style="padding:8px 16px;">
      <button class="sim-btn" onclick="simTick()" data-b00t="action:sim-tick" data-b00t-label="Simulation Tick" style="display:block;width:100%;margin-bottom:4px;padding:6px;font-size:11px;">▶ Tick</button>
      <button class="sim-btn rollback" onclick="simRollback()" data-b00t="action:sim-rollback" data-b00t-label="Simulation Rollback" style="display:block;width:100%;margin-bottom:4px;padding:6px;font-size:11px;">↩ Rollback</button>
      <div style="font-size:10px;color:#64748b;margin-top:4px;"><span style="color:#94a3b8;">Tick:</span> <span id="sim-tick">0</span> · <span style="color:#94a3b8;">History:</span> <span id="sim-history">0</span></div>
      <div style="font-size:10px;margin-top:4px;"><span id="ws-dot" style="display:inline-block;width:6px;height:6px;border-radius:50%;background:#ef4444;"></span> <span id="ws-text">WS: disconnected</span></div>
    </div>
  </div>
  <div class="accordion-section">
    <div class="accordion-header" onclick="toggleSection('viz')" data-b00t="section:viz" data-b00t-action="toggle" data-b00t-label="Visualizations">🎨 Visualizations <span class="accordion-arrow">▶</span></div>
    <div class="accordion-body" id="section-viz" style="padding:8px 16px;">
      <select id="viz-select" data-b00t="control:viz-select" data-b00t-action="select" data-b00t-label="Graph Type Selector" style="width:100%;background:#1e293b;color:#e2e8f0;border:1px solid #334155;padding:4px;border-radius:4px;font-family:inherit;font-size:11px;margin-bottom:4px;" onchange="onVizSelect()">
        <option value="">— Choose —</option>
        <option value="entangle">🔗 Entanglement</option>
        <option value="task">📋 Tasks</option>
        <option value="pipeline">📊 Pipeline</option>
        <option value="ato">🏛️ ATO</option>
        <option value="isometric">🧊 Isometric</option>
        <option value="kg">🕸️ Knowledge Graph</option>
      </select>
      <div id="viz-mode" style="display:flex;gap:2px;margin-bottom:4px;">
        <div class="code-tab active" data-viz="mermaid" data-b00t="tab:mermaid" data-b00t-label="Mermaid View">Mermaid</div>
        <div class="code-tab" data-viz="cytoscape" data-b00t="tab:cytoscape" data-b00t-label="Cytoscape View">Cytoscape</div>
      </div>
      <div style="margin:4px 0;display:flex;align-items:center;gap:6px;font-size:10px;color:#94a3b8;">
        <span style="margin-left:auto;color:#64748b;">Shift+scroll: 10× zoom</span>
      </div>
      <div id="viz-status" style="font-size:9px;color:#64748b;word-break:break-all;">Select a graph</div>
      <div class="progress-bar" id="progress-bar" style="display:none;"><div class="progress-fill" id="progress-fill"></div></div>
      <div class="status-log" id="status-log"></div>
    </div>
  </div>
</div>

<!-- ── Main Content ── -->
<div class="main-content">

<div class="panel" id="pipeline-panel" data-b00t="panel:pipeline" data-b00t-label="Pipeline Status">
  <h2>📊 Pipeline Status</h2>
  <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;">
    <div class="stat"><div class="stat-value" id="stat-chunks-2">0</div><div class="stat-label">Chunks</div></div>
    <div class="stat"><div class="stat-value" id="stat-evidence-2">0</div><div class="stat-label">Evidence</div></div>
    <div class="stat"><div class="stat-value" id="stat-reqs-2">0</div><div class="stat-label">Requirements</div></div>
    <div class="stat"><div class="stat-value" id="stat-fol-2">0</div><div class="stat-label">FOL Formulas</div></div>
  </div>
</div>

<div class="panel" id="type-panel" data-b00t="panel:types" data-b00t-label="Type Explorer">
  <h2>🔬 Type Explorer</h2>
  <div style="margin:0 0 8px;display:flex;align-items:center;gap:6px;font-size:10px;color:#94a3b8;">
    <input type="checkbox" id="hide-orphans" onchange="toggleOrphans()" data-b00t="control:hide-orphans">
    <label for="hide-orphans" data-b00t="label:hide-orphans">Hide orphan nodes (no connections)</label>
  </div>
  <div class="type-detail" id="type-detail">Select a type from the sidebar</div>
</div>

<div class="panel" id="sim-panel" data-b00t="panel:sim" data-b00t-label="Twin Simulation">
  <h2>👥 Twin Simulation</h2>
  <div id="sim-state">
    <div style="font-size:12px;color:#94a3b8;margin-bottom:4px;">Name: <span style="color:#e2e8f0;">doc-pipeline</span></div>
    <div style="font-size:12px;color:#94a3b8;margin-bottom:4px;">Tick: <span id="sim-tick-2" style="color:#e2e8f0;">0</span></div>
    <div style="font-size:12px;color:#94a3b8;margin-bottom:4px;">History: <span id="sim-history-2" style="color:#e2e8f0;">0</span></div>
    <div style="font-size:12px;color:#94a3b8;">Subscribers: <span id="sim-subs" style="color:#e2e8f0;">0</span></div>
  </div>
</div>

<div class="panel" id="viz-panel" data-b00t="panel:viz" data-b00t-label="Visualizations">
  <h2>🎨 <span id="viz-title">Visualization</span></h2>
  <div id="viz-mermaid-container" style="background:#0f172a;border-radius:6px;padding:12px;min-height:200px;overflow:auto;border:1px solid #1e293b;">
    <div class="mermaid" id="mermaid-target" style="text-align:center;color:#64748b;padding:40px;">Select a visualization</div>
  </div>
  <div id="viz-cytoscape-container" style="background:#0f172a;border-radius:6px;min-height:400px;display:none;border:1px solid #1e293b;">
    <div id="cytoscape-target" style="width:100%;height:400px;"></div>
  </div>
</div>

</div>

<script>
// ════════ Accordion ════════
function toggleSection(name) {{
  var body = document.getElementById('section-' + name);
  if (!body) return;
  var isOpen = body.classList.contains('open');
  body.classList.toggle('open', !isOpen);
  body.previousElementSibling.classList.toggle('active', !isOpen);
  var panelMap = {{ pipeline: 'pipeline-panel', types: 'type-panel', sim: 'sim-panel', viz: 'viz-panel' }};
  var panel = document.getElementById(panelMap[name]);
  if (panel) panel.style.display = isOpen ? 'none' : 'block';
  // Persist section state
  try {{ localStorage.setItem('b00t-section', name); localStorage.setItem('b00t-section-open', !isOpen); }} catch(e) {{}}
}}

// ════════ Keyboard Navigation ════════
document.addEventListener('keydown', function(e) {{
  if (e.altKey || e.ctrlKey || e.metaKey) return;
  var map = {{ '1': 'pipeline', '2': 'types', '3': 'sim', '4': 'viz' }};
  var section = map[e.key];
  if (section) {{
    e.preventDefault();
    // Close all sections
    ['pipeline','types','sim','viz'].forEach(function(s) {{
      var body = document.getElementById('section-' + s);
      if (!body) return;
      body.classList.remove('open');
      body.previousElementSibling.classList.remove('active');
    }});
    // Open target
    var body = document.getElementById('section-' + section);
    if (body) {{
      body.classList.add('open');
      body.previousElementSibling.classList.add('active');
    }}
    var panelMap = {{ pipeline: 'pipeline-panel', types: 'type-panel', sim: 'sim-panel', viz: 'viz-panel' }};
    Object.values(panelMap).forEach(function(id) {{
      var p = document.getElementById(id);
      if (p) p.style.display = 'none';
    }});
    var p = document.getElementById(panelMap[section]);
    if (p) p.style.display = 'block';
  }}
}});

// ════════ Mermaid Init (WASM) ════════

// ════════ Pipeline Update ════════
var PIPELINE = {{}};
var TYPES = [];

// Restore persisted UI state — only opens, never closes
(function() {{
  try {{
    var section = localStorage.getItem('b00t-section');
    var wasOpen = localStorage.getItem('b00t-section-open');
    var viz = localStorage.getItem('b00t-viz');
    // Only open if it was previously open; default is all closed
    if (section && wasOpen === 'true') {{
      var body = document.getElementById('section-' + section);
      if (body) {{
        body.classList.add('open');
        body.style.display = '';
        body.previousElementSibling.classList.add('active');
        var panelMap = {{ pipeline: 'pipeline-panel', types: 'type-panel', sim: 'sim-panel', viz: 'viz-panel' }};
        var panel = document.getElementById(panelMap[section]);
        if (panel) panel.style.display = 'block';
      }}
    }}
    if (viz && document.getElementById('viz-select')) {{
      document.getElementById('viz-select').value = viz;
      onVizSelect();
    }}
  }} catch(e) {{}}
}})();

// Load initial data from API (skip in test environments without fetch)
if (typeof fetch !== 'undefined') {{
    fetch('/api/admin/pipeline').then(function(r){{return r.json();}}).then(function(p){{ PIPELINE = p; updatePipeline(); }}).catch(function(e){{}});
    fetch('/api/admin/types').then(function(r){{return r.json();}}).then(function(t){{ TYPES = t.types || []; initTypeExplorer(); }}).catch(function(e){{}});
    setInterval(function(){{ fetch('/api/admin/pipeline').then(function(r){{return r.json();}}).then(function(p){{ PIPELINE = p; updatePipeline(); }}).catch(function(e){{}}); }}, 5000);
}}

function updatePipeline() {{
  var p = PIPELINE;
  ['chunks','evidence','reqs','fol'].forEach(function(k) {{
    var v = p[k + '_count'] || 0;
    var el = document.getElementById('stat-' + k);
    var el2 = document.getElementById('stat-' + k + '-2');
    if (el) el.textContent = v;
    if (el2) el2.textContent = v;
  }});
  var src = document.getElementById('pipeline-source');
  if (src) {{
    src.innerHTML = p.has_pipeline
      ? '<strong style="color:#22d3ee;">' + (p.source_id || 'N/A') + '</strong> — ' + (p.source_title || 'Untitled')
      : '<strong style="color:#22d3ee;">No pipeline</strong>';
  }}
}}

// ════════ Heartbeat + Version ════════
var beatFails = 0;
function beat() {{
  var hb = document.getElementById('heartbeat');
  var vs = document.getElementById('header-version');
  var st = document.getElementById('header-status');
  var sv = document.getElementById('sidebar-version');
  if (!hb || typeof fetch === 'undefined') return;
  fetch('/api/admin/health').then(function(r){{return r.json();}}).then(function(d) {{
    var ver = d.version || '?';
    var built = d.built_at || '';
    if (vs) vs.textContent = 'v' + ver + (built ? ' · built ' + built : '') + ' ·';
    if (sv) sv.textContent = '🥾 v' + ver;
    if (st) st.textContent = d.service || 'Healthy';
    hb.style.background = '#34d399';
    hb.style.animation = 'none';
    void hb.offsetHeight;
    hb.style.animation = 'pulse 2s infinite';
    beatFails = 0;
    // Remove crash banner if present
    var banner = document.getElementById('crash-banner');
    if (banner) banner.remove();
  }}).catch(function() {{
    beatFails++;
    hb.style.background = '#ef4444';
    hb.style.animation = 'none';
    if (st) st.textContent = beatFails > 2 ? 'Server crashed' : 'Offline';
    // Show crash banner after 3 consecutive failures
    if (beatFails >= 3 && !document.getElementById('crash-banner')) {{
      var banner = document.createElement('div');
      banner.id = 'crash-banner';
      banner.style.cssText = 'position:fixed;top:0;left:0;right:0;background:#ef4444;color:#fff;padding:12px 20px;text-align:center;font-size:13px;z-index:9999;animation:pulse 1s infinite;';
      banner.innerHTML = '🥾 Server crashed — <a href="javascript:location.reload()" style="color:#fff;text-decoration:underline;">reload</a> when back (auto-retry every 5s)';
      document.body.prepend(banner);
    }} else if (beatFails >= 3) {{
      document.getElementById('crash-banner').textContent = '🥾 Server down (' + beatFails + ' retries) — reload when back';
    }}
  }});
}}
// Beat on load and every 30s
beat();
setInterval(beat, 30000);

// Initial beat
setTimeout(beat, 100);

// ════════ Viz Panel ════════
var currentVizTab = 'mermaid';
var currentVizData = null;

function onVizSelect() {{
  var sel = document.getElementById('viz-select').value;
  if (!sel) {{ document.getElementById('viz-status').textContent = 'Select a graph type'; return; }}
  try {{ localStorage.setItem('b00t-viz', sel); }} catch(e) {{}}
  // Pick render engine per type
  if (sel === 'kg') {{
    // Knowledge Graph → Cytoscape
    showVizTab('cytoscape');
    loadKnowledgeGraph();
  }} else {{
    // Everything else → Mermaid
    showVizTab('mermaid');
    loadGraph(sel);
  }}
}}

function showVizTab(tab) {{
  document.querySelectorAll('#viz-mode .code-tab').forEach(function(el) {{
    el.classList.toggle('active', el.getAttribute('data-viz') === tab);
  }});
  document.getElementById('viz-mermaid-container').style.display = tab === 'mermaid' ? 'block' : 'none';
  document.getElementById('viz-cytoscape-container').style.display = tab === 'cytoscape' ? 'block' : 'none';
}}


function loadKnowledgeGraph() {{
  document.getElementById('viz-select').value = 'kg';
  var status = document.getElementById('viz-status');
  var title = document.getElementById('viz-title');
  title.textContent = 'Knowledge Graph';
  status.textContent = 'Loading...';
  Promise.all([
    fetch('/api/admin/viz/entangle').then(function(r){{return r.json();}}),
    fetch('/api/admin/viz/task').then(function(r){{return r.json();}}),
  ]).then(function(results) {{
    var elements = [];
    var seen = {{}};
    function addNode(id, label) {{
      if (!id || seen[id]) return;
      seen[id] = true;
      var color = id.startsWith('datum:') ? '#6366f1' : '#f59e0b';
      elements.push({{ data: {{ id: id, label: (label||id).slice(0,25), color: color }} }});
    }}
    function addEdge(src, dst, label) {{
      if (!src || !dst) return;
      addNode(src, src.split(':').pop());
      addNode(dst, dst.split(':').pop());
      elements.push({{ data: {{ source: src, target: dst, label: label||'' }} }});
    }}
    function parseLines(mmd, prefix) {{
      if (!mmd) return;
      mmd.split('\n').forEach(function(line) {{
        line = line.trim();
        if (!line || line.startsWith('graph ') || line.startsWith('flowchart ')) return;
        var nm = line.match(/^(\S+)\["([^"]*)"\]/);
        if (nm) {{ addNode(prefix + ':' + nm[1], nm[2].replace(/\\n/g, ' ')); return; }}
        var em = line.match(/^(\S+)\s*-->\s*(?:\|([^|]*)\|)?\s*(\S+)/);
        if (em) {{ addEdge(prefix + ':' + em[1], prefix + ':' + em[3], em[2]||''); }}
      }});
    }}
    parseLines(results[0].mermaid, 'datum');
    parseLines(results[1].mermaid, 'task');
    status.textContent = elements.length + ' elements';
    if (elements.length === 0) {{ status.textContent = 'No elements found'; return; }}
    setTimeout(function() {{
      var container = document.getElementById('cytoscape-target');
      if (!container) {{ status.textContent = 'Container not found'; return; }}
      if (typeof cytoscape === 'undefined') {{ status.textContent = 'Cytoscape.js not loaded'; return; }}
       var cy;
       try {{
         cy = cytoscape({{
          container: container,
          elements: elements,
          style: [
            {{ selector: 'node', style: {{ label: 'data(label)', 'background-color': 'data(color)', color: '#e2e8f0', 'font-size': '10px', 'text-valign': 'bottom', 'text-halign': 'center', width: 30, height: 30 }} }},
            {{ selector: 'edge', style: {{ 'line-color': '#475569', 'target-arrow-color': '#475569', 'target-arrow-shape': 'triangle', width: 1, 'curve-style': 'bezier', label: 'data(label)', color: '#64748b', 'font-size': '8px', 'text-margin-y': -8 }} }},
            {{ selector: ':selected', style: {{ 'border-color': '#fbbf24', 'border-width': 2 }} }},
          ],
         }});
         // Cassowary-inspired: hubs get more space via degree-scaled repulsion
         elements.nodes.forEach(function(n) {{
           var deg = (elements.edges || []).filter(function(e) {{ return e.data.source === n.data.id || e.data.target === n.data.id; }}).length;
           n.data.weight = 1 + Math.min(deg, 50) * 0.5;
         }});
         // Shift+scroll: 10x zoom
         container.addEventListener('wheel', function(e) {{
           if (e.shiftKey) {{ e.preventDefault(); var d = e.deltaY > 0 ? -0.5 : 0.5; cy.zoom(cy.zoom() * (1 + d * 10)); }}
         }}, {{ passive: false }});
         // Restore viewport from localStorage
         try {{
           var vp = JSON.parse(localStorage.getItem('b00t-cy-vp'));
           if (vp) cy.viewport({{ zoom: vp.zoom, pan: vp.pan }});
         }} catch(e) {{}}
         cy.on('viewport', function() {{
           try {{ localStorage.setItem('b00t-cy-vp', JSON.stringify({{ zoom: cy.zoom(), pan: cy.pan() }})); }} catch(e) {{}}
         }});
         // Orphan filter
         window._cy = cy;
         window._cyElements = elements;
         status.textContent = elements.length + ' elements — Cytoscape ready';
      }} catch(e) {{ status.textContent = 'Cytoscape error: ' + e.message; console.error('Cytoscape:', e); }}
    }}, 300);
  }}).catch(function(e){{ status.textContent = 'Error: ' + e.message; console.error(e); }});
}}

function toggleOrphans() {{
  var hide = document.getElementById('hide-orphans').checked;
  try {{ localStorage.setItem('b00t-hide-orphans', hide); }} catch(e) {{}}
  var cy = window._cy;
  if (cy) {{
    if (hide) {{
      cy.nodes().filter(function(n) {{ return n.degree() === 0; }}).style('display', 'none');
    }} else {{
      cy.nodes().style('display', 'element');
    }}
  }}
  var sel = document.getElementById('viz-select').value;
  if (sel === 'isometric') return loadGraph('isometric');
  renderMermaid();
}}

// Restore orphan filter on load
(function() {{
  try {{ if (localStorage.getItem('b00t-hide-orphans') === 'true') document.getElementById('hide-orphans').checked = true; }} catch(e) {{}}
}})();


function startProgress(total) {{
  progressStart = Date.now();
  var bar = document.getElementById('progress-bar');
  var fill = document.getElementById('progress-fill');
  bar.style.display = 'block';
  fill.className = 'progress-fill indeterminate';
  fill.style.width = '0%';
  addStatus('info', 'Starting (' + total + ' items)...');
}}
function updateProgress(current, total, label) {{
  var fill = document.getElementById('progress-fill');
  var pct = Math.round((current / total) * 100);
  fill.className = 'progress-fill';
  fill.style.width = pct + '%';
  var elapsed = ((Date.now() - progressStart) / 1000).toFixed(1);
  addStatus('info', '[' + elapsed + 's] ' + label + ' (' + current + '/' + total + ')');
}}
function finishProgress() {{
  var fill = document.getElementById('progress-fill');
  fill.style.width = '100%';
  var elapsed = ((Date.now() - progressStart) / 1000).toFixed(1);
  addStatus('done', 'Done in ' + elapsed + 's');
  setTimeout(function() {{
    document.getElementById('progress-bar').style.display = 'none';
  }}, 2000);
}}
function addStatus(type, msg) {{
  var log = document.getElementById('status-log');
  var ts = new Date().toLocaleTimeString();
  var entry = document.createElement('div');
  entry.className = 'status-entry ' + type + ' fade-in';
  entry.innerHTML = '<span class="ts">[' + ts + ']</span><span class="msg">' + msg + '</span>';
  log.appendChild(entry);
  log.scrollTop = log.scrollHeight;
}}

function renderMermaid() {{
  if (!currentVizData || !currentVizData.mermaid) {{ addStatus('error', 'No mermaid data'); return; }}
  var target = document.getElementById('mermaid-target');
  var raw = currentVizData.mermaid;
  if (!raw || !raw.trim()) {{ target.innerHTML = '<div style="color:#64748b;padding:20px;">No mermaid data</div>'; return; }}
  var stripped = raw.replace(/```mermaid\n?/g, '').replace(/```/g, '').trim();
  target.innerHTML = '<div style="display:flex;align-items:center;justify-content:center;padding:60px;color:#64748b;"><div class=\"wasm-spinner\" style=\"width:24px;height:24px;border:3px solid #1e293b;border-top:3px solid #38bdf8;border-radius:50%;margin-right:12px;\"></div><span style=\"font-size:12px;\">Rendering ' + stripped.length + ' chars...</span></div>';
  var hideOrphans = document.getElementById('hide-orphans')?.checked || false;
  fetch('/api/admin/viz/mermaid/render', {{
    method: 'POST',
    headers: {{'Content-Type': 'application/json'}},
    body: JSON.stringify({{text: stripped, hide_orphans: hideOrphans}})
  }}).then(function(r){{return r.json();}}).then(function(d) {{
    var svg = d.svg || '';
    svg = svg.replace(/width=\"[^\"]*\"/, '').replace(/height=\"[^\"]*\"/, '');
    target.innerHTML = '<div class=\"fade-in\" style=\"max-width:100%;overflow:auto;\">' + svg + '</div>';
    document.getElementById('viz-status').textContent = 'mermaid-rs-renderer · ' + (raw||'').length + ' chars';
  }}).catch(function(e) {{
    target.innerHTML = '<div style=\"color:#ef4444;padding:20px;\">Render error: ' + e.message + '</div>';
    addStatus('error', 'Mermaid render failed: ' + e.message);
  }});
}}

function loadGraph(sel) {{
  var status = document.getElementById('viz-status');
  var title = document.getElementById('viz-title');
  status.textContent = 'Loading...';
  if (sel === 'pipeline') {{
    fetch('/api/admin/processes').then(function(r){{return r.json();}}).then(function(d) {{
      currentVizData = {{ mermaid: d.mermaid }};
      title.textContent = 'Pipeline Flow';
      status.textContent = 'Pipeline (' + (d.mermaid||'').length + ' chars)';
      renderMermaid();
    }}).catch(function(e){{ status.textContent = 'Error: ' + e.message; console.error(e); }});
    return;
  }}
  if (sel === 'ato') {{
    fetch('/api/admin/processes').then(function(r){{return r.json();}}).then(function(d) {{
      var m = d.pipelines && d.pipelines['ato-legislation'] ? d.pipelines['ato-legislation'].mermaid : '';
      currentVizData = {{ mermaid: m }};
      title.textContent = 'ATO Pipeline';
      status.textContent = m ? 'ATO (' + m.length + ' chars)' : 'No ATO data';
      renderMermaid();
    }}).catch(function(e){{ status.textContent = 'Error: ' + e.message; console.error(e); }});
    return;
  }}
  if (sel === 'isometric') {{
    var target = document.getElementById('mermaid-target');
    target.style.position = 'relative';
    target.style.overflow = 'hidden';
    target.style.cursor = 'grab';
    target.style.minHeight = '400px';
    target.innerHTML = '<div style="color:#64748b;padding:40px;text-align:center;">Loading isometric view...</div>';
    window._isoViewStack = [];
    window._isoCurrentData = null;
    var hideOrphans = document.getElementById('hide-orphans')?.checked || false;
    var url = '/api/admin/viz/isometric' + (hideOrphans ? '?hide_orphans=true' : '');
    fetch(url).then(function(r){{return r.json();}}).then(function(d) {{
      window._isoCurrentData = d;
      d._roleLegend = ISO_ROLES;
      target.innerHTML = d.svg || '<div style="color:#64748b;padding:20px;">No data</div>';
      title.textContent = d.grouped ? 'Container View' : 'Isometric View';
      status.textContent = (d.grouped ? d.total_components + ' groups, ' : '') + (d.nodes||0) + ' nodes, ' + (d.edges||0) + ' edges · ' + (d.solver||'kasuari');
      setTimeout(function(){{
        attachIsoViewer(target);
        buildIsoLegend(target);
        if (d.grouped) buildContainerDrilldown(target, d);
      }}, 50);
    }}).catch(function(e){{ status.textContent = 'Error: ' + e.message; }});
    return;
  }}
  fetch('/api/admin/viz/' + sel).then(function(r){{return r.json();}}).then(function(d) {{
    currentVizData = d;
    title.textContent = sel.charAt(0).toUpperCase() + sel.slice(1) + ' Dependencies';
    status.textContent = (d.mermaid||'').length + ' chars';
    renderMermaid();
  }}).catch(function(e){{ status.textContent = 'Error: ' + e.message; console.error(e); }});
}}


// ════════ Interactive Isometric Viewer ════════

function attachIsoViewer(container) {{
  var svg = container.querySelector('svg');
  if (!svg) return;
  svg.style.width = '100%';
  svg.style.height = '100%';
  var viewBox = svg.getAttribute('viewBox') || '0 0 800 600';
  var vb = viewBox.split(' ').map(Number);
  var state = {{ x: 0, y: 0, scale: 1, minScale: 0.2, maxScale: 4 }};

  function updateView() {{
    var w = vb[2], h = vb[3];
    var ox = vb[0] + w/2, oy = vb[1] + h/2;
    var tx = ox + state.x/state.scale - ox/state.scale;
    var ty = oy + state.y/state.scale - oy/state.scale;
    svg.setAttribute('viewBox', [tx,ty,w/state.scale,h/state.scale].join(' '));
  }}

  // Pan drag
  var dragging = false, lx = 0, ly = 0;
  function pos(e) {{ return e.touches ? {{x:e.touches[0].clientX,y:e.touches[0].clientY}} : {{x:e.clientX,y:e.clientY}}; }}
  function onStart(e) {{ dragging=true; var p=pos(e); lx=p.x-state.x; ly=p.y-state.y; container.style.cursor='grabbing'; e.preventDefault(); }}
  function onMove(e) {{ if(!dragging)return; var p=pos(e); state.x=p.x-lx; state.y=p.y-ly; updateView(); }}
  function onEnd() {{ dragging=false; container.style.cursor='grab'; }}
  container.addEventListener('mousedown',onStart);
  window.addEventListener('mousemove',onMove);
  window.addEventListener('mouseup',onEnd);
  container.addEventListener('touchstart',onStart,{{passive:false}});
  window.addEventListener('touchmove',onMove,{{passive:false}});
  window.addEventListener('touchend',onEnd);

  // Zoom
  container.addEventListener('wheel',function(e) {{
    e.preventDefault();
    var delta = e.deltaY > 0 ? -0.15 : 0.15;
    if (e.shiftKey) delta *= 3;
    state.scale = Math.min(state.maxScale, Math.max(state.minScale, state.scale * (1 + delta)));
    updateView();
  }},{{passive:false}});

  // Node click → highlight connected
  var allEdges = Array.from(svg.querySelectorAll('[data-edge-from]'));
  function highlightNode(nodeEl) {{
    var nid = nodeEl.getAttribute('data-node-id');
    svg.querySelectorAll('.iso-node').forEach(function(n){{ n.style.opacity='0.25';n.style.filter='grayscale(1)'; }});
    allEdges.forEach(function(e){{ e.setAttribute('opacity','0.1'); }});
    if (nodeEl) {{
      nodeEl.style.opacity = '1'; nodeEl.style.filter = 'none';
      allEdges.forEach(function(e) {{
        if (e.getAttribute('data-edge-from')===nid || e.getAttribute('data-edge-to')===nid) {{
          e.setAttribute('opacity','0.9'); e.setAttribute('stroke-width','3');
        }}
      }});
    }}
  }}
  svg.addEventListener('click',function(e) {{
    var nodeEl = e.target.closest('.iso-node');
    highlightNode(nodeEl);
    if (!nodeEl) {{
      svg.querySelectorAll('.iso-node').forEach(function(n){{ n.style.opacity='1';n.style.filter='none'; }});
      allEdges.forEach(function(e){{ e.setAttribute('opacity','0.5');e.setAttribute('stroke-width','1.5'); }});
    }}
  }});

  // Double-click → center on node
  svg.addEventListener('dblclick',function(e) {{
    var nodeEl = e.target.closest('.iso-node');
    if (!nodeEl) {{ state.x=0;state.y=0;state.scale=1;updateView();return; }}
    var c = nodeEl.querySelector('text');
    if (!c) return;
    var bbox = c.getBBox();
    var cx = bbox.x + bbox.width/2, cy = bbox.y + bbox.height/2;
    state.x = -cx * state.scale + vb[2]/2;
    state.y = -cy * state.scale + vb[3]/2;
    state.scale = Math.min(state.maxScale, state.scale * 1.5);
    updateView();
  }});

  // Keyboard
  document.addEventListener('keydown',function(e) {{
    if (!container.closest('[data-b00t="panel:viz"]')) return;
    var step = 40 / state.scale;
    switch(e.key) {{
      case 'ArrowLeft': state.x+=step;break;
      case 'ArrowRight': state.x-=step;break;
      case 'ArrowUp': state.y+=step;break;
      case 'ArrowDown': state.y-=step;break;
      case '+':case'=': state.scale=Math.min(state.maxScale,state.scale*1.2);break;
      case '-': state.scale=Math.max(state.minScale,state.scale/1.2);break;
      case '0':case'Home': state.x=0;state.y=0;state.scale=1;break;
      case 'Escape': highlightNode(null);break;
      default: return;
    }}
    updateView();e.preventDefault();
  }});

  // Reset button
  var resetBtn = document.createElement('div');
  resetBtn.innerHTML = '↺';
  resetBtn.style.cssText = 'position:absolute;top:4px;right:4px;width:28px;height:28px;background:#334155;color:#e2e8f0;border-radius:4px;text-align:center;line-height:28px;cursor:pointer;font-size:16px;z-index:10;user-select:none;';
  resetBtn.title = 'Reset view (Home key)';
  resetBtn.onclick = function(){{ state.x=0;state.y=0;state.scale=1;updateView(); }};
  container.appendChild(resetBtn);
}}

// ════════ Role Legend ════════
var ISO_ROLES = [
  {{r:'data',e:'📄',c:'#334155'}},{{r:'intelligence',e:'🧠',c:'#0284c7'}},{{r:'rule',e:'⚖️',c:'#b91c1c'}},
  {{r:'security',e:'🛡️',c:'#0f766e'}},{{r:'human',e:'👤',c:'#b45309'}},{{r:'logic',e:'❓',c:'#b91c1c'}},
  {{r:'storage',e:'💾',c:'#15803d'}},{{r:'report',e:'📊',c:'#166534'}},{{r:'task',e:'⚙️',c:'#475569'}},
  {{r:'event',e:'📅',c:'#7e22ce'}},{{r:'ingest',e:'📥',c:'#1d4ed8'}},{{r:'validate',e:'✅',c:'#16a34a'}},
  {{r:'classify',e:'🏷️',c:'#7c3aed'}},{{r:'review',e:'👁️',c:'#c026d3'}},{{r:'reconcile',e:'🔄',c:'#2563eb'}},
  {{r:'commit',e:'💾',c:'#0891b2'}},{{r:'decision',e:'❓',c:'#dc2626'}},{{r:'step',e:'⚙️',c:'#52525b'}}
];

function buildIsoLegend(container) {{
  var used = new Set();
  container.querySelectorAll('[data-node-role]').forEach(function(n){{ used.add(n.getAttribute('data-node-role')); }});
  var html = '<div style="position:absolute;bottom:4px;left:4px;display:flex;flex-wrap:wrap;gap:3px;max-width:70%;z-index:10;padding:4px;border-radius:4px;">';
  ISO_ROLES.forEach(function(r) {{
    if (!used.has(r.r)) return;
    html += '<span style="background:'+r.c+';color:#fff;padding:2px 6px;border-radius:3px;font-size:10px;cursor:pointer;opacity:0.85;" title="'+r.r+'" onclick="var c=this.parentElement.parentElement;c.querySelectorAll(\'.iso-node\').forEach(function(n){{n.style.opacity=n.getAttribute(\'data-node-role\')===\''+r.r+'\'?\'1\':\'0.15\';n.style.filter=n.getAttribute(\'data-node-role\')===\''+r.r+'\'?\'none\':\'grayscale(1)\';}});c.querySelectorAll(\'[data-edge-from]\').forEach(function(e){{e.setAttribute(\'opacity\',\'0.1\');}});">'+r.e+' '+r.r+'</span>';
  }});
  html += '</div>';
  var leg = document.createElement('div'); leg.innerHTML = html;
  container.appendChild(leg);
}}

// ════════ Container drill-down (branch-and-bound sub-graphs) ════════

function buildContainerDrilldown(container, data) {{
  if (!data.components || !data.components.length) return;
  var subMap = {{}};
  data.components.forEach(function(c) {{ subMap[c.id] = c; }});

  container.querySelectorAll('.iso-node').forEach(function(nodeEl) {{
    var nid = nodeEl.getAttribute('data-node-id');
    if (!nid || !nid.startsWith('__container_')) return;
    nodeEl.style.cursor = 'pointer';
    nodeEl.title = 'Double-click to drill down';
    nodeEl.addEventListener('dblclick', function(e) {{
      e.stopPropagation();
      var sub = subMap[nid];
      if (!sub || !sub.svg) return;
      window._isoViewStack.push({{
        svg: container.querySelector('svg').outerHTML,
        svgEl: container.querySelector('svg'),
        legend: container.querySelector('[style*=\"bottom:4px;left:4px\"]')?.outerHTML || ''
      }});
      container.innerHTML = sub.svg;
      container.querySelector('svg').style.width = '100%';
      container.querySelector('svg').style.height = '100%';
      var back = document.createElement('div');
      back.innerHTML = '← Back';
      back.style.cssText = 'position:absolute;top:4px;left:4px;background:#1e293b;color:#e2e8f0;padding:4px 10px;border-radius:4px;font-size:12px;cursor:pointer;z-index:11;';
      back.title = 'Back to container view';
      back.onclick = function() {{
        var prev = window._isoViewStack.pop();
        if (prev) {{
          container.innerHTML = '';
          var wrapper = document.createElement('div');
          wrapper.innerHTML = prev.svg;
          container.appendChild(wrapper.firstChild);
          if (prev.legend) {{ var lw = document.createElement('div'); lw.innerHTML = prev.legend; container.appendChild(lw.firstChild); }}
        }}
        attachIsoViewer(container);
        buildIsoLegend(container);
      }};
      container.appendChild(back);
      buildIsoLegend(container);
      attachIsoViewer(container);
    }});
  }});
}}

</script>

<div id="autopilot-badge">
  <div class="autopilot-dot"></div>
  <span>Autopilot</span>
</div>

</body></html>"#,
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
        // WASM SPA (next-gen)
        .route("/wasm", get(wasm_handler))
        .route("/wasm/{*path}", get(wasm_handler))
        .nest_service("/wasm/wasm", ServeDir::new(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent().unwrap_or(std::path::Path::new("."))
                .join("b00t-ui-wasm/dist/wasm")
        ))
        // Health check
        .route("/health", get(health_handler))
        .route("/healthz", get(health_handler))
        // API — pipeline state
        .route("/api/admin/pipeline", get(pipeline_handler))
        // API — type introspection
        .route("/api/admin/display", get(datum_display_handler))
        .route("/api/admin/datums", get(datum_health_handler))
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
        .route("/api/admin/viz/isometric", get(viz_isometric_handler))
        .route("/api/admin/viz/isometric/demo", get(viz_isometric_demo_handler))
        .route("/api/admin/viz/mermaid/render", get(viz_mermaid_render_handler).post(viz_mermaid_render_handler))
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

#[cfg(test)]
mod html_sanity_tests {
    use super::*;

    fn test_html() -> String {
        dashboard_html(r#"{"has_pipeline":false}"#, r#"[]"#)
    }

    #[test]
    fn no_merge_conflicts() {
        let h = test_html();
        assert!(!h.contains("<<<<<<<"));
        assert!(!h.contains(">>>>>>>"));
    }

    #[test]
    fn no_unconverted_braces() {
        let h = test_html();
        assert!(!h.contains("{{"));
        assert!(!h.contains("}}"));
    }

    #[test]
    fn has_required_cdns() {
        let h = test_html();
        assert!(h.contains("cytoscape"));
        assert!(h.contains("cytoscape.min.js"));
    }

    #[test]
    fn valid_html_structure() {
        let h = test_html();
        assert!(h.contains("<!DOCTYPE html>"));
        assert!(h.contains("</html>"));
        assert!(h.contains("</body>"));
    }

    #[test]
    fn required_elements_exist() {
        let h = test_html();
        // Sidebar sections
        for id in &["section-pipeline", "section-types", "section-sim", "section-viz"] {
            assert!(h.contains(id), "Missing sidebar section: {id}");
        }
        // Viz dropdown
        assert!(h.contains("viz-select"), "Missing viz dropdown");
        for opt in &["entangle", "task", "pipeline", "ato", "isometric", "kg"] {
            assert!(h.contains(&format!("\"{opt}\"")), "Missing viz option: {opt}");
        }
        // JS functions
        for fn_name in &["toggleSection", "renderMermaid", "loadKnowledgeGraph", "beat", "onVizSelect"] {
            assert!(h.contains(&format!("function {fn_name}")), "Missing JS function: {fn_name}");
        }
        // Panels
        for panel in &["pipeline-panel", "type-panel", "sim-panel", "viz-panel"] {
            assert!(h.contains(panel), "Missing panel: {panel}");
        }
    }
}
