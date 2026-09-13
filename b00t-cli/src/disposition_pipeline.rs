//! IS.SYSTEM.N0RMAL — composable disposition pipeline.
//!
//! Chains `Satisfies<C>` implementations into a directed evaluation
//! pipeline where each stage produces `Result<Disposition>`. Stages
//! are abstract — any type that implements `PipelineStage` can service
//! any position in the chain, provided it produces the expected output
//! contract.
//!
//! ```text
//! ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
//! │  GateSpec │───▶│ Checklist│───▶│ Compose  │───▶│  Result  │
//! │  (check)  │    │  (AND)   │    │  (rhai)  │    │ (0/1/2)  │
//! └──────────┘    └──────────┘    └──────────┘    └──────────┘
//!   Disposition     Vec<Disp>       Disposition    exit code
//! ```
//!
//! Every stage is `async` + `Send` — usable from tokio, wasm-bindgen,
//! or synchronous `block_on`. The pipeline is a value, not a singleton;
//! multiple pipelines can run concurrently on independent checklists.

use anyhow::Result;
use std::future::Future;
use std::pin::Pin;
use ufo_types::Disposition;

// ── Core trait ──────────────────────────────────────────────────────────────

/// A single stage in a disposition pipeline.
///
/// Implementors declare their input constraint type `C` and produce
/// `Result<Disposition>`. The trait is object-safe for dynamic dispatch
/// across heterogeneous stage types in the same pipeline.
pub trait PipelineStage: Send + Sync {
    /// Human-readable stage name (for viz, audit, logging).
    fn name(&self) -> &str;

    /// Evaluate this stage. Returns `Disposition` or an infra error
    /// (not a check failure — that's `Disposition::Violated`).
    fn eval<'a>(
        &'a self,
        ctx: &'a PipelineContext,
    ) -> Pin<Box<dyn Future<Output = Result<Disposition>> + Send + 'a>>;
}

// ── Context ─────────────────────────────────────────────────────────────────

/// Shared context threaded through every stage — the "state" that
/// stages can read but not write (immutable pipeline state).
#[derive(Debug, Clone)]
pub struct PipelineContext {
    /// Base path for file/command checks (usually `_b00t_/`).
    pub base_path: String,
    /// Pipeline name (e.g. "system-normal").
    pub pipeline_name: String,
    /// Accumulated dispositions from prior stages (for compose_rhai).
    pub prior: Vec<Disposition>,
}

// ── Gate stage (wraps GateSpec) ──────────────────────────────────────────────

/// Wraps a `GateSpec` as a `PipelineStage`. Evaluates synchronously
/// (file/command/env checks are inherently blocking) and wraps in
/// `ready()`.
pub struct GateStage {
    pub id: String,
    pub spec: crate::gates::GateSpec,
}

impl PipelineStage for GateStage {
    fn name(&self) -> &str {
        &self.id
    }

    fn eval<'a>(
        &'a self,
        ctx: &'a PipelineContext,
    ) -> Pin<Box<dyn Future<Output = Result<Disposition>> + Send + 'a>> {
        Box::pin(async move {
            Ok(self.spec.eval_disposition(&ctx.base_path))
        })
    }
}

// ── Checklist stage (AND-composition of gates) ──────────────────────────────

/// Evaluates a `ChecklistFile` and collapses to a single `Disposition`.
/// This is the implicit-AND composition — all checks must be Satisfied.
pub struct ChecklistStage {
    pub checklist: crate::checklist::ChecklistFile,
}

impl PipelineStage for ChecklistStage {
    fn name(&self) -> &str {
        &self.checklist.b00t.name
    }

    fn eval<'a>(
        &'a self,
        ctx: &'a PipelineContext,
    ) -> Pin<Box<dyn Future<Output = Result<Disposition>> + Send + 'a>> {
        Box::pin(async move {
            let result = self.checklist.evaluate(&ctx.base_path);
            Ok(match result.disposition {
                crate::checklist::ChecklistDisposition::Satisfied => Disposition::Satisfied,
                crate::checklist::ChecklistDisposition::Violated { failing } => {
                    Disposition::Violated {
                        reason: failing.join("; "),
                    }
                }
                crate::checklist::ChecklistDisposition::Unknown { .. } => Disposition::Unknown,
            })
        })
    }
}

// ── Compose stage (Rhai expression over prior dispositions) ─────────────────

/// Evaluates a Rhai expression over accumulated dispositions from
/// prior stages. Variables are named by prior stage `name()`.
///
/// Phase 2 of CONOPS-system-normal.md — deferred until Rhai shell-exec
/// sandbox is designed. This is the scaffold.
pub struct ComposeStage {
    pub expression: String,
}

impl PipelineStage for ComposeStage {
    fn name(&self) -> &str {
        "compose"
    }

    fn eval<'a>(
        &'a self,
        ctx: &'a PipelineContext,
    ) -> Pin<Box<dyn Future<Output = Result<Disposition>> + Send + 'a>> {
        Box::pin(async move {
            // TODO: Phase 2 — evaluate self.expression in Rhai scope
            // populated with bool vars from ctx.prior (Satisfied=true, else=false).
            // For now, fall through to implicit-AND of prior dispositions.
            if ctx.prior.iter().all(|d| matches!(d, Disposition::Satisfied)) {
                Ok(Disposition::Satisfied)
            } else if ctx.prior.iter().any(|d| matches!(d, Disposition::Violated { .. })) {
                let reasons: Vec<String> = ctx.prior.iter().filter_map(|d| match d {
                    Disposition::Violated { reason } => Some(reason.clone()),
                    _ => None,
                }).collect();
                Ok(Disposition::Violated { reason: reasons.join("; ") })
            } else {
                Ok(Disposition::Unknown)
            }
        })
    }
}

// ── Pipeline executor ───────────────────────────────────────────────────────

/// A pipeline is an ordered sequence of stages. Evaluation runs each
/// stage sequentially, threading `PipelineContext` through. The final
/// disposition is the pipeline's answer.
pub struct DispositionPipeline {
    pub name: String,
    pub stages: Vec<Box<dyn PipelineStage>>,
}

impl DispositionPipeline {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            stages: Vec::new(),
        }
    }

    pub fn push(&mut self, stage: impl PipelineStage + 'static) {
        self.stages.push(Box::new(stage));
    }

    /// Run the pipeline. Returns the final `Disposition` after all stages.
    /// Each stage's result is accumulated in `ctx.prior` for downstream
    /// compose stages.
    pub async fn run(&self, base_path: &str) -> Result<Disposition> {
        let mut ctx = PipelineContext {
            base_path: base_path.to_string(),
            pipeline_name: self.name.clone(),
            prior: Vec::new(),
        };

        let mut final_disposition = Disposition::Satisfied;

        for stage in &self.stages {
            let disposition = stage.eval(&ctx).await?;
            ctx.prior.push(disposition.clone());

            // Track the aggregate: Violated wins over Unknown wins over Satisfied.
            match &disposition {
                Disposition::Violated { .. } => final_disposition = disposition.clone(),
                Disposition::Unknown if !matches!(final_disposition, Disposition::Violated { .. }) => {
                    final_disposition = Disposition::Unknown;
                }
                _ => {}
            }
        }

        Ok(final_disposition)
    }

    /// Run and map to exit code: 0=Satisfied, 1=Violated, 2=Unknown.
    pub async fn run_exit_code(&self, base_path: &str) -> i32 {
        match self.run(base_path).await {
            Ok(Disposition::Satisfied) => 0,
            Ok(Disposition::Violated { .. }) => 1,
            Ok(Disposition::Unknown) => 2,
            Err(_) => 2,
        }
    }
}

// ── Visualization ───────────────────────────────────────────────────────────

/// Generate a Mermaid flowchart from a pipeline's stage structure.
/// Each stage becomes a node; edges are sequential.
pub fn pipeline_to_mermaid(pipeline: &DispositionPipeline) -> String {
    let mut out = String::from("graph LR\n");
    out.push_str(&format!("    %% Pipeline: {}\n", pipeline.name));

    for (i, stage) in pipeline.stages.iter().enumerate() {
        let id = format!("S{}", i);
        let name = stage.name();
        out.push_str(&format!("    {}[\"{}\"]\n", id, name));
        if i > 0 {
            let prev = format!("S{}", i - 1);
            out.push_str(&format!("    {} --> {}\n", prev, id));
        }
    }

    // Disposition output node
    let last = format!("S{}", pipeline.stages.len() - 1);
    out.push_str(&format!("    {} --> RESULT{{\"Disposition\"}}\n", last));
    out.push_str("    RESULT --> SATISFIED([\"✅ 0\"])\n");
    out.push_str("    RESULT --> VIOLATED([\"❌ 1\"])\n");
    out.push_str("    RESULT --> UNKNOWN([\"❓ 2\"])\n");

    out
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_pipeline_is_satisfied() {
        let pipeline = DispositionPipeline::new("empty");
        let result = pipeline.run("/tmp").await.unwrap();
        assert_eq!(result, Disposition::Satisfied);
    }

    #[tokio::test]
    async fn test_gate_stage_command_exists() {
        let mut pipeline = DispositionPipeline::new("test");
        pipeline.push(GateStage {
            id: "has-bash".to_string(),
            spec: crate::gates::GateSpec {
                command: Some("bash".to_string()),
                ..Default::default()
            },
        });
        let result = pipeline.run("/tmp").await.unwrap();
        assert_eq!(result, Disposition::Satisfied);
    }

    #[tokio::test]
    async fn test_gate_stage_command_missing() {
        let mut pipeline = DispositionPipeline::new("test");
        pipeline.push(GateStage {
            id: "no-such-binary".to_string(),
            spec: crate::gates::GateSpec {
                command: Some("definitely-not-a-real-command-xyzzy".to_string()),
                ..Default::default()
            },
        });
        let result = pipeline.run("/tmp").await.unwrap();
        assert!(matches!(result, Disposition::Violated { .. }));
    }

    #[tokio::test]
    async fn test_pipeline_exit_code() {
        let mut pipeline = DispositionPipeline::new("test");
        pipeline.push(GateStage {
            id: "fail".to_string(),
            spec: crate::gates::GateSpec {
                command: Some("nope".to_string()),
                ..Default::default()
            },
        });
        assert_eq!(pipeline.run_exit_code("/tmp").await, 1);
    }

    #[tokio::test]
    async fn test_multi_stage_aggregate() {
        let mut pipeline = DispositionPipeline::new("mixed");
        pipeline.push(GateStage {
            id: "pass".to_string(),
            spec: crate::gates::GateSpec {
                command: Some("bash".to_string()),
                ..Default::default()
            },
        });
        pipeline.push(GateStage {
            id: "fail".to_string(),
            spec: crate::gates::GateSpec {
                command: Some("nope".to_string()),
                ..Default::default()
            },
        });
        let result = pipeline.run("/tmp").await.unwrap();
        assert!(matches!(result, Disposition::Violated { .. }));
    }

    #[test]
    fn test_mermaid_generation() {
        let mut pipeline = DispositionPipeline::new("system-normal");
        pipeline.push(GateStage {
            id: "git-clean".to_string(),
            spec: crate::gates::GateSpec::default(),
        });
        pipeline.push(GateStage {
            id: "gh-auth".to_string(),
            spec: crate::gates::GateSpec::default(),
        });
        let mermaid = pipeline_to_mermaid(&pipeline);
        assert!(mermaid.contains("graph LR"));
        assert!(mermaid.contains("git-clean"));
        assert!(mermaid.contains("gh-auth"));
        assert!(mermaid.contains("Disposition"));
    }
}
