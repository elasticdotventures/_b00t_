use crate::traits::*;
use crate::{BootDatum, PipelineConfig, get_config};
use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A registered pipeline — a multi-stage, b00t-native discoverable capability.
///
/// Unlike `JustfileDatum` (which wraps a separately-executable `justfile`
/// artifact interpreted by the external `just` binary), a pipeline's stages
/// are declared inline in the datum's own `*.pipeline.tomllm` config — there
/// is no external pipeline interpreter binary to shell out to. Full stage
/// contracts (input/output/depends_on) are intentionally NOT modeled here;
/// that's the consuming application's job (see
/// `PIPELINE-DATUM-UFO-FOUNDATION.tomllmd` §Task 4). At this layer a stage is
/// just a name, and `CliExecutor::execute` validates + reports the resolved
/// stage sequence rather than dispatching real work — real dispatch is wired
/// in a later task's `JobExecutor`↔`ComputeProvider` bridge.
pub struct PipelineDatum {
    pub datum: BootDatum,
    /// Path to the `*.pipeline.tomllm` file describing this pipeline.
    pub pipeline_path: PathBuf,
    /// Ordered stage names, read once at construction from
    /// `[b00t.pipeline].stages`.
    stages: Vec<String>,
}

impl PipelineDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, filename) = get_config(name, path).map_err(|e| anyhow!("{}", e))?;
        let datum = config.b00t;
        let pipeline_path = Self::resolve_pipeline_path(&datum, path, &filename)?;
        let stages = Self::stage_names(&datum);
        Ok(PipelineDatum {
            datum,
            pipeline_path,
            stages,
        })
    }

    pub fn from_datum(datum: BootDatum, base_dir: &Path) -> Result<Self> {
        let fallback_filename = format!("{}.pipeline.tomllm", datum.name);
        let pipeline_path = Self::resolve_pipeline_path(
            &datum,
            &base_dir.display().to_string(),
            &fallback_filename,
        )?;
        let stages = Self::stage_names(&datum);
        Ok(PipelineDatum {
            datum,
            pipeline_path,
            stages,
        })
    }

    fn stage_names(datum: &BootDatum) -> Vec<String> {
        datum
            .pipeline
            .as_ref()
            .and_then(|p| p.stages.clone())
            .unwrap_or_default()
    }

    /// Resolve the on-disk path of the pipeline definition.
    /// `[b00t.pipeline].path` overrides; otherwise falls back to the file
    /// that `get_config` (or the caller) discovered this datum in — a
    /// pipeline's definition IS its datum file, unlike a justfile which
    /// wraps a separate artifact.
    fn resolve_pipeline_path(
        datum: &BootDatum,
        base_dir: &str,
        discovered_filename: &str,
    ) -> Result<PathBuf> {
        let relative = datum
            .pipeline
            .as_ref()
            .and_then(|p| p.path.as_deref())
            .unwrap_or(discovered_filename);

        let base = Path::new(base_dir);
        let path = if Path::new(relative).is_absolute() {
            PathBuf::from(relative)
        } else {
            base.join(relative)
        };
        Ok(path)
    }

    fn pipeline_config(&self) -> PipelineConfig {
        self.datum.pipeline.clone().unwrap_or_default()
    }

    /// The ordered stage names declared by this pipeline.
    pub fn stages(&self) -> &[String] {
        &self.stages
    }
}

impl TryFrom<(&str, &str)> for PipelineDatum {
    type Error = anyhow::Error;
    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
    }
}

impl DatumChecker for PipelineDatum {
    fn is_installed(&self) -> bool {
        // No single external binary gates a pipeline the way `just` gates a
        // JustfileDatum — a pipeline with zero declared stages is the closest
        // analog to "not installed": it's a datum that exists but declares
        // nothing runnable.
        self.pipeline_path.exists() && !self.stages.is_empty()
    }

    fn current_version(&self) -> Option<String> {
        // Use mtime as the "version" — same convention as JustfileDatum.
        std::fs::metadata(&self.pipeline_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs().to_string())
                    .unwrap_or_else(|_| "unknown".to_string())
            })
    }

    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }

    fn version_status(&self) -> VersionStatus {
        if !self.pipeline_path.exists() {
            VersionStatus::Missing
        } else if self.stages.is_empty() {
            VersionStatus::Missing
        } else {
            VersionStatus::Unknown // pipelines don't carry semver
        }
    }
}

impl StatusProvider for PipelineDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }
    fn subsystem(&self) -> &str {
        "pipeline"
    }
    fn hint(&self) -> &str {
        &self.datum.hint
    }
    fn is_disabled(&self) -> bool {
        false
    }
}

impl FilterLogic for PipelineDatum {
    fn is_available(&self) -> bool {
        self.pipeline_path.exists()
    }
    fn prerequisites_satisfied(&self) -> bool {
        !self.stages.is_empty()
    }
    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
}

impl ConstraintEvaluator for PipelineDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for PipelineDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

// ── CliExecutor impl ──────────────────────────────────────────────────────────

impl CliExecutor for PipelineDatum {
    /// Validates and resolves the requested stage sequence.
    ///
    /// 🤓 Deliberate divergence from `JustfileDatum::execute`: there is no
    /// external interpreter (like `just`) to hand this off to yet. Real
    /// per-stage dispatch (local shell-out, `ComputeProvider` submission,
    /// parallel fan-out) is `JobExecutor`'s job, wired in a later task. This
    /// method's contract for now: validate the pipeline is well-formed and
    /// report the stage order that *would* run — `value` is a `" -> "`
    /// joined stage list, `declared_effects` flags it as a dispatching
    /// operation so sandboxing/audit treats it consistently with a real run.
    fn execute(&self, args: &[String]) -> Result<ExecOutput<String>> {
        if !self.pipeline_path.exists() {
            return Err(anyhow!(
                "pipeline definition not found: {}",
                self.pipeline_path.display()
            ));
        }
        if self.stages.is_empty() {
            return Err(anyhow!(
                "pipeline '{}' declares no stages",
                self.datum.name
            ));
        }

        let selected = self.resolve_selected_stages(args)?;
        let start = Instant::now();
        let value = selected.join(" -> ");

        Ok(ExecOutput {
            value,
            exit_code: 0,
            duration_ms: start.elapsed().as_millis() as u64,
            sandbox: self.sandbox_requirements(),
            sandbox_kind: self
                .allowed_sandboxes()
                .into_iter()
                .next()
                .unwrap_or(SandboxKind::None),
            io_method: self.io_method(),
            declared_effects: vec!["pipeline-stage-dispatch".to_string()],
        })
    }

    fn dry_run(&self, args: &[String]) -> Result<ExecPlan> {
        let selected = self.resolve_selected_stages(args)?;

        let sandbox_kind = self
            .allowed_sandboxes()
            .into_iter()
            .next()
            .unwrap_or(SandboxKind::None);
        let io_method = IoMethod::for_sandbox(&sandbox_kind);

        Ok(ExecPlan {
            command_line: format!("pipeline {}: {}", self.datum.name, selected.join(" -> ")),
            working_dir: self
                .pipeline_path
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(".")),
            env: vec![],
            declared_effects: vec!["pipeline-stage-dispatch".to_string()],
            sandbox_kind,
            io_method,
        })
    }

    /// One `CommandSignature` per declared stage — a pipeline's stages are
    /// its "recipes". No parameters/dependencies are modeled at this layer.
    fn list_commands(&self) -> Result<Vec<CommandSignature>> {
        Ok(self
            .stages
            .iter()
            .map(|s| CommandSignature {
                name: s.clone(),
                description: None,
                parameters: vec![],
                dependencies: vec![],
                private: false,
            })
            .collect())
    }

    fn sandbox_requirements(&self) -> SandboxRequirements {
        let caps = self.pipeline_config().capabilities.unwrap_or_default();
        let pipeline_dir = self
            .pipeline_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        let mut filesystem = vec![pipeline_dir];
        if let Some(paths) = caps.filesystem {
            for p in paths {
                let path = if Path::new(&p).is_absolute() {
                    PathBuf::from(&p)
                } else {
                    self.pipeline_path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(&p)
                };
                if !filesystem.contains(&path) {
                    filesystem.push(path);
                }
            }
        }

        SandboxRequirements {
            network: caps.network.unwrap_or(false),
            filesystem,
            env_vars: caps.env_vars.unwrap_or_default(),
            secrets: caps.secrets.unwrap_or_default(),
            max_duration: None,
        }
    }

    fn allowed_sandboxes(&self) -> Vec<SandboxKind> {
        let config = self.pipeline_config();
        if let Some(kinds) = config.allowed_sandboxes {
            kinds.iter().map(|s| SandboxKind::from_str(s)).collect()
        } else if let Some(single) = config.sandbox {
            vec![SandboxKind::from_str(&single)]
        } else {
            // Default: None (direct execution) is always permitted
            vec![SandboxKind::None]
        }
    }

    fn io_method(&self) -> IoMethod {
        let preferred_sandbox = self
            .allowed_sandboxes()
            .into_iter()
            .next()
            .unwrap_or(SandboxKind::None);
        IoMethod::for_sandbox(&preferred_sandbox)
    }
}

impl PipelineDatum {
    /// Resolve which declared stages to run: explicit `args` select a subset
    /// (order preserved, unknown names rejected), empty `args` selects all
    /// stages in declaration order.
    fn resolve_selected_stages(&self, args: &[String]) -> Result<Vec<String>> {
        if args.is_empty() {
            return Ok(self.stages.clone());
        }
        let mut selected = Vec::with_capacity(args.len());
        for a in args {
            if !self.stages.contains(a) {
                return Err(anyhow!(
                    "pipeline '{}' has no stage named '{}' (known stages: {})",
                    self.datum.name,
                    a,
                    self.stages.join(", ")
                ));
            }
            selected.push(a.clone());
        }
        Ok(selected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PipelineConfig;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn tmp(c: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", c).unwrap();
        f
    }

    fn mkd(path: &std::path::Path, stages: &[&str]) -> PipelineDatum {
        let mut datum = BootDatum {
            name: "test-pipeline".to_string(),
            ..Default::default()
        };
        datum.pipeline = Some(PipelineConfig {
            stages: Some(stages.iter().map(|s| s.to_string()).collect()),
            ..Default::default()
        });
        PipelineDatum {
            datum,
            pipeline_path: path.to_path_buf(),
            stages: stages.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn is_installed_true_when_file_exists_and_has_stages() {
        let f = tmp("[b00t]\nname = \"x\"\n");
        let d = mkd(f.path(), &["extract", "generate", "inject"]);
        assert!(d.is_installed());
    }

    #[test]
    fn is_installed_false_when_no_stages() {
        let f = tmp("[b00t]\nname = \"x\"\n");
        let d = mkd(f.path(), &[]);
        assert!(!d.is_installed());
    }

    #[test]
    fn is_installed_false_when_file_missing() {
        let d = mkd(Path::new("/nonexistent/pipeline.pipeline.tomllm"), &["a"]);
        assert!(!d.is_installed());
    }

    #[test]
    fn list_commands_returns_one_per_stage_in_order() {
        let f = tmp("[b00t]\nname = \"x\"\n");
        let d = mkd(f.path(), &["extract", "generate", "inject"]);
        let cmds = d.list_commands().unwrap();
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["extract", "generate", "inject"]);
        assert!(cmds.iter().all(|c| !c.private));
    }

    #[test]
    fn execute_with_no_args_runs_all_stages_in_order() {
        let f = tmp("[b00t]\nname = \"x\"\n");
        let d = mkd(f.path(), &["extract", "generate", "inject"]);
        let out = d.execute(&[]).unwrap();
        assert_eq!(out.value, "extract -> generate -> inject");
        assert_eq!(out.exit_code, 0);
    }

    #[test]
    fn execute_with_args_selects_subset() {
        let f = tmp("[b00t]\nname = \"x\"\n");
        let d = mkd(f.path(), &["extract", "generate", "inject"]);
        let out = d.execute(&["generate".to_string()]).unwrap();
        assert_eq!(out.value, "generate");
    }

    #[test]
    fn execute_rejects_unknown_stage() {
        let f = tmp("[b00t]\nname = \"x\"\n");
        let d = mkd(f.path(), &["extract", "generate"]);
        let err = d.execute(&["bogus".to_string()]).unwrap_err();
        assert!(err.to_string().contains("bogus"));
    }

    #[test]
    fn execute_fails_when_no_stages_declared() {
        let f = tmp("[b00t]\nname = \"x\"\n");
        let d = mkd(f.path(), &[]);
        assert!(d.execute(&[]).is_err());
    }

    #[test]
    fn execute_fails_when_pipeline_file_missing() {
        let d = mkd(Path::new("/nonexistent/pipeline.pipeline.tomllm"), &["a"]);
        assert!(d.execute(&[]).is_err());
    }

    #[test]
    fn dry_run_reports_stage_sequence_without_side_effects_field_conflation() {
        let f = tmp("[b00t]\nname = \"x\"\n");
        let d = mkd(f.path(), &["extract", "generate"]);
        let plan = d.dry_run(&[]).unwrap();
        assert!(plan.command_line.contains("extract -> generate"));
        assert_eq!(plan.declared_effects, vec!["pipeline-stage-dispatch"]);
    }

    #[test]
    fn allowed_sandboxes_defaults_to_none() {
        let f = tmp("[b00t]\nname = \"x\"\n");
        let d = mkd(f.path(), &["a"]);
        assert_eq!(d.allowed_sandboxes(), vec![SandboxKind::None]);
    }

    #[test]
    fn sandbox_requirements_includes_pipeline_dir() {
        let f = tmp("[b00t]\nname = \"x\"\n");
        let d = mkd(f.path(), &["a"]);
        let reqs = d.sandbox_requirements();
        let pipeline_dir = f.path().parent().unwrap().to_path_buf();
        assert!(reqs.filesystem.contains(&pipeline_dir));
    }

    #[test]
    fn version_status_missing_when_no_stages() {
        let f = tmp("[b00t]\nname = \"x\"\n");
        let d = mkd(f.path(), &[]);
        assert_eq!(d.version_status(), VersionStatus::Missing);
    }

    #[test]
    fn version_status_unknown_when_installed() {
        let f = tmp("[b00t]\nname = \"x\"\n");
        let d = mkd(f.path(), &["a"]);
        assert_eq!(d.version_status(), VersionStatus::Unknown);
    }
}
