//! Type Explorer — browse type names and inspect their JSON definitions.
//!
//! Left panel: alphabetically sorted list of type names.
//! Right panel: JSON detail view for the selected type.

use crate::api;
use dioxus::prelude::*;
use serde_json::Value;

/// Type explorer page component.
pub fn Types() -> Element {
    let mut types_list = use_signal(|| Vec::<String>::new());
    let mut selected = use_signal(|| None::<String>);
    let detail = use_signal(|| Value::Null);
    let mut loading = use_signal(|| true);

    // ── Fetch type list on mount ───────────────────────────────────────
    use_effect(move || {
        spawn(async move {
            if let Ok(val) = api::get_types().await {
                let names = val
                    .as_array()
                    .map(|arr| {
                        let mut v: Vec<String> = arr
                            .iter()
                            .filter_map(|e| e.as_str().map(String::from))
                            .collect();
                        v.sort();
                        v
                    })
                    .unwrap_or_default();
                types_list.set(names);
            }
            loading.set(false);
        });
    });

    // ── Fetch detail whenever `selected` changes ───────────────────────
    // `use_effect` re-runs when any signal it reads during execution is
    // written to.  Reading `selected()` subscribes to it.
    use_effect(move || {
        let mut detail = detail;
        let name = selected();
        if let Some(name_str) = name {
            spawn(async move {
                if let Ok(val) = api::get_type_detail(&name_str).await {
                    detail.set(val);
                }
            });
        } else {
            detail.set(Value::Null);
        }
    });

    // ── Render ─────────────────────────────────────────────────────────
    let left_panel = if loading() {
        rsx! { div { style: "color: #64748b; padding: 16px;", "Loading types…" } }
    } else if types_list().is_empty() {
        rsx! { div { style: "color: #64748b; padding: 16px;", "No types found." } }
    } else {
        let sel = selected();
        rsx! {
            div { style: "display: flex; flex-direction: column; gap: 2px;",
                for name in types_list.iter() {
                    div {
                        key: "{name}",
                        style: if sel.as_deref() == Some(name.as_str()) {
                            "padding: 8px 12px; cursor: pointer; border-radius: 6px; background: #1e293b; color: #38bdf8; font-family: monospace; font-size: 13px; transition: background 0.15s;"
                        } else {
                            "padding: 8px 12px; cursor: pointer; border-radius: 6px; color: #cbd5e1; font-family: monospace; font-size: 13px; transition: background 0.15s;"
                        },
                        onclick: {
                            let n = name.clone();
                            move |_| selected.set(Some(n.clone()))
                        },
                        onmouseenter: move |_| {},
                        "{name}"
                    }
                }
            }
        }
    };

    let right_panel = if selected().is_none() {
        rsx! {
            div { style: "color: #475569; padding: 24px; text-align: center;",
                "Select a type to inspect its definition"
            }
        }
    } else if detail().is_null() {
        rsx! {
            div { style: "color: #64748b; padding: 24px;",
                "Loading detail…"
            }
        }
    } else {
        // Read the signal and serialize the inner Value
        let detail_ref = detail.read();
        let pretty = serde_json::to_string_pretty(&*detail_ref)
            .unwrap_or_else(|_| "(serialisation error)".into());
        rsx! {
            pre { style: "font-family: monospace; font-size: 12px; line-height: 1.6; color: #e2e8f0; white-space: pre-wrap; word-break: break-all;",
                "{pretty}"
            }
        }
    };

    rsx! {
        div { style: "max-width: 1100px;",
            h1 { style: "font-size: 24px; font-weight: 600; margin-bottom: 24px; color: #f1f5f9;",
                "Type Explorer"
            }
            div { style: "display: grid; grid-template-columns: 280px 1fr; gap: 16px;",
                div { style: "background: #0f172a; border: 1px solid #1e293b; border-radius: 12px; padding: 12px; max-height: 75vh; overflow-y: auto;",
                    {left_panel}
                }
                // Right — type detail
                div { style: "background: #0f172a; border: 1px solid #1e293b; border-radius: 12px; padding: 20px; max-height: 75vh; overflow-y: auto;",
                    {right_panel}
                }
            }
        }
    }
}
