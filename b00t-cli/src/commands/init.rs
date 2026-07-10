use crate::commands::cli_cmd::CliCommands;
use crate::load_datum_providers;
use crate::session_memory::SessionMemory;
use crate::traits::*;
use crate::whoami;
use anyhow::{Context, Result};
use clap::Parser;
use std::path::Path;

#[derive(Parser)]
pub enum InitCommands {
    #[clap(
        about = "Initialize a b00t project in the current directory",
        long_about = "Create _b00t_/ with 🥾.tomllmd + overrides.toml.\nValidates system state and sets up .env + .envrc.\nRuns agent onboarding automatically on first init.\n\nExamples:\n  b00t init project\n  b00t init project --name my-project --stack rust\n  b00t init project --setup   (force onboarding)\n  b00t init project --no-setup (skip onboarding)\n  b00t init project --dry-run"
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
        #[clap(long, help = "Check what would be installed without installing")]
        dry_run: bool,
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

    // Must be at the root of a git repo
    if !cwd.join(".git").exists() {
        anyhow::bail!(
            "not a git repository root ({}). Run 'git init' first.",
            cwd.display()
        );
    }
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

    // ── Write project soul: .git/🥾.tomllmd (canonical) ──────────────
    // Symlinked to _b00t_/🥾.tomllmd for visibility.
    // .git/ is never tracked — safe deterministic access point.

    let git_dir = cwd.join(".git");
    let git_boot = git_dir.join("🥾.tomllmd");
    if !git_boot.exists() {
        let content = format!(
            r#"# 🥾 {project_name} — b00t project soul

[project]
name = "{project_name}"
primary_stack = "{primary_stack}"

[overrides]
# Pin tool versions for this project:
# rustc = "1.85.0"
# node = "22.0.0"

# b00t:map v1
# summary: {project_name} b00t project configuration
# tags: project, {project_name}, {primary_stack}
# tier: sm0l
# cmds: b00t init project
# complexity: 1
"#,
        );
        std::fs::write(&git_boot, &content)?;
        println!("  📝 wrote {}", git_boot.display());

        // Symlink into _b00t_/ for visibility
        let b00t_boot = b00t_dir.join("🥾.tomllmd");
        std::os::unix::fs::symlink(&git_boot, &b00t_boot).ok();
        println!("  🔗 linked {}", b00t_boot.display());
    }

    // ── System validation + .env / .envrc setup ───────────────────────

    println!("\n🔍 System validation");
    let env = collect_environment(&cwd, &project_name, &primary_stack);

    write_env_files(&cwd, &env)?;
    print_validation_report(&env);

    println!("\n✅ Project '{}' initialized ({})", project_name, primary_stack);

    // ── Auto-check tool versions (#585) ─────────────────────────────────
    println!("\n🔍 Checking installed tools...");
    let b00t_bin = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("b00t-cli"));
    match std::process::Command::new(&b00t_bin).args(["cli", "up"]).arg("--path").arg(path).status() {
        Ok(s) if s.success() => {}
        Ok(_) => eprintln!("  ⚠️  Some tools need updating — run 'b00t cli up'"),
        Err(e) => eprintln!("  ⚠️  Tool check skipped: {e}"),
    }

    // ── Agent onboarding ─────────────────────────────────────────────
    if !no_setup && (!existed || force_setup) {
        if existed && force_setup {
            println!("\n🔧 Forcing agent setup...");
        } else {
            println!("\n🔧 First-time agent setup...");
        }
        run_setup(path)?;
    } else if existed {
        println!("   Run 'b00t init project --setup' to re-run agent setup");
    }

    Ok(())
}

// ── Environment detection + .env / .envrc generation ──────────────────────

struct ProjectEnv {
    project_name: String,
    primary_stack: String,
    b00t_version: String,
    has_direnv: bool,
    container_runtime: String,
    rust_version: Option<String>,
    node_version: Option<String>,
    python_version: Option<String>,
    go_version: Option<String>,
    missing_critical: Vec<String>,
}

fn collect_environment(_cwd: &Path, project_name: &str, primary_stack: &str) -> ProjectEnv {
    let mut env = ProjectEnv {
        project_name: project_name.to_string(),
        primary_stack: primary_stack.to_string(),
        b00t_version: env!("CARGO_PKG_VERSION").to_string(),
        has_direnv: which::which("direnv").is_ok(),
        container_runtime: detect_container_runtime(),
        rust_version: None,
        node_version: None,
        python_version: None,
        go_version: None,
        missing_critical: Vec::new(),
    };

    // Detect tool versions
    env.rust_version = detect_version("rustc", &["--version"], r"(\d+\.\d+\.\d+)");
    env.node_version = detect_version("node", &["--version"], r"v?(\d+\.\d+\.\d+)");
    env.python_version = detect_version("python3", &["--version"], r"(\d+\.\d+\.\d+)");
    env.go_version = detect_version("go", &["version"], r"go(\d+\.\d+\.\d+)");

    // Critical tools for the detected stack
    match primary_stack {
        "rust" => {
            if env.rust_version.is_none() { env.missing_critical.push("rustc".into()); }
            if !which::which("cargo").is_ok() { env.missing_critical.push("cargo".into()); }
        }
        "nodejs" => {
            if env.node_version.is_none() { env.missing_critical.push("node".into()); }
            if !which::which("npm").is_ok() { env.missing_critical.push("npm".into()); }
        }
        "python" => {
            if env.python_version.is_none() { env.missing_critical.push("python3".into()); }
        }
        "go" => {
            if env.go_version.is_none() { env.missing_critical.push("go".into()); }
        }
        _ => {}
    }
    if !which::which("git").is_ok() { env.missing_critical.push("git".into()); }

    if env.container_runtime == "none" {
        env.missing_critical.push("podman/docker".into());
    }

    env
}

fn detect_version(bin: &str, args: &[&str], regex: &str) -> Option<String> {
    let output = std::process::Command::new(bin).args(args).output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let re = regex::Regex::new(regex).ok()?;
    re.captures(&stdout)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn detect_container_runtime() -> String {
    if which::which("podman").is_ok() { return "podman".into(); }
    if which::which("docker").is_ok() { return "docker".into(); }
    "none".into()
}

fn write_env_files(cwd: &Path, env: &ProjectEnv) -> Result<()> {
    // .envrc (direnv)
    let envrc_path = cwd.join(".envrc");
    let envrc_new = !envrc_path.exists();
    if envrc_new {
        let container_block = if env.container_runtime == "podman" {
            "export DOCKER_HOST=\"unix:///run/user/1000/podman/podman.sock\"\n"
        } else {
            ""
        };
        let content = format!(
            r#"# b00t-managed .envrc — {project}
# Auto-generated by 'b00t init project'

export _B00T_Project="{project}"
export _B00T_Stack="{stack}"
export _B00T_Path="$PWD/_b00t_"

# Container runtime ({runtime})
{container_block}

# Tool overrides (detected versions)
{rust_line}{node_line}{python_line}{go_line}
# end b00t
"#,
            project = env.project_name,
            stack = env.primary_stack,
            runtime = env.container_runtime,
            container_block = container_block,
            rust_line = env.rust_version.as_ref()
                .map(|v| format!("export RUST_VERSION=\"{}\"\n", v))
                .unwrap_or_default(),
            node_line = env.node_version.as_ref()
                .map(|v| format!("export NODE_VERSION=\"{}\"\n", v))
                .unwrap_or_default(),
            python_line = env.python_version.as_ref()
                .map(|v| format!("export PYTHON_VERSION=\"{}\"\n", v))
                .unwrap_or_default(),
            go_line = env.go_version.as_ref()
                .map(|v| format!("export GO_VERSION=\"{}\"\n", v))
                .unwrap_or_default(),
        );
        std::fs::write(&envrc_path, &content)?;
        println!("  📝 wrote {}", envrc_path.display());
    }

    // .env (standard)
    let dotenv_path = cwd.join(".env");
    let dotenv_new = !dotenv_path.exists();
    if dotenv_new {
        let content = format!(
            r#"# b00t-managed .env — {project}
# Auto-generated by 'b00t init project'
_B00T_Project={project}
_B00T_Stack={stack}
_B00T_Path=./_b00t_
"#,
            project = env.project_name,
            stack = env.primary_stack,
        );
        std::fs::write(&dotenv_path, &content)?;
        println!("  📝 wrote {}", dotenv_path.display());
    }

    if envrc_new || dotenv_new {
        if env.has_direnv {
            println!("  💡 Run 'direnv allow' to activate the environment");
        } else {
            println!("  💡 Run 'source .envrc' or 'direnv allow' to load settings");
        }
    }

    Ok(())
}

fn print_validation_report(env: &ProjectEnv) {
    let mut ok = 0;
    let mut warn = 0;

    let checks: Vec<(&str, Option<&str>, bool)> = vec![
        ("rustc", env.rust_version.as_deref(), env.primary_stack == "rust"),
        ("node", env.node_version.as_deref(), env.primary_stack == "nodejs"),
        ("python3", env.python_version.as_deref(), env.primary_stack == "python"),
        ("go", env.go_version.as_deref(), env.primary_stack == "go"),
        ("git", Some(if which::which("git").is_ok() { "✓" } else { "" }), true),
        (&env.container_runtime, Some("✓"), true),
    ];

    for (tool, version, important) in checks {
        match version {
            Some(v) if !v.is_empty() => {
                let tag = if important { "required" } else { "available" };
                println!("  ✅ {:<20} {:>12}  ({})", tool, if v == "✓" { v.into() } else { format!("v{}", v) }, tag);
                ok += 1;
            }
            _ => {
                let tag = if important { "❌ missing" } else { "optional" };
                println!("  ❌ {:<20} {:>12}  ({})", tool, "—", tag);
                if important { warn += 1; }
            }
        }
    }

    if warn > 0 {
        println!("\n  ⚠️  {} critical tool(s) missing. Install them and re-run 'b00t init project --setup'.", warn);
    }
    if ok > 0 && warn == 0 {
        println!("\n  ✅ All critical tools available.");
    }
}

// ── Project stack detection ───────────────────────────────────────────────

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

    let container_runtime = detect_container_runtime();
    memory.set("preferred_container_runtime", &container_runtime)?;
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

    for (label, bin) in &[("git", "git"), ("cargo", "cargo"), ("node", "node"), ("docker", "docker")] {
        if std::process::Command::new(bin).arg("--version").output().is_ok() {
            println!("  ✅ {}: available", label);
            passing += 1;
        } else {
            println!("  ❌ {}: not available", label);
        }
    }

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
            InitCommands::Project { name, stack, setup, no_setup, dry_run } => {
                if *dry_run {
                    let mut memory = SessionMemory::load()?;
                    detect_project_context(&mut memory)?;
                    let stack = memory.get("primary_stack").cloned().unwrap_or_default();
                    println!("\n🥾 Running b00t cli up for {} project…", stack);
                    return CliCommands::Up {
                        yes: false,
                        maintenance: false,
                    }
                    .execute(path);
                }
                handle_project_init(name.clone(), stack.clone(), *setup, *no_setup, path)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Serialize CWD-mutating tests — set_current_dir is process-global.
    static CWD_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

    struct RestoreCwd(std::path::PathBuf);
    impl Drop for RestoreCwd {
        fn drop(&mut self) { let _ = std::env::set_current_dir(&self.0); }
    }

    #[test]
    fn project_init_creates_directory_and_files() {
        let _guard = CWD_LOCK.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap();
        let _restore = RestoreCwd(std::env::current_dir().unwrap());
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        handle_project_init(Some("test-proj".into()), Some("rust".into()), false, true, "test").unwrap();

        let b00t = dir.path().join("_b00t_");
        assert!(b00t.exists());
        // .git/🥾.tomllmd is canonical; _b00t_/🥾.tomllmd is a symlink
        assert!(dir.path().join(".git").join("🥾.tomllmd").exists());
        assert!(b00t.join("🥾.tomllmd").exists());
        assert!(dir.path().join(".envrc").exists());
        assert!(dir.path().join(".env").exists());

        let content = std::fs::read_to_string(dir.path().join(".env")).unwrap();
        assert!(content.contains("test-proj"));
        assert!(content.contains("_B00T_Path=./_b00t_"));
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
    fn env_file_contains_project_vars() {
        let _guard = CWD_LOCK.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap();
        let _restore = RestoreCwd(std::env::current_dir().unwrap());
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        handle_project_init(Some("demo".into()), Some("nodejs".into()), false, true, "test").unwrap();

        let envrc = std::fs::read_to_string(dir.path().join(".envrc")).unwrap();
        assert!(envrc.contains("_B00T_Project=\"demo\""));
        assert!(envrc.contains("_B00T_Stack=\"nodejs\""));
        assert!(envrc.contains("_B00T_Path=\"$PWD/_b00t_\""));
    }

    #[test]
    fn rejects_non_git_directory() {
        let _guard = CWD_LOCK.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap();
        let _restore = RestoreCwd(std::env::current_dir().unwrap());
        let dir = tempfile::tempdir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let result = handle_project_init(None, None, false, true, "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a git repository"));
    }
}
