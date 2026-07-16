//! Typed photo-artifact extraction contracts for b00t AI pipelines.
//!
//! This module keeps provider transport JSON at the edge. Internally, b00t
//! composes lightweight Rust domain types: a `Photo`, a typed
//! `PromptTemplate`, and provider-normalized `ExtractedArtifact` values.

use crate::ai_capability::{
    EncodedMask, ImageFormat, ImageSource as Sam3ImageSource, MaskOutputFormat, Sam3JobRequest,
    Sam3Node, SamSegmentationOutput, SegmentationPrompt,
};
use crate::doc_pipeline::SerializableFOLFormula;
use crate::pipeline_nodes::{
    NodeCategory, NodeShape, NodeStyle, PipelineNode, PortDef, PortDirection, StateMachine,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;
use ufo_types::{Satisfies, SatisfiesResult, Stereotyped, UfoStereotype};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PhotoId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PromptTemplateId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtifactId(pub String);

/// Transport boundary for loading a `Photo`; not used as pipeline edge data
/// after the load node has produced the typed domain object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotoSource {
    Path(std::path::PathBuf),
    Url(String),
    Bytes(Arc<[u8]>),
}

/// UFO Endurant: a persistent source image with stable identity and hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Photo {
    pub id: PhotoId,
    pub bytes: Arc<[u8]>,
    pub format: ImageFormat,
    pub dimensions: (u32, u32),
    pub sha256: String,
}

impl Photo {
    pub fn from_bytes(
        id: impl Into<String>,
        bytes: impl Into<Arc<[u8]>>,
        format: ImageFormat,
        dimensions: (u32, u32),
    ) -> Self {
        let bytes = bytes.into();
        let sha256 = sha256_hex(&bytes);
        Self {
            id: PhotoId(id.into()),
            bytes,
            format,
            dimensions,
            sha256,
        }
    }
}

impl Stereotyped for Photo {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Kind("Photo".into())
    }
}

/// Template parameter kind: intentionally small, serializable, and domain
/// oriented. Provider-specific prompt syntax is rendered later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PromptParameterKind {
    Text,
    Number,
    Boolean,
    Enum { variants: Vec<String> },
    RegionHint,
    SpeciesHint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptParameterSpec {
    pub name: String,
    pub kind: PromptParameterKind,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PromptValue {
    Text(String),
    Number(f64),
    Boolean(bool),
    Enum(String),
    RegionHint([f32; 4]),
    SpeciesHint(String),
}

impl PromptValue {
    fn render(&self) -> String {
        match self {
            PromptValue::Text(value)
            | PromptValue::Enum(value)
            | PromptValue::SpeciesHint(value) => value.clone(),
            PromptValue::Number(value) => value.to_string(),
            PromptValue::Boolean(value) => value.to_string(),
            PromptValue::RegionHint([x1, y1, x2, y2]) => format!("{x1},{y1},{x2},{y2}"),
        }
    }
}

pub type PromptArgumentMap = BTreeMap<String, PromptValue>;

/// UFO Endurant: a versionable prompt artifact with typed slots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: PromptTemplateId,
    pub body: String,
    pub parameters: Vec<PromptParameterSpec>,
}

impl PromptTemplate {
    pub fn render(&self, args: &PromptArgumentMap) -> Result<String, String> {
        self.validate_args(args)?;
        let mut rendered = self.body.clone();
        for parameter in &self.parameters {
            if let Some(value) = args.get(&parameter.name) {
                rendered = rendered.replace(&format!("{{{}}}", parameter.name), &value.render());
            }
        }
        Ok(rendered)
    }

    pub fn validate_args(&self, args: &PromptArgumentMap) -> Result<(), String> {
        for parameter in &self.parameters {
            let placeholder = format!("{{{}}}", parameter.name);
            if parameter.required && !self.body.contains(&placeholder) {
                return Err(format!(
                    "required parameter '{}' is not referenced by template",
                    parameter.name
                ));
            }
            if parameter.required && !args.contains_key(&parameter.name) {
                return Err(format!(
                    "missing required prompt argument '{}'",
                    parameter.name
                ));
            }
            if let Some(value) = args.get(&parameter.name) {
                validate_prompt_value(parameter, value)?;
            }
        }
        Ok(())
    }
}

impl Stereotyped for PromptTemplate {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Kind("PromptTemplate".into())
    }
}

/// UFO Relator: provider-normalized evidence connecting a photo, a prompt,
/// and downstream consumers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExtractedArtifact {
    Segmented(SegmentedRegion),
    BoundingBox(BoundingBoxRegion),
    TextRegion(TextRegion),
    Structured(StructuredRegion),
}

impl ExtractedArtifact {
    pub fn id(&self) -> &ArtifactId {
        match self {
            ExtractedArtifact::Segmented(region) => &region.artifact_id,
            ExtractedArtifact::BoundingBox(region) => &region.artifact_id,
            ExtractedArtifact::TextRegion(region) => &region.artifact_id,
            ExtractedArtifact::Structured(region) => &region.artifact_id,
        }
    }

    pub fn confidence(&self) -> f32 {
        match self {
            ExtractedArtifact::Segmented(region) => region.score,
            ExtractedArtifact::BoundingBox(region) => region.confidence,
            ExtractedArtifact::TextRegion(region) => region.confidence,
            ExtractedArtifact::Structured(region) => region.confidence,
        }
    }
}

impl Stereotyped for ExtractedArtifact {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Relator("ExtractedArtifact".into())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SegmentedRegion {
    pub artifact_id: ArtifactId,
    pub source_photo_id: PhotoId,
    pub prompt_template_id: PromptTemplateId,
    pub provider: String,
    pub model_id: String,
    pub label: String,
    pub score: f32,
    pub box_xyxy: [f32; 4],
    pub mask: EncodedMask,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundingBoxRegion {
    pub artifact_id: ArtifactId,
    pub source_photo_id: PhotoId,
    pub prompt_template_id: PromptTemplateId,
    pub provider: String,
    pub label: Option<String>,
    pub confidence: f32,
    pub box_xyxy: [f32; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextRegion {
    pub artifact_id: ArtifactId,
    pub source_photo_id: PhotoId,
    pub prompt_template_id: PromptTemplateId,
    pub provider: String,
    pub text: String,
    pub language: Option<String>,
    pub confidence: f32,
    pub box_xyxy: Option<[f32; 4]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuredRegion {
    pub artifact_id: ArtifactId,
    pub source_photo_id: PhotoId,
    pub prompt_template_id: PromptTemplateId,
    pub provider: String,
    pub schema_id: Option<String>,
    pub fields: BTreeMap<String, String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactExtractionManifest {
    pub source_photo_id: PhotoId,
    pub prompt_template_id: PromptTemplateId,
    pub provider: String,
    pub model_id: String,
    pub artifacts: Vec<ExtractedArtifact>,
}

impl ArtifactExtractionManifest {
    pub fn from_sam3_output(
        source_photo_id: PhotoId,
        prompt_template_id: PromptTemplateId,
        output: SamSegmentationOutput,
    ) -> Self {
        let provider = "sam3".to_string();
        let model_id = output.model_id;
        let artifacts = output
            .segments
            .into_iter()
            .enumerate()
            .map(|(index, segment)| {
                ExtractedArtifact::Segmented(SegmentedRegion {
                    artifact_id: ArtifactId(format!(
                        "{}:{}:{}",
                        source_photo_id.0,
                        prompt_template_id.0,
                        index + 1
                    )),
                    source_photo_id: source_photo_id.clone(),
                    prompt_template_id: prompt_template_id.clone(),
                    provider: provider.clone(),
                    model_id: model_id.clone(),
                    label: segment.label,
                    score: segment.score,
                    box_xyxy: segment.box_xyxy,
                    mask: segment.mask,
                })
            })
            .collect();

        Self {
            source_photo_id,
            prompt_template_id,
            provider,
            model_id,
            artifacts,
        }
    }

    pub fn from_ocr_output(
        source_photo_id: PhotoId,
        prompt_template_id: PromptTemplateId,
        output: OcrProviderOutput,
    ) -> Self {
        let provider = output.provider;
        let model_id = output.model_id;
        let artifacts = output
            .regions
            .into_iter()
            .enumerate()
            .map(|(index, region)| {
                ExtractedArtifact::TextRegion(TextRegion {
                    artifact_id: ArtifactId(format!(
                        "{}:{}:{}",
                        source_photo_id.0,
                        prompt_template_id.0,
                        index + 1
                    )),
                    source_photo_id: source_photo_id.clone(),
                    prompt_template_id: prompt_template_id.clone(),
                    provider: provider.clone(),
                    text: region.text,
                    language: region.language,
                    confidence: region.confidence,
                    box_xyxy: region.box_xyxy,
                })
            })
            .collect();

        Self {
            source_photo_id,
            prompt_template_id,
            provider,
            model_id,
            artifacts,
        }
    }

    pub fn compose(
        provider: impl Into<String>,
        model_id: impl Into<String>,
        manifests: impl IntoIterator<Item = ArtifactExtractionManifest>,
    ) -> Result<Self, String> {
        let mut manifests = manifests.into_iter();
        let first = manifests
            .next()
            .ok_or_else(|| "cannot compose an empty artifact manifest set".to_string())?;
        let source_photo_id = first.source_photo_id;
        let prompt_template_id = first.prompt_template_id;
        let mut artifacts = first.artifacts;

        for manifest in manifests {
            if manifest.source_photo_id != source_photo_id {
                return Err(format!(
                    "manifest source photo mismatch: '{}' != '{}'",
                    manifest.source_photo_id.0, source_photo_id.0
                ));
            }
            if manifest.prompt_template_id != prompt_template_id {
                return Err(format!(
                    "manifest prompt template mismatch: '{}' != '{}'",
                    manifest.prompt_template_id.0, prompt_template_id.0
                ));
            }
            artifacts.extend(manifest.artifacts);
        }

        Ok(Self {
            source_photo_id,
            prompt_template_id,
            provider: provider.into(),
            model_id: model_id.into(),
            artifacts,
        })
    }
}

impl Stereotyped for ArtifactExtractionManifest {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Relator("ArtifactExtractionManifest".into())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrProviderOutput {
    pub provider: String,
    pub model_id: String,
    pub regions: Vec<OcrTextRegion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrTextRegion {
    pub text: String,
    pub language: Option<String>,
    pub confidence: f32,
    pub box_xyxy: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoArtifactExtractionRequest {
    pub photo_id: PhotoId,
    pub prompt_template: PromptTemplate,
    pub prompt_args: PromptArgumentMap,
}

impl PhotoArtifactExtractionRequest {
    pub fn rendered_prompt(&self) -> Result<String, String> {
        self.prompt_template.render(&self.prompt_args)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sam3ArtifactExtractionInput {
    pub request: PhotoArtifactExtractionRequest,
    pub image: Sam3ImageSource,
}

/// Adapter boundary: turns b00t's typed extraction request into a SAM3
/// transport request, then normalizes SAM3 output back into b00t artifacts.
#[derive(Debug, Clone)]
pub struct Sam3ArtifactExtractionNode {
    pub sam3: Sam3Node,
    pub output_path: String,
    pub score_threshold: f32,
    pub output_format: MaskOutputFormat,
    pub largest_only: bool,
    pub max_segments: u32,
}

impl Default for Sam3ArtifactExtractionNode {
    fn default() -> Self {
        Self {
            sam3: Sam3Node::default(),
            output_path: "/workspace/result.json".to_string(),
            score_threshold: 0.0,
            output_format: MaskOutputFormat::Rle,
            largest_only: false,
            max_segments: 0,
        }
    }
}

impl Sam3ArtifactExtractionNode {
    pub fn build_request(
        &self,
        input: &Sam3ArtifactExtractionInput,
    ) -> Result<Sam3JobRequest, String> {
        let rendered_prompt = input.request.rendered_prompt()?;
        Ok(Sam3JobRequest {
            image: input.image.clone(),
            output_path: self.output_path.clone(),
            prompts: vec![SegmentationPrompt::Text {
                value: rendered_prompt,
            }],
            model_id: self.sam3.model_id.clone(),
            device: self.sam3.device.clone(),
            score_threshold: self.score_threshold,
            output_format: self.output_format.clone(),
            largest_only: self.largest_only,
            max_segments: self.max_segments,
        })
    }

    pub fn normalize_output(
        &self,
        input: &Sam3ArtifactExtractionInput,
        output: SamSegmentationOutput,
    ) -> ArtifactExtractionManifest {
        ArtifactExtractionManifest::from_sam3_output(
            input.request.photo_id.clone(),
            input.request.prompt_template.id.clone(),
            output,
        )
    }
}

impl PipelineNode for Sam3ArtifactExtractionNode {
    type Input = Sam3ArtifactExtractionInput;
    type Output = ArtifactExtractionManifest;

    fn node_id(&self) -> &str {
        "sam3-artifact-extraction"
    }

    fn node_label(&self) -> &str {
        "SAM3 Artifact Extraction"
    }

    fn node_category(&self) -> NodeCategory {
        NodeCategory::Transform
    }

    fn preconditions(&self) -> Vec<SerializableFOLFormula> {
        vec![]
    }

    fn postconditions(&self) -> Vec<SerializableFOLFormula> {
        vec![]
    }

    fn invariants(&self) -> Vec<SerializableFOLFormula> {
        vec![]
    }

    fn execute(&self, input: Self::Input) -> Self::Output {
        let request = self
            .build_request(&input)
            .expect("typed prompt template should render for SAM3");
        let output = self.sam3.execute(request);
        self.normalize_output(&input, output)
    }

    fn state_machine(&self) -> StateMachine {
        StateMachine::idle_run_cycle("sam3-artifact-extraction")
    }

    fn input_ports(&self) -> Vec<PortDef> {
        vec![
            PortDef {
                name: "request".into(),
                port_type: "PhotoArtifactExtractionRequest".into(),
                direction: PortDirection::Input,
            },
            PortDef {
                name: "image".into(),
                port_type: "Sam3ImageSource".into(),
                direction: PortDirection::Input,
            },
        ]
    }

    fn output_ports(&self) -> Vec<PortDef> {
        vec![PortDef {
            name: "artifacts".into(),
            port_type: "ArtifactExtractionManifest".into(),
            direction: PortDirection::Output,
        }]
    }

    fn visual_style(&self) -> NodeStyle {
        NodeStyle {
            fill: "#141414".into(),
            stroke: "#0ea5e9".into(),
            shape: NodeShape::RoundedBox,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactExtractionConstraint {
    pub min_artifacts: usize,
    pub min_confidence: f64,
}

impl Satisfies<ArtifactExtractionConstraint> for ArtifactExtractionManifest {
    fn satisfies(&self, constraint: &ArtifactExtractionConstraint) -> SatisfiesResult {
        if self.artifacts.len() < constraint.min_artifacts {
            return SatisfiesResult::violated(
                format!(
                    "artifact count {} below required {}",
                    self.artifacts.len(),
                    constraint.min_artifacts
                ),
                1.0,
            );
        }

        let below_threshold = self
            .artifacts
            .iter()
            .filter(|artifact| f64::from(artifact.confidence()) < constraint.min_confidence)
            .count();
        if below_threshold > 0 {
            return SatisfiesResult::violated(
                format!("{below_threshold} artifacts below confidence threshold"),
                1.0,
            );
        }

        SatisfiesResult::satisfied(1.0)
    }
}

fn validate_prompt_value(
    parameter: &PromptParameterSpec,
    value: &PromptValue,
) -> Result<(), String> {
    match (&parameter.kind, value) {
        (PromptParameterKind::Text, PromptValue::Text(_))
        | (PromptParameterKind::Number, PromptValue::Number(_))
        | (PromptParameterKind::Boolean, PromptValue::Boolean(_))
        | (PromptParameterKind::RegionHint, PromptValue::RegionHint(_))
        | (PromptParameterKind::SpeciesHint, PromptValue::SpeciesHint(_)) => Ok(()),
        (PromptParameterKind::Enum { variants }, PromptValue::Enum(value)) => {
            if variants.iter().any(|variant| variant == value) {
                Ok(())
            } else {
                Err(format!("argument '{}' is not one of {:?}", value, variants))
            }
        }
        _ => Err(format!(
            "argument '{}' has wrong type for {:?}",
            parameter.name, parameter.kind
        )),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("sha256:{}", hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_capability::{EncodedMask, Segment, SegmentationTiming};

    fn sam3_fixture_json() -> String {
        let output = SamSegmentationOutput {
            schema_version: "sam3.v1".to_string(),
            model_id: "facebook/sam3".to_string(),
            image_size: [640, 480],
            segments: vec![Segment {
                mask: EncodedMask::Polygon {
                    data: vec![[10.0, 20.0], [30.0, 40.0]],
                },
                score: 0.91,
                label: "sheep".to_string(),
                box_xyxy: [10.0, 20.0, 30.0, 40.0],
            }],
            segment_count: 1,
            timing: SegmentationTiming {
                load_s: 0.1,
                infer_s: 0.2,
            },
        };

        serde_json::to_string_pretty(&output).expect("fixture serializes")
    }

    fn sam3_fixture() -> SamSegmentationOutput {
        serde_json::from_str(&sam3_fixture_json()).expect("generated SAM3 fixture parses")
    }

    fn ocr_fixture_json() -> String {
        let output = OcrProviderOutput {
            provider: "document-ocr".to_string(),
            model_id: "ocr/local-v1".to_string(),
            regions: vec![OcrTextRegion {
                text: "sheep".to_string(),
                language: Some("en".to_string()),
                confidence: 0.97,
                box_xyxy: Some([12.0, 24.0, 90.0, 48.0]),
            }],
        };

        serde_json::to_string_pretty(&output).expect("fixture serializes")
    }

    fn ocr_fixture() -> OcrProviderOutput {
        serde_json::from_str(&ocr_fixture_json()).expect("generated OCR fixture parses")
    }

    fn prompt_template() -> PromptTemplate {
        PromptTemplate {
            id: PromptTemplateId("template:extract-object".to_string()),
            body: "extract {object} from the source photo".to_string(),
            parameters: vec![PromptParameterSpec {
                name: "object".to_string(),
                kind: PromptParameterKind::Text,
                required: true,
            }],
        }
    }

    #[test]
    fn photo_hash_is_content_addressed() {
        let photo = Photo::from_bytes(
            "photo:one",
            Arc::<[u8]>::from([1_u8, 2, 3].as_slice()),
            ImageFormat::Png,
            (1, 1),
        );

        assert_eq!(photo.id.0, "photo:one");
        assert!(photo.sha256.starts_with("sha256:"));
        assert_eq!(photo.ufo_stereotype().to_string(), "Kind:Photo");
    }

    #[test]
    fn prompt_template_renders_typed_arguments() {
        let template = prompt_template();
        let mut args = PromptArgumentMap::new();
        args.insert(
            "object".to_string(),
            PromptValue::Text("dog toy".to_string()),
        );

        let rendered = template.render(&args).expect("valid prompt");

        assert_eq!(rendered, "extract dog toy from the source photo");
        assert_eq!(template.ufo_stereotype().to_string(), "Kind:PromptTemplate");
    }

    #[test]
    fn prompt_template_rejects_missing_required_args() {
        let err = prompt_template()
            .render(&PromptArgumentMap::new())
            .expect_err("missing arg should fail");

        assert!(err.contains("missing required prompt argument 'object'"));
    }

    #[test]
    fn sam3_output_becomes_extracted_artifact_manifest() {
        let output = sam3_fixture();

        let manifest = ArtifactExtractionManifest::from_sam3_output(
            PhotoId("photo:one".to_string()),
            PromptTemplateId("template:sheep".to_string()),
            output,
        );

        assert_eq!(manifest.artifacts.len(), 1);
        assert_eq!(
            manifest.ufo_stereotype().to_string(),
            "Relator:ArtifactExtractionManifest"
        );
        match &manifest.artifacts[0] {
            ExtractedArtifact::Segmented(region) => {
                assert_eq!(region.label, "sheep");
                assert_eq!(region.model_id, "facebook/sam3");
                assert_eq!(region.prompt_template_id.0, "template:sheep");
            }
            other => panic!("unexpected artifact: {other:?}"),
        }
    }

    #[test]
    fn sam3_adapter_builds_request_from_typed_prompt_template() {
        let mut args = PromptArgumentMap::new();
        args.insert("object".to_string(), PromptValue::Text("sheep".to_string()));
        let input = Sam3ArtifactExtractionInput {
            request: PhotoArtifactExtractionRequest {
                photo_id: PhotoId("photo:one".to_string()),
                prompt_template: prompt_template(),
                prompt_args: args,
            },
            image: Sam3ImageSource::Path {
                image_path: "/tmp/photo.png".to_string(),
            },
        };
        let node = Sam3ArtifactExtractionNode {
            output_format: MaskOutputFormat::Polygon,
            score_threshold: 0.5,
            max_segments: 3,
            ..Default::default()
        };

        let request = node.build_request(&input).expect("valid request");

        assert_eq!(request.model_id, "facebook/sam3");
        assert_eq!(request.score_threshold, 0.5);
        assert_eq!(request.max_segments, 3);
        match &request.prompts[0] {
            SegmentationPrompt::Text { value } => {
                assert_eq!(value, "extract sheep from the source photo");
            }
            other => panic!("unexpected prompt: {other:?}"),
        }
    }

    #[test]
    fn sam3_adapter_normalizes_provider_output() {
        let input = Sam3ArtifactExtractionInput {
            request: PhotoArtifactExtractionRequest {
                photo_id: PhotoId("photo:fixture".to_string()),
                prompt_template: prompt_template(),
                prompt_args: PromptArgumentMap::new(),
            },
            image: Sam3ImageSource::Path {
                image_path: "/tmp/photo.png".to_string(),
            },
        };
        let node = Sam3ArtifactExtractionNode::default();

        let manifest = node.normalize_output(&input, sam3_fixture());

        assert_eq!(manifest.provider, "sam3");
        assert_eq!(manifest.artifacts.len(), 1);
        assert_eq!(manifest.prompt_template_id.0, "template:extract-object");
    }

    #[test]
    fn ocr_output_becomes_text_region_artifact_manifest() {
        let manifest = ArtifactExtractionManifest::from_ocr_output(
            PhotoId("photo:receipt".to_string()),
            PromptTemplateId("template:read-labels".to_string()),
            ocr_fixture(),
        );

        assert_eq!(manifest.provider, "document-ocr");
        assert_eq!(manifest.model_id, "ocr/local-v1");
        assert_eq!(manifest.artifacts.len(), 1);
        match &manifest.artifacts[0] {
            ExtractedArtifact::TextRegion(region) => {
                assert_eq!(region.text, "sheep");
                assert_eq!(region.language.as_deref(), Some("en"));
                assert_eq!(region.confidence, 0.97);
                assert_eq!(region.box_xyxy, Some([12.0, 24.0, 90.0, 48.0]));
                assert_eq!(region.prompt_template_id.0, "template:read-labels");
            }
            other => panic!("unexpected artifact: {other:?}"),
        }
    }

    #[test]
    fn compose_merges_sam3_and_ocr_artifacts_for_same_extraction() {
        let photo_id = PhotoId("photo:shared".to_string());
        let template_id = PromptTemplateId("template:shared".to_string());
        let sam3_manifest = ArtifactExtractionManifest::from_sam3_output(
            photo_id.clone(),
            template_id.clone(),
            sam3_fixture(),
        );
        let ocr_manifest =
            ArtifactExtractionManifest::from_ocr_output(photo_id, template_id, ocr_fixture());

        let manifest = ArtifactExtractionManifest::compose(
            "b00t-artifact-compose",
            "sam3+ocr",
            vec![sam3_manifest, ocr_manifest],
        )
        .expect("same extraction manifests compose");

        assert_eq!(manifest.provider, "b00t-artifact-compose");
        assert_eq!(manifest.model_id, "sam3+ocr");
        assert_eq!(manifest.artifacts.len(), 2);
        assert!(matches!(manifest.artifacts[0], ExtractedArtifact::Segmented(_)));
        assert!(matches!(
            manifest.artifacts[1],
            ExtractedArtifact::TextRegion(_)
        ));
    }

    #[test]
    fn compose_rejects_manifest_mismatched_prompt_template() {
        let sam3_manifest = ArtifactExtractionManifest::from_sam3_output(
            PhotoId("photo:shared".to_string()),
            PromptTemplateId("template:sam3".to_string()),
            sam3_fixture(),
        );
        let ocr_manifest = ArtifactExtractionManifest::from_ocr_output(
            PhotoId("photo:shared".to_string()),
            PromptTemplateId("template:ocr".to_string()),
            ocr_fixture(),
        );

        let err = ArtifactExtractionManifest::compose(
            "b00t-artifact-compose",
            "sam3+ocr",
            vec![sam3_manifest, ocr_manifest],
        )
        .expect_err("different prompt templates must not compose");

        assert!(err.contains("manifest prompt template mismatch"));
    }

    #[test]
    fn manifest_satisfies_artifact_count_and_confidence_constraint() {
        let manifest = ArtifactExtractionManifest {
            source_photo_id: PhotoId("photo:one".to_string()),
            prompt_template_id: PromptTemplateId("template:one".to_string()),
            provider: "test".to_string(),
            model_id: "model".to_string(),
            artifacts: vec![ExtractedArtifact::TextRegion(TextRegion {
                artifact_id: ArtifactId("artifact:text".to_string()),
                source_photo_id: PhotoId("photo:one".to_string()),
                prompt_template_id: PromptTemplateId("template:one".to_string()),
                provider: "ocr".to_string(),
                text: "hello".to_string(),
                language: Some("en".to_string()),
                confidence: 0.99,
                box_xyxy: None,
            })],
        };

        let result = manifest.satisfies(&ArtifactExtractionConstraint {
            min_artifacts: 1,
            min_confidence: 0.9,
        });

        assert!(result.is_satisfied());
    }
}
