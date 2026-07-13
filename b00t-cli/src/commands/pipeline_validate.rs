//! `b00t pipeline validate` — static validation of pipeline DAG before execution.
//!
//! Validates a named pipeline datum against these rules:
//! - Stage names are unique
//! - Input/output ports reference valid media types
//! - All edge targets reference existing stages
//! - No cycles in the DAG
//! - Resource requirements are sane (no negative values)
//! - Error routes reference valid stage names
//!
//! # Usage
//! ```bash
//! b00t pipeline validate my-pipeline
//! ```

use crate::ansi;
use crate::commands::pipeline::discover_pipeline_datums;
use crate::datum_pipeline::PipelineDatum;
use crate::pipeline_types::{PipelineDag, StageSpec};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;

// ── Report types ──────────────────────────────────────────────────────────────

/// Per-pipeline validation report.
#[derive(Debug, Clone, Serialize)]
pub struct ValidateReport {
    pub pipeline_name: String,
    pub stages: Vec<StageValidation>,
    pub summary: ValidationSummary,
}

/// Validation result for a single stage.
#[derive(Debug, Clone, Serialize)]
pub struct StageValidation {
    pub stage_name: String,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Aggregate validation summary across all stages.
#[derive(Debug, Clone, Serialize)]
pub struct ValidationSummary {
    pub total_stages: usize,
    pub errors: usize,
    pub warnings: usize,
    pub passed: bool,
}

// ── Validation logic ──────────────────────────────────────────────────────────

/// Validate a named pipeline datum. Returns a structured report.
pub fn validate_pipeline(name: &str, b00t_path: &str) -> anyhow::Result<ValidateReport> {
    let pipelines = discover_pipeline_datums(b00t_path)?;
    let (_, datum, file_path) = pipelines
        .into_iter()
        .find(|(key, _, _)| key == name)
        .ok_or_else(|| anyhow::anyhow!("no pipeline datum named '{}'", name))?;

    let base_dir = Path::new(&file_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let pipeline = PipelineDatum::from_datum(datum, &base_dir)?;

    // Build StageSpecs from stage names
    let stage_specs: Vec<StageSpec> = pipeline
        .stages()
        .iter()
        .map(|name| StageSpec::from_name(name))
        .collect();

    let dag = PipelineDag::from_sequential(stage_specs);
    let mut stage_validations: Vec<StageValidation> = Vec::new();
    let mut total_errors = 0usize;
    let mut total_warnings = 0usize;

    // ── Rule 1: Stage names unique ────────────────────────────────────────
    let unique_errors = validate_unique_stage_names(&dag);
    for (stage_name, errs) in &unique_errors {
        total_errors += errs.len();
        stage_validations.push(StageValidation {
            stage_name: stage_name.clone(),
            errors: errs.clone(),
            warnings: vec![],
        });
    }

    // ── Rule 2: Valid media types on ports ────────────────────────────────
    let port_errors = validate_port_media_types(&dag);
    for (stage_name, errs) in &port_errors {
        total_errors += errs.len();
        push_or_create_validation(&mut stage_validations, stage_name, errs, &[]);
    }

    // ── Rule 3: All edge targets exist ────────────────────────────────────
    let edge_target_errors = validate_edge_targets(&dag);
    if !edge_target_errors.is_empty() {
        total_errors += edge_target_errors.len();
        // Edge target errors are global, attach to first stage or create pipeline-level
        for err in &edge_target_errors {
            push_or_create_validation(
                &mut stage_validations,
                &dag.stages.first().map(|s| s.name.as_str()).unwrap_or("(pipeline)"),
                &[err.clone()],
                &[],
            );
        }
    }

    // ── Rule 4: No cycles in DAG ──────────────────────────────────────────
    if let Some(cycle) = dag.detect_cycle() {
        let cycle_path = cycle.join(" → ");
        let cycle_err = format!("cycle detected in DAG: {}", cycle_path);
        total_errors += 1;
        // Attach to the first stage in the cycle
        let first_in_cycle = cycle.first().cloned().unwrap_or_else(|| "(pipeline)".into());
        push_or_create_validation(
            &mut stage_validations,
            &first_in_cycle,
            &[cycle_err],
            &[],
        );
    }

    // ── Rule 5: Resource requirement sanity ───────────────────────────────
    let resource_errors = validate_resource_sanity(&dag);
    for (stage_name, errs) in &resource_errors {
        total_errors += errs.len();
        push_or_create_validation(&mut stage_validations, stage_name, errs, &[]);
    }

    // ── Rule 6: Error routes reference valid stages ───────────────────────
    let error_route_warnings = validate_error_routes(&dag);
    for (stage_name, warns) in &error_route_warnings {
        total_warnings += warns.len();
        push_or_create_validation(&mut stage_validations, stage_name, &[], warns);
    }

    // ── Fill in stages with no validation issues ──────────────────────────
    let validated_names: HashSet<String> =
        stage_validations.iter().map(|v| v.stage_name.clone()).collect();
    let mut clean_stages: Vec<StageValidation> = dag
        .stages
        .iter()
        .filter(|s| !validated_names.contains(&s.name))
        .map(|s| StageValidation {
            stage_name: s.name.clone(),
            errors: vec![],
            warnings: vec![],
        })
        .collect();
    stage_validations.append(&mut clean_stages);

    // Sort stages by declaration order
    let order: std::collections::HashMap<&str, usize> = dag
        .stages
        .iter()
        .enumerate()
        .map(|(i, s)| (s.name.as_str(), i))
        .collect();
    stage_validations.sort_by_key(|v| order.get(v.stage_name.as_str()).copied().unwrap_or(usize::MAX));

    let passed = total_errors == 0;

    Ok(ValidateReport {
        pipeline_name: name.to_string(),
        stages: stage_validations,
        summary: ValidationSummary {
            total_stages: dag.stages.len(),
            errors: total_errors,
            warnings: total_warnings,
            passed,
        },
    })
}

// ── Individual validation rules ───────────────────────────────────────────────

fn validate_unique_stage_names(dag: &PipelineDag) -> Vec<(String, Vec<String>)> {
    let mut seen = HashSet::new();
    let mut duplicates = HashSet::new();
    for stage in &dag.stages {
        if !seen.insert(stage.name.as_str()) {
            duplicates.insert(stage.name.as_str());
        }
    }
    duplicates
        .iter()
        .map(|name| {
            (
                name.to_string(),
                vec![format!("duplicate stage name '{}'", name)],
            )
        })
        .collect()
}

fn validate_port_media_types(dag: &PipelineDag) -> Vec<(String, Vec<String>)> {
    let mut result: Vec<(String, Vec<String>)> = Vec::new();
    let valid_types: HashSet<&str> =
        ["Video", "Audio", "Image", "Json", "Parquet", "Bytes", "Error"]
            .iter()
            .cloned()
            .collect();

    for stage in &dag.stages {
        let mut errors = Vec::new();
        for port in &stage.input_ports {
            let variant = format!("{:?}", port.media_type);
            if !valid_types.contains(variant.as_str()) {
                errors.push(format!(
                    "invalid input port media type '{:?}'",
                    port.media_type
                ));
            }
        }
        for port in &stage.output_ports {
            let variant = format!("{:?}", port.media_type);
            if !valid_types.contains(variant.as_str()) {
                errors.push(format!(
                    "invalid output port media type '{:?}'",
                    port.media_type
                ));
            }
        }
        if !errors.is_empty() {
            result.push((stage.name.clone(), errors));
        }
    }
    result
}

fn validate_edge_targets(dag: &PipelineDag) -> Vec<String> {
    let stage_names: HashSet<&str> = dag.stages.iter().map(|s| s.name.as_str()).collect();
    let mut errors = Vec::new();

    for edge in &dag.edges {
        if !stage_names.contains(edge.from.as_str()) {
            errors.push(format!(
                "edge source '{}' does not reference a known stage",
                edge.from
            ));
        }
        if !stage_names.contains(edge.to.as_str()) {
            errors.push(format!(
                "edge target '{}' does not reference a known stage",
                edge.to
            ));
        }
    }
    errors
}

fn validate_resource_sanity(dag: &PipelineDag) -> Vec<(String, Vec<String>)> {
    let mut result: Vec<(String, Vec<String>)> = Vec::new();
    for stage in &dag.stages {
        let mut errors = Vec::new();
        let r = &stage.profile.resources;
        if r.min_ram_gb < 0.0 {
            errors.push(format!("min_ram_gb ({}) cannot be negative", r.min_ram_gb));
        }
        if r.min_vram_gb < 0.0 {
            errors.push(format!("min_vram_gb ({}) cannot be negative", r.min_vram_gb));
        }
        if let Some(cores) = r.cpu_cores {
            if cores == 0 {
                errors.push("cpu_cores cannot be zero".to_string());
            }
        }
        if let Some(disk) = r.scratch_disk_gb {
            if disk < 0.0 {
                errors.push(format!("scratch_disk_gb ({}) cannot be negative", disk));
            }
        }
        if !errors.is_empty() {
            result.push((stage.name.clone(), errors));
        }
    }
    result
}

fn validate_error_routes(dag: &PipelineDag) -> Vec<(String, Vec<String>)> {
    let stage_names: HashSet<&str> = dag.stages.iter().map(|s| s.name.as_str()).collect();
    let mut result: Vec<(String, Vec<String>)> = Vec::new();

    for stage in &dag.stages {
        let mut warnings = Vec::new();
        for route in &stage.error_routes {
            if !stage_names.contains(route.route_to_stage.as_str()) {
                warnings.push(format!(
                    "error route '{}' → '{}' references unknown stage",
                    route.match_pattern, route.route_to_stage
                ));
            }
        }
        if !warnings.is_empty() {
            result.push((stage.name.clone(), warnings));
        }
    }
    result
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn push_or_create_validation(
    validations: &mut Vec<StageValidation>,
    stage_name: &str,
    errors: &[String],
    warnings: &[String],
) {
    if let Some(existing) = validations.iter_mut().find(|v| v.stage_name == stage_name) {
        existing.errors.extend_from_slice(errors);
        existing.warnings.extend_from_slice(warnings);
    } else {
        validations.push(StageValidation {
            stage_name: stage_name.to_string(),
            errors: errors.to_vec(),
            warnings: warnings.to_vec(),
        });
    }
}

// ── Formatting ────────────────────────────────────────────────────────────────

/// Print a human-readable validation report with ANSI colored formatting.
pub fn print_validate_report(report: &ValidateReport) {
    let header = format!(
        "\n{} {} {}\n",
        ansi::bold("Pipeline:"),
        ansi::cyan(&report.pipeline_name),
        ansi::dim(&format!("({} stages)", report.summary.total_stages))
    );
    println!("{}", header);

    println!("{}", ansi::bold("── Stages ──"));

    for stage in &report.stages {
        let status_icon = if stage.errors.is_empty() && stage.warnings.is_empty() {
            ansi::green("  ✓")
        } else if stage.errors.is_empty() {
            ansi::yellow("  ⚠")
        } else {
            ansi::red("  ✗")
        };

        println!("{} {}", status_icon, ansi::bold(&stage.stage_name));

        for err in &stage.errors {
            println!("    {} {}", ansi::red("error:"), err);
        }
        for warn in &stage.warnings {
            println!("    {} {}", ansi::yellow("warning:"), warn);
        }
    }

    println!();
    println!("{}", ansi::bold("── Summary ──"));
    println!(
        "  {} {} {} {} {} {}",
        ansi::dim("stages:"),
        ansi::bold(&report.summary.total_stages.to_string()),
        ansi::dim("| errors:"),
        if report.summary.errors > 0 {
            ansi::red(&report.summary.errors.to_string())
        } else {
            ansi::green(&report.summary.errors.to_string())
        },
        ansi::dim("| warnings:"),
        if report.summary.warnings > 0 {
            ansi::yellow(&report.summary.warnings.to_string())
        } else {
            ansi::green(&report.summary.warnings.to_string())
        },
    );

    if report.summary.passed {
        println!("\n  {} Validation passed", ansi::green("✅"));
    } else {
        println!(
            "\n  {} Validation failed — {} error(s), {} warning(s)",
            ansi::red("❌"),
            report.summary.errors,
            report.summary.warnings
        );
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_types::{
        CapsuleProfile, ErrorRoute, PipelineEdge, PortDirection, PortMediaType, ResourceRequirements,
        StagePort, StageSpec,
    };

    fn spec(name: &str) -> StageSpec {
        StageSpec {
            name: name.into(),
            profile: CapsuleProfile {
                name: name.into(),
                ports: vec![],
                resources: ResourceRequirements {
                    min_ram_gb: 1.0,
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
                description: None,
            }],
            output_ports: vec![StagePort {
                direction: PortDirection::Output,
                media_type: PortMediaType::Bytes,
                description: None,
            }],
            error_routes: vec![],
            env: None,
            checkpoint_interval_seconds: None,
            secret_refs: None,
            flow_control: None,
        }
    }

    #[test]
    fn test_validate_unique_stage_names_ok() {
        let stages = vec![spec("a"), spec("b")];
        let dag = PipelineDag::from_sequential(stages);
        let results = validate_unique_stage_names(&dag);
        assert!(results.is_empty());
    }

    #[test]
    fn test_validate_unique_stage_names_duplicate() {
        let stages = vec![spec("a"), spec("a")];
        let dag = PipelineDag::from_sequential(stages);
        let results = validate_unique_stage_names(&dag);
        assert!(!results.is_empty());
        assert!(results[0].1[0].contains("duplicate"));
    }

    #[test]
    fn test_validate_port_media_types_valid() {
        let stages = vec![spec("a")];
        let dag = PipelineDag::from_sequential(stages);
        let results = validate_port_media_types(&dag);
        assert!(results.is_empty());
    }

    #[test]
    fn test_validate_edge_targets_all_valid() {
        let stages = vec![spec("a"), spec("b")];
        let dag = PipelineDag::from_sequential(stages);
        let errors = validate_edge_targets(&dag);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_edge_targets_missing() {
        let stages = vec![spec("a")];
        let edges = vec![PipelineEdge {
            from: "a".into(),
            to: "nonexistent".into(),
            via_port: None,
        }];
        let dag = PipelineDag { stages, edges, entry_points: vec!["a".into()], exit_points: vec!["nonexistent".into()] };
        let errors = validate_edge_targets(&dag);
        assert!(!errors.is_empty());
        assert!(errors[0].contains("nonexistent"));
    }

    #[test]
    fn test_detect_cycle_none() {
        let stages = vec![spec("a"), spec("b"), spec("c")];
        let dag = PipelineDag::from_sequential(stages);
        assert!(dag.detect_cycle().is_none());
    }

    #[test]
    fn test_detect_cycle_present() {
        let stages = vec![spec("a"), spec("b")];
        let edges = vec![
            PipelineEdge { from: "a".into(), to: "b".into(), via_port: None },
            PipelineEdge { from: "b".into(), to: "a".into(), via_port: None },
        ];
        let dag = PipelineDag { stages, edges, entry_points: vec![], exit_points: vec![] };
        assert!(dag.detect_cycle().is_some());
    }

    #[test]
    fn test_validate_resource_sanity_ok() {
        let stages = vec![spec("a")];
        let dag = PipelineDag::from_sequential(stages);
        let results = validate_resource_sanity(&dag);
        assert!(results.is_empty());
    }

    #[test]
    fn test_validate_resource_sanity_negative_ram() {
        let mut s = spec("a");
        s.profile.resources.min_ram_gb = -1.0;
        let stages = vec![s];
        let dag = PipelineDag::from_sequential(stages);
        let results = validate_resource_sanity(&dag);
        assert!(!results.is_empty());
        assert!(results[0].1[0].contains("negative"));
    }

    #[test]
    fn test_validate_resource_sanity_zero_cpu_cores() {
        let mut s = spec("a");
        s.profile.resources.cpu_cores = Some(0);
        let stages = vec![s];
        let dag = PipelineDag::from_sequential(stages);
        let results = validate_resource_sanity(&dag);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_validate_error_routes_valid() {
        let stages = vec![spec("a"), spec("b")];
        let dag = PipelineDag::from_sequential(stages);
        let results = validate_error_routes(&dag);
        assert!(results.is_empty());
    }

    #[test]
    fn test_validate_error_routes_unknown_target() {
        let mut s = spec("a");
        s.error_routes = vec![ErrorRoute {
            match_pattern: "*".into(),
            route_to_stage: "nonexistent".into(),
            max_retries: 3,
            backoff_ms: 100,
            fallback_output: None,
            retry_count: 0,
        }];
        let stages = vec![s, spec("b")];
        let dag = PipelineDag::from_sequential(stages);
        let results = validate_error_routes(&dag);
        assert!(!results.is_empty());
        assert!(results[0].1[0].contains("nonexistent"));
    }

    #[test]
    fn test_full_validation_passes() {
        let stages = vec![spec("ingest"), spec("process"), spec("output")];
        let dag = PipelineDag::from_sequential(stages);
        let mut validations = Vec::new();

        // Run all validations
        for (name, errs) in validate_unique_stage_names(&dag) {
            push_or_create_validation(&mut validations, &name, &errs, &[]);
        }
        for (name, errs) in validate_port_media_types(&dag) {
            push_or_create_validation(&mut validations, &name, &errs, &[]);
        }
        let edge_errors = validate_edge_targets(&dag);
        if !edge_errors.is_empty() {
            push_or_create_validation(&mut validations, "ingest", &edge_errors, &[]);
        }
        if let Some(cycle) = dag.detect_cycle() {
            let err = format!("cycle: {}", cycle.join(" → "));
            push_or_create_validation(&mut validations, "ingest", &[err], &[]);
        }
        for (name, errs) in validate_resource_sanity(&dag) {
            push_or_create_validation(&mut validations, &name, &errs, &[]);
        }
        for (name, warns) in validate_error_routes(&dag) {
            push_or_create_validation(&mut validations, &name, &[], &warns);
        }

        let total_errors: usize = validations.iter().map(|v| v.errors.len()).sum();
        assert_eq!(total_errors, 0, "no errors expected for clean pipeline");
    }

    #[test]
    fn test_validate_report_serialization() {
        let report = ValidateReport {
            pipeline_name: "test".into(),
            stages: vec![StageValidation {
                stage_name: "s1".into(),
                errors: vec![],
                warnings: vec!["watch out".into()],
            }],
            summary: ValidationSummary {
                total_stages: 1,
                errors: 0,
                warnings: 1,
                passed: true,
            },
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("pipeline_name"));
        assert!(json.contains("test"));
        assert!(json.contains("watch out"));
    }
}
