use std::fs;
use std::io::Write;
use std::path::PathBuf;

use crate::errors::StoreResult;
use crate::types::{AgentContext, HookToken};

pub struct ContextStore {
    dir: PathBuf,
}

impl ContextStore {
    /// Default: ~/.local/share/b00t/hooks/
    pub fn new() -> StoreResult<Self> {
        let dir = dirs_data_dir()?.join("b00t").join("hooks");
        fs::create_dir_all(&dir)?;
        Ok(ContextStore { dir })
    }

    /// Custom path
    pub fn with_path(path: PathBuf) -> Self {
        ContextStore { dir: path }
    }

    /// Atomic save: write to .tmp, rename to .json
    pub fn save(&self, token: &HookToken, context: &AgentContext) -> StoreResult<()> {
        let hook_id = token.id;
        let json_path = self.dir.join(format!("{}.json", hook_id));
        let tmp_path = self.dir.join(format!("{}.json.tmp", hook_id));

        let json_content = serde_json::to_string_pretty(context)?;

        // Ensure directory exists
        fs::create_dir_all(&self.dir)?;

        // Write to .tmp
        {
            let mut tmp_file = fs::File::create(&tmp_path)?;
            tmp_file.write_all(json_content.as_bytes())?;
            tmp_file.sync_all()?;
        }

        // Atomic rename
        fs::rename(&tmp_path, &json_path)?;

        Ok(())
    }

    /// Load context for a hook token
    pub fn load(&self, token: &HookToken) -> StoreResult<Option<AgentContext>> {
        let hook_id = token.id;
        let path = self.dir.join(format!("{}.json", hook_id));
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        let context: AgentContext = serde_json::from_str(&content)?;
        Ok(Some(context))
    }

    /// Load by hook ID (useful for hook fire callbacks)
    pub fn load_by_id(&self, hook_id: &uuid::Uuid) -> StoreResult<Option<AgentContext>> {
        let path = self.dir.join(format!("{}.json", hook_id));
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)?;
        let context: AgentContext = serde_json::from_str(&content)?;
        Ok(Some(context))
    }

    /// Delete context (after hook fires and agent resumes)
    pub fn delete(&self, hook_id: &uuid::Uuid) -> StoreResult<()> {
        let json_path = self.dir.join(format!("{}.json", hook_id));
        let tmp_path = self.dir.join(format!("{}.json.tmp", hook_id));

        if json_path.exists() {
            fs::remove_file(&json_path)?;
        }
        if tmp_path.exists() {
            fs::remove_file(&tmp_path)?;
        }
        Ok(())
    }

    /// List all pending hooks
    pub fn list_pending(&self) -> StoreResult<Vec<HookToken>> {
        let mut tokens = Vec::new();
        if !self.dir.exists() {
            return Ok(tokens);
        }
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                // Skip .tmp files
                let content = fs::read_to_string(&path)?;
                if let Ok(context) = serde_json::from_str::<AgentContext>(&content) {
                    tokens.push(context.hook_token);
                }
            }
        }
        Ok(tokens)
    }

    /// Total stored hook count
    pub fn count(&self) -> StoreResult<usize> {
        let mut count = 0usize;
        if !self.dir.exists() {
            return Ok(0);
        }
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                count += 1;
            }
        }
        Ok(count)
    }
}

fn dirs_data_dir() -> StoreResult<PathBuf> {
    // Try XDG_DATA_HOME first, fall back to ~/.local/share
    if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        Ok(PathBuf::from(dir))
    } else if let Ok(home) = std::env::var("HOME") {
        Ok(PathBuf::from(home).join(".local").join("share"))
    } else {
        // Last fallback: current directory
        Ok(PathBuf::from(".local").join("share"))
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::types::{HookToken, HookType};
    use chrono::Utc;
    use uuid::Uuid;

    fn test_token() -> HookToken {
        HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::TimerMs(1000),
            created_at: Utc::now(),
            ttl_ms: None,
            description: "test hook".to_string(),
        }
    }

    fn test_context(token: &HookToken) -> AgentContext {
        AgentContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            gate: "test-gate".to_string(),
            result_so_far: serde_json::json!({"status": "pending"}),
            reasoning: "testing".to_string(),
            created_at: Utc::now(),
            hook_token: token.clone(),
            continuation: "resume_here".to_string(),
        }
    }

    #[test]
    fn test_save_and_load() -> StoreResult<()> {
        let tmpdir = tempfile::tempdir().unwrap();
        let store = ContextStore::with_path(tmpdir.path().join("hooks"));

        let token = test_token();
        let context = test_context(&token);

        store.save(&token, &context)?;

        let loaded = store.load(&token)?.expect("Should have loaded context");
        assert_eq!(loaded.agent_id, "test-agent");
        assert_eq!(loaded.continuation, "resume_here");

        Ok(())
    }

    #[test]
    fn test_load_nonexistent() -> StoreResult<()> {
        let tmpdir = tempfile::tempdir().unwrap();
        let store = ContextStore::with_path(tmpdir.path().join("hooks"));

        let token = test_token();
        let loaded = store.load(&token)?;
        assert!(loaded.is_none());

        Ok(())
    }

    #[test]
    fn test_load_by_id() -> StoreResult<()> {
        let tmpdir = tempfile::tempdir().unwrap();
        let store = ContextStore::with_path(tmpdir.path().join("hooks"));

        let token = test_token();
        let context = test_context(&token);
        store.save(&token, &context)?;

        let loaded = store.load_by_id(&token.id)?.expect("Should have loaded");
        assert_eq!(loaded.agent_id, "test-agent");

        Ok(())
    }

    #[test]
    fn test_delete() -> StoreResult<()> {
        let tmpdir = tempfile::tempdir().unwrap();
        let store = ContextStore::with_path(tmpdir.path().join("hooks"));

        let token = test_token();
        let context = test_context(&token);
        store.save(&token, &context)?;
        assert!(store.load(&token)?.is_some());

        store.delete(&token.id)?;
        assert!(store.load(&token)?.is_none());

        Ok(())
    }

    #[test]
    fn test_list_pending() -> StoreResult<()> {
        let tmpdir = tempfile::tempdir().unwrap();
        let store = ContextStore::with_path(tmpdir.path().join("hooks"));

        let token1 = test_token();
        let token2 = test_token();
        let ctx1 = test_context(&token1);
        let ctx2 = test_context(&token2);

        store.save(&token1, &ctx1)?;
        store.save(&token2, &ctx2)?;

        let pending = store.list_pending()?;
        assert_eq!(pending.len(), 2);

        Ok(())
    }

    #[test]
    fn test_count() -> StoreResult<()> {
        let tmpdir = tempfile::tempdir().unwrap();
        let store = ContextStore::with_path(tmpdir.path().join("hooks"));

        assert_eq!(store.count()?, 0);

        let token = test_token();
        let context = test_context(&token);
        store.save(&token, &context)?;
        assert_eq!(store.count()?, 1);

        Ok(())
    }

    #[test]
    fn test_crash_safety_partial_tmp() -> StoreResult<()> {
        // Simulate crash safety: a partial .tmp file should not show up as .json
        let tmpdir = tempfile::tempdir().unwrap();
        let dir = tmpdir.path().join("hooks");
        std::fs::create_dir_all(&dir)?;

        // Write a partial tmp file (simulating crash during write)
        let hook_id = Uuid::new_v4();
        let tmp_path = dir.join(format!("{}.json.tmp", hook_id));
        std::fs::write(&tmp_path, b"partial garbage")?;

        // The .json file should not exist after atomic rename wasn't completed
        let json_path = dir.join(format!("{}.json", hook_id));
        assert!(
            !json_path.exists(),
            "Crash during write should not create .json"
        );

        // Now create a proper file via the store
        let store = ContextStore::with_path(dir.clone());
        let token = HookToken {
            id: hook_id,
            hook_type: HookType::TimerMs(500),
            created_at: chrono::Utc::now(),
            ttl_ms: None,
            description: "crash test".to_string(),
        };
        let context = test_context(&token);
        store.save(&token, &context)?;

        // The tmp file should be gone, json should exist
        assert!(
            !tmp_path.exists(),
            "Tmp file should be gone after successful save"
        );
        assert!(json_path.exists(), "Json file should exist after save");

        Ok(())
    }
}
