use crate::load_datum_providers;
use crate::session_memory::SessionMemory;
use crate::traits::*;
use crate::whoami;
use anyhow::{Context, Result};
use clap::Parser;
use duct::cmd;
use std::path::Path;

#[derive(Parser)]
pub enum InitCommands {
    #[clap(
        about = "Initialize a b00t project in the current directory",
        long_about = "Create _b00t_/ with project.toml + overrides.toml.\nRuns agent onboarding automatically unless _b00t_/ already exists.\n\nExamples:\n  b00t init project\n  b00t init project --name my-project --stack rust\n  b00t init project --setup   (force onboarding even if _b00t_/ exists)\n  b00t init project --no-setup (skip onboarding)"
    )]
    Project {
        #[clap(long, help = "Project name (default: directory name)")]
        name: Option<String>,
        #[clap(long, help = "Primary technology stack (rust|python|nodejs|auto)")]
        stack: Option<String>,
        #[clap(long, help = "Force agent onboarding (detect tools, start Redis, show capabilities)")]
        setup: bool,
        #[clap(long, help = "Skip agent onboarding")]
        no_setup: bool,
    },
}

// ── Project initialization ────────────────────────────────────────────────

fn handle_project_init(
    name: Option<String>,
    stack: Option<String>,
    force_setup: bool,
    no_setup: bool,
    path: &str,
) -> Result<()> {
    let cwd = std::env::current_dir().context("current dir")?;
    let project_name = name.unwrap_or_else(|| {
        cwd.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("b00t-project")
            .to_string()
    });

    let b00t_dir = cwd.join("_b00t_");
    let existed = b00t_dir.exists();

    if existed {
        println!("  ⏭️  _b00t_/ already exists");
    } else {
        std::fs::create_dir(&b00t_dir).context("create _b00t_/")?;
        println!("  📁 created {}", b00t_dir.display());
    }

    let primary_stack = stack.unwrap_or_else(|| detect_project_stack(&cwd));

    let project_toml = b00t_dir.join("project.toml");
    if !project_toml.exists() {
        let content = format!(
            r#"# b00t project config — {project_name}

[project]
name = "{project_name}"
primary_stack = "{primary_stack}"
b00t_version = "{b00t_version}"

# b00t:map v1
# summary: {project_name} b00t project configuration
# tags: project, {project_name}, {primary_stack}
# tier: sm0l
# cmds: b00t init project
# complexity: 1
"#,
            b00t_version = env!("CARGO_PKG_VERSION"),
        );
        std::fs::write(&project_toml, content)?;
        println!("  📝 wrote {}", project_toml.display());
    }

    let overrides_toml = b00t_dir.join("overrides.toml");
    if !overrides_toml.exists() {
        let content = format!(
            r#"# b00t project overrides — per-project datum version pins

# Override a datum's desired version:
# [overrides]
# rustc = "1.85.0"
# node = "22.0.0"

# b00t:map v1
# summary: {project_name} b00t version overrides
# tags: project, {project_name}, overrides
# tier: sm0l
# cmds: b00t init project, b00t cli up
# complexity: 1
"#,
        );
        std::fs::write(&overrides_toml, content)?;
        println!("  📝 wrote {}", overrides_toml.display());
    }

    println!("\n✅ Project '{}' initialized ({})", project_name, primary_stack);

    // Auto-run onboarding unless explicitly skipped or _b00t_/ already existed (and no --setup)
    if !no_setup && (!existed || force_setup) {
        if existed && force_setup {
            println!("\n🔧 Forcing agent setup...");
        } else {
            println!("\n🔧 First-time agent setup...");
        }
        run_setup(path)?;
    } else if existed {
        println!("   Run 'b00t init project --setup' to re-run agent setup");
    } else {
        println!("   Run 'b00t init project' for agent setup");
    }

    println!("   Run 'b00t cli up' to check tool versions");
    Ok(())
}

fn detect_project_stack(cwd: &Path) -> String {
    if cwd.join("Cargo.toml").exists() { return "rust".into(); }
    if cwd.join("package.json").exists() { return "nodejs".into(); }
    if cwd.join("pyproject.toml").exists() || cwd.join("requirements.txt").exists() {
        return "python".into();
    }
    if cwd.join("go.mod").exists() { return "go".into(); }
    "unknown".into()
}

// ── Agent setup (onboarding) ───────────────────────────────────────────────

fn run_setup(path: &str) -> Result<()> {
    let mut memory = SessionMemory::load()?;

    println!("🤖 Agent Identity");
    detect_agent_role(&mut memory)?;

    println!("\n📂 Project Context");
    detect_project_context(&mut memory)?;

    println!("\n🔧 Tool & Service Discovery");
    discover_available_tools(path, &mut memory)?;

    println!("\n💾 Infrastructure Setup");
    setup_infrastructure(&mut memory)?;

    println!("\n🧠 Capability Reference");
    show_capabilities(&mut memory)?;

    memory.incr("hello_world_completions")?;
    println!("\n📊 {}", memory.get_summary());
    Ok(())
}

fn detect_agent_role(memory: &mut SessionMemory) -> Result<()> {
    let agent = whoami::detect_agent(false);
    if !agent.is_empty() {
        memory.set("detected_agent", &agent)?;
        println!("  🎯 Agent: {}", agent);
    } else {
        println!("  🤖 Generic agent mode");
        memory.set("agent_role", "Generic Agent")?;
    }
    Ok(())
}

fn detect_project_context(memory: &mut SessionMemory) -> Result<()> {
    let mut project_types = Vec::new();
    let mut primary_stack = "unknown";

    let checks: &[(&str, &str, &str)] = &[
        ("Cargo.toml", "rust", "🦀 Rust"),
        ("package.json", "nodejs", "🦄 Node.js"),
        ("pyproject.toml", "python", "🐍 Python"),
        ("requirements.txt", "python", "🐍 Python"),
        ("Dockerfile", "docker", "🐳 Docker"),
        (".git", "git", "📂 Git"),
    ];

    for (file, stack, emoji) in checks {
        if Path::new(file).exists() {
            project_types.push(*stack);
            if primary_stack == "unknown" { primary_stack = stack; }
            println!("  {} {} ({})", emoji, stack, file);
        }
    }

    memory.set("primary_stack", primary_stack)?;
    memory.set("project_types", &project_types.join(","))?;

    if project_types.is_empty() {
        println!("  ❓ No specific project type detected");
    }
    Ok(())
}

fn discover_available_tools(path: &str, memory: &mut SessionMemory) -> Result<()> {
    let cli_tools =
        load_datum_providers::<crate::datum_cli::CliDatum>(path, ".cli.toml").unwrap_or_default();
    let mcp_servers =
        load_datum_providers::<crate::datum_mcp::McpDatum>(path, ".mcp.toml").unwrap_or_default();
    let docker_containers =
        load_datum_providers::<crate::datum_docker::DockerDatum>(path, ".docker.toml")
            .unwrap_or_default();

    let mut available_count = 0;
    let mut installed_count = 0;
    let mut missing_important = Vec::new();

    for tool in &cli_tools {
        available_count += 1;
        if DatumChecker::is_installed(tool.as_ref()) {
            installed_count += 1;
        } else {
            let tool_name = StatusProvider::name(tool.as_ref());
            let unknown = "unknown".to_string();
            let project_stack = memory.get("primary_stack").unwrap_or(&unknown);
            if is_tool_important_for_stack(tool_name, project_stack) {
                missing_important.push(tool_name.to_string());
            }
        }
    }

    println!("  🔧 {} CLI tools ({} installed)", available_count, installed_count);
    println!("  🔌 {} MCP servers", mcp_servers.len());
    println!("  🐳 {} Docker containers", docker_containers.len());

    if !missing_important.is_empty() {
        println!("  ⚠️  Missing: {}", missing_important.join(", "));
        memory.set("missing_important_tools", &missing_important.join(","))?;
    }

    memory.set_num("tools_available", available_count as i64)?;
    memory.set_num("tools_installed", installed_count as i64)?;
    memory.set_num("mcp_servers_available", mcp_servers.len() as i64)?;
    Ok(())
}

fn is_tool_important_for_stack(tool_name: &str, stack: &str) -> bool {
    match stack {
        "rust" => matches!(tool_name, "rustc" | "cargo" | "git"),
        "nodejs" => matches!(tool_name, "node" | "npm" | "git"),
        "python" => matches!(tool_name, "python3" | "pip" | "git"),
        _ => matches!(tool_name, "git"),
    }
}

fn setup_infrastructure(memory: &mut SessionMemory) -> Result<()> {
    verify_and_start_redis(memory)?;

    let container_runtime = if cmd!("podman", "--version").read().is_ok() {
        "podman"
    } else if cmd!("docker", "--version").read().is_ok() {
        "docker"
    } else {
        "none"
    };
    memory.set("preferred_container_runtime", container_runtime)?;
    memory.set("last_setup", &chrono::Utc::now().to_rfc3339())?;
    Ok(())
}

fn show_capabilities(memory: &mut SessionMemory) -> Result<()> {
    let unknown = "unknown".to_string();
    let empty = "".to_string();
    let project_stack = memory.get("primary_stack").unwrap_or(&unknown);
    let missing_tools = memory.get("missing_important_tools").unwrap_or(&empty);

    if !missing_tools.is_empty() {
        println!(
            "  💡 Install for {}: b00t install {}",
            project_stack,
            missing_tools.replace(",", " ")
        );
    }
    println!("  🎓 Commands: b00t status, b00t mcp list, b00t cli up");
    memory.set_flag("enlightenment_completed", true)?;
    Ok(())
}

// ── Redis helpers ──────────────────────────────────────────────────────────

fn verify_and_start_redis(memory: &mut SessionMemory) -> Result<()> {
    println!("  🔍 Checking Redis...");

    let redis_running = std::process::Command::new("redis-cli")
        .args(&["ping"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if redis_running {
        println!("  ✅ Redis already running");
        memory.set_flag("redis_running", true)?;
        return Ok(());
    }

    println!("  🚀 Starting Redis...");
    let started = try_start_redis_server()?;
    if started {
        std::thread::sleep(std::time::Duration::from_millis(1000));
        let ping = std::process::Command::new("redis-cli").args(&["ping"]).output();
        match ping {
            Ok(o) if o.status.success() => {
                if String::from_utf8_lossy(&o.stdout).trim() == "PONG" {
                    println!("  ✅ Redis started");
                    memory.set_flag("redis_running", true)?;
                } else {
                    println!("  ⚠️  Unexpected ping response");
                }
            }
            _ => println!("  ❌ Redis ping failed"),
        }
    } else {
        println!("  ❌ Failed to start Redis");
        memory.set_flag("redis_running", false)?;
    }
    Ok(())
}

fn try_start_redis_server() -> Result<bool> {
    for (bin, args) in &[
        ("systemctl", &["start", "redis-server"][..]),
        ("service", &["redis-server", "start"][..]),
    ] {
        if let Ok(s) = std::process::Command::new(bin).args(*args).status() {
            if s.success() {
                println!("  📦 Started via {}", bin);
                return Ok(true);
            }
        }
    }
    if std::process::Command::new("redis-server")
        .args(&["--daemonize", "yes"])
        .spawn()
        .is_ok()
    {
        println!("  📦 Started redis-server directly");
        return Ok(true);
    }
    println!("  ⚠️  All startup methods failed");
    Ok(false)
}

// ── Diagnostics (public — used by whatismy) ───────────────────────────────

pub fn run_system_diagnostics(memory: &mut SessionMemory) -> Result<()> {
    let context = memory.get_agent_context();

    println!("  🩺 Agent Context Diagnostics");
    println!("  🤖 Agent: {}", context.agent_name);
    println!("  📅 Session: {}s ({})", context.session_duration, format_duration(context.session_duration));
    println!("  🌿 Branch: {} ({} builds)", context.current_branch, context.build_count);
    println!(
        "  📊 Activity: {} shells, {} compiles, {} tests",
        context.shell_count, context.compile_count, context.test_count
    );

    let mut passing = 0;
    let total = 4;

    if std::process::Command::new("git").arg("--version").output().is_ok() {
        println!("  ✅ git: available");
        passing += 1;
    } else { println!("  ❌ git: not available"); }

    if std::process::Command::new("cargo").arg("--version").output().is_ok() {
        println!("  ✅ cargo: available");
        passing += 1;
    } else { println!("  ❌ cargo: not available"); }

    if std::process::Command::new("node").arg("--version").output().is_ok() {
        println!("  ✅ node: available");
        passing += 1;
    } else { println!("  ❌ node: not available"); }

    if std::process::Command::new("docker").arg("--version").output().is_ok() {
        println!("  ✅ docker: available");
        passing += 1;
    } else { println!("  ❌ docker: not available"); }

    memory.set_num("diagnostic_passing", passing as i64)?;
    memory.set_num("diagnostic_total", total as i64)?;
    println!("  📊 {}/{} systems operational", passing, total);
    Ok(())
}

fn format_duration(seconds: i64) -> String {
    if seconds < 60 { format!("{}s", seconds) }
    else if seconds < 3600 { format!("{}m{}s", seconds / 60, seconds % 60) }
    else { format!("{}h{}m", seconds / 3600, (seconds % 3600) / 60) }
}

// ── Dispatch ────────────────────────────────────────────────────────────────

impl InitCommands {
    pub fn execute(&self, path: &str) -> Result<()> {
        match self {
            InitCommands::Project { name, stack, setup, no_setup } => {
                handle_project_init(name.clone(), stack.clone(), *setup, *no_setup, path)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_init_creates_directory_and_files() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        handle_project_init(Some("test-proj".into()), Some("rust".into()), false, true, "test").unwrap();

        let b00t = dir.path().join("_b00t_");
        assert!(b00t.exists());
        assert!(b00t.join("project.toml").exists());
        assert!(b00t.join("overrides.toml").exists());

        let content = std::fs::read_to_string(b00t.join("project.toml")).unwrap();
        assert!(content.contains("test-proj"));
        assert!(content.contains("rust"));
    }

    #[test]
    fn detect_project_stack_detects_rust() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(detect_project_stack(dir.path()), "rust");
    }

    #[test]
    fn detect_project_stack_detects_nodejs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), "").unwrap();
        assert_eq!(detect_project_stack(dir.path()), "nodejs");
    }

    #[test]
    fn detect_project_stack_unknown() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect_project_stack(dir.path()), "unknown");
    }

    #[test]
    fn first_time_init_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        // no_setup=true: skip agent onboarding (requires real session config)
        handle_project_init(Some("auto".into()), Some("rust".into()), false, true, "test").unwrap();
        assert!(dir.path().join("_b00t_").exists());
    }
}
