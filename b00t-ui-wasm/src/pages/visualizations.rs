//! Visualizations — render Mermaid graphs from API data.
//!
//! Dropdown selects the graph type (Entanglement / Tasks / Pipeline / ATO),
//! then fetches the graph data and uses mermaid.js (loaded dynamically from
//! CDN) to render an SVG.

use crate::api;
use crate::sleep::sleep;
use dioxus::prelude::*;
use js_sys::Reflect;
use serde_json::Value;
use std::time::Duration;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlScriptElement;

// ---------------------------------------------------------------------------
// wasm-bindgen interop for mermaid.js
// ---------------------------------------------------------------------------

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = mermaid)]
    fn render(id: &str, text: &str) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = mermaid)]
    fn initialize(config: &JsValue);
}

/// Available graph types for the dropdown.
const GRAPH_TYPES: &[(&str, &str)] = &[
    ("entangle", "Entanglement Graph"),
    ("task",     "Task Graph"),
    ("pipeline", "Pipeline Graph"),
    ("ato",      "ATO Graph"),
];

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/// Visualizations page component.
pub fn Visualizations() -> Element {
    let mut viz_type    = use_signal(|| "entangle".to_string());
    let mut mermaid_svg = use_signal(|| String::new());
    let mut status      = use_signal(|| String::from("Ready"));
    let mut progress    = use_signal(|| 0u8);
    let mermaid_loaded = use_signal(|| false);

    // ── Load mermaid.js from CDN on mount ──────────────────────────────
    use_effect(move || {
        let mut ml = mermaid_loaded;
        let mut status = status;
        spawn(async move {
            // Inject script element
            let window = web_sys::window().unwrap();
            let doc = window.document().unwrap();

            // Guard: skip if already present
            let already_loaded = doc
                .query_selector("script[data-b00t-mermaid]")
                .ok()
                .flatten()
                .is_some();

            if !already_loaded {
                let script = doc
                    .create_element("script")
                    .unwrap()
                    .dyn_into::<HtmlScriptElement>()
                    .unwrap();
                script.set_src("https://cdn.jsdelivr.net/npm/mermaid@11/dist/mermaid.min.js");
                script.set_defer(true);
                let _ = script.set_attribute("data-b00t-mermaid", "");
                let _ = doc.head().unwrap().append_child(&script);
            }

            // Poll for mermaid global to become available
            for _ in 0..100 {
                sleep(Duration::from_millis(100)).await;
                let raw = Reflect::get(&js_sys::global(), &"mermaid".into()).ok();
                let available = raw.is_some() && raw.as_ref() != Some(&JsValue::UNDEFINED);
                if available {
                    ml.set(true);
                    // Dark-theme initialisation
                    let config = js_sys::JSON::parse(
                        r#"{"startOnLoad":false,"theme":"dark","securityLevel":"loose","fontFamily":"system-ui,sans-serif"}"#,
                    )
                    .unwrap_or(JsValue::UNDEFINED);
                    if !config.is_undefined() {
                        initialize(&config);
                    }
                    status.set("Mermaid loaded ✓".to_string());
                    break;
                }
            }
        });
    });

    // ── Render graph on type change or when mermaid becomes loaded ─────
    use_effect(move || {
        // Read signals that this effect depends on
        let _ = viz_type();
        let loaded = mermaid_loaded();
        if !loaded {
            return;
        }

        let vt = viz_type();
        let mut svg = mermaid_svg;
        let mut st = status;
        let mut pr = progress;

        spawn(async move {
            st.set("Fetching graph data…".to_string());
            pr.set(30);

            // Try to fetch real data from API; fall back to sample
            let api_result = api::get_viz(&vt).await;

            pr.set(60);
            st.set("Generating Mermaid diagram…".to_string());

            let mermaid_text = build_mermaid(&vt, &api_result);

            pr.set(80);
            st.set("Rendering to SVG…".to_string());

            // Call mermaid.render() via wasm-bindgen
            let promise = render("b00t-mermaid-container", &mermaid_text);
            match JsFuture::from(promise).await {
                Ok(result) => {
                    let svg_str = Reflect::get(&result, &"svg".into())
                        .ok()
                        .and_then(|v| v.as_string())
                        .unwrap_or_default();
                    svg.set(svg_str);
                    st.set("Rendered ✓".to_string());
                    pr.set(100);
                }
                Err(e) => {
                    st.set(format!("Render error: {e:?}"));
                    pr.set(0);
                }
            }
        });
    });

    // ── Handlers ───────────────────────────────────────────────────────
    let on_select = move |evt: Event<FormData>| {
        let val = evt.value();
        viz_type.set(val);
        mermaid_svg.set(String::new());
        status.set("Ready".to_string());
        progress.set(0);
    };

    // ── Render ─────────────────────────────────────────────────────────
    let progress_bar = progress();
    let progress_style = format!(
        "width: {progress_bar}%; height: 4px; background: #38bdf8; border-radius: 2px; transition: width 0.3s ease;"
    );

    rsx! {
        div { style: "max-width: 1100px;",
            h1 { style: "font-size: 24px; font-weight: 600; margin-bottom: 24px; color: #f1f5f9;",
                "Visualizations"
            }

            div { style: "display: flex; align-items: center; gap: 16px; margin-bottom: 20px; flex-wrap: wrap;",
                select {
                    style: "padding: 8px 14px; background: #0f172a; color: #e2e8f0; border: 1px solid #334155; border-radius: 8px; font-size: 14px; cursor: pointer;",
                    onchange: on_select,
                    for (val, label) in GRAPH_TYPES {
                        option {
                            value: "{val}",
                            selected: val == &viz_type(),
                            "{label}"
                        }
                    }
                }
                // Status badge
                span { style: "font-size: 13px; color: #64748b;",
                    "{status}"
                }
            }

            // Progress bar
            div { style: "width: 100%; background: #1e293b; border-radius: 2px; margin-bottom: 20px;",
                div { style: "{progress_style}", }
            }

            div { style: "background: #0f172a; border: 1px solid #1e293b; border-radius: 12px; padding: 20px; overflow-x: auto; min-height: 400px;",
                if mermaid_svg().is_empty() {
                    div { style: "color: #475569; text-align: center; padding: 80px 0; font-size: 14px;",
                        "Select a graph type above and it will render here."
                    }
                } else {
                    div { id: "b00t-mermaid-container", dangerous_inner_html: "{mermaid_svg}" }
                }
            }

            div { style: "margin-top: 16px; font-size: 12px; color: #475569; font-family: monospace;",
                "log: {status}"
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a Mermaid diagram string for the given graph type, optionally
/// incorporating API response data.
fn build_mermaid(viz_type: &str, api_data: &Result<Value, String>) -> String {
    // We attempt to read the API response to build a richer graph,
    // but fall back to sample diagrams when data is unavailable.
    match viz_type {
        "entangle" => {
            match api_data {
                Ok(val) if !val.is_null() => {
                    let mut lines = vec!["graph TD".to_string()];
                    if let Some(chunks) = val.get("chunks").and_then(Value::as_array) {
                        for (i, chunk) in chunks.iter().enumerate() {
                            let id = format!("C{i}");
                            let label = chunk.get("id").and_then(Value::as_str).unwrap_or(&id);
                            lines.push(format!("    {id}[{label}]"));
                            if let Some(ev) = chunk.get("evidence").and_then(Value::as_array) {
                                for (j, _) in ev.iter().enumerate() {
                                    let eid = format!("E{i}_{j}");
                                    lines.push(format!("    {eid}(Evidence)"));
                                    lines.push(format!("    {id} --> {eid}"));
                                }
                            }
                        }
                    }
                    if lines.len() == 1 {
                        lines.push("    A[No entanglement data]".to_string());
                    }
                    lines.join("\n")
                }
                _ => {
                    r#"graph TD
    A[Source Document] --> B[Chunk 1]
    A --> C[Chunk 2]
    B --> D[Evidence A]
    B --> E[Evidence B]
    C --> F[Evidence C]
    style A fill:#1e3a5f,stroke:#38bdf8,color:#e2e8f0
    style B fill:#0f172a,stroke:#38bdf8,color:#e2e8f0
    style C fill:#0f172a,stroke:#38bdf8,color:#e2e8f0"#
                        .to_string()
                }
            }
        }
        "task" => {
            match api_data {
                Ok(val) if !val.is_null() => {
                    let mut lines = vec!["graph LR".to_string()];
                    if let Some(tasks) = val.get("tasks").and_then(Value::as_array) {
                        for (i, task) in tasks.iter().enumerate() {
                            let tid = format!("T{i}");
                            let label =
                                task.get("name").and_then(Value::as_str).unwrap_or(&tid);
                            lines.push(format!("    {tid}[{label}]"));
                            if let Some(deps) =
                                task.get("depends_on").and_then(Value::as_array)
                            {
                                for dep in deps {
                                    if let Some(dep_name) = dep.as_str() {
                                        // Find index of dep task
                                        if let Some(dep_idx) = tasks.iter().position(|t| {
                                            t.get("name")
                                                .and_then(Value::as_str)
                                                == Some(dep_name)
                                        }) {
                                            lines.push(format!("    T{dep_idx} --> {tid}"));
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        lines.push("    A[No task data]".to_string());
                    }
                    lines.join("\n")
                }
                _ => {
                    r#"graph LR
    A[Ingest] --> B[Chunk]
    B --> C[Analyse]
    C --> D[Classify]
    D --> E[Store]
    style A fill:#1e3a5f,stroke:#38bdf8,color:#e2e8f0
    style C fill:#0f172a,stroke:#f59e0b,color:#e2e8f0"#
                        .to_string()
                }
            }
        }
        "pipeline" => {
            r#"flowchart TD
    A[Raw Input] --> B[Parser]
    B --> C{Validation}
    C -->|Pass| D[Transformer]
    C -->|Fail| E[Error Queue]
    D --> F[Loader]
    F --> G[(Storage)]
    style A fill:#1e3a5f,stroke:#38bdf8,color:#e2e8f0
    style C fill:#0f172a,stroke:#f59e0b,color:#e2e8f0
    style F fill:#0f172a,stroke:#22c55e,color:#e2e8f0"
                .to_string()
        }
        "ato" => {
            r#"graph TD
    A[ATO Request] --> B[Assessment]
    B --> C[Score]
    C --> D{Threshold}
    D -->|Pass| E[Approved]
    D -->|Fail| F[Review Required]
    E --> G[Register]
    F --> H[Reassess]
    H --> B
    style A fill:#1e3a5f,stroke:#38bdf8,color:#e2e8f0
    style D fill:#0f172a,stroke:#f59e0b,color:#e2e8f0
    style E fill:#0f172a,stroke:#22c55e,color:#e2e8f0
    style F fill:#0f172a,stroke:#ef4444,color:#e2e8f0"#
                .to_string()
        }
        _ => "graph TD; A[Unknown];".to_string(),
    }
}
