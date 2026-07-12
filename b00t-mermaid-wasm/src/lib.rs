//! Minimal Mermaid WASM renderer — zero Dioxus, zero SPA, just `render_mermaid`.

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn render_mermaid(text: &str) -> String {
    mermaid_rs_renderer::render(text).unwrap_or_else(|e| {
        format!(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 400 60'><rect width='100%' height='100%' fill='#1e293b'/><text x='200' y='35' text-anchor='middle' fill='#ef4444' font-family='monospace' font-size='12'>mmdr: {}</text></svg>",
            e.to_string().replace('\'', "\\'").replace('<', "&lt;")
        )
    })
}
