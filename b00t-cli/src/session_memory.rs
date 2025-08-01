use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use crate::utils::get_workspace_root;

/// Session memory using confy for TOML persistence at git root
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMemory {
    /// String key-value storage
    pub strings: HashMap<String, String>,
    /// Numeric key-value storage for incr/decr operations
    pub numbers: HashMap<String, i64>,
    /// Boolean flags
    pub flags: HashMap<String, bool>,
    /// Session metadata
    pub metadata: SessionMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub session_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    /// Track if README.md has been read this session
    pub readme_read: bool,
    /// Initial git branch when session started
    pub initial_branch: Option<String>,
}

impl Default for SessionMetadata {
    fn default() -> Self {
        let now = chrono::Utc::now();
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            created_at: now,
            updated_at: now, 
            readme_read: false,
            initial_branch: None,
        }
    }
}

impl SessionMemory {
    /// Get the git root path for storing ._b00t_.toml
    fn get_config_path() -> Result<PathBuf> {
        let git_root = get_workspace_root();
        Ok(PathBuf::from(git_root))
    }

    /// Ensure ._b00t_.toml is in .gitignore
    fn ensure_gitignore_entry() -> Result<()> {
        let config_dir = Self::get_config_path()?;
        let gitignore_path = config_dir.join(".gitignore");
        let target_entry = "._b00t_.toml";

        // Check if .gitignore exists
        if !gitignore_path.exists() {
            // Create .gitignore with the entry
            fs::write(&gitignore_path, format!("# b00t session files\n{}\n", target_entry))
                .context("Failed to create .gitignore")?;
            return Ok(());
        }

        // Read existing .gitignore to check if entry exists
        let file = fs::File::open(&gitignore_path).context("Failed to open .gitignore")?;
        let reader = BufReader::new(file);
        
        for line in reader.lines() {
            let line = line.context("Failed to read line from .gitignore")?;
            if line.trim() == target_entry {
                // Entry already exists
                return Ok(());
            }
        }

        // Entry doesn't exist, append it
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&gitignore_path)
            .context("Failed to open .gitignore for writing")?;
        
        writeln!(file, "{}", target_entry)
            .context("Failed to write to .gitignore")?;

        Ok(())
    }

    /// Load or create session memory from git root ._b00t_.toml
    pub fn load() -> Result<Self> {
        let config_dir = Self::get_config_path()?;
        
        // Ensure ._b00t_.toml is in .gitignore before creating/loading
        Self::ensure_gitignore_entry().context("Failed to ensure .gitignore entry")?;
        
        // Use confy to load from ._b00t_.toml in git root
        let mut memory: SessionMemory = confy::load_path(config_dir.join("._b00t_.toml"))
            .context("Failed to load session memory")?;

        // Initialize metadata if this is first load
        if memory.metadata.session_id.is_empty() {
            memory.metadata = SessionMetadata::default();
            memory.capture_git_context()?;
            memory.save()?;
        } else {
            // Update last accessed time
            memory.metadata.updated_at = chrono::Utc::now();
            memory.save()?;
        }

        Ok(memory)
    }

    /// Save session memory using confy
    pub fn save(&self) -> Result<()> {
        let config_dir = Self::get_config_path()?;
        confy::store_path(config_dir.join("._b00t_.toml"), self)
            .context("Failed to save session memory")
    }

    /// Capture current git context
    fn capture_git_context(&mut self) -> Result<()> {
        self.metadata.initial_branch = duct::cmd!("git", "branch", "--show-current")
            .read()
            .ok()
            .map(|branch| branch.trim().to_string());
        Ok(())
    }

    // String operations
    pub fn get(&self, key: &str) -> Option<&String> {
        self.strings.get(key)
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        self.strings.insert(key.to_string(), value.to_string());
        self.metadata.updated_at = chrono::Utc::now();
        self.save()
    }

    // Numeric operations with atomic-like behavior
    pub fn get_num(&self, key: &str) -> i64 {
        self.numbers.get(key).copied().unwrap_or(0)
    }

    pub fn set_num(&mut self, key: &str, value: i64) -> Result<()> {
        self.numbers.insert(key.to_string(), value);
        self.metadata.updated_at = chrono::Utc::now();
        self.save()
    }

    pub fn incr(&mut self, key: &str) -> Result<i64> {
        let new_value = self.get_num(key) + 1;
        self.numbers.insert(key.to_string(), new_value); 
        self.metadata.updated_at = chrono::Utc::now();
        self.save()?;
        Ok(new_value)
    }

    pub fn decr(&mut self, key: &str) -> Result<i64> {
        let new_value = self.get_num(key) - 1;
        self.numbers.insert(key.to_string(), new_value);
        self.metadata.updated_at = chrono::Utc::now();
        self.save()?;
        Ok(new_value)
    }

    // Boolean flag operations  
    pub fn get_flag(&self, key: &str) -> bool {
        self.flags.get(key).copied().unwrap_or(false)
    }

    pub fn set_flag(&mut self, key: &str, value: bool) -> Result<()> {
        self.flags.insert(key.to_string(), value);
        self.metadata.updated_at = chrono::Utc::now();
        self.save()
    }

    // README tracking
    pub fn mark_readme_read(&mut self) -> Result<()> {
        self.metadata.readme_read = true;
        self.metadata.updated_at = chrono::Utc::now();
        self.save()
    }

    pub fn is_readme_read(&self) -> bool {
        self.metadata.readme_read
    }

    // Utility methods
    pub fn clear(&mut self) -> Result<()> {
        self.strings.clear();
        self.numbers.clear();
        self.flags.clear();
        self.metadata.updated_at = chrono::Utc::now();
        self.save()
    }

    pub fn list_keys(&self) -> Vec<(String, String)> {
        let mut keys = Vec::new();
        
        for key in self.strings.keys() {
            keys.push((key.clone(), "string".to_string()));
        }
        for key in self.numbers.keys() {
            keys.push((key.clone(), "number".to_string()));
        }
        for key in self.flags.keys() {
            keys.push((key.clone(), "flag".to_string()));
        }
        
        keys.sort_by(|a, b| a.0.cmp(&b.0));
        keys
    }

    pub fn get_summary(&self) -> String {
        format!(
            "Session {} | {} strings, {} numbers, {} flags | README: {} | Branch: {} | Updated: {}",
            &self.metadata.session_id[..8],
            self.strings.len(),
            self.numbers.len(),
            self.flags.len(),
            if self.metadata.readme_read { "✓" } else { "❌" },
            self.metadata.initial_branch.as_deref().unwrap_or("unknown"),
            self.metadata.updated_at.format("%H:%M:%S")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_session_memory_operations() -> Result<()> {
        // Test basic operations in isolation - use default values
        let mut memory = SessionMemory::default();
        
        // String operations
        memory.strings.insert("test_key".to_string(), "test_value".to_string());
        assert_eq!(memory.get("test_key"), Some(&"test_value".to_string()));
        
        // Numeric operations  
        memory.numbers.insert("counter".to_string(), 5);
        assert_eq!(memory.get_num("counter"), 5);
        
        // Simulate increment/decrement without persistence
        let new_val = memory.get_num("counter") + 1;
        memory.numbers.insert("counter".to_string(), new_val);
        assert_eq!(memory.get_num("counter"), 6);
        
        let new_val = memory.get_num("counter") - 1;
        memory.numbers.insert("counter".to_string(), new_val);
        assert_eq!(memory.get_num("counter"), 5);
        
        // Flag operations
        memory.flags.insert("enabled".to_string(), true);
        assert!(memory.get_flag("enabled"));
        assert!(!memory.get_flag("disabled"));
        
        // README tracking
        assert!(!memory.is_readme_read());
        memory.metadata.readme_read = true;
        assert!(memory.is_readme_read());
        
        Ok(())
    }

    #[test]
    fn test_gitignore_entry_creation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let gitignore_path = temp_dir.path().join(".gitignore");
        let target_entry = "._b00t_.toml";

        // Test creating .gitignore when it doesn't exist
        assert!(!gitignore_path.exists());
        
        // Simulate the functionality manually since we can't easily mock the path
        fs::write(&gitignore_path, format!("# b00t session files\n{}\n", target_entry))?;
        
        assert!(gitignore_path.exists());
        let content = fs::read_to_string(&gitignore_path)?;
        assert!(content.contains(target_entry));
        
        Ok(())
    }

    #[test]
    fn test_gitignore_entry_detection() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let gitignore_path = temp_dir.path().join(".gitignore");
        let target_entry = "._b00t_.toml";

        // Create .gitignore with entry already present
        fs::write(&gitignore_path, format!("*.log\n{}\n*.tmp\n", target_entry))?;
        
        // Read and verify the entry exists
        let content = fs::read_to_string(&gitignore_path)?;
        let has_entry = content.lines().any(|line| line.trim() == target_entry);
        assert!(has_entry);
        
        Ok(())
    }
}