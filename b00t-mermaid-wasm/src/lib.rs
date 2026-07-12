//! Minimal Mermaid WASM renderer — zero Dioxus, zero SPA, just `render_mermaid`.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

fn render_inner(text: &str) -> String {
    mermaid_rs_renderer::render(text).unwrap_or_else(|e| {
        format!(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 400 60'><rect width='100%' height='100%' fill='#1e293b'/><text x='200' y='35' text-anchor='middle' fill='#ef4444' font-family='monospace' font-size='12'>mermaid-rs-renderer: {}</text></svg>",
            e.to_string().replace('\'', "\\'").replace('<', "&lt;")
        )
    })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn render_mermaid(text: &str) -> String {
    render_inner(text)
}

#[cfg(test)]
mod tests {
    use super::render_inner as render_mermaid;

    #[test]
    fn renders_simple_flowchart() {
        let svg = render_mermaid("flowchart LR\n    A[Start] --> B[End]");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("Start"));
        assert!(svg.contains("End"));
        assert!(svg.len() > 200, "SVG too small: {} bytes", svg.len());
    }

    #[test]
    fn renders_class_diagram() {
        let svg = render_mermaid("classDiagram\n    Animal <|-- Duck\n    Animal : +int age\n    Duck : +swim()");
        assert!(svg.contains("<svg"));
        assert!(svg.len() > 200);
    }

    #[test]
    fn renders_sequence_diagram() {
        let svg = render_mermaid("sequenceDiagram\n    Alice->>Bob: Hello\n    Bob-->>Alice: Hi");
        assert!(svg.contains("<svg"));
        assert!(svg.len() > 200);
    }

    #[test]
    fn error_on_invalid_syntax() {
        let svg = render_mermaid("not a valid diagram at all ###");
        assert!(svg.contains("<svg"), "error output must be SVG");
        assert!(svg.len() > 100);
    }

    #[test]
    fn renders_pipeline_style() {
        let svg = render_mermaid(
            "flowchart LR\n  fetch[\"Fetch Document\"]\n  parse[\"Parse\"]\n  validate[\"Validate\"]\n  fetch --> parse\n  parse --> validate"
        );
        assert!(svg.contains("Fetch Document"));
        assert!(svg.contains("Parse"));
        assert!(svg.contains("Validate"));
    }

    #[test]
    fn pipeline_data_from_api_renders() {
        let data = "flowchart LR\n  %% Auto-generated from NodeGraph\n  %% Pipeline: Fetch Document\n\n  fetch[\"Fetch Document\"]\n  style fetch fill:#083344,stroke:#06b6d4\n  parse[\"Parse PDF\"]\n  style parse fill:#083344,stroke:#06b6d4\n  fetch --> parse";
        let svg = render_mermaid(data);
        assert!(svg.contains("<svg"), "must produce SVG from pipeline data");
        assert!(svg.contains("Fetch Document"));
        assert!(svg.len() > 500, "SVG too small: {} bytes", svg.len());
    }
}
