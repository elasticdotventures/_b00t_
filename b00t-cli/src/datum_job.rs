//! Job datum type - Workflow orchestration with checkpoints and sub-agents
//!
//! Jobs define multi-step workflows with:
//! - Sequential, parallel, or DAG execution
//! - Git-based checkpoints for state persistence
//! - Sub-agent spawning for complex tasks
//! - K0mmander script integration
//! - Failure handling and rollback
//!
//! # Example
//!
//! ```toml
//! [b00t]
//! name = "build-release"
//! type = "job"
//!
//! [[b00t.job.steps]]
//! name = "test"
//! checkpoint = "tests-pass"
//!
//! [b00t.job.steps.task]
//! type = "bash"
//! command = "cargo test --all"
//! ```

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Job datum - Defines workflow with steps, checkpoints, and execution config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDatum {
    #[serde(flatten)]
    pub datum: crate::BootDatum,
}

/// Job configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobConfig {
    pub description: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub config: JobExecutionConfig,
    pub steps: Vec<JobStep>,
    #[serde(default)]
    pub rollback: Vec<JobStep>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub outputs: Option<JobOutputs>,
}

/// Execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobExecutionConfig {
    /// Execution mode: sequential, parallel, dag
    #[serde(default = "default_mode")]
    pub mode: String,

    /// Checkpoint behavior
    #[serde(default = "default_checkpoint_mode")]
    pub checkpoint_mode: String,

    #[serde(default)]
    pub checkpoint_after_each_step: bool,

    #[serde(default)]
    pub create_git_tag: bool,

    /// Sub-agent configuration
    #[serde(default)]
    pub use_subagents: bool,

    #[serde(default = "default_timeout")]
    pub subagent_timeout_ms: u64,

    #[serde(default)]
    pub subagent_type: Option<String>,

    /// Failure handling
    #[serde(default)]
    pub continue_on_failure: bool,

    #[serde(default)]
    pub retry_failed_steps: u32,

    #[serde(default)]
    pub rollback_on_failure: bool,
}

fn default_mode() -> String {
    "sequential".to_string()
}

fn default_checkpoint_mode() -> String {
    "auto".to_string()
}

fn default_timeout() -> u64 {
    300000 // 5 minutes
}

/// Job step definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStep {
    pub name: String,
    pub description: String,

    /// Checkpoint tag name
    #[serde(default)]
    pub checkpoint: Option<String>,

    /// Step dependencies
    #[serde(default)]
    pub depends_on: Vec<String>,

    /// Task to execute
    pub task: JobTask,

    /// Execution condition
    #[serde(default)]
    pub condition: Option<JobCondition>,

    /// Artifacts to preserve
    #[serde(default)]
    pub artifacts: Option<JobArtifacts>,

    /// Cognitive tier for this step: "sm0l" | "ch0nky" | "frontier"
    /// Routes to appropriate model from active role's [b00t.role.cognitive_tiers] map.
    /// If None, uses job default or executive judgment.
    #[serde(default)]
    pub cognitive_tier: Option<String>,

    /// Declarative output contract for this step, evaluated after completion.
    /// Format: "PASS|FAIL:<5lines>" for sm0l, "diff_summary+test_count" for ch0nky.
    /// Intended to guide executors on when to gate downstream steps or trigger rollback,
    /// but does not, by itself, guarantee such enforcement.
    #[serde(default)]
    pub output_contract: Option<String>,
}

/// Task definition - multiple execution types supported
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JobTask {
    /// Execute bash command
    #[serde(rename = "bash")]
    Bash {
        command: String,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default)]
        timeout_ms: Option<u64>,
        #[serde(default)]
        env: HashMap<String, String>,
    },

    /// Spawn sub-agent with prompt
    #[serde(rename = "agent")]
    Agent {
        agent_type: String,
        prompt: String,
        #[serde(default)]
        context_files: Vec<String>,
        #[serde(default)]
        timeout_ms: Option<u64>,
    },

    /// Execute k0mmander script
    #[serde(rename = "k0mmander")]
    K0mmander {
        script: String,
        #[serde(default)]
        cwd: Option<String>,
    },

    /// Execute another datum
    #[serde(rename = "datum")]
    Datum {
        datum: String,
        #[serde(default)]
        args: Vec<String>,
    },

    /// Execute MCP tool
    #[serde(rename = "mcp")]
    Mcp {
        server: String,
        tool: String,
        params: serde_json::Value,
    },

    /// Execute Dagu DAG
    #[serde(rename = "dagu")]
    Dagu {
        dag: String,
        #[serde(default)]
        params: HashMap<String, String>,
    },
}

/// Step execution condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobCondition {
    /// When to execute: always, on_success, on_failure, on_previous
    #[serde(default = "default_when")]
    pub when: String,

    /// Optional custom condition script
    #[serde(default)]
    pub script: Option<String>,
}

fn default_when() -> String {
    "always".to_string()
}

/// Artifacts to preserve in checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobArtifacts {
    pub paths: Vec<String>,
    #[serde(default)]
    pub archive: bool,
}

/// Job outputs configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobOutputs {
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub reports: Vec<String>,
    #[serde(default)]
    pub logs: Vec<String>,
}

impl JobDatum {
    /// Load job from TOML file
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        // Strip .job.toml extension if present since get_config adds extensions
        let base_name = name.trim_end_matches(".job.toml");
        let (config, _filename) =
            crate::get_config(base_name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(JobDatum { datum: config.b00t })
    }

    /// Get job configuration
    pub fn job_config(&self) -> Result<JobConfig> {
        let job_value = self
            .datum
            .job
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No job configuration found"))?;

        serde_json::from_value(job_value.clone())
            .map_err(|e| anyhow::anyhow!("Failed to parse job config: {}", e))
    }

    /// Validate job definition
    pub fn validate(&self) -> Result<()> {
        let config = self.job_config()?;

        // Validate execution mode
        match config.config.mode.as_str() {
            "sequential" | "parallel" | "dag" => {}
            mode => anyhow::bail!("Invalid execution mode: {}", mode),
        }

        // Validate checkpoint mode
        match config.config.checkpoint_mode.as_str() {
            "auto" | "manual" | "off" => {}
            mode => anyhow::bail!("Invalid checkpoint mode: {}", mode),
        }

        // Validate step dependencies (DAG mode)
        if config.config.mode == "dag" {
            let step_names: std::collections::HashSet<_> =
                config.steps.iter().map(|s| s.name.as_str()).collect();

            for step in &config.steps {
                for dep in &step.depends_on {
                    if !step_names.contains(dep.as_str()) {
                        anyhow::bail!("Step '{}' depends on unknown step '{}'", step.name, dep);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get step by name
    pub fn get_step(&self, name: &str) -> Result<JobStep> {
        let config = self.job_config()?;
        config
            .steps
            .into_iter()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("Step '{}' not found", name))
    }

    /// Get all steps in execution order
    pub fn execution_order(&self) -> Result<Vec<String>> {
        let config = self.job_config()?;

        match config.config.mode.as_str() {
            "sequential" => {
                // Return steps in definition order
                Ok(config.steps.iter().map(|s| s.name.clone()).collect())
            }
            "dag" => {
                // Topological sort based on dependencies
                self.topological_sort(&config.steps)
            }
            "parallel" => {
                // All steps can run in parallel
                Ok(config.steps.iter().map(|s| s.name.clone()).collect())
            }
            mode => anyhow::bail!("Unsupported execution mode: {}", mode),
        }
    }

    /// Topological sort for DAG execution
    fn topological_sort(&self, steps: &[JobStep]) -> Result<Vec<String>> {
        use std::collections::{HashMap, VecDeque};

        // Build reverse dependency graph (who depends on me?)
        let mut reverse_graph: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        // Initialize all steps
        for step in steps {
            in_degree.insert(step.name.clone(), step.depends_on.len());
            reverse_graph
                .entry(step.name.clone())
                .or_insert_with(Vec::new);
        }

        // Build reverse edges: if B depends on A, then reverse_graph[A] contains B
        for step in steps {
            for dep in &step.depends_on {
                reverse_graph
                    .entry(dep.clone())
                    .or_insert_with(Vec::new)
                    .push(step.name.clone());
            }
        }

        // Kahn's algorithm: start with nodes that have no dependencies
        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(name, _)| name.clone())
            .collect();

        let mut result = Vec::new();

        while let Some(node) = queue.pop_front() {
            result.push(node.clone());

            // Reduce in-degree of nodes that depend on this node
            if let Some(dependents) = reverse_graph.get(&node) {
                for dependent in dependents {
                    if let Some(deg) = in_degree.get_mut(dependent) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dependent.clone());
                        }
                    }
                }
            }
        }

        if result.len() != steps.len() {
            anyhow::bail!("Circular dependency detected in job steps");
        }

        Ok(result)
    }
}

/// Resolve a cognitive tier name to a concrete model identifier.
///
/// Looks up `tier_name` in the provided tier map (from a role datum's
/// `[b00t.role.cognitive_tiers]` section). Returns the first model listed for that tier.
///
/// # Example role datum snippet
/// ```toml
/// [b00t.role.cognitive_tiers]
/// sm0l     = { models = ["qwen2.5-3b", "haiku"], tasks = ["test", "lint"] }
/// ch0nky   = { models = ["qwen3-coder-next", "sonnet"], tasks = ["implement"] }
/// frontier = { models = ["claude-opus", "gpt-4o"], tasks = ["architecture"] }
/// ```
pub fn resolve_cognitive_tier(
    tier_name: &str,
    tier_map: &serde_json::Value,
) -> Option<String> {
    tier_map
        .get(tier_name)
        .and_then(|t| t.get("models"))
        .and_then(|m| m.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_datum_structure() {
        let job_toml = r#"
[b00t]
name = "test-job"
type = "job"
hint = "Test job"
usage = []

[b00t.job]
description = "Test job"
config = { mode = "sequential", checkpoint_mode = "auto" }
steps = [
    { name = "test1", description = "First test", task = { type = "bash", command = "echo test" } }
]
"#;

        let config: crate::UnifiedConfig = toml::from_str(job_toml).unwrap();
        let datum = JobDatum { datum: config.b00t };

        assert_eq!(datum.datum.name, "test-job");
        assert!(datum.validate().is_ok());
    }

    #[test]
    fn test_topological_sort() {
        // Test DAG with dependencies: A -> B -> C
        let steps = vec![
            JobStep {
                name: "C".to_string(),
                description: "Third".to_string(),
                depends_on: vec!["B".to_string()],
                checkpoint: None,
                task: JobTask::Bash {
                    command: "echo C".to_string(),
                    cwd: None,
                    timeout_ms: None,
                    env: HashMap::new(),
                },
                condition: None,
                artifacts: None,
                cognitive_tier: None,
                output_contract: None,
            },
            JobStep {
                name: "A".to_string(),
                description: "First".to_string(),
                depends_on: vec![],
                checkpoint: None,
                task: JobTask::Bash {
                    command: "echo A".to_string(),
                    cwd: None,
                    timeout_ms: None,
                    env: HashMap::new(),
                },
                condition: None,
                artifacts: None,
                cognitive_tier: None,
                output_contract: None,
            },
            JobStep {
                name: "B".to_string(),
                description: "Second".to_string(),
                depends_on: vec!["A".to_string()],
                checkpoint: None,
                task: JobTask::Bash {
                    command: "echo B".to_string(),
                    cwd: None,
                    timeout_ms: None,
                    env: HashMap::new(),
                },
                condition: None,
                artifacts: None,
                cognitive_tier: None,
                output_contract: None,
            },
        ];

        let datum = JobDatum {
            datum: crate::BootDatum {
                name: "test".to_string(),
                datum_type: Some(crate::DatumType::Job),
                hint: "Test job".to_string(),
                desires: None,
                ..crate::BootDatum::default()
            },
        };

        let order = datum.topological_sort(&steps).unwrap();
        assert_eq!(order, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_job_step_cognitive_tier_field_parses() {
        let job_toml = r#"
[b00t]
name = "tier-test-job"
type = "job"
hint = "Test job with cognitive tiers"
usage = []

[b00t.job]
description = "Test tier routing"
tags = ["test"]

[b00t.job.config]
mode = "sequential"
checkpoint_mode = "auto"
use_subagents = true

[[b00t.job.steps]]
name = "run-tests"
description = "Run unit tests"
cognitive_tier = "sm0l"
output_contract = "PASS|FAIL:<5lines>"

[b00t.job.steps.task]
type = "bash"
command = "cargo test"

[[b00t.job.steps]]
name = "implement-feature"
description = "Implement the feature"
cognitive_tier = "ch0nky"

[b00t.job.steps.task]
type = "agent"
agent_type = "general"
prompt = "implement foo"
"#;
        let config: crate::UnifiedConfig = toml::from_str(job_toml).unwrap();
        let datum = JobDatum { datum: config.b00t };
        let job_config = datum.job_config().unwrap();

        let test_step = &job_config.steps[0];
        assert_eq!(test_step.cognitive_tier.as_deref(), Some("sm0l"));
        assert_eq!(test_step.output_contract.as_deref(), Some("PASS|FAIL:<5lines>"));

        let impl_step = &job_config.steps[1];
        assert_eq!(impl_step.cognitive_tier.as_deref(), Some("ch0nky"));
        assert!(impl_step.output_contract.is_none());
    }

    #[test]
    fn test_resolve_cognitive_tier() {
        let tier_map = serde_json::json!({
            "sm0l": { "models": ["qwen2.5-3b", "haiku"], "tasks": ["test", "lint"] },
            "ch0nky": { "models": ["qwen3-coder-next", "sonnet"], "tasks": ["implement"] },
            "frontier": { "models": ["claude-opus", "gpt-4o"], "tasks": ["architecture"] }
        });

        assert_eq!(resolve_cognitive_tier("sm0l", &tier_map).as_deref(), Some("qwen2.5-3b"));
        assert_eq!(resolve_cognitive_tier("ch0nky", &tier_map).as_deref(), Some("qwen3-coder-next"));
        assert_eq!(resolve_cognitive_tier("frontier", &tier_map).as_deref(), Some("claude-opus"));
        assert!(resolve_cognitive_tier("unknown", &tier_map).is_none());
    }
}
