//! RHAI scripting engine for b00t ecosystem
//! 
//! Provides dynamic script execution with access to b00t context variables,
//! system commands, and package management operations. Replaces bash scripting
//! with Rust-embedded RHAI for safer, more maintainable automation.

use crate::context::B00tContext;
use crate::B00tResult;
use anyhow::{Context, Result};
use rhai::{Dynamic, Engine, Scope, AST};
use std::path::{Path, PathBuf};
use std::process::Command;

/// RHAI engine wrapper with b00t-specific functionality
#[derive(Debug)]
pub struct RhaiEngine {
    engine: Engine,
    context: B00tContext,
    scripts_dir: PathBuf,
}

impl RhaiEngine {
    /// Create new RHAI engine with b00t context and functions
    pub fn new(context: B00tContext) -> B00tResult<Self> {
        let mut engine = Engine::new();
        
        // Register b00t-specific functions
        Self::register_b00t_functions(&mut engine)?;
        
        let scripts_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".dotfiles")
            .join("_b00t_")
            .join("scripts");

        Ok(Self {
            engine,
            context,
            scripts_dir,
        })
    }

    /// Register b00t-specific functions in RHAI engine
    fn register_b00t_functions(engine: &mut Engine) -> B00tResult<()> {
        // System command execution
        engine.register_fn("run_cmd", |cmd: &str| -> Result<String, Box<rhai::EvalAltResult>> {
            let output = Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .output()
                .map_err(|e| format!("Command execution failed: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Command failed: {}", stderr).into());
            }

            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        });

        // Conditional command execution (only if not Docker/CI)
        engine.register_fn("run_cmd_if", |cmd: &str, condition: bool| -> Result<String, Box<rhai::EvalAltResult>> {
            if !condition {
                return Ok("skipped".to_string());
            }
            
            let output = Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .output()
                .map_err(|e| format!("Command execution failed: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Command failed: {}", stderr).into());
            }

            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        });

        // Command existence check
        engine.register_fn("command_exists", |cmd: &str| -> bool {
            Command::new("which")
                .arg(cmd)
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        });

        // Package installation with automatic sudo handling
        engine.register_fn("install_package", |package: &str, is_docker: bool| -> Result<String, Box<rhai::EvalAltResult>> {
            let cmd = if is_docker {
                format!("apt update && apt install -y {}", package)
            } else {
                format!("sudo apt update && sudo apt install -y {}", package)
            };
            
            let output = Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .output()
                .map_err(|e| format!("Package installation failed: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("Package installation failed: {}", stderr).into());
            }

            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        });

        // File operations
        engine.register_fn("file_exists", |path: &str| -> bool {
            Path::new(path).exists()
        });

        engine.register_fn("create_dir", |path: &str| -> Result<(), Box<rhai::EvalAltResult>> {
            std::fs::create_dir_all(path)
                .map_err(|e| format!("Failed to create directory {}: {}", path, e).into())
        });

        engine.register_fn("write_file", |path: &str, content: &str| -> Result<(), Box<rhai::EvalAltResult>> {
            std::fs::write(path, content)
                .map_err(|e| format!("Failed to write file {}: {}", path, e).into())
        });

        engine.register_fn("read_file", |path: &str| -> Result<String, Box<rhai::EvalAltResult>> {
            std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read file {}: {}", path, e).into())
        });

        // Environment variables
        engine.register_fn("get_env", |var: &str| -> String {
            std::env::var(var).unwrap_or_default()
        });

        engine.register_fn("set_env", |var: &str, value: &str| {
            unsafe { std::env::set_var(var, value); }
        });

        // Logging
        engine.register_fn("log_info", |msg: &str| {
            println!("ℹ️  {}", msg);
        });

        engine.register_fn("log_warn", |msg: &str| {
            println!("⚠️  {}", msg);
        });

        engine.register_fn("log_error", |msg: &str| {
            eprintln!("❌ {}", msg);
        });

        engine.register_fn("log_success", |msg: &str| {
            println!("✅ {}", msg);
        });

        Ok(())
    }

    /// Create scope with b00t context variables
    pub fn create_scope(&self) -> Scope {
        let mut scope = Scope::new();
        
        // Add b00t context variables
        scope.push("PID", self.context.pid as i64);
        scope.push("TIMESTAMP", self.context.timestamp.clone());
        scope.push("USER", self.context.user.clone());
        scope.push("BRANCH", self.context.branch.clone());
        scope.push("AGENT", self.context.agent.clone());
        scope.push("MODEL_SIZE", self.context.model_size.clone());
        scope.push("PRIVACY", self.context.privacy.clone());
        scope.push("WORKSPACE_ROOT", self.context.workspace_root.clone());
        scope.push("IS_GIT_REPO", self.context.is_git_repo);
        scope.push("HOSTNAME", self.context.hostname.clone());
        
        // Add environment detection
        let is_ci = std::env::var("CI").unwrap_or_default() == "true";
        let is_docker = std::env::var("IS_DOCKER_BUILD").unwrap_or_default() == "true";
        
        scope.push("IS_CI", is_ci);
        scope.push("IS_DOCKER", is_docker);
        
        scope
    }

    /// Execute RHAI script from string
    pub fn execute_script(&self, script: &str) -> B00tResult<Dynamic> {
        let mut scope = self.create_scope();
        
        let result = self.engine
            .eval_with_scope::<Dynamic>(&mut scope, script)
            .map_err(|e| anyhow::anyhow!("Failed to execute RHAI script: {}", e))?;

        Ok(result)
    }

    /// Execute RHAI script from file
    pub fn execute_file<P: AsRef<Path>>(&self, path: P) -> B00tResult<Dynamic> {
        let script_content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read RHAI script: {}", path.as_ref().display()))?;

        self.execute_script(&script_content)
    }

    /// Compile and execute RHAI script (for performance)
    pub fn compile_and_execute(&self, script: &str) -> B00tResult<Dynamic> {
        let ast = self.engine
            .compile(script)
            .map_err(|e| anyhow::anyhow!("Failed to compile RHAI script: {}", e))?;

        self.execute_ast(&ast)
    }

    /// Execute pre-compiled AST
    pub fn execute_ast(&self, ast: &AST) -> B00tResult<Dynamic> {
        let mut scope = self.create_scope();
        
        let result = self.engine
            .eval_ast_with_scope::<Dynamic>(&mut scope, ast)
            .map_err(|e| anyhow::anyhow!("Failed to execute compiled RHAI script: {}", e))?;

        Ok(result)
    }

    /// List available RHAI scripts in scripts directory
    pub fn list_scripts(&self) -> B00tResult<Vec<PathBuf>> {
        if !self.scripts_dir.exists() {
            return Ok(vec![]);
        }

        let mut scripts = Vec::new();
        for entry in std::fs::read_dir(&self.scripts_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("rhai") {
                scripts.push(path);
            }
        }

        scripts.sort();
        Ok(scripts)
    }

    /// Get scripts directory path
    pub fn scripts_dir(&self) -> &Path {
        &self.scripts_dir
    }

    /// Update context for the engine
    pub fn set_context(&mut self, context: B00tContext) {
        self.context = context;
    }

    /// Get current context
    pub fn context(&self) -> &B00tContext {
        &self.context
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_context() -> B00tContext {
        B00tContext {
            pid: 12345,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            user: "testuser".to_string(),
            branch: "main".to_string(),
            agent: "TestAgent".to_string(),
            model_size: "large".to_string(),
            privacy: "standard".to_string(),
            workspace_root: "/tmp/test".to_string(),
            is_git_repo: true,
            hostname: "testhost".to_string(),
        }
    }

    #[test]
    fn test_rhai_engine_creation() {
        let context = create_test_context();
        let engine = RhaiEngine::new(context);
        assert!(engine.is_ok());
    }

    #[test]
    fn test_simple_script_execution() {
        let context = create_test_context();
        let engine = RhaiEngine::new(context).unwrap();
        
        let script = r#"
            let result = PID + 100;
            result
        "#;
        
        let result = engine.execute_script(script).unwrap();
        assert_eq!(result.cast::<i64>(), 12445);
    }

    #[test]
    fn test_context_variables() {
        let context = create_test_context();
        let engine = RhaiEngine::new(context).unwrap();
        
        let script = r#"
            USER + "@" + HOSTNAME
        "#;
        
        let result = engine.execute_script(script).unwrap();
        assert_eq!(result.cast::<String>(), "testuser@testhost");
    }

    #[test]
    fn test_command_exists() {
        let context = create_test_context();
        let engine = RhaiEngine::new(context).unwrap();
        
        let script = r#"
            command_exists("sh")
        "#;
        
        let result = engine.execute_script(script).unwrap();
        assert!(result.cast::<bool>());
    }

    #[test]
    fn test_file_operations() {
        let context = create_test_context();
        let engine = RhaiEngine::new(context).unwrap();
        
        let temp_dir = tempdir().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        
        let script = format!(r#"
            let path = "{}";
            write_file(path, "Hello, RHAI!");
            let content = read_file(path);
            content
        "#, test_file.to_string_lossy());
        
        let result = engine.execute_script(&script).unwrap();
        assert_eq!(result.cast::<String>(), "Hello, RHAI!");
    }

    #[test]
    fn test_logging_functions() {
        let context = create_test_context();
        let engine = RhaiEngine::new(context).unwrap();
        
        let script = r#"
            log_info("Test info message");
            log_warn("Test warning message");
            log_success("Test success message");
            true
        "#;
        
        let result = engine.execute_script(script).unwrap();
        assert!(result.cast::<bool>());
    }
}