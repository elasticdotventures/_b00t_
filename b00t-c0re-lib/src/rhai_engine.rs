//! RHAI scripting engine for b00t ecosystem
//!
//! Provides dynamic script execution with access to b00t context variables,
//! system commands, and package management operations. Replaces bash scripting
//! with Rust-embedded RHAI for safer, more maintainable automation.

use crate::B00tResult;
use crate::context::B00tContext;
use anyhow::{Context, Result};
use rhai::{AST, Dynamic, Engine, Scope};
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
        engine.register_fn(
            "run_cmd",
            |cmd: &str| -> Result<String, Box<rhai::EvalAltResult>> {
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
            },
        );

        // Conditional command execution (only if not Docker/CI)
        engine.register_fn(
            "run_cmd_if",
            |cmd: &str, condition: bool| -> Result<String, Box<rhai::EvalAltResult>> {
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
            },
        );

        // Command existence check
        engine.register_fn("command_exists", |cmd: &str| -> bool {
            Command::new("which")
                .arg(cmd)
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        });

        // Package installation with automatic sudo handling
        engine.register_fn(
            "install_package",
            |package: &str, is_docker: bool| -> Result<String, Box<rhai::EvalAltResult>> {
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
            },
        );

        // File operations
        engine.register_fn("file_exists", |path: &str| -> bool {
            Path::new(path).exists()
        });

        engine.register_fn(
            "create_dir",
            |path: &str| -> Result<(), Box<rhai::EvalAltResult>> {
                std::fs::create_dir_all(path)
                    .map_err(|e| format!("Failed to create directory {}: {}", path, e).into())
            },
        );

        engine.register_fn(
            "write_file",
            |path: &str, content: &str| -> Result<(), Box<rhai::EvalAltResult>> {
                std::fs::write(path, content)
                    .map_err(|e| format!("Failed to write file {}: {}", path, e).into())
            },
        );

        engine.register_fn(
            "read_file",
            |path: &str| -> Result<String, Box<rhai::EvalAltResult>> {
                std::fs::read_to_string(path)
                    .map_err(|e| format!("Failed to read file {}: {}", path, e).into())
            },
        );

        // Environment variables - return String with empty default (RHAI-friendly)
        engine.register_fn("get_env", |var: &str| -> String {
            std::env::var(var).unwrap_or_default()
        });

        engine.register_fn("compiled_knowledge_backend", || -> String {
            crate::compiled_knowledge_backend().to_string()
        });

        engine.register_fn("knowledge_backend_matches", |expected: &str| -> bool {
            crate::compiled_knowledge_backend() == expected
        });

        engine.register_fn("set_env", |var: &str, value: &str| unsafe {
            std::env::set_var(var, value);
        });

        // String utilities
        engine.register_fn("str_trim", |s: &str| -> String {
            s.trim().to_string()
        });

        engine.register_fn("str_downcase", |s: &str| -> String {
            s.to_lowercase()
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

        // Gate precondition evaluation
        // gate_check("command", "docker") → true if docker on PATH
        // gate_check("file", "~/.aws/credentials") → true if file exists (expands ~)
        // gate_check("env", "AWS_REGION") → true if env var or .env entry is set
        // gate_check("rhai", "command_exists(\"docker\")") → evaluates arbitrary rhai
        engine.register_fn(
            "gate_check",
            |kind: &str, spec: &str| -> Result<bool, Box<rhai::EvalAltResult>> {
                let result = match kind {
                    "command" => {
                        Command::new("which")
                            .arg(spec)
                            .output()
                            .map(|o| o.status.success())
                            .unwrap_or(false)
                    }
                    "knowledge_backend" => crate::compiled_knowledge_backend() == spec,
                    "file" => {
                        let expanded = if spec.starts_with('~') {
                            let home = std::env::var("HOME").unwrap_or_default();
                            Path::new(&home).join(spec.strip_prefix("~/").unwrap_or(spec))
                        } else {
                            Path::new(spec).to_path_buf()
                        };
                        expanded.exists()
                    }
                    "env" => {
                        let direct = std::env::var(spec);
                        if direct.is_ok() && !direct.unwrap_or_default().is_empty() {
                            true
                        } else {
                            // check .env at WORKSPACE_ROOT or HOME
                            let ws = std::env::var("WORKSPACE_ROOT")
                                .or_else(|_| std::env::var("HOME"))
                                .unwrap_or_default();
                            let env_path = Path::new(&ws).join(".env");
                            if env_path.exists() {
                                if let Ok(content) = std::fs::read_to_string(&env_path) {
                                    let prefix = format!("{}=", spec);
                                    for line in content.lines() {
                                        if line.trim().starts_with(&prefix) {
                                            let val = line.trim()[prefix.len()..].trim();
                                            return Ok(!val.is_empty()
                                                && !val.starts_with('#'));
                                        }
                                    }
                                }
                            }
                            false
                        }
                    }
                    "rhai" => {
                        // Evaluate arbitrary rhai expression in a fresh sub-engine.
                        // ⚠️  Custom b00t fns (gate_check, command_exists, etc.) are NOT available
                        // in the sub-engine — only built-in rhai primitives.
                        // Use gate_check("command", ...) / gate_check("file", ...) / gate_check("env", ...)
                        // for those checks; reserve "rhai" for pure-logic expressions only.
                        let sub_engine = Engine::new();
                        let ast = sub_engine.compile(spec)
                            .map_err(|e| format!("gate rhai compile error: {}", e))?;
                        let result = sub_engine.eval_ast::<bool>(&ast)
                            .map_err(|e| format!("gate rhai eval error: {}", e))?;
                        result
                    }
                    _ => return Err(format!("unknown gate kind: {}", kind).into()),
                };
                Ok(result)
            },
        );

        // Knowledge graph query (stub) — probes irontology-mcp via raw stdio pipe.
        // ⚠️  This does NOT form a JSON-RPC request; query_val is piped as raw text.
        // Returns {} silently on any error. Intended as placeholder until proper
        // JSON-RPC framing is added to the irontology bridge.
        engine.register_fn(
            "kg_query",
            |query_kind: &str, query_val: &str| -> Result<String, Box<rhai::EvalAltResult>> {
                match query_kind {
                    "subject" | "facts" => {
                        // Pipe query_val to b00t-mcp --stdio via stdin.
                        // ⚠️  NEVER use sh -c with string interpolation here — query_val
                        // comes from a rhai script and could be attacker-controlled.
                        use std::io::Write;
                        let mut child = match Command::new("b00t-mcp")
                            .arg("--stdio")
                            .stdin(std::process::Stdio::piped())
                            .stdout(std::process::Stdio::piped())
                            .stderr(std::process::Stdio::null())
                            .spawn()
                        {
                            Ok(c) => c,
                            Err(_) => return Ok("{}".to_string()),
                        };
                        let _ = child.stdin.take()
                            .and_then(|mut stdin| stdin.write_all(query_val.as_bytes()).ok());
                        match child.wait_with_output() {
                            Ok(output) => match String::from_utf8(output.stdout) {
                                Ok(s) => Ok(s),
                                Err(_) => Ok("{}".to_string()),
                            },
                            Err(_) => Ok("{}".to_string()),
                        }
                    }
                    _ => Err(format!("unknown kg_query kind: {}", query_kind).into()),
                }
            },
        );

        // Telemetry: track events to ~/.b00t/events.jsonl
        // session_track("mcp_install", "github:installed") appends a JSON line
        // 🤓 legacy telemetry.jsonl write removed — unified to events.jsonl
        engine.register_fn(
            "session_track",
            |event: &str, detail: &str| -> Result<(), Box<rhai::EvalAltResult>> {
                crate::events::write_event(event, detail);
                Ok(())
            },
        );

        Ok(())
    }

    /// Create scope with b00t context variables
    pub fn create_scope(&self) -> Scope<'_> {
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

        let result = self
            .engine
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
        let ast = self
            .engine
            .compile(script)
            .map_err(|e| anyhow::anyhow!("Failed to compile RHAI script: {}", e))?;

        self.execute_ast(&ast)
    }

    /// Execute pre-compiled AST
    pub fn execute_ast(&self, ast: &AST) -> B00tResult<Dynamic> {
        let mut scope = self.create_scope();

        let result = self
            .engine
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
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};
    use tempfile::tempdir;

    static HOME_LOCK: Mutex<()> = Mutex::new(());

    struct TempHome {
        _guard: MutexGuard<'static, ()>,
        old_home: Option<String>,
        _temp_dir: tempfile::TempDir,
        b00t_dir: PathBuf,
    }

    impl TempHome {
        fn new() -> Self {
            let guard = HOME_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let temp_dir = tempdir().unwrap();
            let b00t_dir = temp_dir.path().join(".b00t");
            std::fs::create_dir_all(&b00t_dir).unwrap();
            let old_home = std::env::var("HOME").ok();
            unsafe {
                std::env::set_var("HOME", temp_dir.path().to_str().unwrap());
            }

            Self {
                _guard: guard,
                old_home,
                _temp_dir: temp_dir,
                b00t_dir,
            }
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            if let Some(old) = &self.old_home {
                unsafe { std::env::set_var("HOME", old); }
            } else {
                unsafe { std::env::remove_var("HOME"); }
            }
        }
    }

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

        let script = format!(
            r#"
            let path = "{}";
            write_file(path, "Hello, RHAI!");
            let content = read_file(path);
            content
        "#,
            test_file.to_string_lossy()
        );

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

    #[test]
    fn test_knowledge_backend_functions() {
        let context = create_test_context();
        let engine = RhaiEngine::new(context).unwrap();

        let backend = engine
            .execute_script("compiled_knowledge_backend()")
            .unwrap()
            .cast::<String>();
        assert!(["neumann", "oxigraph"].contains(&backend.as_str()));

        let result = engine
            .execute_script("knowledge_backend_matches(compiled_knowledge_backend())")
            .unwrap();
        assert!(result.cast::<bool>());
    }

    #[test]
    fn test_session_track_writes_events() {
        use std::fs;

        let temp_home = TempHome::new();

        let context = create_test_context();
        let engine = RhaiEngine::new(context).unwrap();

        let script = r#"
            session_track("mcp_install", "github:installed")
        "#;

        let result = engine.execute_script(script);
        assert!(result.is_ok(), "session_track should succeed");

        // Verify events.jsonl was written
        let events_path = temp_home.b00t_dir.join("events.jsonl");
        assert!(events_path.exists(), "events.jsonl should exist");
        let content = fs::read_to_string(&events_path).unwrap();
        let line = content.lines().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(parsed["event"], "mcp_install");
        assert_eq!(parsed["detail"], "github:installed");
        assert!(parsed["ts"].as_str().is_some());
        assert!(parsed["pid"].as_u64().is_some());
    }

    #[test]
    fn test_session_track_no_legacy_telemetry() {
        let temp_home = TempHome::new();

        let context = create_test_context();
        let engine = RhaiEngine::new(context).unwrap();

        let script = r#"
            session_track("test_event", "test_detail")
        "#;

        let result = engine.execute_script(script);
        assert!(result.is_ok());

        // Verify legacy telemetry.jsonl is NOT created
        let legacy_path = temp_home.b00t_dir.join("telemetry.jsonl");
        assert!(
            !legacy_path.exists(),
            "legacy telemetry.jsonl should NOT be created"
        );
    }
}
