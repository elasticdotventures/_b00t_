use crate::load_datum_providers;
use crate::session_memory::SessionMemory;
use crate::traits::*;
use crate::whoami;
use anyhow::Result;
use clap::Parser;
use duct::cmd;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Parser)]
pub enum InitCommands {
    #[clap(
        about = "Initialize command aliases",
        long_about = "Initialize command aliases for CLI tools.\n\nExamples:\n  b00t-cli init aliases"
    )]
    Aliases,
    #[clap(
        about = "👋 Initialize hello world protocol - wake up all systems",
        long_about = "Execute the b00t hello_world protocol:\n  • Verify and start Redis server\n  • Load MCP server configurations\n  • Run system diagnostics\n  • Configure agent preferences\n  • Display interactive system tour\n\nExamples:\n  b00t-cli init hello-world\n  b00t-cli init hello-world --skip-redis"
    )]
    HelloWorld {
        #[clap(long, help = "Skip Redis server startup")]
        skip_redis: bool,
        #[clap(long, help = "Skip system diagnostics")]
        skip_diagnostics: bool,
        #[clap(long, help = "Skip interactive tour")]
        skip_tour: bool,
    },
}

/// Execute the comprehensive b00t hello_world protocol
fn execute_hello_world_protocol(
    path: &str,
    skip_redis: bool,
    _skip_diagnostics: bool,
    skip_tour: bool,
) -> Result<()> {
    println!("👋 b00t hello_world protocol - enlightening agent capabilities...\n");

    // Initialize session memory and agent identity
    let mut memory = SessionMemory::load()?;

    // Phase 1: Agent Identity & Role Detection
    println!("🤖 Phase 1: Agent Identity Detection");
    detect_agent_role(&mut memory)?;

    // Phase 2: Project Context Detection
    println!("\n📂 Phase 2: Project Context Analysis");
    detect_project_context(&mut memory)?;

    // Phase 3: Datum-based Tool Discovery
    println!("\n🔧 Phase 3: Tool & Service Discovery");
    discover_available_tools(path, &mut memory)?;

    // Phase 4: Redis & Infrastructure (if needed)
    if !skip_redis {
        println!("\n💾 Phase 4: Infrastructure Setup");
        setup_infrastructure(&mut memory)?;
    }

    // Phase 5: Agent Enlightenment - What can you do?
    if !skip_tour {
        println!("\n🧠 Phase 5: Agent Capability Mapping");
        enlighten_agent_capabilities(path, &mut memory)?;
    }

    // Final status report
    memory.incr("hello_world_completions")?;
    println!("\n✅ Agent enlightenment complete!");
    println!("📊 {}", memory.get_summary());

    Ok(())
}

/// Verify Redis server is running and start if needed
fn verify_and_start_redis(memory: &mut SessionMemory) -> Result<()> {
    println!("  🔍 Checking Redis server status...");

    // Check if Redis is already running
    let redis_running = Command::new("redis-cli")
        .args(&["ping"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);

    if redis_running {
        println!("  ✅ Redis server is already running");
        memory.set_flag("redis_running", true)?;
        return Ok(());
    }

    println!("  🚀 Starting Redis server...");

    // Try to start Redis using different methods
    let redis_started = try_start_redis_server()?;

    if redis_started {
        // Wait a moment for Redis to fully start
        std::thread::sleep(std::time::Duration::from_millis(1000));

        // Verify it's running
        let ping_result = Command::new("redis-cli").args(&["ping"]).output();

        match ping_result {
            Ok(output) if output.status.success() => {
                let response = String::from_utf8_lossy(&output.stdout);
                if response.trim() == "PONG" {
                    println!("  ✅ Redis server started and verified (PONG received)");
                    memory.set_flag("redis_running", true)?;
                    memory.incr("redis_start_count")?;
                } else {
                    println!(
                        "  ⚠️  Redis server started but ping response unexpected: {}",
                        response.trim()
                    );
                    memory.set_flag("redis_running", false)?;
                }
            }
            _ => {
                println!("  ❌ Redis server may have started but ping verification failed");
                memory.set_flag("redis_running", false)?;
            }
        }
    } else {
        println!("  ❌ Failed to start Redis server");
        memory.set_flag("redis_running", false)?;
        memory.incr("redis_start_failures")?;
    }

    Ok(())
}

/// Try different methods to start Redis server
fn try_start_redis_server() -> Result<bool> {
    // Method 1: systemctl (systemd)
    if let Ok(status) = Command::new("systemctl")
        .args(&["start", "redis-server"])
        .status()
    {
        if status.success() {
            println!("  📦 Started Redis via systemctl");
            return Ok(true);
        }
    }

    // Method 2: service command
    if let Ok(status) = Command::new("service")
        .args(&["redis-server", "start"])
        .status()
    {
        if status.success() {
            println!("  📦 Started Redis via service command");
            return Ok(true);
        }
    }

    // Method 3: direct redis-server command
    if let Ok(_child) = Command::new("redis-server")
        .args(&["--daemonize", "yes"])
        .spawn()
    {
        println!("  📦 Started Redis server directly");
        return Ok(true);
    }

    // Method 4: Docker fallback
    if let Ok(status) = Command::new("docker")
        .args(&[
            "run",
            "-d",
            "--name",
            "b00t-redis",
            "-p",
            "6379:6379",
            "redis:alpine",
        ])
        .status()
    {
        if status.success() {
            println!("  🐳 Started Redis via Docker");
            return Ok(true);
        }
    }

    println!("  ⚠️  All Redis startup methods failed");
    Ok(false)
}

/// Detect agent role and capabilities
fn detect_agent_role(memory: &mut SessionMemory) -> Result<()> {
    let agent = whoami::detect_agent(false);

    if !agent.is_empty() {
        memory.set("detected_agent", &agent)?;
        println!("  🎯 Agent: {}", agent);

        // Map agent to role/capabilities
        let role = match agent.as_str() {
            "claude" => "AI Assistant (Development, Analysis, Documentation)",
            _ => "Generic Agent (Multi-purpose)",
        };
        memory.set("agent_role", role)?;
        println!("  🏷️  Role: {}", role);
    } else {
        println!("  🤖 Generic agent mode");
        memory.set("agent_role", "Generic Agent")?;
    }

    Ok(())
}

/// Detect project context by analyzing current directory
fn detect_project_context(memory: &mut SessionMemory) -> Result<()> {
    let mut project_types = Vec::new();
    let mut primary_stack = "unknown";

    // Check for common project indicators
    if Path::new("Cargo.toml").exists() {
        project_types.push("rust");
        primary_stack = "rust";
        println!("  🦀 Rust project detected (Cargo.toml)");
    }

    if Path::new("package.json").exists() {
        project_types.push("nodejs");
        if primary_stack == "unknown" {
            primary_stack = "nodejs";
        }
        println!("  🦄 Node.js project detected (package.json)");
    }

    if Path::new("pyproject.toml").exists() || Path::new("requirements.txt").exists() {
        project_types.push("python");
        if primary_stack == "unknown" {
            primary_stack = "python";
        }
        println!("  🐍 Python project detected");
    }

    if Path::new("Dockerfile").exists() || Path::new("docker-compose.yml").exists() {
        project_types.push("docker");
        println!("  🐳 Docker configuration detected");
    }

    if Path::new(".git").exists() {
        project_types.push("git");
        println!("  📂 Git repository detected");
    }

    // Store project context
    memory.set("primary_stack", primary_stack)?;
    memory.set("project_types", &project_types.join(","))?;

    if project_types.is_empty() {
        println!("  ❓ No specific project type detected");
    }

    Ok(())
}

/// Discover available tools using datum system
fn discover_available_tools(path: &str, memory: &mut SessionMemory) -> Result<()> {
    // Load all datums from different subsystems
    let cli_tools =
        load_datum_providers::<crate::datum_cli::CliDatum>(path, ".cli.toml").unwrap_or_default();
    let mcp_servers =
        load_datum_providers::<crate::datum_mcp::McpDatum>(path, ".mcp.toml").unwrap_or_default();
    let docker_containers =
        load_datum_providers::<crate::datum_docker::DockerDatum>(path, ".docker.toml")
            .unwrap_or_default();

    // Count available vs installed
    let mut available_count = 0;
    let mut installed_count = 0;
    let mut missing_important = Vec::new();

    // Check CLI tools
    for tool in &cli_tools {
        available_count += 1;
        if DatumChecker::is_installed(tool.as_ref()) {
            installed_count += 1;
        } else {
            // Check if this is an important tool for the current project
            let tool_name = StatusProvider::name(tool.as_ref());
            let unknown = "unknown".to_string();
            let project_stack = memory.get("primary_stack").unwrap_or(&unknown);

            if is_tool_important_for_stack(tool_name, project_stack) {
                missing_important.push(tool_name.to_string());
            }
        }
    }

    println!(
        "  🔧 {} CLI tools available, {} installed",
        available_count, installed_count
    );
    println!("  🔌 {} MCP servers configured", mcp_servers.len());
    println!(
        "  🐳 {} Docker containers available",
        docker_containers.len()
    );

    // Report missing important tools
    if !missing_important.is_empty() {
        println!(
            "  ⚠️  Missing important tools for {} stack: {}",
            memory
                .get("primary_stack")
                .unwrap_or(&"project".to_string()),
            missing_important.join(", ")
        );
        memory.set("missing_important_tools", &missing_important.join(","))?;
    }

    memory.set_num("tools_available", available_count as i64)?;
    memory.set_num("tools_installed", installed_count as i64)?;
    memory.set_num("mcp_servers_available", mcp_servers.len() as i64)?;

    Ok(())
}

/// Check if a tool is important for a given technology stack
fn is_tool_important_for_stack(tool_name: &str, stack: &str) -> bool {
    match stack {
        "rust" => matches!(tool_name, "rustc" | "cargo" | "git"),
        "nodejs" => matches!(tool_name, "node" | "npm" | "git"),
        "python" => matches!(tool_name, "python3" | "pip" | "git"),
        _ => matches!(tool_name, "git"),
    }
}

/// Setup infrastructure (renamed from configure_system_preferences)
fn setup_infrastructure(memory: &mut SessionMemory) -> Result<()> {
    verify_and_start_redis(memory)?;

    // Detect container runtime
    let container_runtime = if cmd!("podman", "--version").read().is_ok() {
        "podman"
    } else if cmd!("docker", "--version").read().is_ok() {
        "docker"
    } else {
        "none"
    };
    memory.set("preferred_container_runtime", container_runtime)?;

    memory.set("last_hello_world", &chrono::Utc::now().to_rfc3339())?;
    Ok(())
}

/// Agent enlightenment (renamed from interactive_documentation_tour)
fn enlighten_agent_capabilities(_path: &str, memory: &mut SessionMemory) -> Result<()> {
    let unknown = "unknown".to_string();
    let empty = "".to_string();
    let project_stack = memory.get("primary_stack").unwrap_or(&unknown);
    let missing_tools = memory.get("missing_important_tools").unwrap_or(&empty);

    if !missing_tools.is_empty() {
        println!(
            "  💡 Install for {}: b00t cli install {}",
            project_stack,
            missing_tools.replace(",", " ")
        );
    }

    println!("  🎓 Key commands: b00t status, b00t mcp list, b00t cli up");
    memory.set_flag("enlightenment_completed", true)?;
    Ok(())
}

/// Run session-aware system diagnostics with agent context
pub fn run_system_diagnostics(memory: &mut SessionMemory) -> Result<()> {
    let context = memory.get_agent_context();

    println!("  🩺 Agent Context Diagnostics");
    println!("  🤖 Agent: {}", context.agent_name);
    println!(
        "  📅 Session: {}s ({})",
        context.session_duration,
        format_duration(context.session_duration)
    );
    println!(
        "  🌿 Branch: {} ({}🔨 builds)",
        context.current_branch, context.build_count
    );
    println!(
        "  📊 Activity: {}🐚 shells, {}⚙️ compiles, {}🧪 tests",
        context.shell_count, context.compile_count, context.test_count
    );

    let mut diagnostic_results = Vec::new();

    // Check Git status with session context
    if let Ok(output) = cmd!("git", "--version").read() {
        println!("  ✅ Git: {}", output.trim());
        diagnostic_results.push(("git", true));

        // Add git context
        if let Ok(branch) = cmd!("git", "branch", "--show-current").read() {
            let branch = branch.trim();
            if branch != context.current_branch {
                println!(
                    "  ⚠️  Branch changed: {} → {}",
                    context.current_branch, branch
                );
            }
        }

        if let Ok(status) = cmd!("git", "status", "--porcelain").read() {
            if !status.trim().is_empty() {
                let file_count = status.lines().count();
                println!("  📝 Git: {} modified files", file_count);
            }
        }
    } else {
        println!("  ❌ Git: Not available");
        diagnostic_results.push(("git", false));
    }

    // Check Rust/Cargo with build context
    if let Ok(output) = cmd!("cargo", "--version").read() {
        println!("  ✅ Cargo: {}", output.trim());
        diagnostic_results.push(("cargo", true));

        // Check for Cargo.toml and recent build artifacts
        if std::path::Path::new("Cargo.toml").exists() {
            if let Ok(metadata) = std::fs::metadata("target") {
                let target_age = chrono::Utc::now().timestamp()
                    - metadata
                        .modified()
                        .unwrap_or(std::time::UNIX_EPOCH)
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64;
                if target_age < 300 {
                    // 5 minutes
                    println!("  🔥 Cargo: Recent build ({}s ago)", target_age);
                }
            }
        }
    } else {
        println!("  ❌ Cargo: Not available");
        diagnostic_results.push(("cargo", false));
    }

    // Check Node.js with package context
    if let Ok(output) = cmd!("node", "--version").read() {
        println!("  ✅ Node.js: {}", output.trim());
        diagnostic_results.push(("node", true));

        if std::path::Path::new("package.json").exists() {
            if let Ok(metadata) = std::fs::metadata("node_modules") {
                let modules_age = chrono::Utc::now().timestamp()
                    - metadata
                        .modified()
                        .unwrap_or(std::time::UNIX_EPOCH)
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64;
                if modules_age < 1800 {
                    // 30 minutes
                    println!("  📦 Node: Recent install ({}s ago)", modules_age);
                }
            }
        }
    } else {
        println!("  ❌ Node.js: Not available");
        diagnostic_results.push(("node", false));
    }

    // Check Docker with container context
    if let Ok(output) = cmd!("docker", "--version").read() {
        println!(
            "  ✅ Docker: {}",
            output.lines().next().unwrap_or("").trim()
        );
        diagnostic_results.push(("docker", true));

        // Check for running containers
        if let Ok(containers) = cmd!("docker", "ps", "-q").read() {
            let container_count = containers.lines().count();
            if container_count > 0 {
                println!("  🐳 Docker: {} active containers", container_count);
            }
        }
    } else {
        println!("  ❌ Docker: Not available");
        diagnostic_results.push(("docker", false));
    }

    // Check available package managers
    let package_managers = vec!["npm", "pnpm", "yarn", "bun"];
    let mut available_pms = Vec::new();
    for pm in package_managers {
        if cmd!(pm, "--version").read().is_ok() {
            available_pms.push(pm);
            diagnostic_results.push((pm, true));
        }
    }

    if !available_pms.is_empty() {
        println!("  ✅ Package Managers: {}", available_pms.join(", "));
    }

    // Store enhanced diagnostic results in session memory
    let passing_count = diagnostic_results
        .iter()
        .filter(|(_, passed)| *passed)
        .count();
    memory.set_num("diagnostic_passing", passing_count as i64)?;
    memory.set_num("diagnostic_total", diagnostic_results.len() as i64)?;

    // OODA Loop Decision Support
    println!("  🧠 OODA Analysis:");
    if context.diagnostic_total > 0 {
        let health_ratio = context.diagnostic_passing as f64 / context.diagnostic_total as f64;
        if health_ratio >= 0.8 {
            println!(
                "  ✅ System Health: Excellent ({:.0}%)",
                health_ratio * 100.0
            );
        } else if health_ratio >= 0.6 {
            println!(
                "  ⚠️  System Health: Adequate ({:.0}%)",
                health_ratio * 100.0
            );
        } else {
            println!("  ❌ System Health: Poor ({:.0}%)", health_ratio * 100.0);
        }
    }

    // Session productivity insights
    if context.session_duration > 300 {
        // 5+ minutes
        let builds_per_hour =
            (context.build_count as f64 / context.session_duration as f64) * 3600.0;
        if builds_per_hour > 10.0 {
            println!(
                "  🚀 High build velocity: {:.1} builds/hour",
                builds_per_hour
            );
        } else if builds_per_hour > 2.0 {
            println!(
                "  ⚙️  Moderate build pace: {:.1} builds/hour",
                builds_per_hour
            );
        }
    }

    // Next action recommendations
    if context.compile_count == 0 && std::path::Path::new("Cargo.toml").exists() {
        println!("  💡 Suggestion: Run `cargo build` to verify compilation");
    }
    if context.test_count == 0 && std::path::Path::new("tests").exists() {
        println!("  💡 Suggestion: Run tests to ensure quality");
    }

    println!(
        "  📊 Final: {}/{} systems operational",
        passing_count,
        diagnostic_results.len()
    );

    Ok(())
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

/// Configure system preferences in session memory
fn configure_system_preferences(memory: &mut SessionMemory) -> Result<()> {
    println!("  ⚙️  Configuring system preferences...");

    // Detect preferred container runtime
    let container_runtime = if cmd!("podman", "--version").read().is_ok() {
        "podman"
    } else if cmd!("docker", "--version").read().is_ok() {
        "docker"
    } else {
        "none"
    };

    memory.set("preferred_container_runtime", container_runtime)?;
    println!("  🐳 Container runtime: {}", container_runtime);

    // Detect Kubernetes environment
    let k8s_env = if cmd!("minikube", "status").read().is_ok() {
        "minikube"
    } else if cmd!("kubectl", "cluster-info").read().is_ok() {
        "kubectl"
    } else {
        "none"
    };

    memory.set("k8s_environment", k8s_env)?;
    println!("  ☸️  Kubernetes: {}", k8s_env);

    // Store initialization timestamp
    memory.set("last_hello_world", &chrono::Utc::now().to_rfc3339())?;

    println!("  ✅ System preferences configured");

    Ok(())
}

/// Interactive documentation tour
fn interactive_documentation_tour(_path: &str, memory: &mut SessionMemory) -> Result<()> {
    println!("  📖 Starting interactive documentation tour...");

    // Check for README.md
    let git_root = crate::utils::get_workspace_root();
    let readme_path = std::path::PathBuf::from(&git_root).join("README.md");

    if readme_path.exists() {
        if !memory.is_readme_read() {
            println!("  📋 README.md found - marking as available for reading");
            println!("  💡 Remember to run 'b00t-cli session mark-readme-read' after reading");
        } else {
            println!("  ✅ README.md already marked as read this session");
        }
    } else {
        println!("  ℹ️  No README.md found in git root");
    }

    // Show sample b00t commands for different scenarios
    println!("  🎓 b00t Command Reference:");
    println!("    # Session management");
    println!("    b00t-cli session get <key>           # Get session value");
    println!("    b00t-cli session incr <key>          # Increment counter");
    println!("    b00t-cli session keys                # List all keys");
    println!("");
    println!("    # MCP server management");
    println!("    b00t-cli mcp list                    # List MCP servers");
    println!("    b00t-cli app claude-code mcp install <server>  # Install to Claude Code");
    println!("");
    println!("    # System status");
    println!("    b00t-cli status                      # Show all tools status");
    println!("    b00t-cli checkpoint                  # Create git checkpoint");

    memory.set_flag("tour_completed", true)?;
    println!("  ✅ Interactive tour completed");

    Ok(())
}

impl InitCommands {
    pub fn execute(&self, path: &str) -> Result<()> {
        match self {
            InitCommands::Aliases => {
                println!("🔗 Init aliases functionality coming soon...");
                Ok(())
            }
            InitCommands::HelloWorld {
                skip_redis,
                skip_diagnostics,
                skip_tour,
            } => execute_hello_world_protocol(path, *skip_redis, *skip_diagnostics, *skip_tour),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_commands_exist() {
        let aliases_cmd = InitCommands::Aliases;
        assert!(aliases_cmd.execute("test").is_ok());
    }
}
