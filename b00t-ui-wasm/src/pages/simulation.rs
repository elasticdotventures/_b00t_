//! Digital Twin Simulation — display simulation state and tick/rollback controls.
//!
//! Auto-refreshes state every 5 seconds.  Tick and Rollback buttons invoke
//! the corresponding API endpoints and update the display immediately.

use crate::api;
use crate::sleep::sleep;
use dioxus::prelude::*;
use serde_json::Value;
use std::time::Duration;

/// Simulation page component.
pub fn Simulation() -> Element {
    let mut state = use_signal(|| Value::Null);
    let mut loading = use_signal(|| true);
    let tick_in_flight = use_signal(|| false);

    // ── Auto-refresh state every 5 s ───────────────────────────────────
    use_effect(move || {
        spawn(async move {
            loop {
                if let Ok(val) = api::sim_state().await {
                    state.set(val);
                    loading.set(false);
                } else {
                    loading.set(false);
                }
                sleep(Duration::from_secs(5)).await;
            }
        });
    });

    // ── Handlers ───────────────────────────────────────────────────────
    let on_tick = move |_| {
        let mut state = state.clone();
        let mut in_flight = tick_in_flight.clone();
        spawn(async move {
            in_flight.set(true);
            if let Ok(val) = api::sim_tick().await {
                state.set(val);
            }
            in_flight.set(false);
        });
    };

    let on_rollback = move |_| {
        let mut state = state.clone();
        spawn(async move {
            if let Ok(val) = api::sim_rollback().await {
                state.set(val);
            }
        });
    };

    // ── Render ─────────────────────────────────────────────────────────
    let body: Element = if loading() {
        rsx! {
            div { style: "text-align: center; padding: 48px; color: #64748b;", "Loading simulation…" }
        }
    } else if state().is_null() {
        rsx! {
            div { style: "text-align: center; padding: 48px; color: #ef4444;", "No simulation data available." }
        }
    } else {
        let s = state.read();
        let name = s.get("name").and_then(Value::as_str).unwrap_or("—");
        let tick = s.get("tick").and_then(Value::as_u64).unwrap_or(0);
        let history_len = s
            .get("history")
            .and_then(Value::as_array)
            .map(|a| a.len())
            .unwrap_or(0);
        let subscribers = s
            .get("subscribers")
            .and_then(Value::as_array)
            .map(|a| a.len())
            .unwrap_or(0);
        let opacity = if tick_in_flight() { "0.6" } else { "1" };

        rsx! {
            div { style: "display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-bottom: 24px;",
                StatCard { label: "Simulation", value: name.to_string() }
                StatCard { label: "Current Tick", value: tick.to_string() }
                StatCard { label: "History Entries", value: history_len.to_string() }
                StatCard { label: "Subscribers", value: subscribers.to_string() }
            }
            div { style: "display: flex; gap: 12px; margin-bottom: 24px;",
                button {
                    disabled: tick_in_flight(),
                    onclick: on_tick,
                    style: "padding: 10px 24px; background: #0ea5e9; color: #fff; border: none; border-radius: 8px; font-size: 14px; font-weight: 600; cursor: pointer; opacity: {opacity};",
                    if tick_in_flight() { "Ticking…" } else { "Tick ⟳" }
                }
                button {
                    style: "padding: 10px 24px; background: transparent; color: #e2e8f0; border: 1px solid #334155; border-radius: 8px; font-size: 14px; cursor: pointer;",
                    onclick: on_rollback,
                    "Rollback ↩"
                }
            }
            div { style: "background: #0f172a; border: 1px solid #1e293b; border-radius: 12px; padding: 16px;",
                h2 { style: "font-size: 13px; font-weight: 600; color: #94a3b8; text-transform: uppercase; letter-spacing: 0.05em; margin-bottom: 8px;",
                    "Full State"
                }
                pre { style: "font-family: monospace; font-size: 12px; line-height: 1.6; color: #94a3b8; max-height: 400px; overflow-y: auto;",
                    "{serde_json::to_string_pretty(&*s).unwrap_or_default()}"
                }
            }
        }
    };

    rsx! {
        div { style: "max-width: 900px;",
            h1 { style: "font-size: 24px; font-weight: 600; margin-bottom: 24px; color: #f1f5f9;",
                "Digital Twin Simulation"
            }
            {body}
        }
    }
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

#[component]
fn StatCard(label: String, value: String) -> Element {
    rsx! {
        div { style: "background: #0f172a; border: 1px solid #1e293b; border-radius: 12px; padding: 20px;",
            div { style: "font-size: 13px; color: #94a3b8; margin-bottom: 4px; text-transform: uppercase; letter-spacing: 0.03em;",
                "{label}"
            }
            div { style: "font-size: 28px; font-weight: 700; color: #38bdf8; font-family: monospace;",
                "{value}"
            }
        }
    }
}
