//! Job state tracking and persistence
//!
//! Tracks job execution state for resume, status queries, and auditing.
//! State files stored in `.b00t/jobs/<job-name>/<run-id>.json`

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Job execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobState {
    pub job_name: String,
    pub run_id: String,
    pub status: JobStatus,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub current_step: Option<String>,
    pub steps: HashMap<String, StepState>,
    pub checkpoints: Vec<CheckpointInfo>,
    pub error: Option<String>,
    pub metadata: JobMetadata,
}

/// Job execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
    Paused,
    RollingBack,
    RolledBack,
}

/// Step execution state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepState {
    pub name: String,
    pub status: StepStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    pub retries: u32,
}

/// Step execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

/// Checkpoint information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointInfo {
    pub step_name: String,
    pub checkpoint_name: String,
    pub git_commit: Option<String>,
    pub git_tag: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Job metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobMetadata {
    pub execution_mode: String,
    pub total_steps: usize,
    pub completed_steps: usize,
    pub failed_steps: usize,
    pub skipped_steps: usize,
    pub env: HashMap<String, String>,
}

impl JobState {
    /// Create new job state
    pub fn new(job_name: String, execution_mode: String, total_steps: usize) -> Self {
        let run_id = uuid::Uuid::new_v4().to_string();

        Self {
            job_name,
            run_id,
            status: JobStatus::Running,
            started_at: Utc::now(),
            completed_at: None,
            current_step: None,
            steps: HashMap::new(),
            checkpoints: Vec::new(),
            error: None,
            metadata: JobMetadata {
                execution_mode,
                total_steps,
                completed_steps: 0,
                failed_steps: 0,
                skipped_steps: 0,
                env: HashMap::new(),
            },
        }
    }

    /// Start executing a step
    pub fn start_step(&mut self, step_name: String) {
        self.current_step = Some(step_name.clone());
        self.steps.insert(
            step_name.clone(),
            StepState {
                name: step_name,
                status: StepStatus::Running,
                started_at: Some(Utc::now()),
                completed_at: None,
                duration_ms: None,
                error: None,
                retries: 0,
            },
        );
    }

    /// Complete a step successfully
    pub fn complete_step(&mut self, step_name: &str) {
        if let Some(step) = self.steps.get_mut(step_name) {
            step.status = StepStatus::Completed;
            step.completed_at = Some(Utc::now());

            if let Some(started) = step.started_at {
                step.duration_ms = Some((Utc::now() - started).num_milliseconds() as u64);
            }

            self.metadata.completed_steps += 1;
        }
    }

    /// Fail a step
    pub fn fail_step(&mut self, step_name: &str, error: String) {
        if let Some(step) = self.steps.get_mut(step_name) {
            step.status = StepStatus::Failed;
            step.completed_at = Some(Utc::now());
            step.error = Some(error);

            if let Some(started) = step.started_at {
                step.duration_ms = Some((Utc::now() - started).num_milliseconds() as u64);
            }

            self.metadata.failed_steps += 1;
        }
    }

    /// Increment retry count for a step
    pub fn increment_retry(&mut self, step_name: &str) {
        if let Some(step) = self.steps.get_mut(step_name) {
            step.retries += 1;
            step.status = StepStatus::Running;
            step.started_at = Some(Utc::now());
            step.completed_at = None;
            step.error = None;
        }
    }

    /// Add checkpoint
    pub fn add_checkpoint(
        &mut self,
        step_name: String,
        checkpoint_name: String,
        git_tag: Option<String>,
    ) {
        self.checkpoints.push(CheckpointInfo {
            step_name,
            checkpoint_name,
            git_commit: None, // TODO: Get from git
            git_tag,
            created_at: Utc::now(),
        });
    }

    /// Complete job
    pub fn complete(&mut self) {
        self.status = JobStatus::Completed;
        self.completed_at = Some(Utc::now());
        self.current_step = None;
    }

    /// Fail job
    pub fn fail(&mut self, error: String) {
        self.status = JobStatus::Failed;
        self.completed_at = Some(Utc::now());
        self.error = Some(error);
        self.current_step = None;
    }

    /// Get state file path
    fn state_file_path(base_path: &str, job_name: &str, run_id: &str) -> PathBuf {
        let mut path = PathBuf::from(shellexpand::tilde(base_path).to_string());
        path.push(".b00t");
        path.push("jobs");
        path.push(job_name);
        path.push(format!("{}.json", run_id));
        path
    }

    /// Save state to disk
    pub fn save(&self, base_path: &str) -> Result<()> {
        let state_path = Self::state_file_path(base_path, &self.job_name, &self.run_id);

        // Create parent directories
        if let Some(parent) = state_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Write state file
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&state_path, json)?;

        // Also create/update symlink to "latest"
        if let Some(parent) = state_path.parent() {
            let mut latest_path = parent.to_path_buf();
            latest_path.push("latest.json");

            // Remove old symlink if exists
            let _ = std::fs::remove_file(&latest_path);

            // Create new symlink (or copy on Windows)
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&state_path, &latest_path)?;
            }
            #[cfg(not(unix))]
            {
                std::fs::copy(&state_path, &latest_path)?;
            }
        }

        Ok(())
    }

    /// Load state from disk
    pub fn load(base_path: &str, job_name: &str, run_id: &str) -> Result<Self> {
        let state_path = Self::state_file_path(base_path, job_name, run_id);
        let json = std::fs::read_to_string(&state_path)?;
        let state: JobState = serde_json::from_str(&json)?;
        Ok(state)
    }

    /// Load latest state for job
    pub fn load_latest(base_path: &str, job_name: &str) -> Result<Self> {
        let mut latest_path = PathBuf::from(shellexpand::tilde(base_path).to_string());
        latest_path.push(".b00t");
        latest_path.push("jobs");
        latest_path.push(job_name);
        latest_path.push("latest.json");

        let json = std::fs::read_to_string(&latest_path)?;
        let state: JobState = serde_json::from_str(&json)?;
        Ok(state)
    }

    /// List all job states
    pub fn list_all(base_path: &str) -> Result<Vec<JobState>> {
        let mut jobs_path = PathBuf::from(shellexpand::tilde(base_path).to_string());
        jobs_path.push(".b00t");
        jobs_path.push("jobs");

        if !jobs_path.exists() {
            return Ok(Vec::new());
        }

        let mut states = Vec::new();

        // Iterate through job directories
        for entry in std::fs::read_dir(&jobs_path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                // Load latest state for each job
                let job_name = entry.file_name().to_string_lossy().to_string();
                if let Ok(state) = Self::load_latest(base_path, &job_name) {
                    states.push(state);
                }
            }
        }

        // Sort by started_at (most recent first)
        states.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        Ok(states)
    }

    /// Get progress percentage
    pub fn progress_percent(&self) -> f64 {
        if self.metadata.total_steps == 0 {
            return 0.0;
        }

        (self.metadata.completed_steps as f64 / self.metadata.total_steps as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_state_lifecycle() {
        let mut state = JobState::new("test-job".to_string(), "sequential".to_string(), 3);

        assert_eq!(state.status, JobStatus::Running);
        assert_eq!(state.metadata.total_steps, 3);

        // Start step
        state.start_step("step1".to_string());
        assert_eq!(state.current_step, Some("step1".to_string()));

        // Complete step
        state.complete_step("step1");
        assert_eq!(state.metadata.completed_steps, 1);
        assert_eq!(state.progress_percent(), 33.333333333333336);

        // Complete job
        state.complete();
        assert_eq!(state.status, JobStatus::Completed);
        assert!(state.completed_at.is_some());
    }
}
