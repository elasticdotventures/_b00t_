use crate::pipeline_types::*;
use std::path::Path;
use std::fs;

// ── Trait ────────────────────────────────────────────────────────────────────

/// Serialize pipeline constructs to KerML (SysML v2 textual notation).
///
/// KerML is the Kernel Modeling Language — the foundational textual notation
/// underlying SysML v2.  This trait family emits KerML fragments that capture
/// stage topology, port contracts, error handling, and resource constraints.
pub trait ToKerML {
    /// Render `self` as a KerML text fragment.
    fn to_kerml(&self) -> String;
}

// ── PortMediaType → KerML type name ──────────────────────────────────────────

impl ToKerML for PortMediaType {
    fn to_kerml(&self) -> String {
        match self {
            PortMediaType::Video   => "Video".into(),
            PortMediaType::Audio   => "Audio".into(),
            PortMediaType::Image   => "Image".into(),
            PortMediaType::Json    => "Json".into(),
            PortMediaType::Parquet => "Parquet".into(),
            PortMediaType::Bytes   => "Bytes".into(),
            PortMediaType::Error   => "Error".into(),
        }
    }
}

// ── StagePort → KerML port definition ────────────────────────────────────────

impl ToKerML for StagePort {
    fn to_kerml(&self) -> String {
        let dir = match self.direction {
            PortDirection::Input  => "input",
            PortDirection::Output => "output",
        };
        let mt = self.media_type.to_kerml();
        let desc = self
            .description
            .as_ref()
            .map(|d| format!(" // {}", d))
            .unwrap_or_default();
        format!("        port p : {} {{ direction: {}; }}{}", mt, dir, desc)
    }
}

// ── CapsuleProfile → KerML part definition ──────────────────────────────────

impl ToKerML for CapsuleProfile {
    fn to_kerml(&self) -> String {
        let mut out = String::new();

        // Resource constraint block
        out.push_str(&format!(
            "    constraint ResourceReq_{} {{\n", self.name
        ));
        out.push_str(&format!(
            "        min_ram_gb <= available_ram_gb; /* {} GB */\n",
            self.resources.min_ram_gb
        ));
        if self.resources.min_vram_gb > 0.0 {
            out.push_str(&format!(
                "        min_vram_gb <= available_vram_gb; /* {} GB */\n",
                self.resources.min_vram_gb
            ));
        }
        if self.resources.requires_gpu {
            out.push_str("        requires_gpu == true;\n");
        }
        if let Some(cores) = self.resources.cpu_cores {
            out.push_str(&format!(
                "        cpu_cores >= {};\n",
                cores
            ));
        }
        out.push_str("    }\n");

        // Port definitions
        for port in &self.ports {
            out.push_str(&port.to_kerml());
            out.push('\n');
        }

        out
    }
}

// ── PipelineError → KerML error type identifier ─────────────────────────────

impl ToKerML for PipelineError {
    fn to_kerml(&self) -> String {
        match self {
            PipelineError::InputValidation(msg) => {
                format!("InputValidation(\"{}\")", msg)
            }
            PipelineError::ResourceExhausted { needed, available } => {
                format!("ResourceExhausted(needed: \"{}\", available: \"{}\")", needed, available)
            }
            PipelineError::StageCrashed(stage) => {
                format!("StageCrashed(\"{}\")", stage)
            }
            PipelineError::Timeout { stage, elapsed_ms } => {
                format!("Timeout(stage: \"{}\", elapsed_ms: {})", stage, elapsed_ms)
            }
            PipelineError::MediaTypeMismatch { expected, got } => {
                format!("MediaTypeMismatch(expected: {}, got: {})", expected.to_kerml(), got.to_kerml())
            }
            PipelineError::TranscodeError(msg) => {
                format!("TranscodeError(\"{}\")", msg)
            }
        }
    }
}

// ── ErrorRoute → KerML action definition ─────────────────────────────────────

impl ToKerML for ErrorRoute {
    fn to_kerml(&self) -> String {
        let pattern = &self.match_pattern;
        let route_to = &self.route_to_stage;
        let backoff = self.backoff_ms;
        let retries = self.max_retries;

        let fallback = self
            .fallback_output
            .as_ref()
            .map(|p| {
                let mt = p.media_type.to_kerml();
                format!("; fallback: {}", mt)
            })
            .unwrap_or_default();

        format!(
            "    action ErrorRoute_{} {{\n\
             {spaces}if error matches \"{}\" {{\n\
             {spaces}    route_to = \"{}\";\n\
             {spaces}    retry_limit = {};\n\
             {spaces}    backoff_ms = {};{}\n\
             {spaces}}}\n\
             {spaces}}}",
            pattern.replace('*', "star").replace('%', "pct"),
            pattern, route_to, retries, backoff, fallback,
            spaces = "        "
        )
    }
}

// ── StageSpec → KerML part definition with ports, error routes, constraints ──

impl ToKerML for StageSpec {
    fn to_kerml(&self) -> String {
        let mut out = String::new();

        out.push_str(&format!("    part def {} {{\n", self.name));

        // Input ports
        for port in &self.input_ports {
            out.push_str(&port.to_kerml());
            out.push('\n');
        }

        // Output ports
        for port in &self.output_ports {
            out.push_str(&port.to_kerml());
            out.push('\n');
        }

        // Error routes as nested actions
        for route in &self.error_routes {
            out.push_str(&route.to_kerml());
            out.push('\n');
        }

        // Resource constraints from profile
        let profile_kerml = self.profile.to_kerml();
        // Only include the constraint block part, not port defs (those are on the stage itself)
        for line in profile_kerml.lines() {
            if line.contains("constraint") || line.contains("min_ram_gb")
                || line.contains("min_vram_gb") || line.contains("requires_gpu")
                || line.contains("cpu_cores")
            {
                out.push_str("        ");
                out.push_str(line.trim());
                out.push('\n');
            }
        }

        // Image info as comment
        if let Some(ref img) = self.profile.image {
            out.push_str(&format!("        // image: {}\n", img));
        }

        // Timeout
        if let Some(t) = self.profile.timeout_seconds {
            out.push_str(&format!("        // timeout_seconds: {}\n", t));
        }

        // Checkpoint interval
        if let Some(ckpt) = self.checkpoint_interval_seconds {
            out.push_str(&format!("        // checkpoint_interval_seconds: {}\n", ckpt));
        }

        out.push_str("    }\n");

        out
    }
}

// ── PipelineDag → full KerML package ────────────────────────────────────────

impl ToKerML for PipelineDag {
    fn to_kerml(&self) -> String {
        pipeline_to_kerml(self)
    }
}

/// Render a full pipeline DAG as a KerML package.
///
/// The output is a self-contained KerML textual notation fragment:
///
/// ```kerml
/// package MyPipeline {
///     import ScalarValues::*;
///
///     part def StageA { ... }
///     part def StageB { ... }
///
///     connect StageA.p to StageB.p;
/// }
/// ```
pub fn pipeline_to_kerml(dag: &PipelineDag) -> String {
    let mut out = String::new();

    // Derive a package name from the first stage, or use "Pipeline"
    let pkg_name = dag
        .stages
        .first()
        .map(|s| {
            // PascalCase the stage name
            let mut chars = s.name.chars();
            match chars.next() {
                None => "Pipeline".to_string(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .unwrap_or_else(|| "Pipeline".to_string());

    out.push_str(&format!("package {} {{\n", pkg_name));
    out.push_str("    import ScalarValues::*;\n");
    out.push_str("    import SI::*;\n\n");

    // Stage definitions
    for stage in &dag.stages {
        out.push_str(&stage.to_kerml());
        out.push('\n');
    }

    // Connection statements
    for edge in &dag.edges {
        let port_spec = edge
            .via_port
            .as_ref()
            .map(|p| {
                let dir = match p.direction {
                    PortDirection::Input => "input",
                    PortDirection::Output => "output",
                };
                format!(" /* {}: {} */", dir, p.media_type.to_kerml())
            })
            .unwrap_or_default();
        out.push_str(&format!(
            "    connect {}.p to {}.p;{}\n",
            edge.from, edge.to, port_spec
        ));
    }

    out.push_str("}\n");

    out
}

/// Write a KerML representation of `dag` to the file at `path`.
///
/// Returns an I/O error if the file cannot be created or written.
pub fn kerml_to_file(dag: &PipelineDag, path: &Path) -> std::io::Result<()> {
    let kerml = pipeline_to_kerml(dag);
    fs::write(path, kerml)
}

// ── Helper: Render resource constraints in KerML ─────────────────────────────

/// Render a `ResourceRequirements` block as a KerML constraint definition.
fn resource_to_kerml(name: &str, res: &ResourceRequirements) -> String {
    let mut out = format!("    constraint {} {{\n", sanitize_name(name));
    out.push_str(&format!("        min_ram_gb <= available_ram_gb;  // {} GB\n", res.min_ram_gb));
    if res.min_vram_gb > 0.0 {
        out.push_str(&format!("        min_vram_gb <= available_vram_gb;  // {} GB\n", res.min_vram_gb));
    }
    if res.requires_gpu {
        out.push_str("        requires_gpu == true;\n");
    }
    if let Some(cores) = res.cpu_cores {
        out.push_str(&format!("        cpu_cores >= {};\n", cores));
    }
    out.push_str("    }\n");
    out
}

/// Replace characters unsuitable for KerML identifiers with safe alternatives.
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ── Helper: build a minimal StageSpec ──────────────────────────────────

    fn make_stage(name: &str, input_types: &[PortMediaType], output_types: &[PortMediaType]) -> StageSpec {
        StageSpec {
            name: name.into(),
            profile: CapsuleProfile {
                name: name.into(),
                ports: vec![],
                resources: ResourceRequirements {
                    min_ram_gb: 2.0,
                    min_vram_gb: 0.0,
                    requires_gpu: false,
                    cpu_cores: Some(2),
                    scratch_disk_gb: None,
                },
                image: Some("alpine:latest".into()),
                timeout_seconds: Some(300),
            },
            input_ports: input_types
                .iter()
                .map(|mt| StagePort {
                    direction: PortDirection::Input,
                    media_type: mt.clone(),
                    description: Some(format!("input {}", mt.to_kerml())),
                })
                .collect(),
            output_ports: output_types
                .iter()
                .map(|mt| StagePort {
                    direction: PortDirection::Output,
                    media_type: mt.clone(),
                    description: Some(format!("output {}", mt.to_kerml())),
                })
                .collect(),
            error_routes: vec![],
            env: None,
            checkpoint_interval_seconds: None,
            secret_refs: None,
            flow_control: None,
        }
    }

    // ── PortMediaType tests ────────────────────────────────────────────────

    #[test]
    fn port_media_type_renders_kerml_type_name() {
        assert_eq!(PortMediaType::Video.to_kerml(), "Video");
        assert_eq!(PortMediaType::Audio.to_kerml(), "Audio");
        assert_eq!(PortMediaType::Json.to_kerml(), "Json");
        assert_eq!(PortMediaType::Bytes.to_kerml(), "Bytes");
        assert_eq!(PortMediaType::Error.to_kerml(), "Error");
    }

    // ── StagePort tests ────────────────────────────────────────────────────

    #[test]
    fn stage_port_renders_input_port() {
        let port = StagePort {
            direction: PortDirection::Input,
            media_type: PortMediaType::Video,
            description: Some("raw feed".into()),
        };
        let kerml = port.to_kerml();
        assert!(kerml.contains("port p : Video"), "got: {}", kerml);
        assert!(kerml.contains("direction: input"), "got: {}", kerml);
        assert!(kerml.contains("raw feed"), "got: {}", kerml);
    }

    #[test]
    fn stage_port_renders_output_port() {
        let port = StagePort {
            direction: PortDirection::Output,
            media_type: PortMediaType::Json,
            description: Some("transcript".into()),
        };
        let kerml = port.to_kerml();
        assert!(kerml.contains("port p : Json"));
        assert!(kerml.contains("direction: output"));
    }

    #[test]
    fn stage_port_renders_without_description() {
        let port = StagePort {
            direction: PortDirection::Input,
            media_type: PortMediaType::Bytes,
            description: None,
        };
        let kerml = port.to_kerml();
        assert!(kerml.contains("port p : Bytes"));
        assert!(!kerml.contains("//"));
    }

    // ── ErrorRoute tests ───────────────────────────────────────────────────

    #[test]
    fn error_route_renders_as_action() {
        let route = ErrorRoute {
            match_pattern: "TranscodeError".into(),
            route_to_stage: "transcode-fallback".into(),
            max_retries: 3,
            backoff_ms: 1000,
            fallback_output: None,
            retry_count: 0,
        };
        let kerml = route.to_kerml();
        assert!(kerml.contains("action ErrorRoute_TranscodeError"), "got: {}", kerml);
        assert!(kerml.contains("route_to = \"transcode-fallback\""), "got: {}", kerml);
        assert!(kerml.contains("retry_limit = 3"), "got: {}", kerml);
        assert!(kerml.contains("backoff_ms = 1000"), "got: {}", kerml);
    }

    #[test]
    fn error_route_renders_fallback_output() {
        let route = ErrorRoute {
            match_pattern: "*".into(),
            route_to_stage: "sink".into(),
            max_retries: 1,
            backoff_ms: 500,
            fallback_output: Some(StagePort {
                direction: PortDirection::Output,
                media_type: PortMediaType::Error,
                description: None,
            }),
            retry_count: 0,
        };
        let kerml = route.to_kerml();
        assert!(kerml.contains("fallback: Error"), "got: {}", kerml);
    }

    #[test]
    fn error_route_renders_wildcard_pattern() {
        let route = ErrorRoute {
            match_pattern: "Timeout*".into(),
            route_to_stage: "retry-queue".into(),
            max_retries: 5,
            backoff_ms: 200,
            fallback_output: None,
            retry_count: 0,
        };
        let kerml = route.to_kerml();
        assert!(kerml.contains("matches \"Timeout*\""), "got: {}", kerml);
    }

    // ── StageSpec tests ────────────────────────────────────────────────────

    #[test]
    fn stage_spec_renders_as_part_def() {
        let stage = make_stage("Ingest", &[], &[PortMediaType::Video]);
        let kerml = stage.to_kerml();
        assert!(kerml.contains("part def Ingest"), "got: {}", kerml);
        assert!(kerml.contains("port p : Video"), "got: {}", kerml);
    }

    #[test]
    fn stage_spec_renders_input_and_output_ports() {
        let stage = make_stage(
            "Transcoder",
            &[PortMediaType::Video],
            &[PortMediaType::Audio, PortMediaType::Image],
        );
        let kerml = stage.to_kerml();
        assert!(kerml.contains("part def Transcoder"));
        assert!(kerml.contains("port p : Video"));
        assert!(kerml.contains("port p : Audio"));
        assert!(kerml.contains("port p : Image"));
    }

    #[test]
    fn stage_spec_includes_resource_constraints() {
        let stage = make_stage("Render", &[], &[PortMediaType::Image]);
        let kerml = stage.to_kerml();
        assert!(kerml.contains("min_ram_gb"), "missing RAM constraint");
        assert!(kerml.contains("cpu_cores"), "missing CPU constraint");
        // Image comment
        assert!(kerml.contains("image: alpine:latest"), "missing image comment");
        assert!(kerml.contains("timeout_seconds: 300"), "missing timeout comment");
    }

    #[test]
    fn stage_spec_renders_error_routes() {
        let mut stage = make_stage("Encode", &[PortMediaType::Video], &[PortMediaType::Bytes]);
        stage.error_routes = vec![
            ErrorRoute {
                match_pattern: "TranscodeError".into(),
                route_to_stage: "transcode-fallback".into(),
                max_retries: 2,
                backoff_ms: 500,
                fallback_output: None,
                retry_count: 0,
            },
        ];
        let kerml = stage.to_kerml();
        assert!(kerml.contains("action ErrorRoute_TranscodeError"), "got: {}", kerml);
        assert!(kerml.contains("route_to = \"transcode-fallback\""), "got: {}", kerml);
    }

    // ── PipelineError tests ────────────────────────────────────────────────

    #[test]
    fn pipeline_error_renders_variant_names() {
        let variants: Vec<(PipelineError, &str)> = vec![
            (PipelineError::InputValidation("bad".into()), "InputValidation"),
            (PipelineError::ResourceExhausted { needed: "8GB".into(), available: "4GB".into() }, "ResourceExhausted"),
            (PipelineError::StageCrashed("encoder".into()), "StageCrashed"),
            (PipelineError::Timeout { stage: "encode".into(), elapsed_ms: 5000 }, "Timeout"),
            (
                PipelineError::MediaTypeMismatch { expected: PortMediaType::Json, got: PortMediaType::Bytes },
                "MediaTypeMismatch",
            ),
            (PipelineError::TranscodeError("codec".into()), "TranscodeError"),
        ];
        for (err, name) in &variants {
            let kerml = err.to_kerml();
            assert!(kerml.contains(name), "expected '{}' in '{}'", name, kerml);
        }
    }

    // ── PipelineDag / pipeline_to_kerml tests ──────────────────────────────

    #[test]
    fn full_pipeline_renders_valid_kerml_package() {
        let stages = vec![
            make_stage("Ingest", &[], &[PortMediaType::Video]),
            make_stage("Transcode", &[PortMediaType::Video], &[PortMediaType::Audio]),
            make_stage("Publish", &[PortMediaType::Audio], &[]),
        ];
        let dag = PipelineDag::build(stages).unwrap();
        let kerml = pipeline_to_kerml(&dag);

        // Package wrapper
        assert!(kerml.starts_with("package "), "must start with package decl");
        assert!(kerml.contains("import ScalarValues::*;"), "missing import");
        assert!(kerml.contains("import SI::*;"), "missing SI import");

        // Stage definitions
        assert!(kerml.contains("part def Ingest"), "missing Ingest");
        assert!(kerml.contains("part def Transcode"), "missing Transcode");
        assert!(kerml.contains("part def Publish"), "missing Publish");

        // Connections
        assert!(kerml.contains("connect Ingest.p to Transcode.p;"), "missing Ingest→Transcode edge");
        assert!(kerml.contains("connect Transcode.p to Publish.p;"), "missing Transcode→Publish edge");

        // Closing brace
        assert!(kerml.trim_end().ends_with('}'), "must end with closing brace");
    }

    #[test]
    fn empty_pipeline_renders_minimal_package() {
        let dag = PipelineDag::build(vec![]).unwrap();
        let kerml = pipeline_to_kerml(&dag);
        assert!(kerml.starts_with("package Pipeline"));
        assert!(kerml.contains("import ScalarValues::*;"));
        assert!(kerml.ends_with("}\n"));
    }

    #[test]
    fn pipeline_with_fan_out_renders_multiple_connections() {
        let stages = vec![
            make_stage("Source", &[], &[PortMediaType::Video]),
            make_stage("SinkA", &[PortMediaType::Video], &[]),
            make_stage("SinkB", &[PortMediaType::Video], &[]),
        ];
        let dag = PipelineDag::build(stages).unwrap();
        let kerml = pipeline_to_kerml(&dag);
        assert!(kerml.contains("connect Source.p to SinkA.p;"));
        assert!(kerml.contains("connect Source.p to SinkB.p;"));
    }

    #[test]
    fn pipeline_with_error_routes_renders_actions() {
        let mut stage = make_stage("Processor", &[PortMediaType::Bytes], &[PortMediaType::Json]);
        stage.error_routes = vec![
            ErrorRoute {
                match_pattern: "*".into(),
                route_to_stage: "dead-letter".into(),
                max_retries: 3,
                backoff_ms: 1000,
                fallback_output: None,
                retry_count: 0,
            },
        ];
        let stages = vec![stage, make_stage("dead-letter", &[PortMediaType::Error], &[])];
        let dag = PipelineDag::build(stages).unwrap();
        let kerml = pipeline_to_kerml(&dag);
        assert!(kerml.contains("action ErrorRoute_star"), "got: {}", kerml);
    }

    // ── File write tests ───────────────────────────────────────────────────

    #[test]
    fn kerml_to_file_writes_valid_content() {
        let stages = vec![
            make_stage("StageA", &[], &[PortMediaType::Audio]),
            make_stage("StageB", &[PortMediaType::Audio], &[]),
        ];
        let dag = PipelineDag::build(stages).unwrap();

        let mut tmp = NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        // Write via the public API
        kerml_to_file(&dag, &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("package StageA"));
        assert!(content.contains("part def StageA"));
        assert!(content.contains("part def StageB"));
        assert!(content.contains("connect StageA.p to StageB.p;"));
    }

    #[test]
    fn kerml_to_file_overwrites_existing() {
        let stages = vec![make_stage("Only", &[], &[PortMediaType::Bytes])];
        let dag = PipelineDag::build(stages).unwrap();

        let mut tmp = NamedTempFile::new().unwrap();
        // Pre-write garbage
        write!(tmp, "garbage").unwrap();
        let path = tmp.path().to_path_buf();

        kerml_to_file(&dag, &path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("part def Only"), "expected overwritten content, got: {}", content);
    }

    // ── CapsuleProfile tests ───────────────────────────────────────────────

    #[test]
    fn capsule_profile_renders_constraint_block() {
        let profile = CapsuleProfile {
            name: "TestProfile".into(),
            ports: vec![],
            resources: ResourceRequirements {
                min_ram_gb: 4.0,
                min_vram_gb: 8.0,
                requires_gpu: true,
                cpu_cores: Some(8),
                scratch_disk_gb: Some(50.0),
            },
            image: None,
            timeout_seconds: None,
        };
        let kerml = profile.to_kerml();
        assert!(kerml.contains("constraint ResourceReq_TestProfile"));
        assert!(kerml.contains("min_ram_gb <= available_ram_gb"));
        assert!(kerml.contains("min_vram_gb <= available_vram_gb"));
        assert!(kerml.contains("requires_gpu == true"));
        assert!(kerml.contains("cpu_cores >= 8"));
    }

    // ── PipelineDag trait test ─────────────────────────────────────────────

    #[test]
    fn pipeline_dag_to_kerml_trait_works() {
        let stages = vec![
            make_stage("A", &[], &[PortMediaType::Bytes]),
            make_stage("B", &[PortMediaType::Bytes], &[]),
        ];
        let dag = PipelineDag::build(stages).unwrap();
        let kerml = dag.to_kerml();
        assert!(kerml.contains("package A"));
        assert!(kerml.contains("part def A"));
        assert!(kerml.contains("part def B"));
    }
}
