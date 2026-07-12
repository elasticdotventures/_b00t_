//! Pipeline Dashboard — 2×2 stat cards + pipeline metadata.
//!
//! Auto-refreshes every 5 seconds via a background `spawn` loop.

use crate::api;
use crate::sleep::sleep;
use dioxus::prelude::*;
use serde_json::Value;
use std::time::Duration;

/// Pipeline dashboard page component.
pub fn Pipeline() -> Element {
    let mut data = use_signal(|| Value::Null);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| String::new());

    // ── Auto-refresh: fetch on mount, then poll every 5 s ──────────────
    use_effect(move || {
        spawn(async move {
            loop {
                match api::get_pipeline().await {
                    Ok(val) => {
                        data.set(val);
                        loading.set(false);
                        error.set(String::new());
                    }
                    Err(e) => {
                        loading.set(false);
                        error.set(e);
                    }
                }
                sleep(Duration::from_secs(5)).await;
            }
        });
    });

    // ── Render ─────────────────────────────────────────────────────────
    let page_body: Element = if loading() {
        rsx! {
            div { style: "text-align: center; padding: 48px; color: #64748b; font-size: 18px;",
                "Loading pipeline data…"
            }
        }
    } else if !error().is_empty() {
        rsx! {
            div { style: "text-align: center; padding: 48px; color: #ef4444; font-size: 16px;",
                "⚠ {error}"
            }
        }
    } else {
        let v = data.read();
        let chunks      = v.get("chunks").and_then(Value::as_u64).unwrap_or(0);
        let evidence    = v.get("evidence").and_then(Value::as_u64).unwrap_or(0);
        let requirements = v.get("requirements").and_then(Value::as_u64).unwrap_or(0);
        let fol         = v.get("fol_count").and_then(Value::as_u64).unwrap_or(0);
        let version     = v.get("version").and_then(Value::as_str).unwrap_or("—");
        let source_id   = v.get("source_id").and_then(Value::as_str).unwrap_or("—");
        let last_exec   = v.get("last_execution").and_then(Value::as_str).unwrap_or("—");

        rsx! {
            div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-bottom: 24px;",
                StatCard { label: "Chunks", value: chunks }
                StatCard { label: "Evidence", value: evidence }
                StatCard { label: "Requirements", value: requirements }
                StatCard { label: "FOL Count", value: fol }
            }
            div { style: "background: #0f172a; border: 1px solid #1e293b; border-radius: 12px; padding: 20px;",
                h2 { style: "font-size: 13px; font-weight: 600; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 12px;",
                    "Pipeline Info"
                }
                InfoRow { label: "Version", value: version.to_string() }
                InfoRow { label: "Source ID", value: source_id.to_string() }
                InfoRow { label: "Last Execution", value: last_exec.to_string() }
            }
            div { style: "margin-top: 16px; font-size: 12px; color: #475569; text-align: right;",
                "auto-refreshing every 5s"
            }
        }
    };

    rsx! {
        div { style: "max-width: 900px;",
            h1 { style: "font-size: 24px; font-weight: 600; margin-bottom: 24px; color: #f1f5f9;",
                "Pipeline Dashboard"
            }
            {page_body}
        }
    }
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

/// A single stat card showing a labelled numeric value.
#[component]
fn StatCard(label: String, value: u64) -> Element {
    rsx! {
        div { style: "background: #0f172a; border: 1px solid #1e293b; border-radius: 12px; padding: 20px;",
            div { style: "font-size: 13px; color: #94a3b8; margin-bottom: 4px; text-transform: uppercase; letter-spacing: 0.03em;",
                "{label}"
            }
            div { style: "font-size: 28px; font-weight: 700; color: #38bdf8;",
                "{value}"
            }
        }
    }
}

/// A labelled info row (key / value) with bottom border separator.
#[component]
fn InfoRow(label: String, value: String) -> Element {
    rsx! {
        div { style: "display: flex; justify-content: space-between; padding: 8px 0; border-bottom: 1px solid #1e293b;",
            span { style: "color: #94a3b8;", "{label}" }
            span { style: "color: #e2e8f0; font-weight: 500; font-family: monospace;", "{value}" }
        }
    }
}
