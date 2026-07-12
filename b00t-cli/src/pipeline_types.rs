use crate::pipeline_secrets::SecretRef;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ── GH #719: Port types ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PortMediaType {
    Video,
    Audio,
    Image,
    Json,
    Parquet,
    Bytes,
    Error,
}

impl PortMediaType {
    pub fn mime_type(&self) -> &'static str {
        match self {
            PortMediaType::Video => "video/mp4",
            PortMediaType::Audio => "audio/wav",
            PortMediaType::Image => "image/png",
            PortMediaType::Json => "application/json",
            PortMediaType::Parquet => "application/x-parquet",
            PortMediaType::Bytes => "application/octet-stream",
            PortMediaType::Error => "application/x-error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StagePort {
    pub direction: PortDirection,
    pub media_type: PortMediaType,
    pub description: Option<String>,
}

impl StagePort {
    pub fn compatible_with(&self, other: &StagePort) -> bool {
        self.direction != other.direction
            && (self.media_type == other.media_type
                || matches!((&self.media_type, &other.media_type), (PortMediaType::Bytes, _) | (_, PortMediaType::Bytes)))
    }
}

// ── GH #720: Resource types ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub min_ram_gb: f64,
    pub min_vram_gb: f64,
    pub requires_gpu: bool,
    pub cpu_cores: Option<u32>,
    pub scratch_disk_gb: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostResources {
    pub ram_gb: f64,
    pub vram_gb: f64,
    pub gpu_count: u32,
    pub cpu_cores: u32,
}

pub trait ResourceFit {
    fn fits_on(&self, available: &HostResources) -> bool;
}

impl ResourceFit for ResourceRequirements {
    fn fits_on(&self, available: &HostResources) -> bool {
        if self.min_ram_gb > available.ram_gb {
            return false;
        }
        if self.requires_gpu && available.gpu_count == 0 {
            return false;
        }
        if self.min_vram_gb > available.vram_gb {
            return false;
        }
        if let Some(cores) = self.cpu_cores {
            if cores > available.cpu_cores {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapsuleProfile {
    pub name: String,
    pub ports: Vec<StagePort>,
    pub resources: ResourceRequirements,
    pub image: Option<String>,
    pub timeout_seconds: Option<u64>,
}

// ── GH #722: Error types ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PipelineError {
    InputValidation(String),
    ResourceExhausted { needed: String, available: String },
    StageCrashed(String),
    Timeout { stage: String, elapsed_ms: u64 },
    MediaTypeMismatch { expected: PortMediaType, got: PortMediaType },
    TranscodeError(String),
}

impl From<anyhow::Error> for PipelineError {
    fn from(err: anyhow::Error) -> Self {
        PipelineError::InputValidation(err.to_string())
    }
}

impl From<String> for PipelineError {
    fn from(msg: String) -> Self {
        PipelineError::InputValidation(msg)
    }
}

impl PipelineError {
    pub fn variant_name(&self) -> &str {
        match self {
            PipelineError::InputValidation(_) => "InputValidation",
            PipelineError::ResourceExhausted { .. } => "ResourceExhausted",
            PipelineError::StageCrashed(_) => "StageCrashed",
            PipelineError::Timeout { .. } => "Timeout",
            PipelineError::MediaTypeMismatch { .. } => "MediaTypeMismatch",
            PipelineError::TranscodeError(_) => "TranscodeError",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorRoute {
    pub match_pattern: String,
    pub route_to_stage: String,
    pub max_retries: u32,
    pub backoff_ms: u64,
    pub fallback_output: Option<StagePort>,
    #[serde(skip)]
    pub retry_count: u32,
}

impl ErrorRoute {
    pub fn matches(&self, error: &PipelineError) -> bool {
        glob_match(&self.match_pattern, error.variant_name())
    }

    pub fn retries_left(&self) -> u32 {
        self.max_retries.saturating_sub(self.retry_count)
    }

    pub fn record_retry(&mut self) {
        self.retry_count = self.retry_count.saturating_add(1);
    }

    pub fn can_retry(&self) -> bool {
        self.retries_left() > 0
    }
}

// ── GH #721: StageSpec ──

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageSpec {
    pub name: String,
    pub profile: CapsuleProfile,
    pub input_ports: Vec<StagePort>,
    pub output_ports: Vec<StagePort>,
    pub error_routes: Vec<ErrorRoute>,
    pub env: Option<HashMap<String, String>>,
    pub checkpoint_interval_seconds: Option<u64>,
    pub secret_refs: Option<Vec<SecretRef>>,
}

impl StageSpec {
    pub fn from_name(name: &str) -> Self {
        StageSpec {
            name: name.to_string(),
            profile: CapsuleProfile {
                name: name.to_string(),
                ports: vec![],
                resources: ResourceRequirements {
                    min_ram_gb: 0.0,
                    min_vram_gb: 0.0,
                    requires_gpu: false,
                    cpu_cores: None,
                    scratch_disk_gb: None,
                },
                image: None,
                timeout_seconds: None,
            },
            input_ports: vec![StagePort {
                direction: PortDirection::Input,
                media_type: PortMediaType::Bytes,
                description: Some("auto-generated".into()),
            }],
            output_ports: vec![StagePort {
                direction: PortDirection::Output,
                media_type: PortMediaType::Bytes,
                description: Some("auto-generated".into()),
            }],
            error_routes: vec![],
            env: None,
            checkpoint_interval_seconds: None,
            secret_refs: None,
        }
    }
}

/// Backward-compatible stage entry: plain string or full StageSpec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StageEntry {
    Name(String),
    Spec(StageSpec),
}

impl StageEntry {
    pub fn resolve(self) -> StageSpec {
        match self {
            StageEntry::Name(name) => StageSpec::from_name(&name),
            StageEntry::Spec(spec) => spec,
        }
    }
}

fn glob_match(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else {
        pattern == name
    }
}

// ── GH #723: StagePort compatibility negotiation ──

/// Result of negotiating a connection between two stage ports.
///
/// `Direct` — ports match natively (same type or Bytes-wildcard).
/// `Convertible` — a known conversion stage bridges the media types.
/// `Incompatible` — no path exists; reason describes the gap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NegotiationResult {
    Direct,
    Convertible {
        /// Name of the conversion stage to insert (e.g. "ffmpeg-frame-extract").
        via_stage: String,
        /// Whether the conversion is lossy (may degrade quality/fidelity).
        lossy: bool,
    },
    Incompatible(String),
}

/// A registered conversion mapping between two port media types.
struct ConversionEntry {
    stage_name: &'static str,
    lossy: bool,
}

/// Build the default conversion registry.
///
/// Maps `(output_media_type, input_media_type)` to a conversion stage
/// that can bridge the two.  The registry is intentionally limited:
/// all pipelines eventually pass through `Bytes` for transport, and
/// the `compatible_with` check already handles `Bytes` as a wildcard.
fn default_conversions() -> Vec<((PortMediaType, PortMediaType), ConversionEntry)> {
    vec![
        // Video → Image: ffmpeg frame extraction (lossy: drops temporal dimension)
        (
            (PortMediaType::Video, PortMediaType::Image),
            ConversionEntry { stage_name: "ffmpeg-frame-extract", lossy: true },
        ),
        // Audio → Json: speech-to-text transcript (typically non-lossy for content)
        (
            (PortMediaType::Audio, PortMediaType::Json),
            ConversionEntry { stage_name: "whisper-transcript", lossy: false },
        ),
        // Json → Parquet: columnar sink for structured data (non-lossy)
        (
            (PortMediaType::Json, PortMediaType::Parquet),
            ConversionEntry { stage_name: "parquet-sink", lossy: false },
        ),
        // Video → Bytes: raw bitstream packaging (non-lossy)
        (
            (PortMediaType::Video, PortMediaType::Bytes),
            ConversionEntry { stage_name: "video-to-bytes", lossy: false },
        ),
        // Image → Bytes: raw pixel/serialized packaging (non-lossy)
        (
            (PortMediaType::Image, PortMediaType::Bytes),
            ConversionEntry { stage_name: "image-to-bytes", lossy: false },
        ),
        // Audio → Bytes: raw waveform packaging (non-lossy)
        (
            (PortMediaType::Audio, PortMediaType::Bytes),
            ConversionEntry { stage_name: "audio-to-bytes", lossy: false },
        ),
    ]
}

/// Determine whether an output port can be connected to an input port.
///
/// Returns:
/// - `Direct` if the ports match natively (same type or Bytes wildcard).
/// - `Convertible` if a known conversion stage bridges the media types.
/// - `Incompatible` if no path exists (with a diagnostic reason).
pub fn can_negotiate(out_port: &StagePort, inp_port: &StagePort) -> NegotiationResult {
    // Direct path: compatible_with handles same-type and Bytes ↔ anything.
    if out_port.compatible_with(inp_port) {
        return NegotiationResult::Direct;
    }

    // Same-direction ports cannot be wired together.
    if out_port.direction == inp_port.direction {
        return NegotiationResult::Incompatible(
            "ports have the same direction (both input or both output)".into(),
        );
    }

    // Check the conversion registry for a (from_media, to_media) mapping.
    let from = &out_port.media_type;
    let to = &inp_port.media_type;
    for ((src, dst), entry) in default_conversions() {
        if src == *from && dst == *to {
            return NegotiationResult::Convertible {
                via_stage: entry.stage_name.to_string(),
                lossy: entry.lossy,
            };
        }
    }

    NegotiationResult::Incompatible(format!(
        "no conversion from {:?} to {:?}",
        out_port.media_type, inp_port.media_type
    ))
}

/// Insert automatic conversion stages between mismatched adjacent ports.
///
/// Walks `stages` in order.  For each adjacent pair `(stage[i], stage[i+1])`,
/// negotiates the first output port of `stage[i]` with the first input port
/// of `stage[i+1]`.  When a `Convertible` result is found, a new `StageSpec`
/// for the conversion stage is inserted between them.
///
/// Direct matches are left untouched.  Incompatible pairs are left as-is
/// (the caller must handle them).
pub fn auto_insert_conversions(stages: &mut Vec<StageSpec>) {
    let mut i = 0;
    while i + 1 < stages.len() {
        // Clone media types to avoid borrow conflicts with the mutable Vec.
        let prev_out_media = stages[i]
            .output_ports
            .first()
            .map(|p| p.media_type.clone())
            .unwrap_or(PortMediaType::Bytes);
        let next_in_media = stages[i + 1]
            .input_ports
            .first()
            .map(|p| p.media_type.clone())
            .unwrap_or(PortMediaType::Bytes);

        let prev_port = StagePort {
            direction: PortDirection::Output,
            media_type: prev_out_media,
            description: None,
        };
        let next_port = StagePort {
            direction: PortDirection::Input,
            media_type: next_in_media,
            description: None,
        };

        match can_negotiate(&prev_port, &next_port) {
            NegotiationResult::Direct => {
                i += 1;
            }
            NegotiationResult::Convertible { via_stage, lossy } => {
                let conv_stage = StageSpec {
                    name: format!("auto-conv-{via_stage}"),
                    profile: CapsuleProfile {
                        name: format!("auto-conv-{via_stage}"),
                        ports: vec![],
                        resources: ResourceRequirements {
                            min_ram_gb: 0.0,
                            min_vram_gb: 0.0,
                            requires_gpu: false,
                            cpu_cores: None,
                            scratch_disk_gb: None,
                        },
                        image: None,
                        timeout_seconds: None,
                    },
                    secret_refs: None,
                    input_ports: vec![StagePort {
                        direction: PortDirection::Input,
                        media_type: prev_port.media_type.clone(),
                        description: Some("auto-conv input".into()),
                    }],
                    output_ports: vec![StagePort {
                        direction: PortDirection::Output,
                        media_type: next_port.media_type.clone(),
                        description: Some(format!("auto-conv output (lossy: {lossy})")),
                    }],
                    error_routes: vec![],
                    env: None,
                    checkpoint_interval_seconds: None,
                };
                stages.insert(i + 1, conv_stage);
                i += 2; // skip past the inserted conversion stage
            }
            NegotiationResult::Incompatible(_) => {
                i += 1;
            }
        }
    }
}

// ── GH #724: Pipeline DAG ──

/// Edge connecting two stages in a pipeline DAG.
///
/// `from` and `to` refer to stage names.  `via_port` records which output
/// port on the source stage matched the destination's input port during
/// automatic wiring.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineEdge {
    pub from: String,
    pub to: String,
    pub via_port: Option<StagePort>,
}

/// A directed acyclic graph representation of a pipeline.
///
/// Built automatically from a `Vec<StageSpec>` by matching output ports
/// to compatible input ports across all stages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineDag {
    pub stages: Vec<StageSpec>,
    pub edges: Vec<PipelineEdge>,
    pub entry_points: Vec<String>,
    pub exit_points: Vec<String>,
}

impl PipelineDag {
    /// Build a DAG from a list of stage specs.
    ///
    /// For every ordered pair of stages `(i, j)` where `i != j`, the builder
    /// checks each output port of `i` against each input port of `j` via
    /// `StagePort::compatible_with`.  The first compatible pair produces an
    /// edge `i → j`.
    ///
    /// After wiring, a topological sort (Kahn's algorithm) confirms the
    /// graph is acyclic.  Entry points (no incoming edges) and exit points
    /// (no outgoing edges) are inferred automatically.
    pub fn build(stages: Vec<StageSpec>) -> anyhow::Result<Self> {
        // Check for duplicate stage names.
        let mut seen = HashSet::new();
        for stage in &stages {
            if !seen.insert(stage.name.as_str()) {
                anyhow::bail!("duplicate stage name: {}", stage.name);
            }
        }

        // Wire edges by matching output ports → input ports.
        let mut edges = Vec::new();
        for i in 0..stages.len() {
            for j in 0..stages.len() {
                if i == j {
                    continue;
                }
                // Find the first compatible output→input pair.
                let mut matched: Option<StagePort> = None;
                'outer: for out_port in &stages[i].output_ports {
                    for in_port in &stages[j].input_ports {
                        if out_port.compatible_with(in_port) {
                            matched = Some(out_port.clone());
                            break 'outer;
                        }
                    }
                }
                if let Some(port) = matched {
                    edges.push(PipelineEdge {
                        from: stages[i].name.clone(),
                        to: stages[j].name.clone(),
                        via_port: Some(port),
                    });
                }
            }
        }

        // Entry points: stages with no incoming edges.
        let entry_points: Vec<String> = stages
            .iter()
            .map(|s| s.name.clone())
            .filter(|name| !edges.iter().any(|e| e.to == *name))
            .collect();

        // Exit points: stages with no outgoing edges.
        let exit_points: Vec<String> = stages
            .iter()
            .map(|s| s.name.clone())
            .filter(|name| !edges.iter().any(|e| e.from == *name))
            .collect();

        let dag = PipelineDag {
            stages,
            edges,
            entry_points,
            exit_points,
        };

        // Topological sort validates acyclicity.
        dag.topological_sort()?;

        Ok(dag)
    }

    /// Internal topological sort via Kahn's algorithm.
    ///
    /// Returns the stage names in execution order, or an error if a cycle
    /// is detected.
    fn topological_sort(&self) -> anyhow::Result<Vec<String>> {
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

        // Initialise all stages with in-degree 0 and an empty adjacency list.
        for stage in &self.stages {
            in_degree.entry(stage.name.as_str()).or_insert(0);
            adj.entry(stage.name.as_str()).or_default();
        }

        // Populate adjacency and in-degrees from edges.
        for edge in &self.edges {
            adj.get_mut(edge.from.as_str())
                .unwrap()
                .push(&edge.to);
            *in_degree.get_mut(edge.to.as_str()).unwrap() += 1;
        }

        // Seed the queue with nodes that have no dependencies.
        let mut queue: Vec<&str> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(&name, _)| name)
            .collect();

        let mut order = Vec::new();
        while let Some(node) = queue.pop() {
            order.push(node.to_string());
            for &next in adj.get(node).unwrap_or(&vec![]) {
                let deg = in_degree.get_mut(next).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    queue.push(next);
                }
            }
        }

        if order.len() != self.stages.len() {
            anyhow::bail!("cycle detected in pipeline DAG");
        }

        Ok(order)
    }

    /// Validate structural integrity of the DAG.
    ///
    /// Checks:
    /// - All stage names are unique
    /// - Every edge source and target refers to an existing stage
    /// - No disconnected stages (no incoming AND no outgoing edges)
    /// - No dangling ports (edge `via_port` exists on both source output and
    ///   target input ports)
    pub fn validate(&self) -> anyhow::Result<()> {
        // ── 1. Unique stage names ──
        let mut seen = HashSet::new();
        for stage in &self.stages {
            if !seen.insert(stage.name.as_str()) {
                anyhow::bail!("duplicate stage name: {}", stage.name);
            }
        }

        let stage_names: HashSet<&str> =
            self.stages.iter().map(|s| s.name.as_str()).collect();

        // ── 2. All edge targets refer to existing stages ──
        for edge in &self.edges {
            if !stage_names.contains(edge.from.as_str()) {
                anyhow::bail!(
                    "edge source '{}' not found in stages",
                    edge.from
                );
            }
            if !stage_names.contains(edge.to.as_str()) {
                anyhow::bail!(
                    "edge target '{}' not found in stages",
                    edge.to
                );
            }
        }

        // ── 3. No disconnected stages (singleton pipeline is always valid) ──
        for stage in &self.stages {
            let has_incoming = self.edges.iter().any(|e| e.to == stage.name);
            let has_outgoing = self.edges.iter().any(|e| e.from == stage.name);
            if !has_incoming && !has_outgoing && self.stages.len() > 1 {
                anyhow::bail!(
                    "stage '{}' is disconnected (no edges)",
                    stage.name
                );
            }
        }

        // ── 4. No dangling ports ──
        for edge in &self.edges {
            if let Some(ref port) = edge.via_port {
                let src = self
                    .stages
                    .iter()
                    .find(|s| s.name == edge.from)
                    .unwrap();
                let dst = self
                    .stages
                    .iter()
                    .find(|s| s.name == edge.to)
                    .unwrap();

                let src_has_port = src
                    .output_ports
                    .iter()
                    .any(|p| p == port);
                if !src_has_port {
                    anyhow::bail!(
                        "edge from '{}' references port {:?} not found in output ports",
                        edge.from,
                        port
                    );
                }

                let dst_has_port = dst.input_ports.iter().any(|p| {
                    p.direction == PortDirection::Input
                        && p.media_type == port.media_type
                });
                if !dst_has_port {
                    anyhow::bail!(
                        "edge to '{}' references port {:?} not found in input ports",
                        edge.to,
                        port
                    );
                }
            }
        }

        Ok(())
    }

    /// Return a topological execution order of stage names.
    ///
    /// Stages with no dependencies come first; stages that depend on others
    /// appear after their dependencies.
    pub fn execution_order(&self) -> anyhow::Result<Vec<String>> {
        self.topological_sort()
    }

    // ── Backward-compatible helpers (used by pipeline_validate.rs) ──

    /// Build a DAG by connecting stages sequentially.
    ///
    /// Each consecutive pair `(i, i+1)` gets a `PipelineEdge` — no port
    /// compatibility checking.  Entry/exit points are inferred.
    pub fn from_sequential(stages: Vec<StageSpec>) -> Self {
        let edges: Vec<PipelineEdge> = stages
            .windows(2)
            .map(|w| PipelineEdge {
                from: w[0].name.clone(),
                to: w[1].name.clone(),
                via_port: None,
            })
            .collect();

        let entry_points: Vec<String> = stages
            .iter()
            .map(|s| s.name.clone())
            .filter(|name| !edges.iter().any(|e| e.to == *name))
            .collect();

        let exit_points: Vec<String> = stages
            .iter()
            .map(|s| s.name.clone())
            .filter(|name| !edges.iter().any(|e| e.from == *name))
            .collect();

        PipelineDag {
            stages,
            edges,
            entry_points,
            exit_points,
        }
    }

    /// Find a stage by name.
    pub fn find_stage(&self, name: &str) -> Option<&StageSpec> {
        self.stages.iter().find(|s| s.name == name)
    }

    /// Stage names in declaration order.
    pub fn stage_names(&self) -> Vec<&str> {
        self.stages.iter().map(|s| s.name.as_str()).collect()
    }

    /// DFS-based cycle detection — returns the first cycle path if found.
    ///
    /// This complements `topological_sort` (which returns an error) by
    /// providing the actual cycle path for user-facing diagnostics.
    pub fn detect_cycle(&self) -> Option<Vec<String>> {
        let mut visited: HashSet<&str> = HashSet::new();
        let mut in_stack: HashSet<&str> = HashSet::new();
        let mut path: Vec<String> = Vec::new();

        fn dfs<'a>(
            node: &'a str,
            edges: &'a [PipelineEdge],
            visited: &mut HashSet<&'a str>,
            in_stack: &mut HashSet<&'a str>,
            path: &mut Vec<String>,
        ) -> Option<Vec<String>> {
            visited.insert(node);
            in_stack.insert(node);
            path.push(node.to_string());

            for edge in edges {
                if edge.from == node {
                    let next = edge.to.as_str();
                    if !visited.contains(next) {
                        if let Some(cycle) = dfs(next, edges, visited, in_stack, path) {
                            return Some(cycle);
                        }
                    } else if in_stack.contains(next) {
                        // Found a cycle — extract it
                        let cycle_start = path.iter().position(|n| n == next).unwrap_or(0);
                        let mut cycle: Vec<String> = path[cycle_start..].to_vec();
                        cycle.push(next.to_string()); // close the cycle
                        return Some(cycle);
                    }
                }
            }

            path.pop();
            in_stack.remove(node);
            None
        }

        for stage in &self.stages {
            if !visited.contains(stage.name.as_str()) {
                if let Some(cycle) = dfs(&stage.name, &self.edges, &mut visited, &mut in_stack, &mut path) {
                    return Some(cycle);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── #719 tests ──

    #[test]
    fn port_media_type_mime() {
        assert_eq!(PortMediaType::Video.mime_type(), "video/mp4");
        assert_eq!(PortMediaType::Audio.mime_type(), "audio/wav");
        assert_eq!(PortMediaType::Json.mime_type(), "application/json");
        assert_eq!(PortMediaType::Bytes.mime_type(), "application/octet-stream");
    }

    #[test]
    fn stage_port_compatible_direct() {
        let out = StagePort { direction: PortDirection::Output, media_type: PortMediaType::Video, description: None };
        let inp = StagePort { direction: PortDirection::Input, media_type: PortMediaType::Video, description: None };
        assert!(out.compatible_with(&inp));
    }

    #[test]
    fn stage_port_incompatible_same_direction() {
        let a = StagePort { direction: PortDirection::Input, media_type: PortMediaType::Video, description: None };
        let b = StagePort { direction: PortDirection::Input, media_type: PortMediaType::Video, description: None };
        assert!(!a.compatible_with(&b));
    }

    #[test]
    fn stage_port_bytes_compatible_with_any() {
        let out = StagePort { direction: PortDirection::Output, media_type: PortMediaType::Bytes, description: None };
        let inp = StagePort { direction: PortDirection::Input, media_type: PortMediaType::Json, description: None };
        assert!(out.compatible_with(&inp));
    }

    #[test]
    fn stage_port_incompatible_type_mismatch() {
        let out = StagePort { direction: PortDirection::Output, media_type: PortMediaType::Video, description: None };
        let inp = StagePort { direction: PortDirection::Input, media_type: PortMediaType::Audio, description: None };
        assert!(!out.compatible_with(&inp));
    }

    #[test]
    fn port_serialize_round_trip() {
        let p = StagePort { direction: PortDirection::Input, media_type: PortMediaType::Json, description: Some("test".into()) };
        let back: StagePort = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(p, back);
    }

    // ── #720 tests ──

    #[test]
    fn resource_fits_basic() {
        let req = ResourceRequirements { min_ram_gb: 4.0, min_vram_gb: 0.0, requires_gpu: false, cpu_cores: None, scratch_disk_gb: None };
        let host = HostResources { ram_gb: 8.0, vram_gb: 0.0, gpu_count: 0, cpu_cores: 4 };
        assert!(req.fits_on(&host));
    }

    #[test]
    fn resource_fails_ram() {
        let req = ResourceRequirements { min_ram_gb: 32.0, min_vram_gb: 0.0, requires_gpu: false, cpu_cores: None, scratch_disk_gb: None };
        let host = HostResources { ram_gb: 16.0, vram_gb: 0.0, gpu_count: 0, cpu_cores: 4 };
        assert!(!req.fits_on(&host));
    }

    #[test]
    fn resource_fails_gpu() {
        let req = ResourceRequirements { min_ram_gb: 1.0, min_vram_gb: 8.0, requires_gpu: true, cpu_cores: None, scratch_disk_gb: None };
        let host = HostResources { ram_gb: 16.0, vram_gb: 0.0, gpu_count: 0, cpu_cores: 4 };
        assert!(!req.fits_on(&host));
    }

    #[test]
    fn resource_fits_gpu() {
        let req = ResourceRequirements { min_ram_gb: 1.0, min_vram_gb: 8.0, requires_gpu: true, cpu_cores: None, scratch_disk_gb: None };
        let host = HostResources { ram_gb: 16.0, vram_gb: 16.0, gpu_count: 1, cpu_cores: 8 };
        assert!(req.fits_on(&host));
    }

    #[test]
    fn resource_fails_cpu_cores() {
        let req = ResourceRequirements { min_ram_gb: 1.0, min_vram_gb: 0.0, requires_gpu: false, cpu_cores: Some(16), scratch_disk_gb: None };
        let host = HostResources { ram_gb: 64.0, vram_gb: 0.0, gpu_count: 0, cpu_cores: 8 };
        assert!(!req.fits_on(&host));
    }

    #[test]
    fn capsule_profile_serialize() {
        let p = CapsuleProfile {
            name: "test".into(),
            ports: vec![],
            resources: ResourceRequirements { min_ram_gb: 1.0, min_vram_gb: 0.0, requires_gpu: false, cpu_cores: None, scratch_disk_gb: None },
            image: Some("alpine:latest".into()),
            timeout_seconds: Some(300),
        };
        let toml_str = toml::to_string(&p).unwrap();
        assert!(toml_str.contains("name"));
        assert!(toml_str.contains("alpine:latest"));
    }

    // ── #722 tests ──

    #[test]
    fn input_validation_holds_string() {
        let err = PipelineError::InputValidation("bad input".into());
        assert_eq!(format!("{err:?}"), "InputValidation(\"bad input\")");
    }

    #[test]
    fn resource_exhausted_holds_needed_and_available() {
        let err = PipelineError::ResourceExhausted { needed: "512 MiB".into(), available: "256 MiB".into() };
        assert_eq!(format!("{err:?}"), "ResourceExhausted { needed: \"512 MiB\", available: \"256 MiB\" }");
    }

    #[test]
    fn from_anyhow_error() {
        let e: PipelineError = anyhow::anyhow!("disk full").into();
        assert_eq!(e.variant_name(), "InputValidation");
    }

    #[test]
    fn from_string() {
        let e: PipelineError = "oops".to_string().into();
        assert_eq!(e.variant_name(), "InputValidation");
    }

    #[test]
    fn variant_name_all() {
        for (e, n) in [
            (PipelineError::InputValidation("x".into()), "InputValidation"),
            (PipelineError::ResourceExhausted { needed: "a".into(), available: "b".into() }, "ResourceExhausted"),
            (PipelineError::StageCrashed("x".into()), "StageCrashed"),
            (PipelineError::Timeout { stage: "x".into(), elapsed_ms: 1 }, "Timeout"),
            (PipelineError::MediaTypeMismatch { expected: PortMediaType::Json, got: PortMediaType::Bytes }, "MediaTypeMismatch"),
            (PipelineError::TranscodeError("x".into()), "TranscodeError"),
        ] {
            assert_eq!(e.variant_name(), n);
        }
    }

    #[test]
    fn glob_exact() { assert!(glob_match("TranscodeError", "TranscodeError")); assert!(!glob_match("TranscodeError", "Timeout")); }
    #[test]
    fn glob_prefix() { assert!(glob_match("Transcode*", "TranscodeError")); assert!(!glob_match("Transcode*", "Timeout")); }
    #[test]
    fn glob_catch_all() { assert!(glob_match("*", "InputValidation")); assert!(glob_match("*", "Anything")); }
    #[test]
    fn glob_empty() { assert!(!glob_match("", "TranscodeError")); }

    #[test]
    fn route_exact_match() {
        let r = ErrorRoute { match_pattern: "TranscodeError".into(), route_to_stage: "r".into(), max_retries: 3, backoff_ms: 100, fallback_output: None, retry_count: 0 };
        assert!(r.matches(&PipelineError::TranscodeError("bad".into())));
        assert!(!r.matches(&PipelineError::Timeout { stage: "t".into(), elapsed_ms: 1 }));
    }

    #[test]
    fn route_glob_match() {
        let r = ErrorRoute { match_pattern: "Transcode*".into(), route_to_stage: "r".into(), max_retries: 3, backoff_ms: 100, fallback_output: None, retry_count: 0 };
        assert!(r.matches(&PipelineError::TranscodeError("x".into())));
        assert!(!r.matches(&PipelineError::Timeout { stage: "x".into(), elapsed_ms: 1 }));
    }

    #[test]
    fn route_catch_all() {
        let r = ErrorRoute { match_pattern: "*".into(), route_to_stage: "r".into(), max_retries: 0, backoff_ms: 0, fallback_output: None, retry_count: 0 };
        assert!(r.matches(&PipelineError::InputValidation("x".into())));
        assert!(r.matches(&PipelineError::TranscodeError("x".into())));
    }

    #[test]
    fn retry_within_limit() {
        let mut r = ErrorRoute { match_pattern: "*".into(), route_to_stage: "r".into(), max_retries: 3, backoff_ms: 100, fallback_output: None, retry_count: 0 };
        assert!(r.can_retry()); assert_eq!(r.retries_left(), 3);
        r.record_retry(); assert!(r.can_retry()); assert_eq!(r.retries_left(), 2);
        r.record_retry(); assert!(r.can_retry()); assert_eq!(r.retries_left(), 1);
        r.record_retry(); assert!(!r.can_retry()); assert_eq!(r.retries_left(), 0);
    }

    #[test]
    fn retry_exhausted() {
        let mut r = ErrorRoute { match_pattern: "*".into(), route_to_stage: "r".into(), max_retries: 2, backoff_ms: 100, fallback_output: None, retry_count: 0 };
        r.record_retry(); r.record_retry();
        assert!(!r.can_retry()); assert_eq!(r.retries_left(), 0);
    }

    #[test]
    fn retry_zero_max() {
        let r = ErrorRoute { match_pattern: "*".into(), route_to_stage: "r".into(), max_retries: 0, backoff_ms: 0, fallback_output: None, retry_count: 0 };
        assert!(!r.can_retry()); assert_eq!(r.retries_left(), 0);
    }

    #[test]
    fn serialize_round_trip() {
        let err = PipelineError::Timeout { stage: "encode".into(), elapsed_ms: 5000 };
        let back: PipelineError = serde_json::from_str(&serde_json::to_string(&err).unwrap()).unwrap();
        assert_eq!(err, back);
    }

    #[test]
    fn serialize_skips_retry_count() {
        let r = ErrorRoute { match_pattern: "*".into(), route_to_stage: "r".into(), max_retries: 3, backoff_ms: 100, fallback_output: None, retry_count: 5 };
        assert!(!serde_json::to_string(&r).unwrap().contains("retry_count"));
    }

    #[test]
    fn deserialize_defaults_retry_count_to_zero() {
        let r: ErrorRoute = serde_json::from_str(r#"{"match_pattern":"T","route_to_stage":"s","max_retries":2,"backoff_ms":500,"fallback_output":null}"#).unwrap();
        assert_eq!(r.retry_count, 0);
    }

    #[test]
    fn stage_port_enum() {
        let p = StagePort { direction: PortDirection::Input, media_type: PortMediaType::Video, description: Some("in".into()) };
        assert_eq!(p.direction, PortDirection::Input);
        assert_eq!(p.media_type, PortMediaType::Video);
    }

    // ── #721 tests ──

    #[test]
    fn stage_spec_from_name() {
        let spec = StageSpec::from_name("transcode");
        assert_eq!(spec.name, "transcode");
        assert_eq!(spec.profile.name, "transcode");
        assert_eq!(spec.input_ports.len(), 1);
        assert_eq!(spec.input_ports[0].media_type, PortMediaType::Bytes);
        assert_eq!(spec.output_ports[0].media_type, PortMediaType::Bytes);
    }

    #[test]
    fn stage_entry_name_resolves() {
        let entry: StageEntry = serde_json::from_str(r#""video-ingest""#).unwrap();
        let spec = entry.resolve();
        assert_eq!(spec.name, "video-ingest");
        assert_eq!(spec.input_ports[0].media_type, PortMediaType::Bytes);
    }

    #[test]
    fn stage_entry_spec_resolves() {
        let json = r#"{"name":"transcode","profile":{"name":"transcode","ports":[],"resources":{"min_ram_gb":1.0,"min_vram_gb":8.0,"requires_gpu":true,"cpu_cores":null,"scratch_disk_gb":null},"image":null,"timeout_seconds":600},"input_ports":[{"direction":"Input","media_type":"Video","description":"in"}],"output_ports":[{"direction":"Output","media_type":"Video","description":"out"}],"error_routes":[],"env":null,"checkpoint_interval_seconds":null}"#;
        let entry: StageEntry = serde_json::from_str(json).unwrap();
        let spec = entry.resolve();
        assert_eq!(spec.name, "transcode");
        assert_eq!(spec.profile.resources.requires_gpu, true);
        assert_eq!(spec.profile.resources.min_vram_gb, 8.0);
        assert_eq!(spec.profile.timeout_seconds, Some(600));
        assert_eq!(spec.input_ports[0].media_type, PortMediaType::Video);
    }

    #[test]
    fn stage_spec_serialize_round_trip() {
        let spec = StageSpec::from_name("embed");
        let back: StageSpec = serde_json::from_str(&serde_json::to_string(&spec).unwrap()).unwrap();
        assert_eq!(spec.name, back.name);
    }

    #[test]
    fn port_media_type_all() {
        let v = format!("{:?}", vec![PortMediaType::Video,PortMediaType::Audio,PortMediaType::Image,PortMediaType::Json,PortMediaType::Parquet,PortMediaType::Bytes,PortMediaType::Error]);
        for t in &["Video","Audio","Image","Json","Parquet","Bytes","Error"] {
            assert!(v.contains(t), "missing {t}");
        }
    }

    // ── #723 tests: StagePort compatibility negotiation ──

    #[test]
    fn negotiate_direct_match_same_type() {
        let out = StagePort { direction: PortDirection::Output, media_type: PortMediaType::Audio, description: None };
        let inp = StagePort { direction: PortDirection::Input, media_type: PortMediaType::Audio, description: None };
        assert_eq!(can_negotiate(&out, &inp), NegotiationResult::Direct);
    }

    #[test]
    fn negotiate_direct_match_bytes_wildcard() {
        let out = StagePort { direction: PortDirection::Output, media_type: PortMediaType::Bytes, description: None };
        let inp = StagePort { direction: PortDirection::Input, media_type: PortMediaType::Json, description: None };
        assert_eq!(can_negotiate(&out, &inp), NegotiationResult::Direct);
    }

    #[test]
    fn negotiate_convertible_lossy_false() {
        // Audio → Json via whisper-transcript (non-lossy)
        let out = StagePort { direction: PortDirection::Output, media_type: PortMediaType::Audio, description: None };
        let inp = StagePort { direction: PortDirection::Input, media_type: PortMediaType::Json, description: None };
        match can_negotiate(&out, &inp) {
            NegotiationResult::Convertible { via_stage, lossy } => {
                assert!(!lossy, "whisper-transcript should be non-lossy");
                assert_eq!(via_stage, "whisper-transcript");
            }
            other => panic!("expected Convertible, got {other:?}"),
        }
    }

    #[test]
    fn negotiate_convertible_lossy_true() {
        // Video → Image via ffmpeg-frame-extract (lossy: drops temporal dimension)
        let out = StagePort { direction: PortDirection::Output, media_type: PortMediaType::Video, description: None };
        let inp = StagePort { direction: PortDirection::Input, media_type: PortMediaType::Image, description: None };
        match can_negotiate(&out, &inp) {
            NegotiationResult::Convertible { via_stage, lossy } => {
                assert!(lossy, "ffmpeg-frame-extract should be lossy");
                assert_eq!(via_stage, "ffmpeg-frame-extract");
            }
            other => panic!("expected Convertible, got {other:?}"),
        }
    }

    #[test]
    fn negotiate_incompatible_undefined_conversion() {
        // Video → Audio has no registered conversion
        let out = StagePort { direction: PortDirection::Output, media_type: PortMediaType::Video, description: None };
        let inp = StagePort { direction: PortDirection::Input, media_type: PortMediaType::Audio, description: None };
        match can_negotiate(&out, &inp) {
            NegotiationResult::Incompatible(reason) => {
                assert!(reason.contains("Video"), "reason should mention Video: {reason}");
                assert!(reason.contains("Audio"), "reason should mention Audio: {reason}");
            }
            other => panic!("expected Incompatible, got {other:?}"),
        }
    }

    #[test]
    fn negotiate_incompatible_same_direction() {
        let out = StagePort { direction: PortDirection::Output, media_type: PortMediaType::Video, description: None };
        let inp = StagePort { direction: PortDirection::Output, media_type: PortMediaType::Image, description: None };
        match can_negotiate(&out, &inp) {
            NegotiationResult::Incompatible(reason) => {
                assert!(reason.contains("same direction"), "reason: {reason}");
            }
            other => panic!("expected Incompatible, got {other:?}"),
        }
    }

    #[test]
    fn negotiate_result_serialize_round_trip() {
        let cases: Vec<NegotiationResult> = vec![
            NegotiationResult::Direct,
            NegotiationResult::Convertible { via_stage: "ffmpeg-frame-extract".into(), lossy: true },
            NegotiationResult::Incompatible("no route".into()),
        ];
        for r in &cases {
            let json = serde_json::to_string(r).unwrap();
            let back: NegotiationResult = serde_json::from_str(&json).unwrap();
            assert_eq!(*r, back);
        }
    }

    #[test]
    fn auto_insert_creates_conversion_stage() {
        // Two stages: source(Video→) and sink(→Image) → needs ffmpeg-frame-extract
        let mut stages = vec![
            StageSpec {
                name: "video-source".into(),
                profile: CapsuleProfile {
                    name: "video-source".into(), ports: vec![],
                    resources: ResourceRequirements { min_ram_gb: 0.0, min_vram_gb: 0.0, requires_gpu: false, cpu_cores: None, scratch_disk_gb: None },
                    image: None, timeout_seconds: None,
                },
                input_ports: vec![],
                output_ports: vec![StagePort { direction: PortDirection::Output, media_type: PortMediaType::Video, description: None }],
                error_routes: vec![], env: None, checkpoint_interval_seconds: None, secret_refs: None,
            },
            StageSpec {
                name: "image-processor".into(),
                profile: CapsuleProfile {
                    name: "image-processor".into(), ports: vec![],
                    resources: ResourceRequirements { min_ram_gb: 0.0, min_vram_gb: 0.0, requires_gpu: false, cpu_cores: None, scratch_disk_gb: None },
                    image: None, timeout_seconds: None,
                },
                input_ports: vec![StagePort { direction: PortDirection::Input, media_type: PortMediaType::Image, description: None }],
                output_ports: vec![],
                error_routes: vec![], env: None, checkpoint_interval_seconds: None, secret_refs: None,
            },
        ];
        auto_insert_conversions(&mut stages);
        assert_eq!(stages.len(), 3, "should have inserted one conversion stage");
        assert!(stages[1].name.starts_with("auto-conv-"), "conversion stage name: {}", stages[1].name);
        // Verify ports are wired correctly
        assert_eq!(stages[1].input_ports[0].media_type, PortMediaType::Video);
        assert_eq!(stages[1].output_ports[0].media_type, PortMediaType::Image);
    }

    // ── #724 tests: PipelineDag ──

    fn make_stage(name: &str, input_types: &[PortMediaType], output_types: &[PortMediaType]) -> StageSpec {
        StageSpec {
            name: name.into(),
            profile: CapsuleProfile {
                name: name.into(), ports: vec![],
                resources: ResourceRequirements { min_ram_gb: 0.0, min_vram_gb: 0.0, requires_gpu: false, cpu_cores: None, scratch_disk_gb: None },
                image: None, timeout_seconds: None,
            },
            input_ports: input_types.iter().map(|mt| StagePort {
                direction: PortDirection::Input, media_type: mt.clone(), description: None,
            }).collect(),
            output_ports: output_types.iter().map(|mt| StagePort {
                direction: PortDirection::Output, media_type: mt.clone(), description: None,
            }).collect(),
            error_routes: vec![], env: None, checkpoint_interval_seconds: None, secret_refs: None,
        }
    }

    #[test]
    fn dag_linear_pipeline() {
        // A → B → C  where A outputs Audio, B inputs Audio & outputs Video, C inputs Video
        let stages = vec![
            make_stage("A", &[], &[PortMediaType::Audio]),
            make_stage("B", &[PortMediaType::Audio], &[PortMediaType::Video]),
            make_stage("C", &[PortMediaType::Video], &[]),
        ];
        let dag = PipelineDag::build(stages).unwrap();
        assert_eq!(dag.edges.len(), 2);
        assert_eq!(dag.edges[0].from, "A"); assert_eq!(dag.edges[0].to, "B");
        assert_eq!(dag.edges[1].from, "B"); assert_eq!(dag.edges[1].to, "C");
        assert_eq!(dag.entry_points, vec!["A"]);
        assert_eq!(dag.exit_points, vec!["C"]);
        // Execution order: A → B → C
        let order = dag.execution_order().unwrap();
        assert_eq!(order, vec!["A", "B", "C"]);
    }

    #[test]
    fn dag_fan_out() {
        // A → B, A → C  where A outputs Video, both B and C input Video
        let stages = vec![
            make_stage("A", &[], &[PortMediaType::Video]),
            make_stage("B", &[PortMediaType::Video], &[]),
            make_stage("C", &[PortMediaType::Video], &[]),
        ];
        let dag = PipelineDag::build(stages).unwrap();
        assert_eq!(dag.edges.len(), 2);
        assert!(dag.edges.iter().any(|e| e.from == "A" && e.to == "B"));
        assert!(dag.edges.iter().any(|e| e.from == "A" && e.to == "C"));
        assert_eq!(dag.entry_points, vec!["A"]);
        // B and C are both exit points
        assert_eq!(dag.exit_points.len(), 2);
        assert!(dag.exit_points.contains(&"B".into()));
        assert!(dag.exit_points.contains(&"C".into()));
    }

    #[test]
    fn dag_fan_in() {
        // A → C, B → C  where A outputs Audio, B outputs Video, C inputs both
        let stages = vec![
            make_stage("A", &[], &[PortMediaType::Audio]),
            make_stage("B", &[], &[PortMediaType::Video]),
            make_stage("C", &[PortMediaType::Audio, PortMediaType::Video], &[]),
        ];
        let dag = PipelineDag::build(stages).unwrap();
        assert_eq!(dag.edges.len(), 2);
        assert!(dag.edges.iter().any(|e| e.from == "A" && e.to == "C"));
        assert!(dag.edges.iter().any(|e| e.from == "B" && e.to == "C"));
        // A and B are entry points
        assert_eq!(dag.entry_points.len(), 2);
        assert!(dag.entry_points.contains(&"A".into()));
        assert!(dag.entry_points.contains(&"B".into()));
        assert_eq!(dag.exit_points, vec!["C"]);
    }

    #[test]
    fn dag_cycle_detected() {
        // A → B → A  (cycle)
        let stages = vec![
            make_stage("A", &[PortMediaType::Bytes], &[PortMediaType::Video]),
            make_stage("B", &[PortMediaType::Video], &[PortMediaType::Bytes]),
        ];
        let err = PipelineDag::build(stages).unwrap_err();
        assert!(err.to_string().contains("cycle"), "expected cycle error, got: {err}");
    }

    #[test]
    fn dag_empty_pipeline() {
        let dag = PipelineDag::build(vec![]).unwrap();
        assert_eq!(dag.stages.len(), 0);
        assert_eq!(dag.edges.len(), 0);
        assert!(dag.entry_points.is_empty());
        assert!(dag.exit_points.is_empty());
        let order = dag.execution_order().unwrap();
        assert!(order.is_empty());
        assert!(dag.validate().is_ok());
    }

    #[test]
    fn dag_duplicate_names() {
        let stages = vec![
            make_stage("dup", &[], &[PortMediaType::Bytes]),
            make_stage("dup", &[PortMediaType::Bytes], &[]),
        ];
        let err = PipelineDag::build(stages).unwrap_err();
        assert!(err.to_string().contains("duplicate"), "expected duplicate name error, got: {err}");
    }

    #[test]
    fn dag_disconnected_stage() {
        // A → B,  C is disconnected
        let stages = vec![
            make_stage("A", &[], &[PortMediaType::Audio]),
            make_stage("B", &[PortMediaType::Audio], &[]),
            make_stage("C", &[PortMediaType::Video], &[PortMediaType::Video]),
        ];
        let dag = PipelineDag::build(stages).unwrap();
        // Build succeeds (acyclic), but validate catches the disconnected stage
        let err = dag.validate().unwrap_err();
        assert!(err.to_string().contains("disconnected"), "expected disconnected error, got: {err}");
    }

    #[test]
    fn auto_insert_direct_match_no_change() {
        // Two stages: both Audio → no conversion needed
        let mut stages = vec![
            StageSpec {
                name: "audio-source".into(),
                profile: CapsuleProfile {
                    name: "audio-source".into(), ports: vec![],
                    resources: ResourceRequirements { min_ram_gb: 0.0, min_vram_gb: 0.0, requires_gpu: false, cpu_cores: None, scratch_disk_gb: None },
                    image: None, timeout_seconds: None,
                },
                input_ports: vec![],
                output_ports: vec![StagePort { direction: PortDirection::Output, media_type: PortMediaType::Audio, description: None }],
                error_routes: vec![], env: None, checkpoint_interval_seconds: None, secret_refs: None,
            },
            StageSpec {
                name: "audio-processor".into(),
                profile: CapsuleProfile {
                    name: "audio-processor".into(), ports: vec![],
                    resources: ResourceRequirements { min_ram_gb: 0.0, min_vram_gb: 0.0, requires_gpu: false, cpu_cores: None, scratch_disk_gb: None },
                    image: None, timeout_seconds: None,
                },
                input_ports: vec![StagePort { direction: PortDirection::Input, media_type: PortMediaType::Audio, description: None }],
                output_ports: vec![],
                error_routes: vec![], env: None, checkpoint_interval_seconds: None, secret_refs: None,
            },
        ];
        auto_insert_conversions(&mut stages);
        assert_eq!(stages.len(), 2, "direct match should not insert conversion");
    }

}
