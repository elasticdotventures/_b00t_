//! Left sidebar navigation — 200 px, #0f172a background.
//!
//! Features collapsible accordion sections and active-route highlighting.

use dioxus::prelude::*;
use dioxus_router::prelude::{use_navigator, use_route};
use std::collections::HashSet;

use crate::Route;

// ---------------------------------------------------------------------------
// Navigation data
// ---------------------------------------------------------------------------

#[derive(PartialEq)]
struct NavSectionData {
    title: &'static str,
    items: &'static [NavItemData],
}

#[derive(PartialEq)]
struct NavItemData {
    label: &'static str,
    route: Route,
}

const SECTIONS: &[NavSectionData] = &[
    NavSectionData {
        title: "Data",
        items: &[
            NavItemData {
                label: "Pipeline",
                route: Route::Pipeline {},
            },
            NavItemData {
                label: "Types",
                route: Route::Types {},
            },
        ],
    },
    NavSectionData {
        title: "Simulation",
        items: &[NavItemData {
            label: "Simulation",
            route: Route::Simulation {},
        }],
    },
    NavSectionData {
        title: "Visualizations",
        items: &[NavItemData {
            label: "Visualizations",
            route: Route::Visualizations {},
        }],
    },
];

// ---------------------------------------------------------------------------
// Sidebar root component
// ---------------------------------------------------------------------------

/// Sidebar root component.
pub fn Sidebar() -> Element {
    // All sections expanded by default
    let expanded = use_signal(|| {
        let mut m = HashSet::new();
        m.insert("Data");
        m.insert("Simulation");
        m.insert("Visualizations");
        m
    });

    rsx! {
        aside {
            style: "width: 200px; min-width: 200px; background: #0f172a; border-right: 1px solid #1e293b; display: flex; flex-direction: column; height: 100vh; position: fixed; left: 0; top: 0; overflow-y: auto;",

            // ── Brand header ────────────────────────────────────────────
            div { style: "padding: 20px 16px 16px; border-bottom: 1px solid #1e293b;",
                div { style: "font-size: 16px; font-weight: 700; color: #f1f5f9; letter-spacing: -0.02em;",
                    "b00t Admin"
                }
                div { style: "font-size: 11px; color: #475569; margin-top: 2px;",
                    "dashboard v0.1"
                }
            }

            // ── Nav sections ───────────────────────────────────────────
            nav { style: "flex: 1; padding: 8px;",
                for section in SECTIONS {
                    NavSection {
                        key: "{section.title}",
                        section_data: section,
                        expanded: expanded,
                    }
                }
            }

            // ── Footer ─────────────────────────────────────────────────
            div { style: "padding: 12px 16px; border-top: 1px solid #1e293b; font-size: 11px; color: #475569;",
                span { "@PromptExecution" }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// NavSection sub-component
// ---------------------------------------------------------------------------

/// A single collapsible nav section with header + items.
#[component]
fn NavSection(
    section_data: &'static NavSectionData,
    expanded: Signal<HashSet<&'static str>>,
) -> Element {
    let title = section_data.title;
    let is_expanded = expanded().contains(title);

    let toggle = move |_| {
        let mut e = expanded();
        if e.contains(title) {
            e.remove(title);
        } else {
            e.insert(title);
        }
        expanded.set(e);
    };

    rsx! {
        // Section header (clickable toggle)
        div {
            key: "{title}-header",
            style: "display: flex; align-items: center; justify-content: space-between; padding: 10px 8px 6px; cursor: pointer; user-select: none;",
            onclick: toggle,
            span { style: "font-size: 11px; font-weight: 600; color: #64748b; text-transform: uppercase; letter-spacing: 0.08em;",
                "{title}"
            }
            span { style: "font-size: 10px; color: #475569; transition: transform 0.15s;",
                if is_expanded { "▼" } else { "▶" }
            }
        }

        // Section items (visible when expanded)
        if is_expanded {
            for item in section_data.items {
                NavItem {
                    key: "{item.label}",
                    item_data: item,
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// NavItem sub-component
// ---------------------------------------------------------------------------

/// A single navigation link.  Uses `use_route`/`use_navigator` for
/// active-state highlighting and programmatic navigation.
#[component]
fn NavItem(item_data: &'static NavItemData) -> Element {
    let navigator = use_navigator();
    let current_route = use_route::<Route>();
    let is_active = current_route == item_data.route;
    let label = item_data.label;
    let route = item_data.route.clone();

    let item_style = if is_active {
        "display: flex; align-items: center; gap: 8px; padding: 8px 12px; margin: 2px 0; border-radius: 8px; cursor: pointer; background: #1e293b; color: #38bdf8; font-size: 13px; font-weight: 500; transition: background 0.15s;"
    } else {
        "display: flex; align-items: center; gap: 8px; padding: 8px 12px; margin: 2px 0; border-radius: 8px; cursor: pointer; color: #94a3b8; font-size: 13px; transition: background 0.15s;"
    };
    let dot_color = if is_active { "#38bdf8" } else { "transparent" };

    let onclick = move |_| {
        let r = route.clone();
        navigator.push(r);
    };

    rsx! {
        div {
            key: "{label}",
            style: "{item_style}",
            onclick: onclick,
            span { style: "width: 6px; height: 6px; border-radius: 50%; background: {dot_color}; flex-shrink: 0;" }
            span { "{label}" }
        }
    }
}
