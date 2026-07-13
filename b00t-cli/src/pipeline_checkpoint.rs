// 🤓 Pipeline checkpoint and resume — stores intermediate state between stages
//    for restartable pipeline execution (#726).
//
//    Types:
//      CheckpointStatus     — InProgress / Completed / Failed(String)
//      StageCheckpoint      — snapshot of a single completed stage
//      PipelineCheckpoint   — overall checkpoint for a pipeline run
//      CheckpointStore      — trait for persistence (save/load/list/delete)
//      FileCheckpointStore  — JSON file-backed store in ~/.b00t/checkpoints/
//      InMemoryCheckpointStore — HashMap-backed store for tests

use crate::pipeline_executor::{
    PipelineExecutor, PipelineRunReport,
};
use crate::pipeline_types::PipelineDag;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

// ── CheckpointStatus ──────────────────────────────────────────────────────────

/// Overall status of a checkpointed pipeline run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CheckpointStatus {
    InProgress,
    Completed,
    Failed(String),
}

// ── StageCheckpoint ───────────────────────────────────────────────────────────

/// Snapshot of a single completed pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageCheckpoint {
    pub run_id: String,
    pub stage_name: String,
    pub stage_index: usize,
    pub state: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

// ── PipelineCheckpoint ────────────────────────────────────────────────────────

/// Full checkpoint for a pipeline run.
///
/// Tracks which stages have completed, the current position in the execution
/// order, and a `dag_hash` that validates the checkpoint is still compatible
/// with the current DAG at resume time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineCheckpoint {
    pub run_id: String,
    pub completed_stages: Vec<StageCheckpoint>,
    pub current_stage_index: usize,
    pub dag_hash: String,
    pub status: CheckpointStatus,
}

impl PipelineCheckpoint {
    /// Create a new checkpoint for the given run and DAG.
    ///
    /// The `dag_hash` is computed automatically from the DAG's structure so
    /// that `resume_from` can detect DAG changes that would invalidate the
    /// checkpoint.
    pub fn new(run_id: &str, dag: &PipelineDag) -> Self {
        let dag_hash = compute_dag_hash(dag);
        Self {
            run_id: run_id.to_string(),
            completed_stages: Vec::new(),
            current_stage_index: 0,
            dag_hash,
            status: CheckpointStatus::InProgress,
        }
    }

    /// Record that a stage completed successfully.
    ///
    /// `stage_index` is the stage's position in the topological execution
    /// order (not the declaration order).  `output` is the stage's output
    /// data, which will be passed as input to the next stage on resume.
    pub fn record_stage_complete(&mut self, stage_index: usize, output: Vec<u8>) {
        let checkpoint = StageCheckpoint {
            run_id: self.run_id.clone(),
            stage_name: String::new(),
            stage_index,
            state: output,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        };
        self.completed_stages.push(checkpoint);
        self.current_stage_index = stage_index + 1;
    }

    /// Resume execution from this checkpoint.
    ///
    /// Validates that the supplied `dag` produces the same hash as the one
    /// that was used when the checkpoint was created.  If the hashes match,
    /// delegates to `executor.execute()` which will find this checkpoint in
    /// its store and skip already-completed stages.
    ///
    /// Returns an error if the DAG hash has changed (pipeline definition
    /// modified since checkpoint was saved).
    pub async fn resume_from(
        &self,
        executor: &PipelineExecutor,
        dag: &PipelineDag,
    ) -> Result<PipelineRunReport> {
        let current_hash = compute_dag_hash(dag);
        if self.dag_hash != current_hash {
            anyhow::bail!(
                "DAG hash mismatch: checkpoint hash '{}', current DAG hash '{}'. \
                 The pipeline definition has changed since the checkpoint was saved",
                self.dag_hash,
                current_hash
            );
        }

        // Get the last output from the most recently completed stage.
        let last_output = self
            .completed_stages
            .last()
            .map(|cp| cp.state.clone());

        // Delegate to the executor — it will find the checkpoint via its
        // store and skip already-completed stages automatically.
        Ok(executor.execute(&self.run_id, last_output).await)
    }
}

// ── DAG hash computation ──────────────────────────────────────────────────────

/// Compute a deterministic hash of a pipeline DAG for checkpoint validation.
///
/// Uses SHA-256 over the JSON representation of the DAG's stages and edges.
/// The hash changes whenever the pipeline topology or stage configuration
/// changes, invalidating stale checkpoints.
pub(crate) fn compute_dag_hash(dag: &PipelineDag) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"stages:");
    for stage in &dag.stages {
        if let Ok(json) = serde_json::to_string(stage) {
            hasher.update(json.as_bytes());
        }
    }
    hasher.update(b"edges:");
    for edge in &dag.edges {
        if let Ok(json) = serde_json::to_string(edge) {
            hasher.update(json.as_bytes());
        }
    }
    hex::encode(hasher.finalize())
}

// ── CheckpointStore trait ─────────────────────────────────────────────────────

/// Abstract storage backend for pipeline checkpoints.
///
/// Implementations must be `Send + Sync` to allow sharing across threads
/// and async boundaries.
pub trait CheckpointStore: Send + Sync {
    /// Persist a checkpoint.
    fn save(&self, cp: &PipelineCheckpoint) -> Result<()>;

    /// Load a checkpoint by run ID.  Returns `None` if no checkpoint exists
    /// for the given `run_id`.
    fn load(&self, run_id: &str) -> Result<Option<PipelineCheckpoint>>;

    /// List all available checkpoint run IDs.
    fn list(&self) -> Result<Vec<String>>;

    /// Delete a checkpoint by run ID.
    fn delete(&self, run_id: &str) -> Result<()>;
}

// ── FileCheckpointStore ───────────────────────────────────────────────────────

/// File-system backed checkpoint store.
///
/// Saves each checkpoint as an individual JSON file at
/// `{base_path}/{run_id}.json`.  The default base path is
/// `~/.b00t/checkpoints/`.
pub struct FileCheckpointStore {
    base_path: PathBuf,
}

impl FileCheckpointStore {
    /// Create a new store rooted at the given directory.
    ///
    /// The directory is created if it does not exist.
    pub fn new(base_path: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_path)
            .with_context(|| format!("creating checkpoint dir: {}", base_path.display()))?;
        Ok(Self { base_path })
    }

    /// Create a store using the default path `~/.b00t/checkpoints/`.
    pub fn default_path() -> Result<Self> {
        let home = dirs::home_dir().context("cannot determine home directory")?;
        let base = home.join(".b00t").join("checkpoints");
        Self::new(base)
    }

    /// Build the file path for a given run ID.
    fn path_for(&self, run_id: &str) -> PathBuf {
        // Sanitize run_id to prevent directory traversal.
        let safe_name = run_id.replace('/', "_").replace('\\', "_");
        self.base_path.join(format!("{}.json", safe_name))
    }
}

impl CheckpointStore for FileCheckpointStore {
    fn save(&self, cp: &PipelineCheckpoint) -> Result<()> {
        let path = self.path_for(&cp.run_id);
        let json = serde_json::to_string_pretty(cp)
            .with_context(|| format!("serializing checkpoint for '{}'", cp.run_id))?;
        // Write atomically via temp file + rename to avoid partial writes.
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, &json)
            .with_context(|| format!("writing checkpoint to {}", tmp_path.display()))?;
        fs::rename(&tmp_path, &path)
            .with_context(|| format!("finalizing checkpoint at {}", path.display()))?;
        Ok(())
    }

    fn load(&self, run_id: &str) -> Result<Option<PipelineCheckpoint>> {
        let path = self.path_for(run_id);
        if !path.exists() {
            return Ok(None);
        }
        let json = fs::read_to_string(&path)
            .with_context(|| format!("reading checkpoint from {}", path.display()))?;
        let cp: PipelineCheckpoint = serde_json::from_str(&json)
            .with_context(|| format!("parsing checkpoint from {}", path.display()))?;
        Ok(Some(cp))
    }

    fn list(&self) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        if !self.base_path.exists() {
            return Ok(ids);
        }
        for entry in fs::read_dir(&self.base_path)
            .with_context(|| format!("reading checkpoint dir {}", self.base_path.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    ids.push(stem.to_string());
                }
            }
        }
        ids.sort();
        Ok(ids)
    }

    fn delete(&self, run_id: &str) -> Result<()> {
        let path = self.path_for(run_id);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("deleting checkpoint at {}", path.display()))?;
        }
        Ok(())
    }
}

// ── InMemoryCheckpointStore ───────────────────────────────────────────────────

/// In-memory checkpoint store backed by a `HashMap`.
///
/// Primarily intended for testing, but also useful when pipeline state does
/// not need to survive process restarts.
pub struct InMemoryCheckpointStore {
    store: std::sync::Mutex<HashMap<String, PipelineCheckpoint>>,
}

impl InMemoryCheckpointStore {
    pub fn new() -> Self {
        Self {
            store: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

impl CheckpointStore for InMemoryCheckpointStore {
    fn save(&self, cp: &PipelineCheckpoint) -> Result<()> {
        let mut store = self.store.lock().expect("InMemoryCheckpointStore lock");
        store.insert(cp.run_id.clone(), cp.clone());
        Ok(())
    }

    fn load(&self, run_id: &str) -> Result<Option<PipelineCheckpoint>> {
        let store = self.store.lock().expect("InMemoryCheckpointStore lock");
        Ok(store.get(run_id).cloned())
    }

    fn list(&self) -> Result<Vec<String>> {
        let store = self.store.lock().expect("InMemoryCheckpointStore lock");
        let mut keys: Vec<String> = store.keys().cloned().collect();
        keys.sort();
        Ok(keys)
    }

    fn delete(&self, run_id: &str) -> Result<()> {
        let mut store = self.store.lock().expect("InMemoryCheckpointStore lock");
        store.remove(run_id);
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_logs::VecLogStore;
    use crate::pipeline_types::{
        CapsuleProfile, PipelineDag, PortDirection, PortMediaType, ResourceRequirements,
        StagePort, StageSpec,
    };
    use std::sync::Arc;

    /// Helper: build a minimal stage spec with a name.
    fn make_stage(name: &str) -> StageSpec {
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
                description: Some("auto".into()),
            }],
            output_ports: vec![StagePort {
                direction: PortDirection::Output,
                media_type: PortMediaType::Bytes,
                description: Some("auto".into()),
            }],
            error_routes: vec![],
            env: None,
            checkpoint_interval_seconds: None,
            secret_refs: None,
            flow_control: None,
        }
    }

    /// Helper: build a sequential DAG from stage names.
    ///
    /// Uses `from_sequential` to create edges only between consecutive stages
    /// (avoiding bidirectional edges that `build` would create when all ports
    /// use `Bytes`).
    fn sequential_dag(names: &[&str]) -> PipelineDag {
        let stages: Vec<StageSpec> = names.iter().map(|n| make_stage(n)).collect();
        PipelineDag::from_sequential(stages)
    }

    // ── Checkpoint round-trip (InMemory) ───────────────────────────────

    #[tokio::test]
    async fn test_save_and_load_round_trip() {
        let store = Arc::new(InMemoryCheckpointStore::new()) as Arc<dyn CheckpointStore>;
        let dag = sequential_dag(&["a", "b", "c"]);

        let cp = PipelineCheckpoint::new("round-trip-test", &dag);
        store.save(&cp).expect("save should succeed");

        let loaded = store
            .load("round-trip-test")
            .expect("load should succeed")
            .expect("checkpoint should exist");
        assert_eq!(loaded.run_id, "round-trip-test");
        assert_eq!(loaded.completed_stages.len(), 0);
        assert_eq!(loaded.current_stage_index, 0);
        assert_eq!(loaded.status, CheckpointStatus::InProgress);
        assert_eq!(loaded.dag_hash, cp.dag_hash);
    }

    // ── Record stage complete updates tracking ─────────────────────────

    #[test]
    fn test_record_stage_complete_updates_tracking() {
        let dag = sequential_dag(&["alpha", "beta", "gamma"]);
        let mut cp = PipelineCheckpoint::new("tracking-test", &dag);

        assert_eq!(cp.current_stage_index, 0);
        assert_eq!(cp.completed_stages.len(), 0);

        cp.record_stage_complete(0, b"alpha-output".to_vec());
        assert_eq!(cp.current_stage_index, 1);
        assert_eq!(cp.completed_stages.len(), 1);
        assert_eq!(cp.completed_stages[0].stage_index, 0);
        assert_eq!(cp.completed_stages[0].state, b"alpha-output");

        cp.record_stage_complete(1, b"beta-output".to_vec());
        assert_eq!(cp.current_stage_index, 2);
        assert_eq!(cp.completed_stages.len(), 2);
        assert_eq!(cp.completed_stages[1].stage_index, 1);
        assert_eq!(cp.completed_stages[1].state, b"beta-output");
    }

    // ── Resume skips completed stages ──────────────────────────────────

    #[tokio::test]
    async fn test_resume_skips_completed_stages() {
        let store = Arc::new(InMemoryCheckpointStore::new()) as Arc<dyn CheckpointStore>;
        let dag = sequential_dag(&["first", "second", "third"]);
        let run_id = "resume-skip-test";

        // Run the pipeline to completion with checkpoint, then verify it ran
        // all three stages.
        let executor = crate::pipeline_executor::PipelineExecutor::new(dag.clone())
            .with_checkpoint_store(store.clone());
        let report = executor.execute(run_id, Some(b"start".to_vec())).await;
        assert_eq!(report.stages.len(), 3, "all three stages should have run");
        for sr in &report.stages {
            assert_eq!(
                sr.status,
                StageStatus::Completed,
                "stage '{}' should be Completed",
                sr.stage_name
            );
        }

        // Verify the checkpoint was saved.
        let cp = store
            .load(run_id)
            .expect("load should succeed")
            .expect("checkpoint should exist after execution");
        assert_eq!(cp.completed_stages.len(), 3);
        assert_eq!(cp.status, CheckpointStatus::InProgress); // executor never marks Completed

        // Create a fresh executor with the same store and DAG — it should
        // find the checkpoint and skip already-completed stages.
        let fresh_executor = crate::pipeline_executor::PipelineExecutor::new(dag)
            .with_checkpoint_store(store.clone());
        let resume_report = fresh_executor.execute(run_id, None).await;

        // All stages should be present in the report.
        assert_eq!(
            resume_report.stages.len(),
            3,
            "resume report should have all 3 stages"
        );
        for sr in &resume_report.stages {
            assert_eq!(
                sr.status,
                StageStatus::Completed,
                "resumed stage '{}' should be Completed",
                sr.stage_name
            );
        }
        // The output should reflect the full chain.
        let last_output = resume_report.stages[2]
            .output
            .as_ref()
            .expect("last stage should have output");
        let output_str = String::from_utf8_lossy(last_output);
        assert!(
            output_str.contains(":third"),
            "output should contain ':third', got: {}",
            output_str
        );
    }

    // ── FileCheckpointStore persists across instances ───────────────────

    #[test]
    fn test_file_checkpoint_store_persists() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store_a = FileCheckpointStore::new(dir.path().to_path_buf())
            .expect("store creation");
        let dag = sequential_dag(&["x", "y"]);

        let cp = PipelineCheckpoint::new("persist-test", &dag);
        store_a.save(&cp).expect("save should succeed");

        // Create a *new* store instance pointing at the same directory.
        let store_b = FileCheckpointStore::new(dir.path().to_path_buf())
            .expect("store creation");
        let loaded = store_b
            .load("persist-test")
            .expect("load should succeed")
            .expect("checkpoint should persist");
        assert_eq!(loaded.run_id, "persist-test");
        assert_eq!(loaded.dag_hash, cp.dag_hash);

        // Verify list() returns the run ID.
        let ids = store_b.list().expect("list should succeed");
        assert!(ids.contains(&"persist-test".to_string()));

        // Verify delete() removes it.
        store_b.delete("persist-test").expect("delete should succeed");
        let gone = store_b
            .load("persist-test")
            .expect("load after delete should succeed");
        assert!(gone.is_none(), "checkpoint should be gone after delete");
    }

    // ── Missing checkpoint returns None ─────────────────────────────────

    #[test]
    fn test_missing_checkpoint_returns_none() {
        let store = InMemoryCheckpointStore::new();
        let loaded = store
            .load("nonexistent-run")
            .expect("load of missing should succeed");
        assert!(loaded.is_none(), "missing checkpoint should return None");
    }

    // ── DAG hash changes invalidate checkpoint ─────────────────────────

    #[tokio::test]
    async fn test_dag_hash_mismatch_on_resume() {
        let store = Arc::new(InMemoryCheckpointStore::new()) as Arc<dyn CheckpointStore>;
        let dag_a = sequential_dag(&["stage-x", "stage-y"]);
        let dag_b = sequential_dag(&["stage-x", "stage-y", "stage-z"]);

        let cp = PipelineCheckpoint::new("hash-mismatch", &dag_a);
        store.save(&cp).expect("save should succeed");

        let executor = crate::pipeline_executor::PipelineExecutor::new(dag_b.clone())
            .with_checkpoint_store(store.clone());

        // The dag has changed (new stage), so resume_from should fail.
        let result = cp.resume_from(&executor, &dag_b).await;
        assert!(
            result.is_err(),
            "resume_from should fail when DAG hash changed"
        );
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("hash mismatch"),
            "error should mention hash mismatch, got: {}",
            err_msg
        );
    }

    // ── FileCheckpointStore handles missing directory ──────────────────

    #[test]
    fn test_file_store_creates_directory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let checkpoints_dir = dir.path().join("nested").join("checkpoints");
        assert!(!checkpoints_dir.exists(), "dir should not exist yet");

        let store = FileCheckpointStore::new(checkpoints_dir.clone())
            .expect("store creation should create directory");
        assert!(checkpoints_dir.exists(), "store should create the directory");

        let dag = sequential_dag(&["a"]);
        let cp = PipelineCheckpoint::new("dir-creation", &dag);
        store.save(&cp).expect("save should succeed");
    }

    // ── Empty checkpoint list ──────────────────────────────────────────

    #[test]
    fn test_empty_checkpoint_list() {
        let store = InMemoryCheckpointStore::new();
        let ids = store.list().expect("list should succeed");
        assert!(ids.is_empty(), "new store should have no checkpoints");
    }

    // ── Multiple checkpoints in file store ─────────────────────────────

    #[test]
    fn test_multiple_checkpoints_in_file_store() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = FileCheckpointStore::new(dir.path().to_path_buf())
            .expect("store creation");
        let dag = sequential_dag(&["a"]);

        store
            .save(&PipelineCheckpoint::new("run-1", &dag))
            .expect("save run-1");
        store
            .save(&PipelineCheckpoint::new("run-2", &dag))
            .expect("save run-2");
        store
            .save(&PipelineCheckpoint::new("run-3", &dag))
            .expect("save run-3");

        let ids = store.list().expect("list should succeed");
        assert_eq!(ids.len(), 3, "should have 3 checkpoints");
        assert!(ids.contains(&"run-1".to_string()));
        assert!(ids.contains(&"run-2".to_string()));
        assert!(ids.contains(&"run-3".to_string()));
    }

    // ── PipelineCheckpoint::new generates unique hashes ────────────────

    #[test]
    fn test_different_dags_have_different_hashes() {
        let dag_abc = sequential_dag(&["a", "b", "c"]);
        let dag_xyz = sequential_dag(&["x", "y", "z"]);

        let cp1 = PipelineCheckpoint::new("test", &dag_abc);
        let cp2 = PipelineCheckpoint::new("test", &dag_xyz);

        assert_ne!(
            cp1.dag_hash, cp2.dag_hash,
            "different DAGs must produce different hashes"
        );
    }

    // ── Same DAG produces same hash ────────────────────────────────────

    #[test]
    fn test_same_dag_produces_same_hash() {
        let dag = sequential_dag(&["a", "b"]);
        let cp1 = PipelineCheckpoint::new("test", &dag);
        let cp2 = PipelineCheckpoint::new("test", &dag);
        assert_eq!(
            cp1.dag_hash, cp2.dag_hash,
            "same DAG must produce same hash"
        );
    }

    // ── FileCheckpointStore sanitizes run_id ───────────────────────────

    #[test]
    fn test_run_id_sanitization() {
        let dir = tempfile::tempdir().expect("temp dir");
        let store = FileCheckpointStore::new(dir.path().to_path_buf())
            .expect("store creation");
        let dag = sequential_dag(&["a"]);

        let cp = PipelineCheckpoint::new("path/../traversal", &dag);
        store.save(&cp).expect("save should not panic");

        // Should have been saved as a file with underscores
        let ids = store.list().expect("list");
        // The sanitized name replaces / with _
        assert!(
            ids.iter().any(|id| id.contains("path") || id.contains("traversal")),
            "checkpoint should be listed even with tricky run_id: {:?}",
            ids
        );
    }

    // ── Delete nonexistent is a no-op ──────────────────────────────────

    #[test]
    fn test_delete_nonexistent() {
        let store = InMemoryCheckpointStore::new();
        // Should not panic.
        store.delete("does-not-exist").expect("delete non-existent should be ok");
    }
}
