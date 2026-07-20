//! Pipeline DAG visualization: Mermaid, JSON, SVG formats.
//!
//! Provides the [`VizFormat`] trait with implementations for rendering
//! [`PipelineDag`](crate::pipeline_types::PipelineDag) as Mermaid flowcharts,
//! JSON structures, and inline SVG.
//!
//! # Example
//!
//! ```rust,ignore
//! use b00t_cli::pipeline_viz::{render_pipeline, MermaidViz, VizFormat};
//!
//! let mermaid = MermaidViz.render(&dag);
//! let json = render_pipeline(&dag, "json").unwrap();
//! ```

use crate::pipeline_types::{PipelineDag, PortMediaType};
use anyhow::Result;

/// Trait for rendering a PipelineDag into a specific output format.
pub trait VizFormat {
    /// Render the DAG into the target format string.
    fn render(&self, dag: &PipelineDag) -> String;
}

// ── MermaidViz ──────────────────────────────────────────────────────────────

/// Mermaid flowchart (`graph TD`) renderer.
///
/// - Stages become `Name["Stage Name"]` nodes
/// - Edges annotated with MIME type labels (`A -->|video/mp4| B`)
/// - GPU stages get `:::gpu` class marker via CSS styling
/// - Entry / exit points highlighted with `:::entry` / `:::exit`
pub struct MermaidViz;

impl VizFormat for MermaidViz {
    fn render(&self, dag: &PipelineDag) -> String {
        let mut out = String::from("graph TD\n");

        // ── Nodes ────────────────────────────────────────────────────────
        for stage in &dag.stages {
            let id = sanitize_id(&stage.name);
            let is_gpu = stage.profile.resources.requires_gpu;

            out.push_str(&format!("    {}[\"{}\"]", id, stage.name));

            let mut classes = Vec::new();
            if is_gpu {
                classes.push("gpu");
            }
            if dag.entry_points.contains(&stage.name) {
                classes.push("entry");
            }
            if dag.exit_points.contains(&stage.name) {
                classes.push("exit");
            }
            if !classes.is_empty() {
                out.push_str(":::");
                out.push_str(&classes.join(","));
            }
            out.push('\n');
        }

        // ── Edges ────────────────────────────────────────────────────────
        for edge in &dag.edges {
            let from = sanitize_id(&edge.from);
            let to = sanitize_id(&edge.to);
            let label = edge
                .via_port
                .as_ref()
                .map(|p| p.media_type.mime_type())
                .unwrap_or("");

            if label.is_empty() {
                out.push_str(&format!("    {} --> {}\n", from, to));
            } else {
                out.push_str(&format!("    {} -->|\"{}\" {}\n", from, label, to));
            }
        }

        out
    }
}

// ── JsonViz ─────────────────────────────────────────────────────────────────

/// JSON structure renderer.
///
/// Produces a JSON object with:
/// - `nodes` — array of `{ id, label, type: "entry"|"exit"|"stage", gpu: bool }`
/// - `edges` — array of `{ source, target, via_port: { media_type, direction } | null }`
pub struct JsonViz;

impl VizFormat for JsonViz {
    fn render(&self, dag: &PipelineDag) -> String {
        let entry_set: std::collections::HashSet<&str> =
            dag.entry_points.iter().map(|s| s.as_str()).collect();
        let exit_set: std::collections::HashSet<&str> =
            dag.exit_points.iter().map(|s| s.as_str()).collect();

        let nodes: Vec<serde_json::Value> = dag
            .stages
            .iter()
            .map(|s| {
                let node_type = if entry_set.contains(s.name.as_str()) {
                    "entry"
                } else if exit_set.contains(s.name.as_str()) {
                    "exit"
                } else {
                    "stage"
                };
                serde_json::json!({
                    "id": s.name,
                    "label": s.name,
                    "type": node_type,
                    "gpu": s.profile.resources.requires_gpu,
                })
            })
            .collect();

        let edges: Vec<serde_json::Value> = dag
            .edges
            .iter()
            .map(|e| {
                let port = e.via_port.as_ref().map(|p| {
                    serde_json::json!({
                        "media_type": format!("{:?}", p.media_type),
                        "direction": format!("{:?}", p.direction),
                    })
                });
                serde_json::json!({
                    "source": e.from,
                    "target": e.to,
                    "via_port": port,
                })
            })
            .collect();

        let output = serde_json::json!({ "nodes": nodes, "edges": edges });
        serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
    }
}

// ── SvgViz ───────────────────────────────────────────────────────────────────

/// Inline SVG renderer with grid-based layout.
///
/// - Topological (or declaration) order from top to bottom
/// - Rounded rectangles for stages
/// - Entry / exit highlighted with distinct stroke colors
/// - GPU stages shaded with a warm fill
/// - Directed arrows between connected stages, color-coded by port media type
pub struct SvgViz;

const NODE_W: f64 = 200.0;
const NODE_H: f64 = 60.0;
const GAP_Y: f64 = 80.0;
const START_X: f64 = 60.0;
const START_Y: f64 = 40.0;

impl VizFormat for SvgViz {
    fn render(&self, dag: &PipelineDag) -> String {
        let n = dag.stages.len();
        if n == 0 {
            return r##"<svg xmlns="http://www.w3.org/2000/svg" width="400" height="100">
  <rect width="100%" height="100%" fill="#fafafa" rx="8"/>
  <text x="20" y="50" font-family="monospace" font-size="14" fill="#888">(empty pipeline)</text>
</svg>"##
                .to_string();
        }

        // Determine order: topological sort or declaration order fallback
        let order: Vec<String> = dag
            .execution_order()
            .unwrap_or_else(|_| dag.stages.iter().map(|s| s.name.clone()).collect());

        // Map stage name → position index
        let stage_to_idx: std::collections::HashMap<&str, usize> = order
            .iter()
            .enumerate()
            .map(|(i, name)| (name.as_str(), i))
            .collect();

        let svg_w = START_X * 2.0 + NODE_W;
        let svg_h = START_Y * 2.0 + n as f64 * (NODE_H + GAP_Y);
        let center_x = START_X + NODE_W / 2.0;

        let mut out = String::new();

        // ── SVG header + defs ─────────────────────────────────────────────
        // Note: using r##"..."## raw strings because SVG attributes contain "#xxx" hex colors
        // which would prematurely terminate r#"..."# strings
        out.push_str(&format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{:.0}" height="{:.0}" font-family="sans-serif">
  <defs>
    <marker id="arr" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#666"/>
    </marker>
    <marker id="arr-video" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#e74c3c"/>
    </marker>
    <marker id="arr-audio" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#3498db"/>
    </marker>
    <marker id="arr-image" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#2ecc71"/>
    </marker>
    <marker id="arr-json" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#f39c12"/>
    </marker>
    <marker id="arr-parquet" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#9b59b6"/>
    </marker>
    <marker id="arr-bytes" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#95a5a6"/>
    </marker>
    <marker id="arr-error" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto">
      <path d="M 0 0 L 10 5 L 0 10 z" fill="#e74c3c"/>
    </marker>
  </defs>
  <rect width="100%" height="100%" fill="#fafafa" rx="8"/>
"##,
            svg_w, svg_h
        ));

        // Compute vertical positions (single-column grid)
        let positions: Vec<(f64, f64)> = (0..n)
            .map(|i| {
                let y = START_Y + i as f64 * (NODE_H + GAP_Y);
                (START_X, y)
            })
            .collect();

        // ── Edges (drawn first, behind nodes) ─────────────────────────────
        for edge in &dag.edges {
            let from_idx = stage_to_idx.get(edge.from.as_str());
            let to_idx = stage_to_idx.get(edge.to.as_str());
            if let (Some(&fi), Some(&ti)) = (from_idx, to_idx) {
                let x1 = center_x;
                let y1 = positions[fi].1 + NODE_H;
                let x2 = center_x;
                let y2 = positions[ti].1;

                let (marker_id, stroke_color) = edge
                    .via_port
                    .as_ref()
                    .map(|p| media_type_marker_and_color(&p.media_type))
                    .unwrap_or(("arr", "#666"));

                // Adjacent → straight line; non-adjacent → S-curve
                if ti == fi + 1 {
                    out.push_str(&format!(
                        r##"  <line x1="{x1:.0}" y1="{y1:.0}" x2="{x2:.0}" y2="{y2:.0}" stroke="{stroke_color}" stroke-width="2" marker-end="url(#{marker_id})"/>"##,
                    ));
                } else {
                    let mid_y = (y1 + y2) / 2.0;
                    out.push_str(&format!(
                        r##"  <path d="M {x1:.0} {y1:.0} C {x1:.0} {mid_y:.0} {x2:.0} {mid_y:.0} {x2:.0} {y2:.0}" stroke="{stroke_color}" stroke-width="2" fill="none" marker-end="url(#{marker_id})"/>"##,
                    ));
                }

                // Edge label
                if let Some(ref port) = edge.via_port {
                    let label_y = (y1 + y2) / 2.0 - 6.0;
                    out.push_str(&format!(
                        r##"  <text x="{center_x:.0}" y="{label_y:.0}" text-anchor="middle" font-size="11" fill="{stroke_color}">{}</text>"##,
                        port.media_type.mime_type()
                    ));
                }
            }
        }

        // ── Nodes ─────────────────────────────────────────────────────────
        for (i, stage) in dag.stages.iter().enumerate() {
            let (x, y) = positions[i];
            let is_entry = dag.entry_points.contains(&stage.name);
            let is_exit = dag.exit_points.contains(&stage.name);
            let is_gpu = stage.profile.resources.requires_gpu;

            let fill = if is_gpu { "#fff3e0" } else { "#ffffff" };
            let (stroke, stroke_width) = if is_entry {
                ("#27ae60", 3.0)
            } else if is_exit {
                ("#2980b9", 3.0)
            } else {
                ("#555", 1.5)
            };

            out.push_str(&format!(
                r##"  <rect x="{x:.0}" y="{y:.0}" width="{NODE_W:.0}" height="{NODE_H:.0}" rx="8" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_width:.1}"/>"##,
            ));
            out.push_str(&format!(
                r##"  <text x="{center_x:.0}" y="{:.0}" text-anchor="middle" dominant-baseline="middle" font-size="14" fill="#333">{}</text>"##,
                y + NODE_H / 2.0,
                stage.name
            ));

            // Badges
            if is_gpu {
                out.push_str(&format!(
                    r##"  <text x="{:.0}" y="{:.0}" text-anchor="end" font-size="9" fill="#e67e22">⚡GPU</text>"##,
                    x + NODE_W - 4.0, y + 12.0
                ));
            }
            if is_entry {
                out.push_str(&format!(
                    r##"  <text x="{:.0}" y="{:.0}" text-anchor="start" font-size="9" fill="#27ae60">▶ IN</text>"##,
                    x + 4.0, y + 12.0
                ));
            } else if is_exit {
                out.push_str(&format!(
                    r##"  <text x="{:.0}" y="{:.0}" text-anchor="start" font-size="9" fill="#2980b9">⏹ OUT</text>"##,
                    x + 4.0, y + 12.0
                ));
            }
        }

        out.push_str("</svg>");
        out
    }
}

/// Return the SVG marker ID and stroke color for a port media type.
fn media_type_marker_and_color(mt: &PortMediaType) -> (&'static str, &'static str) {
    match mt {
        PortMediaType::Video => ("arr-video", "#e74c3c"),
        PortMediaType::Audio => ("arr-audio", "#3498db"),
        PortMediaType::Image => ("arr-image", "#2ecc71"),
        PortMediaType::Json => ("arr-json", "#f39c12"),
        PortMediaType::Parquet => ("arr-parquet", "#9b59b6"),
        PortMediaType::Bytes => ("arr-bytes", "#95a5a6"),
        PortMediaType::Error => ("arr-error", "#e74c3c"),
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Sanitize a stage name for use as a Mermaid node ID.
fn sanitize_id(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

/// Render a [`PipelineDag`] in the specified format.
///
/// Supported formats: `"mermaid"`, `"json"`, `"svg"`
///
/// # Errors
/// Returns an error if the format is not one of the supported values.
pub fn render_pipeline(dag: &PipelineDag, format: &str) -> Result<String> {
    match format.to_lowercase().as_str() {
        "mermaid" => Ok(MermaidViz.render(dag)),
        "json" => Ok(JsonViz.render(dag)),
        "svg" => Ok(SvgViz.render(dag)),
        other => Err(anyhow::anyhow!(
            "unknown pipeline viz format '{other}'; expected one of: mermaid, json, svg"
        )),
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_types::{
        CapsuleProfile, PipelineDag, PortDirection, PortMediaType, ResourceRequirements, StagePort,
        StageSpec,
    };

    fn make_stage(
        name: &str,
        input_types: &[PortMediaType],
        output_types: &[PortMediaType],
        gpu: bool,
    ) -> StageSpec {
        StageSpec {
            name: name.into(),
            profile: CapsuleProfile {
                name: name.into(),
                ports: vec![],
                resources: ResourceRequirements {
                    min_ram_gb: 0.0,
                    min_vram_gb: if gpu { 8.0 } else { 0.0 },
                    requires_gpu: gpu,
                    cpu_cores: None,
                    scratch_disk_gb: None,
                },
                image: None,
                timeout_seconds: None,
            },
            input_ports: input_types
                .iter()
                .map(|mt| StagePort {
                    direction: PortDirection::Input,
                    media_type: mt.clone(),
                    description: None,
                })
                .collect(),
            output_ports: output_types
                .iter()
                .map(|mt| StagePort {
                    direction: PortDirection::Output,
                    media_type: mt.clone(),
                    description: None,
                })
                .collect(),
            error_routes: vec![],
            env: None,
            checkpoint_interval_seconds: None,
            secret_refs: None,
            flow_control: None,
        }
    }

    fn sample_dag() -> PipelineDag {
        // ingest → transcode (GPU) → export
        // Audio→Audio, Video→Video avoids unwanted extra edges from all-Video ports
        let stages = vec![
            make_stage("ingest", &[], &[PortMediaType::Audio], false),
            make_stage(
                "transcode",
                &[PortMediaType::Audio],
                &[PortMediaType::Video],
                true,
            ),
            make_stage("export", &[PortMediaType::Video], &[], false),
        ];
        PipelineDag::build(stages).unwrap()
    }

    // ── Mermaid ──────────────────────────────────────────────────────────

    #[test]
    fn mermaid_contains_expected_nodes_and_edges() {
        let dag = sample_dag();
        let output = MermaidViz.render(&dag);
        assert!(output.contains("ingest"), "should contain ingest node");
        assert!(
            output.contains("transcode"),
            "should contain transcode node"
        );
        assert!(output.contains("export"), "should contain export node");
        assert!(output.contains("-->"), "should contain edges with arrow");
        assert!(
            output.contains("video/mp4"),
            "should contain mime type label"
        );
    }

    #[test]
    fn mermaid_gpu_stage_has_class_marker() {
        let dag = sample_dag();
        let output = MermaidViz.render(&dag);
        assert!(
            output.contains("gpu"),
            "GPU stage should have gpu class marker"
        );
        // transcode is the GPU stage
        assert!(
            output.contains("transcode") && output.contains("gpu"),
            "transcode should be marked as gpu"
        );
    }

    #[test]
    fn mermaid_empty_pipeline() {
        let dag = PipelineDag::build(vec![]).unwrap();
        let output = MermaidViz.render(&dag);
        assert!(output.contains("graph TD"), "should have graph header");
        assert!(!output.contains("-->"), "no edges in empty pipeline");
    }

    // ── JSON ─────────────────────────────────────────────────────────────

    #[test]
    fn json_parses_back_to_valid_structure() {
        let dag = sample_dag();
        let output = JsonViz.render(&dag);
        let parsed: serde_json::Value =
            serde_json::from_str(&output).expect("JSON output should be valid");
        assert!(parsed.get("nodes").is_some(), "should have nodes array");
        assert!(parsed.get("edges").is_some(), "should have edges array");
        let nodes = parsed["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 3, "should have 3 nodes");
        let edges = parsed["edges"].as_array().unwrap();
        assert_eq!(edges.len(), 2, "should have 2 edges");
    }

    #[test]
    fn json_gpu_stage_has_marker() {
        let dag = sample_dag();
        let output = JsonViz.render(&dag);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let nodes = parsed["nodes"].as_array().unwrap();
        let transcode = nodes.iter().find(|n| n["id"] == "transcode").unwrap();
        assert_eq!(transcode["gpu"], true, "transcode should have gpu=true");
        let ingest = nodes.iter().find(|n| n["id"] == "ingest").unwrap();
        assert_eq!(ingest["gpu"], false, "ingest should have gpu=false");
    }

    #[test]
    fn json_node_types() {
        let dag = sample_dag();
        let output = JsonViz.render(&dag);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        let nodes = parsed["nodes"].as_array().unwrap();
        let ingest = nodes.iter().find(|n| n["id"] == "ingest").unwrap();
        assert_eq!(ingest["type"], "entry", "ingest should be entry");
        let export = nodes.iter().find(|n| n["id"] == "export").unwrap();
        assert_eq!(export["type"], "exit", "export should be exit");
        let transcode = nodes.iter().find(|n| n["id"] == "transcode").unwrap();
        assert_eq!(transcode["type"], "stage", "transcode should be stage");
    }

    #[test]
    fn json_empty_pipeline() {
        let dag = PipelineDag::build(vec![]).unwrap();
        let output = JsonViz.render(&dag);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["nodes"].as_array().unwrap().len(), 0);
        assert_eq!(parsed["edges"].as_array().unwrap().len(), 0);
    }

    // ── SVG ──────────────────────────────────────────────────────────────

    #[test]
    fn svg_contains_stage_rectangles() {
        let dag = sample_dag();
        let output = SvgViz.render(&dag);
        assert!(output.contains("<svg"), "should be SVG");
        assert!(output.contains("</svg>"), "should close SVG");
        assert!(output.contains("ingest"), "should label ingest stage");
        assert!(output.contains("transcode"), "should label transcode stage");
        assert!(output.contains("export"), "should label export stage");
    }

    #[test]
    fn svg_gpu_stage_has_marker() {
        let dag = sample_dag();
        let output = SvgViz.render(&dag);
        assert!(
            output.contains("GPU") || output.contains("gpu"),
            "GPU stage should have GPU marker"
        );
    }

    #[test]
    fn svg_empty_pipeline() {
        let dag = PipelineDag::build(vec![]).unwrap();
        let output = SvgViz.render(&dag);
        assert!(
            output.contains("empty pipeline"),
            "should indicate empty pipeline"
        );
    }

    // ── Dispatch ─────────────────────────────────────────────────────────

    #[test]
    fn dispatch_mermaid() {
        let dag = sample_dag();
        let output = render_pipeline(&dag, "mermaid").unwrap();
        assert!(output.contains("graph TD"));
    }

    #[test]
    fn dispatch_json() {
        let dag = sample_dag();
        let output = render_pipeline(&dag, "json").unwrap();
        let _: serde_json::Value = serde_json::from_str(&output).unwrap();
    }

    #[test]
    fn dispatch_svg() {
        let dag = sample_dag();
        let output = render_pipeline(&dag, "svg").unwrap();
        assert!(output.contains("<svg"));
    }

    #[test]
    fn dispatch_unknown_format() {
        let dag = PipelineDag::build(vec![]).unwrap();
        let err = render_pipeline(&dag, "plantuml").unwrap_err();
        assert!(
            err.to_string().contains("unknown"),
            "should error on unknown format"
        );
    }

    #[test]
    fn dispatch_case_insensitive() {
        let dag = PipelineDag::build(vec![]).unwrap();
        assert!(render_pipeline(&dag, "MERMAID").is_ok());
        assert!(render_pipeline(&dag, "JSON").is_ok());
        assert!(render_pipeline(&dag, "Svg").is_ok());
    }
}
