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
        std::fs::create_dir_all(&jobs_dir).context("Failed to create .b00t/jobs directory")?;

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
        job_datum.validate().context("Job validation failed")?;

        let job_config = job_datum.job_config()?;

        println!("📋 Job: {}", job_config.description);
        println!("   Mode: {}", job_config.config.mode);
        println!("   Steps: {}", job_config.steps.len());

        // For MVP, only support sequential mode
        if job_config.config.mode != "sequential" {
            anyhow::bail!(
                "Only sequential mode supported in MVP (got: {})",
                job_config.config.mode
            );
        }

        // Note: SqliteStorage not used in MVP (synchronous execution)
        // Future enhancement: Use SqliteStorage with worker pool for async execution

        // Execute steps sequentially
        for (idx, step) in job_config.steps.iter().enumerate() {
            println!(
                "\n📍 Step {}/{}: {}",
                idx + 1,
                job_config.steps.len(),
                step.name
            );
            println!("   {}", step.description);

            match &step.task {
                JobTask::Bash {
                    command,
                    cwd,
                    timeout_ms,
                    env,
                } => {
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
                    self.execute_bash_task(task_job)
                        .await
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
    fn create_checkpoint(
        &self,
        job_name: &str,
        step_name: &str,
        checkpoint_name: &str,
    ) -> Result<()> {
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
        assert!(
            job_file_path.exists(),
            "Job file should exist at: {}",
            job_file_path.display()
        );
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

// ============================================================================
/// Session metadata for git checkpoints
/// Stores session info in git tag messages for resume functionality
// ============================================================================

/// Session metadata to store in git checkpoint tags
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CheckpointMetadata {
    /// Current session ID
    pub session_id: String,
    /// Current role (from _B00T_ROLE env var)
    pub role: Option<String>,
    /// Number of checkpoints created in this session
    pub checkpoint_count: i64,
    /// List of loaded capabilities with their use counts
    pub capabilities_loaded: Vec<CapabilityInfo>,
}

/// Individual capability info
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CapabilityInfo {
    pub name: String,
    pub use_count: i64,
}

impl JobExecutor {
    /// Create git checkpoint with session metadata
    fn create_checkpoint_with_metadata(
        &self,
        job_name: &str,
        step_name: &str,
        checkpoint_name: &str,
        metadata: &CheckpointMetadata,
    ) -> Result<()> {
        use duct::cmd;

        // Format capabilities as a readable string
        let capabilities_str = if metadata.capabilities_loaded.is_empty() {
            "none".to_string()
        } else {
            metadata
                .capabilities_loaded
                .iter()
                .map(|c| format!("{}:{}", c.name, c.use_count))
                .collect::<Vec<_>>()
                .join(",")
        };

        // Format role (use "none" if not set)
        let role_str = metadata.role.as_deref().unwrap_or("none");

        // Create enriched tag message with session metadata
        let tag_name = format!("checkpoint/{}/{}", job_name, checkpoint_name);
        let message = format!(
            "Checkpoint: {} - {} | session:{} role:{} checkpoint_count:{} capabilities:{}",
            step_name,
            checkpoint_name,
            &metadata.session_id[..8.min(metadata.session_id.len())],
            role_str,
            metadata.checkpoint_count,
            capabilities_str
        );

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

    /// Resume from a checkpoint - parse tag message and restore session state
    pub fn resume_from_checkpoint(&self, tag_name: &str) -> Result<CheckpointMetadata> {
        use duct::cmd;

        // Get tag message
        let output = cmd!("git", "tag", "-l", "--format=%contents", tag_name)
            .dir(&self.project_root)
            .read()
            .context(format!("Failed to read tag: {}", tag_name))?;

        // Parse metadata from tag message
        // Format: "Checkpoint: step - name | session:xxx role:xxx checkpoint_count:xxx capabilities:xxx"
        let metadata = parse_checkpoint_message(&output)?;

        println!("📂 Resuming from checkpoint: {}", tag_name);
        println!("   Session: {}", metadata.session_id);
        if let Some(ref role) = metadata.role {
            println!("   Role: {}", role);
        }
        println!("   Checkpoint #: {}", metadata.checkpoint_count);
        if !metadata.capabilities_loaded.is_empty() {
            println!(
                "   Capabilities: {}",
                metadata
                    .capabilities_loaded
                    .iter()
                    .map(|c| c.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        Ok(metadata)
    }

    /// List all checkpoint tags for a job
    pub fn list_checkpoints(&self, job_name: Option<&str>) -> Result<Vec<CheckpointInfo>> {
        use duct::cmd;

        let pattern = if let Some(name) = job_name {
            format!("checkpoint/{}/*", name)
        } else {
            "checkpoint/*/*".to_string()
        };

        let output = cmd!("git", "tag", "-l", &pattern)
            .dir(&self.project_root)
            .read()
            .context("Failed to list checkpoint tags")?;

        let mut checkpoints = Vec::new();
        for tag in output.lines() {
            if tag.trim().is_empty() {
                continue;
            }

            // Get tag message for metadata
            if let Ok(msg_output) = cmd!("git", "tag", "-l", "--format=%contents", tag)
                .dir(&self.project_root)
                .read()
            {
                if let Ok(metadata) = parse_checkpoint_message(&msg_output) {
                    checkpoints.push(CheckpointInfo {
                        tag_name: tag.to_string(),
                        metadata,
                    });
                }
            }
        }

        Ok(checkpoints)
    }
}

/// Checkpoint info from git tag
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CheckpointInfo {
    pub tag_name: String,
    pub metadata: CheckpointMetadata,
}

/// Parse checkpoint message to extract metadata
fn parse_checkpoint_message(message: &str) -> Result<CheckpointMetadata> {
    let mut metadata = CheckpointMetadata::default();

    // Extract session_id: after "session:" before " role:" or " checkpoint_count:"
    const SESSION_PREFIX: &str = "session:";
    if let Some(start) = message.find(SESSION_PREFIX) {
        let rest = &message[start + SESSION_PREFIX.len()..];
        let end = rest
            .find(' ')
            .or_else(|| rest.find('|'))
            .unwrap_or(rest.len());
        metadata.session_id = rest[..end].to_string();
    }

    // Extract role: after "role:" before " checkpoint_count:"
    const ROLE_PREFIX: &str = "role:";
    if let Some(start) = message.find(ROLE_PREFIX) {
        let rest = &message[start + ROLE_PREFIX.len()..];
        let end = rest
            .find(' ')
            .or_else(|| rest.find('|'))
            .unwrap_or(rest.len());
        let role = rest[..end].to_string();
        if role != "none" {
            metadata.role = Some(role);
        }
    }

    // Extract checkpoint_count: after "checkpoint_count:" before " capabilities:"
    const CHECKPOINT_COUNT_PREFIX: &str = "checkpoint_count:";
    if let Some(start) = message.find(CHECKPOINT_COUNT_PREFIX) {
        let rest = &message[start + CHECKPOINT_COUNT_PREFIX.len()..];
        let end = rest
            .find(' ')
            .or_else(|| rest.find('|'))
            .unwrap_or(rest.len());
        metadata.checkpoint_count = rest[..end].parse().unwrap_or(0);
    }

    // Extract capabilities: after "capabilities:" to end
    const CAPABILITIES_PREFIX: &str = "capabilities:";
    if let Some(start) = message.find(CAPABILITIES_PREFIX) {
        let rest = &message[start + CAPABILITIES_PREFIX.len()..];
        let capabilities_str = rest.trim();

        if capabilities_str != "none" && !capabilities_str.is_empty() {
            for cap in capabilities_str.split(',') {
                let parts: Vec<&str> = cap.split(':').collect();
                if parts.len() == 2 {
                    metadata.capabilities_loaded.push(CapabilityInfo {
                        name: parts[0].to_string(),
                        use_count: parts[1].parse().unwrap_or(1),
                    });
                } else {
                    metadata.capabilities_loaded.push(CapabilityInfo {
                        name: cap.to_string(),
                        use_count: 1,
                    });
                }
            }
        }
    }

    if metadata.session_id.is_empty() {
        anyhow::bail!("Failed to parse checkpoint message: missing session_id");
    }

    Ok(metadata)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod checkpoint_tests {
    use super::*;

    #[test]
    fn test_parse_checkpoint_message() {
        let message = "Checkpoint: step1 - complete | session:abc123 role:captain checkpoint_count:3 capabilities:git:5,bash:10";

        let metadata = parse_checkpoint_message(message).unwrap();

        assert_eq!(metadata.session_id, "abc123");
        assert_eq!(metadata.role, Some("captain".to_string()));
        assert_eq!(metadata.checkpoint_count, 3);
        assert_eq!(metadata.capabilities_loaded.len(), 2);
        assert_eq!(metadata.capabilities_loaded[0].name, "git");
        assert_eq!(metadata.capabilities_loaded[0].use_count, 5);
        assert_eq!(metadata.capabilities_loaded[1].name, "bash");
        assert_eq!(metadata.capabilities_loaded[1].use_count, 10);
    }

    #[test]
    fn test_parse_checkpoint_message_no_role() {
        let message = "Checkpoint: step1 - complete | session:abc123 role:none checkpoint_count:1 capabilities:none";

        let metadata = parse_checkpoint_message(message).unwrap();

        assert_eq!(metadata.session_id, "abc123");
        assert!(metadata.role.is_none());
        assert_eq!(metadata.checkpoint_count, 1);
        assert!(metadata.capabilities_loaded.is_empty());
    }
}
