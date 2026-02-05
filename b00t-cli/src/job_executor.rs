//! Job executor using apalis for background task processing
//!
//! Executes `.job.toml` workflow definitions using apalis-sqlite backend
//! with git checkpoints for state persistence.

use anyhow::{Context, Result};
use apalis::prelude::*;
use apalis_sqlite::SqlitePool;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::datum_job::{JobDatum, JobTask};

/// Bash command task for apalis execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashTaskJob {
    /// Job name for identification
    pub job_name: String,

    /// Step name for checkpoint tracking
    pub step_name: String,

    /// Bash command to execute
    pub command: String,

    /// Working directory (optional)
    pub cwd: Option<String>,

    /// Timeout in milliseconds (optional)
    pub timeout_ms: Option<u64>,

    /// Environment variables
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,

    /// Checkpoint tag name (optional)
    pub checkpoint: Option<String>,

    /// Project root for git operations
    pub project_root: PathBuf,
}

/// Job executor managing apalis workers and SQLite backend
pub struct JobExecutor {
    /// SQLite connection pool
    pool: SqlitePool,

    /// Project root directory
    project_root: PathBuf,

    /// Database file path
    db_path: PathBuf,
}

impl JobExecutor {
    /// Create new job executor with SQLite backend
    pub async fn new(project_root: &Path) -> Result<Self> {
        let jobs_dir = project_root.join(".b00t/jobs");
        std::fs::create_dir_all(&jobs_dir)
            .context("Failed to create .b00t/jobs directory")?;

        let db_path = jobs_dir.join("apalis.db");

        // Use sqlite: URL with absolute path
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

        println!("🥾 Initializing job executor with SQLite backend");
        println!("   Database: {}", db_path.display());

        let pool = SqlitePool::connect(&db_url)
            .await
            .context("Failed to connect to SQLite database")?;

        // Setup apalis tables
        // Note: setup() is called on first use of SqliteStorage
        // No explicit setup needed for apalis-sqlite

        Ok(JobExecutor {
            pool,
            project_root: project_root.to_path_buf(),
            db_path,
        })
    }

    /// Load and execute a job from .job.toml file
    pub async fn run_job(&self, job_name: &str) -> Result<()> {
        println!("🚀 Loading job: {}", job_name);

        // Load job datum (from_config expects _b00t_ directory path)
        let b00t_path = self.project_root.join("_b00t_");
        let job_datum = JobDatum::from_config(job_name, b00t_path.to_str().unwrap())
            .context("Failed to load job configuration")?;

        // Validate job
        job_datum.validate()
            .context("Job validation failed")?;

        let job_config = job_datum.job_config()?;

        println!("📋 Job: {}", job_config.description);
        println!("   Mode: {}", job_config.config.mode);
        println!("   Steps: {}", job_config.steps.len());

        // For MVP, only support sequential mode
        if job_config.config.mode != "sequential" {
            anyhow::bail!("Only sequential mode supported in MVP (got: {})", job_config.config.mode);
        }

        // Note: SqliteStorage not used in MVP (synchronous execution)
        // Future enhancement: Use SqliteStorage with worker pool for async execution

        // Execute steps sequentially
        for (idx, step) in job_config.steps.iter().enumerate() {
            println!("\n📍 Step {}/{}: {}", idx + 1, job_config.steps.len(), step.name);
            println!("   {}", step.description);

            match &step.task {
                JobTask::Bash { command, cwd, timeout_ms, env } => {
                    // Create bash task job
                    let task_job = BashTaskJob {
                        job_name: job_datum.datum.name.clone(),
                        step_name: step.name.clone(),
                        command: command.clone(),
                        cwd: cwd.clone(),
                        timeout_ms: *timeout_ms,
                        env: env.clone(),
                        checkpoint: step.checkpoint.clone(),
                        project_root: self.project_root.clone(),
                    };

                    // Execute task directly (MVP: synchronous execution)
                    self.execute_bash_task(task_job).await
                        .with_context(|| format!("Step '{}' failed", step.name))?;

                    println!("   ✅ Step completed");
                }
                _ => {
                    println!("   ⚠️  Skipping unsupported task type (MVP limitation)");
                }
            }
        }

        println!("\n🎉 Job completed successfully");
        Ok(())
    }

    /// Execute bash task (synchronous for MVP)
    async fn execute_bash_task(&self, task: BashTaskJob) -> Result<()> {
        use duct::cmd;

        println!("   🔧 Executing: {}", task.command);

        // Parse command (simple split for MVP)
        let parts: Vec<&str> = task.command.split_whitespace().collect();
        if parts.is_empty() {
            anyhow::bail!("Empty command");
        }

        let program = parts[0];
        let args = &parts[1..];

        // Build command
        let mut command = cmd(program, args);

        // Set working directory
        if let Some(cwd) = &task.cwd {
            let cwd_path = if Path::new(cwd).is_absolute() {
                PathBuf::from(cwd)
            } else {
                task.project_root.join(cwd)
            };
            command = command.dir(cwd_path);
        } else {
            command = command.dir(&task.project_root);
        }

        // Set environment variables
        for (key, value) in &task.env {
            command = command.env(key, value);
        }

        // Execute command
        let output = command
            .stdout_to_stderr()
            .run()
            .with_context(|| format!("Failed to execute: {}", task.command))?;

        if !output.status.success() {
            anyhow::bail!("Command failed with exit code: {:?}", output.status.code());
        }

        // Create git checkpoint if specified
        if let Some(checkpoint_name) = &task.checkpoint {
            println!("   📌 Creating checkpoint: {}", checkpoint_name);
            self.create_checkpoint(&task.job_name, &task.step_name, checkpoint_name)?;
        }

        Ok(())
    }

    /// Create git checkpoint after successful step
    fn create_checkpoint(&self, job_name: &str, step_name: &str, checkpoint_name: &str) -> Result<()> {
        use duct::cmd;

        let tag_name = format!("checkpoint/{}/{}", job_name, checkpoint_name);
        let message = format!("Checkpoint: {} - {}", step_name, checkpoint_name);

        // Create git tag
        let result = cmd!("git", "tag", "-a", &tag_name, "-m", &message)
            .dir(&self.project_root)
            .stdout_to_stderr()
            .run();

        match result {
            Ok(_) => {
                println!("   ✅ Checkpoint created: {}", tag_name);
                Ok(())
            }
            Err(e) => {
                // Don't fail job if checkpoint creation fails
                eprintln!("   ⚠️  Failed to create checkpoint: {}", e);
                Ok(())
            }
        }
    }
}

/// Async bash task handler for apalis (future use with worker pool)
#[allow(dead_code)]
async fn bash_task_handler(
    task: BashTaskJob,
    _ctx: WorkerContext,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use duct::cmd;

    // Parse command
    let parts: Vec<&str> = task.command.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty command".into());
    }

    let program = parts[0];
    let args = &parts[1..];

    // Build and execute command
    let mut command = cmd(program, args);

    if let Some(cwd) = &task.cwd {
        command = command.dir(cwd);
    }

    for (key, value) in &task.env {
        command = command.env(key, value);
    }

    let output = command.stdout_to_stderr().run()?;

    if !output.status.success() {
        return Err(format!("Command failed: {:?}", output.status.code()).into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_job_executor_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let executor = JobExecutor::new(temp_dir.path()).await;
        assert!(executor.is_ok());

        // Verify database file created
        let db_path = temp_dir.path().join(".b00t/jobs/apalis.db");
        assert!(db_path.exists());
    }

    #[test]
    fn test_bash_task_job_serialization() {
        let task = BashTaskJob {
            job_name: "test".to_string(),
            step_name: "step1".to_string(),
            command: "echo hello".to_string(),
            cwd: None,
            timeout_ms: None,
            env: std::collections::HashMap::new(),
            checkpoint: Some("test-checkpoint".to_string()),
            project_root: PathBuf::from("/tmp"),
        };

        let json = serde_json::to_string(&task).unwrap();
        let deserialized: BashTaskJob = serde_json::from_str(&json).unwrap();

        assert_eq!(task.job_name, deserialized.job_name);
        assert_eq!(task.command, deserialized.command);
    }

    #[tokio::test]
    async fn test_simple_job_execution() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_root = temp_dir.path();

        // Create _b00t_ directory
        let b00t_dir = project_root.join("_b00t_");
        fs::create_dir_all(&b00t_dir).unwrap();

        // Create a simple test job file
        let job_toml = r#"
[b00t]
name = "test-simple"
type = "job"

[b00t.job]
description = "Simple test job"
tags = ["test"]

[b00t.job.config]
mode = "sequential"
checkpoint_mode = "off"

[[b00t.job.steps]]
name = "echo-test"
description = "Echo test message"

[b00t.job.steps.task]
type = "bash"
command = "echo 'Test passed'"
"#;

        let job_file_path = b00t_dir.join("test-simple.job.toml");
        fs::write(&job_file_path, job_toml).unwrap();

        // Verify file was created
        assert!(job_file_path.exists(), "Job file should exist at: {}", job_file_path.display());
        println!("Created job file at: {}", job_file_path.display());

        // Initialize git repo (required for checkpoints)
        std::process::Command::new("git")
            .args(&["init"])
            .current_dir(project_root)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(project_root)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(project_root)
            .output()
            .unwrap();

        // Create and run job executor
        let executor = JobExecutor::new(project_root).await.unwrap();

        // Debug: Show paths being used
        println!("Project root: {}", project_root.display());
        println!("_b00t_ path: {}", b00t_dir.display());

        let result = executor.run_job("test-simple").await;

        // Verify job executed successfully
        assert!(result.is_ok(), "Job execution failed: {:?}", result.err());
    }
}
