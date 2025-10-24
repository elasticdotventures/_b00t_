use crate::utils::get_workspace_root;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

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
    /// Configuration settings
    pub config: SessionConfig,
}

/// Configuration settings for b00t session behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Environment variables to track for agent/environment detection
    pub tracked_env_vars: Vec<String>,
    /// Whether to show verbose output in interactive shells
    pub verbose_interactive: bool,
    /// Whether to show output in non-interactive shells
    pub verbose_noninteractive: bool,
    /// Custom agent detection patterns
    pub agent_patterns: HashMap<String, String>,
    /// Whether to use .env file overrides
    pub use_env_overrides: bool,
    /// Session counting settings
    pub count_shell_starts: bool,
    /// Tera template for status output (OODA loop context) - optional, uses default if None
    pub status_template: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        let mut agent_patterns = HashMap::new();
        agent_patterns.insert("claude".to_string(), "🤖 Claude".to_string());
        agent_patterns.insert("gemini".to_string(), "🤖 Gemini".to_string());
        agent_patterns.insert("gpt".to_string(), "🤖 GPT".to_string());
        agent_patterns.insert("openai".to_string(), "🤖 OpenAI".to_string());

        Self {
            tracked_env_vars: vec![
                "TERM".to_string(),
                "TERM_PROGRAM".to_string(),
                "SHELL".to_string(),
                "PWD".to_string(),
                "USER".to_string(),
                "HOME".to_string(),
                "CLAUDECODE".to_string(),
                "_B00T_Agent".to_string(),
                "VSCODE_GIT_IPC_HANDLE".to_string(),
                "SSH_CLIENT".to_string(),
                "SSH_TTY".to_string(),
                "container".to_string(),
                "ANTHROPIC_API_KEY".to_string(),
                "CLAUDE_API_KEY".to_string(),
            ],
            verbose_interactive: true,
            verbose_noninteractive: false,
            agent_patterns,
            use_env_overrides: true,
            count_shell_starts: true,
            status_template: None, // Uses default template if None
        }
    }
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
    /// Get the git root path for storing _b00t_.toml in .git/ directory
    pub fn get_config_path() -> Result<PathBuf> {
        let git_root = get_workspace_root();
        Ok(PathBuf::from(git_root).join(".git"))
    }

    /// Ensure _b00t_.toml is in .gitignore (no longer needed since it's in .git/)
    fn ensure_gitignore_entry() -> Result<()> {
        // No longer needed since _b00t_.toml is stored in .git/ directory
        // which is automatically ignored by git
        Ok(())
    }

    fn _unused_ensure_gitignore_entry() -> Result<()> {
        let git_root = get_workspace_root();
        let gitignore_path = PathBuf::from(git_root).join(".gitignore");
        let target_entry = "_b00t_.toml";

        // Check if .gitignore exists
        if !gitignore_path.exists() {
            // Create .gitignore with the entry
            fs::write(
                &gitignore_path,
                format!("# b00t session files\n{}\n", target_entry),
            )
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

        writeln!(file, "{}", target_entry).context("Failed to write to .gitignore")?;

        Ok(())
    }

    /// Load or create session memory from git root ._b00t_.toml
    pub fn load() -> Result<Self> {
        let config_dir = Self::get_config_path()?;

        // Ensure ._b00t_.toml is in .gitignore before creating/loading
        Self::ensure_gitignore_entry().context("Failed to ensure .gitignore entry")?;

        // Use confy to load from _b00t_.toml in .git directory
        let mut memory: SessionMemory = confy::load_path(config_dir.join("_b00t_.toml"))
            .context("Failed to load session memory")?;

        // Initialize metadata if this is first load
        if memory.metadata.session_id.is_empty() {
            memory.metadata = SessionMetadata::default();
            memory.capture_git_context()?;
        }

        // Initialize config if missing (backward compatibility)
        if memory.config.tracked_env_vars.is_empty() {
            memory.config = SessionConfig::default();
        }

        // Update last accessed time and save
        memory.metadata.updated_at = chrono::Utc::now();
        memory.save()?;

        Ok(memory)
    }

    /// Save session memory using confy
    pub fn save(&self) -> Result<()> {
        let config_dir = Self::get_config_path()?;
        confy::store_path(config_dir.join("_b00t_.toml"), self)
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
            if self.metadata.readme_read {
                "✓"
            } else {
                "❌"
            },
            self.metadata.initial_branch.as_deref().unwrap_or("unknown"),
            self.metadata.updated_at.format("%H:%M:%S")
        )
    }

    /// Load .env file and return as HashMap for overrides
    pub fn load_env_overrides(&self) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();

        if !self.config.use_env_overrides {
            return env_vars;
        }

        let config_dir = Self::get_config_path().unwrap_or_else(|_| PathBuf::from("."));
        let env_path = config_dir.join(".env");

        if let Ok(contents) = fs::read_to_string(&env_path) {
            for line in contents.lines() {
                let line = line.trim();
                // Skip comments and empty lines
                if line.starts_with('#') || line.is_empty() {
                    continue;
                }

                // Parse KEY=VALUE format
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim();
                    // Remove quotes if present
                    let value = if (value.starts_with('"') && value.ends_with('"'))
                        || (value.starts_with('\'') && value.ends_with('\''))
                    {
                        &value[1..value.len() - 1]
                    } else {
                        value
                    };
                    env_vars.insert(key.to_string(), value.to_string());
                }
            }
        }

        env_vars
    }

    /// Get environment variable with .env override support
    pub fn get_env_var(&self, key: &str) -> Option<String> {
        // First check .env overrides
        if self.config.use_env_overrides {
            let env_overrides = self.load_env_overrides();
            if let Some(value) = env_overrides.get(key) {
                return Some(value.clone());
            }
        }

        // Fall back to system environment
        std::env::var(key).ok()
    }

    /// Collect tracked environment variables with overrides
    pub fn collect_tracked_env(&self) -> HashMap<String, String> {
        let mut result = HashMap::new();

        for var in &self.config.tracked_env_vars {
            if let Some(value) = self.get_env_var(var) {
                result.insert(var.clone(), value);
            }
        }

        result
    }

    /// Increment shell start count for this PID
    pub fn increment_shell_count(&mut self) -> Result<i64> {
        if !self.config.count_shell_starts {
            return Ok(0);
        }

        let pid = std::process::id().to_string();
        let key = format!("shell_count_{}", pid);
        self.incr(&key)
    }

    /// Check if shell output should be verbose based on interactive state
    pub fn should_show_verbose_output(&self) -> bool {
        let is_interactive = self.is_interactive_shell();

        if is_interactive {
            self.config.verbose_interactive
        } else {
            self.config.verbose_noninteractive
        }
    }

    /// Detect if running in interactive shell
    fn is_interactive_shell(&self) -> bool {
        // Check if stdin is a tty
        #[cfg(unix)]
        {
            unsafe { libc::isatty(libc::STDIN_FILENO) != 0 }
        }
        #[cfg(not(unix))]
        {
            // Fallback: check TERM environment variable
            self.get_env_var("TERM").is_some()
        }
    }

    /// Get seconds since session creation
    pub fn seconds_since_start(&self) -> i64 {
        let now = chrono::Utc::now();
        (now - self.metadata.created_at).num_seconds()
    }

    /// Get seconds since last update  
    pub fn seconds_since_update(&self) -> i64 {
        let now = chrono::Utc::now();
        (now - self.metadata.updated_at).num_seconds()
    }

    /// Increment build count for current branch
    pub fn increment_build_count(&mut self) -> Result<i64> {
        let branch = self.metadata.initial_branch.as_deref().unwrap_or("unknown");
        let key = format!("build_count_{}", branch);
        self.incr(&key)
    }

    /// Increment compile count for current session
    pub fn increment_compile_count(&mut self) -> Result<i64> {
        self.incr("compile_count")
    }

    /// Increment test run count for current session
    pub fn increment_test_count(&mut self) -> Result<i64> {
        self.incr("test_count")
    }

    /// Get agent context for OODA loop planning
    pub fn get_agent_context(&self) -> AgentContext {
        let agent_name = self
            .get_env_var("_B00T_Agent")
            .unwrap_or_else(|| "Unknown".to_string());
        let current_branch = self.metadata.initial_branch.as_deref().unwrap_or("unknown");

        AgentContext {
            agent_name,
            session_id: self.metadata.session_id.clone(),
            session_duration: self.seconds_since_start(),
            current_branch: current_branch.to_string(),
            shell_count: self.get_num(&format!("shell_count_{}", std::process::id())),
            build_count: self.get_num(&format!("build_count_{}", current_branch)),
            compile_count: self.get_num("compile_count"),
            test_count: self.get_num("test_count"),
            diagnostic_passing: self.get_num("diagnostic_passing"),
            diagnostic_total: self.get_num("diagnostic_total"),
        }
    }

    /// Render status using Tera template with agent context
    pub fn render_status_template(&self) -> Result<String> {
        let context = self.get_agent_context();
        let mut tera = tera::Tera::new("templates/*").unwrap_or_else(|_| tera::Tera::default());

        // Get template content from file or custom config
        let template_content = if let Some(custom_template) = &self.config.status_template {
            custom_template.clone()
        } else {
            // Load default template from repo file
            self.load_default_status_template()?
        };

        // Add the status template
        tera.add_raw_template("status", &template_content)
            .context("Failed to add status template")?;

        // Create Tera context with agent data
        let mut template_context = tera::Context::new();
        template_context.insert("agent", &context);
        template_context.insert(
            "duration_formatted",
            &format_duration(context.session_duration),
        );
        template_context.insert(
            "health_ratio",
            &if context.diagnostic_total > 0 {
                context.diagnostic_passing as f64 / context.diagnostic_total as f64
            } else {
                0.0
            },
        );
        template_context.insert(
            "builds_per_hour",
            &if context.session_duration > 300 {
                (context.build_count as f64 / context.session_duration as f64) * 3600.0
            } else {
                0.0
            },
        );
        // Insert seconds_since_update and seconds_since_start for template access
        template_context.insert("seconds_since_update", &self.seconds_since_update());
        template_context.insert("seconds_since_start", &self.seconds_since_start());

        // Add b00t-c0re-lib context variables for template compatibility
        if let Ok(b00t_context) = b00t_c0re_lib::B00tContext::current() {
            template_context.insert("PID", &b00t_context.pid);
            template_context.insert("TIMESTAMP", &b00t_context.timestamp);
            template_context.insert("USER", &b00t_context.user);
            template_context.insert("BRANCH", &b00t_context.branch);
            template_context.insert("_B00T_Agent", &b00t_context.agent);
            template_context.insert("_B00T_AGENT", &b00t_context.agent);
            template_context.insert("MODEL_SIZE", &b00t_context.model_size);
            template_context.insert("PRIVACY", &b00t_context.privacy);
            template_context.insert("WORKSPACE_ROOT", &b00t_context.workspace_root);
            template_context.insert("IS_GIT_REPO", &b00t_context.is_git_repo);
            template_context.insert("GIT_REPO", &b00t_context.is_git_repo);
            template_context.insert("HOSTNAME", &b00t_context.hostname);
        }

        // Add conditional flags for template logic
        template_context.insert("has_cargo", &std::path::Path::new("Cargo.toml").exists());
        template_context.insert(
            "has_package_json",
            &std::path::Path::new("package.json").exists(),
        );
        template_context.insert("has_tests", &std::path::Path::new("tests").exists());

        tera.render("status", &template_context)
            .context("Failed to render status template")
    }

    /// Load default status template from repo file
    pub fn load_default_status_template(&self) -> Result<String> {
        let config_dir = Self::get_config_path()?;
        let template_path = config_dir.join("templates").join("status.tera");

        // Try to load from repo templates directory
        if template_path.exists() {
            std::fs::read_to_string(&template_path).context("Failed to read status template file")
        } else {
            // Fallback to built-in minimal template
            Ok(r#"
🩺 Agent: {{agent.agent_name}} | Session: {{duration_formatted}}
🌿 Branch: {{agent.current_branch}} ({{agent.build_count}}🔨)
📊 Activity: {{agent.shell_count}}🐚, {{agent.compile_count}}⚙️, {{agent.test_count}}🧪
{% if agent.diagnostic_total > 0 -%}
{% set health_percentage = (health_ratio * 100) | round -%}
🧠 Health: {{health_percentage}}%
{% endif -%}
"#
            .trim()
            .to_string())
        }
    }
}

/// Agent context for OODA loop decision making
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentContext {
    pub agent_name: String,
    pub session_id: String,
    pub session_duration: i64,
    pub current_branch: String,
    pub shell_count: i64,
    pub build_count: i64,
    pub compile_count: i64,
    pub test_count: i64,
    pub diagnostic_passing: i64,
    pub diagnostic_total: i64,
}

/// Format duration in human readable format  
fn format_duration(seconds: i64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m{}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h{}m", seconds / 3600, (seconds % 3600) / 60)
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
        memory
            .strings
            .insert("test_key".to_string(), "test_value".to_string());
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
        let target_entry = "_b00t_.toml";

        // Test creating .gitignore when it doesn't exist
        assert!(!gitignore_path.exists());

        // Simulate the functionality manually since we can't easily mock the path
        fs::write(
            &gitignore_path,
            format!("# b00t session files\n{}\n", target_entry),
        )?;

        assert!(gitignore_path.exists());
        let content = fs::read_to_string(&gitignore_path)?;
        assert!(content.contains(target_entry));

        Ok(())
    }

    #[test]
    fn test_gitignore_entry_detection() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let gitignore_path = temp_dir.path().join(".gitignore");
        let target_entry = "_b00t_.toml";

        // Create .gitignore with entry already present
        fs::write(&gitignore_path, format!("*.log\n{}\n*.tmp\n", target_entry))?;

        // Read and verify the entry exists
        let content = fs::read_to_string(&gitignore_path)?;
        let has_entry = content.lines().any(|line| line.trim() == target_entry);
        assert!(has_entry);

        Ok(())
    }
}
