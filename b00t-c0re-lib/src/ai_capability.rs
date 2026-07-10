//! AICapability ontological trait hierarchy for the b00t ML pipeline.
//!
//! Ontology (UFO-aligned):
//!   AICapability
//!     └─ ImageProcessingCapability<F: ImageFormat>
//!           └─ SegmentationCapability
//!                 └─ Sam3Node  (concrete, implements PipelineNode)
//!
//! Polymorphic I/O via enums so pipeline graph edges type-match automatically.
//! The `PipelineNode` trait in `pipeline_nodes.rs` is the execution contract;
//! `AICapability` is the ontological identity.

use serde::{Deserialize, Serialize};
use std::fmt::Debug;

// ═══════════════════════════════════════════════════════════════════════════
// Section A: Ontological capability markers
// ═══════════════════════════════════════════════════════════════════════════

/// Top-level AI capability marker.  Every concrete AI node implements this.
/// Provides stable IRI-like IDs for ontology graph edges.
pub trait AICapability: Debug + Send + Sync {
    /// Stable IRI-style ID, e.g. "b00t:cap:ImageProcessing:Segmentation:SAM3"
    fn capability_id(&self) -> &'static str;
    /// Human name for display / MLflow tags
    fn capability_name(&self) -> &'static str;
    /// JSON Schema of accepted input (for auto-wiring)
    fn input_schema_id(&self) -> &'static str;
    /// JSON Schema of produced output (for auto-wiring)
    fn output_schema_id(&self) -> &'static str;
}

/// Image format enum — generic parameter on ImageProcessingCapability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Rgb,
    Rgba,
    Grayscale,
    Jpeg,
    Png,
    Webp,
    Tensor, // raw float tensor
}

/// Any capability that processes images.  Generic over accepted format.
pub trait ImageProcessingCapability: AICapability {
    fn supported_formats(&self) -> &[ImageFormat];
    fn max_resolution(&self) -> Option<(u32, u32)>;
}

/// A capability that segments image regions — text/box/point prompted.
/// Subtrait of ImageProcessingCapability.
pub trait SegmentationCapability: ImageProcessingCapability {
    type Request: Serialize + for<'de> Deserialize<'de> + Debug + Clone + Send + Sync;
    type Response: Serialize + for<'de> Deserialize<'de> + Debug + Clone + Send + Sync;

    fn supported_prompt_types(&self) -> &[PromptType];
    fn supports_video(&self) -> bool { false }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptType {
    Text,
    BoundingBox,
    Point,
    ExemplarImage,
    AutomaticGrid,
}

// ═══════════════════════════════════════════════════════════════════════════
// Section B: Sam3JobRequest — polymorphic input
// ═══════════════════════════════════════════════════════════════════════════

/// Polymorphic image source — file path, URL, or base64.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "source_type")]
pub enum ImageSource {
    Path { image_path: String },
    Url { image_url: String },
    Base64 { image_base64: String },
}

/// One segmentation prompt — text noun phrase, bounding box, point, or exemplar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum SegmentationPrompt {
    Text { value: String },
    Box { value: [f32; 4] },                             // [x1, y1, x2, y2]
    Point { value: PointPrompt },
    ExemplarImage { value: ImageSource },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointPrompt {
    pub coords: [f32; 2],
    /// 1 = foreground, 0 = background
    pub label: i32,
}

/// Mask output encoding format — caller chooses tradeoff.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskOutputFormat {
    #[default]
    Rle,        // COCO RLE — compact, server-friendly
    PngBase64,  // lossless pixels — display-friendly
    Polygon,    // outline contour — geometry-friendly
}

/// Full request type for SAM3 (and any SegmentationCapability node).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sam3JobRequest {
    // ── required ──────────────────────────────────────────────────────────
    #[serde(flatten)]
    pub image: ImageSource,
    /// Written to /workspace/result.json inside the container
    pub output_path: String,

    // ── optional tuning ───────────────────────────────────────────────────
    #[serde(default)]
    pub prompts: Vec<SegmentationPrompt>,
    #[serde(default = "default_model_id")]
    pub model_id: String,
    #[serde(default = "default_device")]
    pub device: String,
    #[serde(default)]
    pub score_threshold: f32,
    #[serde(default)]
    pub output_format: MaskOutputFormat,
    /// If true, return the largest mask only (useful for single-object crops)
    #[serde(default)]
    pub largest_only: bool,
    /// Max segments to return (0 = no limit)
    #[serde(default)]
    pub max_segments: u32,
}

fn default_model_id() -> String { "facebook/sam3".into() }
fn default_device()   -> String { "cuda".into() }

impl Sam3JobRequest {
    pub fn text_prompt(image_path: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            image: ImageSource::Path { image_path: image_path.into() },
            output_path: "/workspace/result.json".into(),
            prompts: vec![SegmentationPrompt::Text { value: text.into() }],
            model_id: default_model_id(),
            device: default_device(),
            score_threshold: 0.0,
            output_format: MaskOutputFormat::Rle,
            largest_only: false,
            max_segments: 0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Section C: SamSegmentationOutput — polymorphic output
// ═══════════════════════════════════════════════════════════════════════════

/// Mask in any of the three supported encodings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "format")]
pub enum EncodedMask {
    Rle      { data: RleMask },
    PngBase64 { data: String },
    Polygon  { data: Vec<[f32; 2]> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RleMask {
    pub size: [u32; 2],   // [height, width]
    pub counts: Vec<u64>,
}

/// One detected segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub mask: EncodedMask,
    pub score: f32,
    pub label: String,
    pub box_xyxy: [f32; 4],
}

/// Full output from any SegmentationCapability node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamSegmentationOutput {
    pub schema_version: String,
    pub model_id: String,
    pub image_size: [u32; 2],
    pub segments: Vec<Segment>,
    pub segment_count: usize,
    pub timing: SegmentationTiming,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentationTiming {
    pub load_s: f64,
    pub infer_s: f64,
}

// ═══════════════════════════════════════════════════════════════════════════
// Section D: Sam3Node — implements PipelineNode + SegmentationCapability
// ═══════════════════════════════════════════════════════════════════════════

use crate::doc_pipeline::SerializableFOLFormula;
use crate::pipeline_nodes::{NodeCategory, NodeShape, NodeStyle, PipelineNode, PortDef, PortDirection, StateMachine};

/// SAM3 as a typed pipeline node.  Delegates execution to LocalProvider /
/// RunpodProvider via BatchJobSpec — the node itself is compute-agnostic.
#[derive(Debug, Clone)]
pub struct Sam3Node {
    pub model_id: String,
    pub device: String,
}

impl Default for Sam3Node {
    fn default() -> Self {
        Self {
            model_id: "facebook/sam3".into(),
            device: "cuda".into(),
        }
    }
}

impl AICapability for Sam3Node {
    fn capability_id(&self) -> &'static str {
        "b00t:cap:ImageProcessing:Segmentation:SAM3"
    }
    fn capability_name(&self) -> &'static str { "SAM3 Instance Segmentation" }
    fn input_schema_id(&self) -> &'static str { "b00t:schema:Sam3JobRequest:v1" }
    fn output_schema_id(&self) -> &'static str { "b00t:schema:SamSegmentationOutput:v1" }
}

impl ImageProcessingCapability for Sam3Node {
    fn supported_formats(&self) -> &[ImageFormat] {
        &[ImageFormat::Rgb, ImageFormat::Rgba, ImageFormat::Jpeg, ImageFormat::Png, ImageFormat::Webp]
    }
    fn max_resolution(&self) -> Option<(u32, u32)> { None }
}

impl SegmentationCapability for Sam3Node {
    type Request = Sam3JobRequest;
    type Response = SamSegmentationOutput;
    fn supported_prompt_types(&self) -> &[PromptType] {
        &[PromptType::Text, PromptType::BoundingBox, PromptType::Point, PromptType::ExemplarImage, PromptType::AutomaticGrid]
    }
    fn supports_video(&self) -> bool { false }
}

impl PipelineNode for Sam3Node {
    type Input = Sam3JobRequest;
    type Output = SamSegmentationOutput;

    fn node_id(&self) -> &str { "sam3" }
    fn node_label(&self) -> &str { "SAM3 Segmentation" }
    fn node_category(&self) -> NodeCategory { NodeCategory::Transform }

    fn preconditions(&self) -> Vec<SerializableFOLFormula> { vec![] }
    fn postconditions(&self) -> Vec<SerializableFOLFormula> { vec![] }
    fn invariants(&self) -> Vec<SerializableFOLFormula> { vec![] }
    fn state_machine(&self) -> StateMachine { StateMachine::idle_run_cycle("sam3") }

    fn input_ports(&self) -> Vec<PortDef> {
        vec![
            PortDef { name: "image".into(), port_type: "ImageSource".into(), direction: PortDirection::Input },
            PortDef { name: "prompts".into(), port_type: "Vec<SegmentationPrompt>".into(), direction: PortDirection::Input },
        ]
    }
    fn output_ports(&self) -> Vec<PortDef> {
        vec![PortDef { name: "segments".into(), port_type: "SamSegmentationOutput".into(), direction: PortDirection::Output }]
    }
    fn visual_style(&self) -> NodeStyle {
        NodeStyle { fill: "#1a1a2e".into(), stroke: "#7c3aed".into(), shape: NodeShape::RoundedBox }
    }

    // 🤓 Synchronous execute: writes request to a temp file, runs the container
    //    via podman, reads result.json back.  Async path: use LocalProvider directly.
    fn execute(&self, req: Sam3JobRequest) -> SamSegmentationOutput {
        use std::process::Command;

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        let out_dir = std::env::temp_dir().join(format!("sam3-{ts}"));
        std::fs::create_dir_all(&out_dir).expect("create temp dir");
        let req_path = out_dir.join("request.json");
        let result_path = out_dir.join("result.json");

        // Write the request JSON, overriding output_path to our temp dir
        let mut req_out = req.clone();
        req_out.output_path = result_path.to_str().unwrap().to_string();
        serde_json::to_writer(std::fs::File::create(&req_path).unwrap(), &req_out).unwrap();

        let req_str = req_path.to_str().unwrap().to_string();
        let out_str = out_dir.to_str().unwrap().to_string();
        let status = Command::new("podman")
            .args([
                "run", "--rm",
                "--device", "nvidia.com/gpu=all",
                "--security-opt=label=disable",
                "-v", &format!("{req_str}:/workspace/request.json:ro"),
                "-v", &format!("{out_str}:/workspace:rw"),
                Sam3JobRequest::container_image_local(),
            ])
            .status()
            .expect("podman run failed");

        if !status.success() {
            panic!("sam3-runner container exited with {status}");
        }

        let json = std::fs::read_to_string(&result_path).expect("result.json not found");
        serde_json::from_str(&json).expect("result.json parse failed")
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Section E: BatchJobSpec bridge — converts Sam3JobRequest → provider job
// ═══════════════════════════════════════════════════════════════════════════

/// Convert a Sam3JobRequest into the image + config_path needed by
/// LocalProvider / RunpodProvider via BatchJobSpec.
/// Caller writes the request JSON to a temp file and passes its path.
impl Sam3JobRequest {
    pub fn container_image_local() -> &'static str {
        "app4dog/sam3-runner:local"
    }
    pub fn container_image_cloud() -> &'static str {
        "app4dog/sam3-runner:cloud"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sam3_request_roundtrip() {
        let req = Sam3JobRequest::text_prompt("/tmp/dog.jpg", "dog");
        let json = serde_json::to_string(&req).unwrap();
        let back: Sam3JobRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.model_id, "facebook/sam3");
        assert_eq!(back.prompts.len(), 1);
    }

    #[test]
    fn sam3_node_capability_id() {
        let node = Sam3Node::default();
        assert_eq!(
            node.capability_id(),
            "b00t:cap:ImageProcessing:Segmentation:SAM3"
        );
    }

    #[test]
    fn segmentation_output_roundtrip() {
        let out = SamSegmentationOutput {
            schema_version: "sam3.v1".into(),
            model_id: "facebook/sam3".into(),
            image_size: [640, 480],
            segments: vec![Segment {
                mask: EncodedMask::Polygon { data: vec![[10.0, 20.0], [30.0, 40.0]] },
                score: 0.95,
                label: "dog".into(),
                box_xyxy: [100.0, 200.0, 300.0, 400.0],
            }],
            segment_count: 1,
            timing: SegmentationTiming { load_s: 2.1, infer_s: 0.3 },
        };
        let json = serde_json::to_string_pretty(&out).unwrap();
        let back: SamSegmentationOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(back.segment_count, 1);
        assert_eq!(back.segments[0].label, "dog");
    }
}
