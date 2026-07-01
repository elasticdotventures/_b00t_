//! b00t Admin Dashboard — Dioxus WASM SPA
//!
//! Routes: /wasm (Pipeline), /wasm/types (Types), /wasm/sim (Simulation), /wasm/viz (Visualizations)
//! Dark theme: #020617 bg, #e2e8f0 text, #38bdf8 accent.
//!
//! Component functions follow Dioxus convention (PascalCase) for use in
//! the `rsx!` macro; suppress the otherwise-correct `non_snake_case` lint.
#![allow(non_snake_case)]

mod api;
mod components;
mod pages;
mod sleep;

use dioxus::prelude::*;
use dioxus_router::prelude::{Outlet, Router, Routable};

// Route component functions must be in scope for the `#[route]` attributes.
use crate::pages::{pipeline::Pipeline, types::Types, simulation::Simulation, visualizations::Visualizations};

/// Application routes — each variant maps to a page component.
/// The `#[layout]` attribute wraps all routes in the sidebar layout.
#[derive(Routable, Clone, PartialEq)]
pub enum Route {
    #[layout(AppLayout)]
    #[route("/wasm")]
    Pipeline {},
    #[route("/wasm/types")]
    Types {},
    #[route("/wasm/sim")]
    Simulation {},
    #[route("/wasm/viz")]
    Visualizations {},
}

/// Shared layout: sidebar + main content outlet rendered inside Router context.
fn AppLayout() -> Element {
    rsx! {
        div {
            style: "display: flex; min-height: 100vh; background: #020617; color: #e2e8f0; font-family: system-ui, -apple-system, sans-serif;",
            components::Sidebar {}
            main {
                style: "flex: 1; padding: 28px; overflow-y: auto;",
                Outlet::<Route> {}
            }
        }
    }
}

/// Root component — mounts the router.
fn App() -> Element {
    rsx! { Router::<Route> {} }
}

/// WASM entry point — called automatically when the module loads.
#[wasm_bindgen::prelude::wasm_bindgen(start)]
fn start() {
    dioxus::launch(App);
}
