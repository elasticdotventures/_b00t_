// ── GH #734: Reference transmogrifier implementations for pipeline stages ──
//
// A transmogrifier is an executable pipeline stage: it receives input bytes
// on its input ports, transforms them according to its implementation, and
// produces output bytes on its output ports.
//
// This module provides the `Transmogrifier` trait plus five reference
// implementations covering common media-processing operations (ingest,
// transcode, transcribe, embed, frame-extract).  The `TransmogrifierRegistry`
// aggregates all known transmogrifiers and exposes them as `CapsuleProfile`s
// for pipeline wiring.

use crate::pipeline_types::{
    CapsuleProfile, PortDirection, PortMediaType, ResourceRequirements, StagePort,
};
use std::collections::HashMap;

// ── Trait ───────────────────────────────────────────────────────────────────

/// A pipeline stage that transforms input bytes into output bytes.
///
/// Implementors declare their name, I/O ports, resource requirements, and a
/// `profile()` that bundles these into a `CapsuleProfile` for the pipeline
/// registry.  The `transform` method performs the actual work (or a mock of
/// it for reference implementations).
pub trait Transmogrifier: Send + Sync {
    /// Human-readable stage name (e.g. `"VideoIngest"`, `"WhisperTranscribe"`).
    fn name(&self) -> &str;

    /// Transform input bytes into output bytes.
    ///
    /// `params` carries per-invocation configuration (e.g. codec, bitrate,
    /// model name).  Returns the transformed bytes or a pipeline error.
    fn transform(
        &self,
        input: &[u8],
        params: &HashMap<String, String>,
    ) -> anyhow::Result<Vec<u8>>;

    /// Declare input and output ports for this stage.
    ///
    /// Returns `(input_ports, output_ports)`.  At least one port must be
    /// present in each direction for a well-formed stage.
    fn ports(&self) -> (Vec<StagePort>, Vec<StagePort>);

    /// Resource requirements needed to run this stage.
    fn resources(&self) -> ResourceRequirements;

    /// Full `CapsuleProfile` derived from `name()`, `ports()`, and
    /// `resources()`.  Override to customise `image` or `timeout_seconds`.
    fn profile(&self) -> CapsuleProfile {
        let name = self.name().to_string();
        let (inputs, outputs) = self.ports();
        let mut ports: Vec<StagePort> = inputs;
        ports.extend(outputs);
        CapsuleProfile {
            name,
            ports,
            resources: self.resources(),
            image: None,
            timeout_seconds: None,
        }
    }
}

// ── Reference implementations ───────────────────────────────────────────────

/// Ingest a video file and output raw frame bytes.
///
/// This is a passthrough stage: the input bytes are returned verbatim with
/// stage metadata prepended.  In production, this would wrap a demuxer
/// (ffmpeg, rav1e, etc.) that splits a container into elementary streams.
pub struct VideoIngest;

impl Transmogrifier for VideoIngest {
    fn name(&self) -> &str {
        "VideoIngest"
    }

    fn transform(
        &self,
        input: &[u8],
        _params: &HashMap<String, String>,
    ) -> anyhow::Result<Vec<u8>> {
        // Passthrough: return input bytes unchanged, prepended with metadata
        // header for test verifiability.
        let meta = format!(
            "metadata: stage=VideoIngest input_bytes={}\n",
            input.len()
        );
        let mut output = meta.into_bytes();
        output.extend_from_slice(input);
        Ok(output)
    }

    fn ports(&self) -> (Vec<StagePort>, Vec<StagePort>) {
        (
            vec![StagePort {
                direction: PortDirection::Input,
                media_type: PortMediaType::Video,
                description: Some("Raw video file bytes".into()),
            }],
            vec![StagePort {
                direction: PortDirection::Output,
                media_type: PortMediaType::Bytes,
                description: Some("Raw frame bytes with metadata prefix".into()),
            }],
        )
    }

    fn resources(&self) -> ResourceRequirements {
        ResourceRequirements {
            min_ram_gb: 2.0,
            min_vram_gb: 0.0,
            requires_gpu: false,
            cpu_cores: Some(2),
            scratch_disk_gb: Some(4.0),
        }
    }
}

/// Transcode a video stream (mock implementation).
///
/// Accepts video input and passthrough parameters (codec, bitrate, resolution)
/// as `params`.  The reference implementation echoes the input bytes; a real
/// stage would invoke an encoder (ffmpeg, x264, NVENC).
///
/// Requires GPU resources for hardware-accelerated encoding.
pub struct Transcode;

impl Transmogrifier for Transcode {
    fn name(&self) -> &str {
        "Transcode"
    }

    fn transform(
        &self,
        input: &[u8],
        params: &HashMap<String, String>,
    ) -> anyhow::Result<Vec<u8>> {
        // Mock: prepend a header with the transcode parameters so tests can
        // verify parameter propagation.
        let mut header = format!(
            "transcoded: input_bytes={}",
            input.len(),
        );
        if let Some(codec) = params.get("codec") {
            header.push_str(&format!(" codec={codec}"));
        }
        if let Some(bitrate) = params.get("bitrate") {
            header.push_str(&format!(" bitrate={bitrate}"));
        }
        header.push('\n');

        let mut output = header.into_bytes();
        output.extend_from_slice(input);
        Ok(output)
    }

    fn ports(&self) -> (Vec<StagePort>, Vec<StagePort>) {
        (
            vec![StagePort {
                direction: PortDirection::Input,
                media_type: PortMediaType::Video,
                description: Some("Input video stream".into()),
            }],
            vec![StagePort {
                direction: PortDirection::Output,
                media_type: PortMediaType::Video,
                description: Some("Transcoded video stream".into()),
            }],
        )
    }

    fn resources(&self) -> ResourceRequirements {
        ResourceRequirements {
            min_ram_gb: 4.0,
            min_vram_gb: 8.0,
            requires_gpu: true,
            cpu_cores: Some(4),
            scratch_disk_gb: Some(10.0),
        }
    }
}

/// Transcribe audio to structured JSON (mock implementation).
///
/// Mimics a Whisper-style speech-to-text model.  The mock returns a JSON
/// payload with `text`, `segments`, `language`, and `duration` fields.
pub struct WhisperTranscribe;

impl Transmogrifier for WhisperTranscribe {
    fn name(&self) -> &str {
        "WhisperTranscribe"
    }

    fn transform(
        &self,
        input: &[u8],
        params: &HashMap<String, String>,
    ) -> anyhow::Result<Vec<u8>> {
        // Produce a structured JSON response that mimics Whisper output.
        let input_duration_s = (input.len() as f64 / 16000.0 * 100.0).round() / 100.0;
        let language = params
            .get("language")
            .map(|s| s.as_str())
            .unwrap_or("en");

        let json = serde_json::json!({
            "text": "This is a mock transcription of the provided audio input.",
            "segments": [
                {
                    "start": 0.0,
                    "end": input_duration_s,
                    "text": "This is a mock transcription of the provided audio input.",
                    "confidence": 0.95,
                }
            ],
            "language": language,
            "duration": input_duration_s,
        });

        serde_json::to_vec(&json).map_err(anyhow::Error::from)
    }

    fn ports(&self) -> (Vec<StagePort>, Vec<StagePort>) {
        (
            vec![StagePort {
                direction: PortDirection::Input,
                media_type: PortMediaType::Audio,
                description: Some("Raw audio waveform bytes".into()),
            }],
            vec![StagePort {
                direction: PortDirection::Output,
                media_type: PortMediaType::Json,
                description: Some("Whisper-style transcription JSON".into()),
            }],
        )
    }

    fn resources(&self) -> ResourceRequirements {
        ResourceRequirements {
            min_ram_gb: 2.0,
            min_vram_gb: 4.0,
            requires_gpu: false, // Whisper small/tiny can run on CPU
            cpu_cores: Some(4),
            scratch_disk_gb: Some(2.0),
        }
    }
}

/// Generate text embeddings (mock implementation).
///
/// Accepts JSON input with a `text` field and returns a JSON payload
/// containing a `vector` (embedding), `dimension`, and `model` name.
///
/// In production this would call a model (bert, sentence-transformers, etc.).
pub struct Embed;

impl Transmogrifier for Embed {
    fn name(&self) -> &str {
        "Embed"
    }

    fn transform(
        &self,
        _input: &[u8],
        params: &HashMap<String, String>,
    ) -> anyhow::Result<Vec<u8>> {
        // Produce a mock embedding vector of dimension 384.
        let dimension: usize = params
            .get("dimension")
            .and_then(|v| v.parse().ok())
            .unwrap_or(384);
        let model = params
            .get("model")
            .map(|s| s.as_str())
            .unwrap_or("mock-embedding-model");

        // Deterministic mock vector based on dimension (all 0.1 values).
        let vector: Vec<f64> = std::iter::repeat(0.1).take(dimension).collect();

        let json = serde_json::json!({
            "vector": vector,
            "dimension": dimension,
            "model": model,
            "tokens": 42,
        });

        serde_json::to_vec(&json).map_err(anyhow::Error::from)
    }

    fn ports(&self) -> (Vec<StagePort>, Vec<StagePort>) {
        (
            vec![StagePort {
                direction: PortDirection::Input,
                media_type: PortMediaType::Json,
                description: Some("JSON with `text` field to embed".into()),
            }],
            vec![StagePort {
                direction: PortDirection::Output,
                media_type: PortMediaType::Json,
                description: Some("Embedding vector JSON with `vector` field".into()),
            }],
        )
    }

    fn resources(&self) -> ResourceRequirements {
        ResourceRequirements {
            min_ram_gb: 2.0,
            min_vram_gb: 0.0,
            requires_gpu: false,
            cpu_cores: Some(2),
            scratch_disk_gb: None,
        }
    }
}

/// Extract the first frame from a video (mock implementation).
///
/// Returns the first N bytes of the video as a mock "frame".  In production
/// this would decode the video container, seek to the first keyframe, and
/// return a decoded image (PNG/JPEG) via ffmpeg or similar.
pub struct FrameExtract;

impl Transmogrifier for FrameExtract {
    fn name(&self) -> &str {
        "FrameExtract"
    }

    fn transform(
        &self,
        input: &[u8],
        _params: &HashMap<String, String>,
    ) -> anyhow::Result<Vec<u8>> {
        // Return first frame: mock extraction — take first 1024 bytes as
        // the "frame".  Real impl would decode and extract.
        let frame_size = input.len().min(1024);
        let frame = &input[..frame_size];
        let mut output = Vec::with_capacity(frame.len() + 64);
        output.extend_from_slice(b"frame:extracted first frame\n");
        output.extend_from_slice(frame);
        Ok(output)
    }

    fn ports(&self) -> (Vec<StagePort>, Vec<StagePort>) {
        (
            vec![StagePort {
                direction: PortDirection::Input,
                media_type: PortMediaType::Video,
                description: Some("Input video stream".into()),
            }],
            vec![StagePort {
                direction: PortDirection::Output,
                media_type: PortMediaType::Image,
                description: Some("Extracted frame as image bytes".into()),
            }],
        )
    }

    fn resources(&self) -> ResourceRequirements {
        ResourceRequirements {
            min_ram_gb: 1.0,
            min_vram_gb: 0.0,
            requires_gpu: false,
            cpu_cores: Some(2),
            scratch_disk_gb: Some(1.0),
        }
    }
}

// ── Registry ────────────────────────────────────────────────────────────────

/// An in-memory registry of named `Transmogrifier` implementations.
///
/// Use `TransmogrifierRegistry::builtin()` to obtain a registry pre-populated
/// with all reference implementations, or construct an empty registry and
/// `register()` custom stages.
pub struct TransmogrifierRegistry {
    transmogrifiers: HashMap<String, Box<dyn Transmogrifier>>,
}

impl TransmogrifierRegistry {
    /// Create an empty registry.
    pub fn empty() -> Self {
        Self {
            transmogrifiers: HashMap::new(),
        }
    }

    /// Create a registry pre-populated with all built-in transmogrifiers.
    pub fn builtin() -> Self {
        let mut reg = Self::empty();
        reg.register("VideoIngest", Box::new(VideoIngest));
        reg.register("Transcode", Box::new(Transcode));
        reg.register("WhisperTranscribe", Box::new(WhisperTranscribe));
        reg.register("Embed", Box::new(Embed));
        reg.register("FrameExtract", Box::new(FrameExtract));
        reg
    }

    /// Retrieve a transmogrifier by name.
    pub fn get(&self, name: &str) -> Option<&dyn Transmogrifier> {
        self.transmogrifiers.get(name).map(|b| b.as_ref())
    }

    /// Register a named transmogrifier.
    ///
    /// If a transmogrifier with the same name already exists, it is replaced.
    pub fn register(&mut self, name: &str, t: Box<dyn Transmogrifier>) {
        self.transmogrifiers.insert(name.to_string(), t);
    }

    /// Return `CapsuleProfile` for every registered transmogrifier.
    ///
    /// Useful for wiring stages into a `PipelineDag` or listing available
    /// stages in a UI.
    pub fn all_stages(&self) -> Vec<CapsuleProfile> {
        let mut profiles: Vec<CapsuleProfile> = self
            .transmogrifiers
            .values()
            .map(|t| t.profile())
            .collect();
        profiles.sort_by(|a, b| a.name.cmp(&b.name));
        profiles
    }

    /// Number of registered transmogrifiers.
    pub fn len(&self) -> usize {
        self.transmogrifiers.len()
    }

    /// Returns `true` if no transmogrifiers are registered.
    pub fn is_empty(&self) -> bool {
        self.transmogrifiers.is_empty()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// All five built-in transmogrifiers must be registered.
    #[test]
    fn builtin_all_registered() {
        let reg = TransmogrifierRegistry::builtin();
        assert_eq!(reg.len(), 5, "expected 5 built-in transmogrifiers");

        let expected = [
            "VideoIngest",
            "Transcode",
            "WhisperTranscribe",
            "Embed",
            "FrameExtract",
        ];
        for name in &expected {
            assert!(
                reg.get(name).is_some(),
                "built-in transmogrifier '{name}' not found"
            );
        }
    }

    /// VideoIngest's `transform` must return bytes (passthrough with metadata).
    #[test]
    fn video_ingest_returns_bytes() {
        let ingest = VideoIngest;
        let input = b"fake video bytes";
        let params = HashMap::new();
        let result = ingest.transform(input, &params).unwrap();

        assert!(!result.is_empty(), "VideoIngest output must not be empty");
        // Metadata prefix should be present.
        let meta_end = result
            .windows(1)
            .position(|w| w == b"\n")
            .map(|i| i + 1)
            .unwrap_or(0);
        let meta = std::str::from_utf8(&result[..meta_end]).unwrap();
        assert!(
            meta.contains("VideoIngest"),
            "metadata should mention VideoIngest: {meta}"
        );
        assert!(
            meta.contains("input_bytes"),
            "metadata should contain input_bytes: {meta}"
        );
        // Passthrough bytes follow the metadata line.
        assert!(
            result.ends_with(b"fake video bytes"),
            "output should end with original input bytes"
        );
    }

    /// Transcode stage must declare `requires_gpu: true`.
    #[test]
    fn transcode_requires_gpu() {
        let transcode = Transcode;
        let res = transcode.resources();
        assert!(
            res.requires_gpu,
            "Transcode should require GPU (requires_gpu = true)"
        );
        assert!(
            res.min_vram_gb > 0.0,
            "Transcode should require VRAM > 0"
        );
    }

    /// WhisperTranscribe output must be valid JSON with expected fields.
    #[test]
    fn whisper_transcribe_outputs_json() {
        let wt = WhisperTranscribe;
        let input = b"mock audio data";
        let params = HashMap::new();
        let result = wt.transform(input, &params).unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&result)
            .expect("WhisperTranscribe output must be valid JSON");

        assert!(
            parsed.get("text").is_some(),
            "JSON must contain 'text' field"
        );
        assert!(
            parsed.get("segments").is_some(),
            "JSON must contain 'segments' field"
        );
        assert!(
            parsed.get("language").is_some(),
            "JSON must contain 'language' field"
        );
        assert!(
            parsed.get("duration").is_some(),
            "JSON must contain 'duration' field"
        );
        assert_eq!(
            parsed["language"], "en",
            "default language should be 'en'"
        );
    }

    /// WhisperTranscribe respects the `language` parameter.
    #[test]
    fn whisper_transcribe_respects_language_param() {
        let wt = WhisperTranscribe;
        let input = b"mock audio data";
        let mut params = HashMap::new();
        params.insert("language".to_string(), "fr".to_string());
        let result = wt.transform(input, &params).unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&result).unwrap();
        assert_eq!(parsed["language"], "fr");
    }

    /// Embed output must be valid JSON with a `vector` array.
    #[test]
    fn embed_output_valid_json_with_vector() {
        let embed = Embed;
        let input = br#"{"text": "hello world"}"#;
        let params = HashMap::new();
        let result = embed.transform(input, &params).unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&result)
            .expect("Embed output must be valid JSON");

        let vector = parsed
            .get("vector")
            .and_then(|v| v.as_array())
            .expect("JSON must contain 'vector' as an array");

        assert!(!vector.is_empty(), "vector must not be empty");
        assert_eq!(
            parsed["dimension"], 384,
            "default dimension should be 384"
        );
        assert_eq!(
            vector.len(),
            384,
            "vector length should match dimension"
        );
        assert!(
            vector.iter().all(|v| v.as_f64() == Some(0.1)),
            "mock vector entries should all be 0.1"
        );
    }

    /// Embed respects the `dimension` parameter.
    #[test]
    fn embed_respects_dimension_param() {
        let embed = Embed;
        let input = br#"{"text": "test"}"#;
        let mut params = HashMap::new();
        params.insert("dimension".to_string(), "128".to_string());
        let result = embed.transform(input, &params).unwrap();

        let parsed: serde_json::Value = serde_json::from_slice(&result).unwrap();
        let vector = parsed["vector"].as_array().unwrap();
        assert_eq!(vector.len(), 128);
        assert_eq!(parsed["dimension"], 128);
    }

    /// FrameExtract output must be bytes (mock first-frame extraction).
    #[test]
    fn frame_extract_output_is_bytes() {
        let fe = FrameExtract;
        let input = b"fake video content bytes for frame extraction";
        let params = HashMap::new();
        let result = fe.transform(input, &params).unwrap();

        assert!(!result.is_empty(), "FrameExtract output must not be empty");
        // Should start with the frame header.
        assert!(
            result.starts_with(b"frame:"),
            "output should start with 'frame:' prefix, got: {:?}",
            &result[..8.min(result.len())]
        );
        // Should end with the original frame content.
        assert!(
            result.ends_with(b"fake video content bytes for frame extraction"),
            "output should include input bytes"
        );
    }

    /// Registry round-trip: get returns the same transmogrifier.
    #[test]
    fn registry_get_round_trip() {
        let reg = TransmogrifierRegistry::builtin();
        let t = reg.get("VideoIngest").unwrap();
        assert_eq!(t.name(), "VideoIngest");
    }

    /// Registry get on unknown name returns None.
    #[test]
    fn registry_get_unknown_returns_none() {
        let reg = TransmogrifierRegistry::builtin();
        assert!(reg.get("NonExistent").is_none());
    }

    /// Registry all_stages returns profiles matching registered names.
    #[test]
    fn registry_all_stages_returns_profiles() {
        let reg = TransmogrifierRegistry::builtin();
        let profiles = reg.all_stages();
        assert_eq!(profiles.len(), 5);

        let names: Vec<&str> = profiles.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"VideoIngest"));
        assert!(names.contains(&"Transcode"));
        assert!(names.contains(&"WhisperTranscribe"));
        assert!(names.contains(&"Embed"));
        assert!(names.contains(&"FrameExtract"));
    }

    /// Register and retrieve a custom transmogrifier.
    #[test]
    fn registry_register_custom() {
        struct CustomStage;
        impl Transmogrifier for CustomStage {
            fn name(&self) -> &str { "CustomStage" }
            fn transform(&self, input: &[u8], _: &HashMap<String, String>) -> anyhow::Result<Vec<u8>> {
                let mut out = b"custom:".to_vec();
                out.extend_from_slice(input);
                Ok(out)
            }
            fn ports(&self) -> (Vec<StagePort>, Vec<StagePort>) {
                (
                    vec![StagePort { direction: PortDirection::Input, media_type: PortMediaType::Bytes, description: None }],
                    vec![StagePort { direction: PortDirection::Output, media_type: PortMediaType::Bytes, description: None }],
                )
            }
            fn resources(&self) -> ResourceRequirements {
                ResourceRequirements {
                    min_ram_gb: 0.5, min_vram_gb: 0.0, requires_gpu: false,
                    cpu_cores: None, scratch_disk_gb: None,
                }
            }
        }

        let mut reg = TransmogrifierRegistry::empty();
        reg.register("CustomStage", Box::new(CustomStage));

        let t = reg.get("CustomStage").unwrap();
        assert_eq!(t.name(), "CustomStage");

        let result = t.transform(b"data", &HashMap::new()).unwrap();
        assert!(result.starts_with(b"custom:"));
    }

    /// Empty registry has no stages.
    #[test]
    fn empty_registry() {
        let reg = TransmogrifierRegistry::empty();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(reg.all_stages().is_empty());
    }

    /// Each built-in stage returns a valid CapsuleProfile with name set.
    #[test]
    fn each_stage_has_valid_profile() {
        let reg = TransmogrifierRegistry::builtin();
        for profile in reg.all_stages() {
            assert!(!profile.name.is_empty(), "profile name must not be empty");
            assert!(
                !profile.ports.is_empty(),
                "profile '{}' must have at least one port",
                profile.name
            );
        }
    }

    /// TransmogrifierRegistry satisfies `Send` (important for pipeline exec).
    #[test]
    fn registry_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TransmogrifierRegistry>();
    }

    /// VideoIngest port types are correct.
    #[test]
    fn video_ingest_correct_ports() {
        let ingest = VideoIngest;
        let (inputs, outputs) = ingest.ports();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].media_type, PortMediaType::Video);
        assert_eq!(inputs[0].direction, PortDirection::Input);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].media_type, PortMediaType::Bytes);
        assert_eq!(outputs[0].direction, PortDirection::Output);
    }

    /// Transcode resource requirements include GPU.
    #[test]
    fn transcode_resources_include_gpu_and_vram() {
        let transcode = Transcode;
        let res = transcode.resources();
        assert!(res.requires_gpu);
        assert_eq!(res.min_vram_gb, 8.0);
        assert_eq!(res.min_ram_gb, 4.0);
    }
}
