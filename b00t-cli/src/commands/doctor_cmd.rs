//! `b00t doctor` — dependency verification, IDE wiring, env documentation
//!
//! # Usage
//! ```bash
//! b00t doctor check             # verify deps
//! b00t doctor check --json      # JSON report
//! b00t doctor check --probe gh  # single dep
//! b00t doctor setup --role=executive,operator  # verify + wire MCP
//! b00t doctor env               # env docs for model
//! b00t doctor ide list          # MCP servers in IDEs
//! ```

use crate::datum_store::{DatumStore, HashMapStore, ReferenceError};
use anyhow::{Context, Result};
use b00t_c0re_lib::redis::{RedisComms, RedisConfig};
use clap::Parser;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/brianh"))
}

fn sh(cmd: &str) -> (bool, String) {
    Command::new("sh")
        .args(["-c", cmd])
        .output()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let e = String::from_utf8_lossy(&o.stderr).trim().to_string();
            (
                o.status.success() && !s.is_empty(),
                if !s.is_empty() {
                    s
                } else if !e.is_empty() {
                    e
                } else {
                    "not found".into()
                },
            )
        })
        .unwrap_or((false, "exec failed".into()))
}

fn check_version(name: &str) -> Value {
    let which = sh(&format!("which {} 2>/dev/null", name));
    let ver = if which.0 {
        sh(&format!("{} --version 2>/dev/null | head -1", name))
    } else {
        (false, String::new())
    };
    json!({"id": name, "pass": which.0 || ver.0, "detail": if which.0 || ver.0 {
        format!("{} {}", which.1.trim(), ver.1.trim())
    } else { "not found".into() }})
}

/// Probe the optional Redis-backed agent registry (issue #83). Reuses
/// `RedisComms::is_available()` rather than shelling out to `redis-cli` so
/// this exercises the same connection path `agent discover`/`agent
/// capability` use. Always `pass: true` — Redis is an optional accelerant
/// for live multi-host discovery, not a hard dependency; both commands fall
/// back to local `_b00t_/*.agent.toml` when it's unreachable.
fn check_redis_agent_registry() -> Value {
    let start = Instant::now();
    let reachable = RedisComms::new(RedisConfig::default(), "doctor-probe".into())
        .map(|c| c.is_available())
        .unwrap_or(false);
    json!({
        "id": "redis-agent-registry",
        "pass": true,
        "detail": if reachable {
            "reachable — live multi-host agent discovery available"
        } else {
            "unreachable — optional, accelerates live multi-host agent discovery; falls back to local `_b00t_/*.agent.toml` when unavailable"
        },
        "latency_ms": start.elapsed().as_millis(),
    })
}

fn all_deps() -> Vec<Value> {
    vec![
        check_version("b00t-cli"),
        check_version("b00t-mcp"),
        check_version("b00t-task"),
        check_version("git"),
        check_version("gh"),
        check_version("just"),
        check_version("cargo"),
        check_version("rustc"),
        check_version("node"),
        check_version("npm"),
        check_version("python3"),
        check_version("docker"),
        check_version("jq"),
        check_version("curl"),
        // Special checks with auth/daemon info
        json!({"id": "gh-auth", "check": "gh auth status 2>&1 | grep -q 'Logged in' && echo yes || echo no"}),
        json!({"id": "docker-daemon", "check": "docker info --format '{{.ServerVersion}}' 2>/dev/null"}),
        check_redis_agent_registry(),
        // Filesystem
        json!({"id": "b00t-repo", "check": "test -d $HOME/.b00t/.git && cd $HOME/.b00t && git log --oneline -1 2>/dev/null"}),
        json!({"id": "soul-db", "check": "test -f $HOME/._b00t_/soul.db && ls -la $HOME/._b00t_/soul.db || echo missing"}),
        json!({"id": "task-queue", "check": "test -d $HOME/.local/share/b00t/task-queue/pending && ls $HOME/.local/share/b00t/task-queue/pending/*.json 2>/dev/null | wc -l || echo 0"}),
        json!({"id": "epoch-state", "check": "cat $HOME/.local/share/b00t/epoch-state.json 2>/dev/null | jq -e '.epoch' >/dev/null 2>&1 && echo valid || echo invalid"}),
        // Network
        json!({"id": "dns", "check": "host github.com 2>/dev/null | head -1 | grep -q 'has address' && echo ok || echo fail"}),
        json!({"id": "gh-api", "check": "curl -sf --max-time 5 -o /dev/null -w '%{http_code}' https://api.github.com/zen 2>/dev/null"}),
        // Skill datum integrity: no loose .skill.* files in _b00t_/
        json!({"id": "skill-symlinks", "check": "cd $HOME/.dotfiles 2>/dev/null && find _b00t_/ -name '*.skill.*' ! -type l 2>/dev/null | wc -l | tr -d ' ' || echo 0"}),
        // Stray root-level test/ dir (#935): bats-core/bats-assert/bats-support
        // are registered as submodules under _b00t_/test/ only. A root-level
        // test/ dir is an untracked, unregistered footgun — a duplicate
        // checkout that could silently diverge from the real submodule.
        json!({"id": "no-stray-root-test-dir", "check": "test -d $HOME/.b00t/test && echo FAIL || echo PASS"}),
    ].into_iter().map(|mut v| {
        let check = v.get("check").and_then(|c| c.as_str()).unwrap_or("");
        if !check.is_empty() {
            let start = Instant::now();
            let (ok, detail) = sh(check);
            // For skill-symlinks, pass is only when count is 0
            if v["id"] == "skill-symlinks" {
                let count: usize = detail.trim().parse().unwrap_or(1);
                v["pass"] = json!(count == 0);
                v["detail"] = if count == 0 {
                    json!("all .skill.* files are symlinks")
                } else {
                    json!(format!("{} loose .skill.* files in _b00t_/ (must be symlinks to skills/*/SKILL.md)", count))
                };
            } else if v["id"] == "no-stray-root-test-dir" {
                let pass = detail.trim() == "PASS";
                v["pass"] = json!(pass);
                v["detail"] = if pass {
                    json!("no stray root-level test/ dir")
                } else {
                    json!("stray $HOME/.b00t/test/ dir present — use _b00t_/test/* submodules instead (#935)")
                };
            } else {
                v["pass"] = json!(ok);
                v["detail"] = json!(detail);
            }
            v["latency_ms"] = json!(start.elapsed().as_millis());
        }
        v
    }).collect()
}

#[derive(Default)]
struct RoleComposite {
    agents: Vec<String>,
    cli: Vec<String>,
    mcps: Vec<String>,
    skills: Vec<String>,
    compliance: Vec<String>,
}

fn compose_roles(roles: &[String], b00t_path: &str) -> Result<RoleComposite> {
    let mut merged = RoleComposite::default();
    for role_name in roles {
        let ext = if *role_name == "executive" {
            "role.tomllm"
        } else {
            "role.toml"
        };
        let path = PathBuf::from(b00t_path).join(format!("{}.{}", role_name, ext));
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("role datum not found: {}", path.display()))?;
        let v: Value = toml::from_str(&content)?;
        let b = &v["b00t"];
        if let Some(arr) = b["entangled_agents"].as_array() {
            merged
                .agents
                .extend(arr.iter().filter_map(|s| s.as_str().map(String::from)));
        }
        if let Some(arr) = b["entangled_cli"].as_array() {
            merged
                .cli
                .extend(arr.iter().filter_map(|s| s.as_str().map(String::from)));
        }
        if let Some(arr) = b["entangled_mcp"].as_array() {
            merged
                .mcps
                .extend(arr.iter().filter_map(|s| s.as_str().map(String::from)));
        }
        if let Some(arr) = b["skills"].as_array() {
            merged
                .skills
                .extend(arr.iter().filter_map(|s| s.as_str().map(String::from)));
        }
        if let Some(arr) = b["compliance"].as_array() {
            merged
                .compliance
                .extend(arr.iter().filter_map(|s| s.as_str().map(String::from)));
        }
    }
    Ok(merged)
}

fn list_ide_mcp(name: &str) -> Value {
    match name {
        "vscode" => {
            let exts = sh("code --list-extensions 2>/dev/null | grep -i mcp || true");
            let mcp = home().join(".config/Code/User/globalStorage/ms-vscode.mcp-server/mcp.json");
            json!({"ide":"vscode", "mcp_json": mcp.exists(), "extensions": exts.1.trim()})
        }
        "claudecode" => {
            let out = sh("claude mcp list 2>/dev/null | head -20 || echo 'not configured'");
            json!({"ide":"claudecode", "mcp_list": out.1.trim()})
        }
        "geminicli" => {
            let out = sh("geminicli mcp list 2>/dev/null | head -10 || echo 'not configured'");
            json!({"ide":"geminicli", "mcp_list": out.1.trim()})
        }
        "copilot" => {
            let mcp = home().join(".vscode/mcp.json");
            json!({"ide":"copilot", "mcp_json": mcp.exists(), "path": mcp.display().to_string()})
        }
        _ => json!({"ide": name, "error": "unknown IDE"}),
    }
}

fn install_role_mcps(composite: &RoleComposite, target: &str) -> Vec<String> {
    composite
        .mcps
        .iter()
        .map(|mcp| {
            let name = mcp.trim_end_matches(".mcp");
            let out = sh(&format!("b00t-cli mcp install {} {} 2>&1", name, target));
            format!("{} {}: {}", name, target, out.1.trim())
        })
        .collect()
}

fn generate_env_doc(b00t_path: &str) -> Value {
    let deps = all_deps();
    json!({
        "hostname": sh("hostname 2>/dev/null").1.trim(),
        "os": sh("cat /etc/os-release 2>/dev/null | grep PRETTY_NAME | cut -d= -f2 | tr -d '\"'").1.trim(),
        "memory": sh("free -h 2>/dev/null | grep Mem | awk '{print $2}'").1.trim(),
        "disk": sh("df -h / 2>/dev/null | tail -1 | awk '{print $3\"/\"$2\" (\"$5\")\"}'").1.trim(),
        "b00t_path": b00t_path,
        "home": home().display().to_string(),
        "whoami": sh("whoami 2>/dev/null").1.trim(),
        "deps": deps,
        "ide_mcp": vec![list_ide_mcp("vscode"), list_ide_mcp("claudecode"), list_ide_mcp("geminicli"), list_ide_mcp("copilot")],
        "epoch": sh("cat ~/.local/share/b00t/epoch-state.json 2>/dev/null | jq -c '{epoch,cycle,phase}' 2>/dev/null || echo none").1.trim(),
    })
}

// ─── Commands ─────────────────────────────────────────────────────────────────

#[derive(Parser, Clone)]
pub enum DoctorCommands {
    #[clap(about = "Verify b00t system dependencies")]
    Check {
        #[clap(long, help = "JSON output")]
        json: bool,
        #[clap(long, help = "Check a single dependency by id")]
        probe: Option<String>,
    },
    #[clap(about = "Verify role deps + wire MCP into IDEs")]
    Setup {
        #[clap(long, help = "Roles (comma-separated, e.g. executive,operator)")]
        role: Option<String>,
        #[clap(
            long,
            help = "Target IDE: vscode, claudecode, geminicli, copilot (default: all)"
        )]
        target: Option<String>,
        #[clap(long, help = "JSON output")]
        json: bool,
        #[clap(long, help = "Skip MCP install, verify only")]
        dry_run: bool,
    },
    #[clap(about = "Environment documentation for the AI model")]
    Env {
        #[clap(long)]
        json: bool,
    },
    #[clap(about = "List MCP servers registered in IDEs")]
    Ide {
        #[clap(subcommand)]
        cmd: Option<IdeAction>,
    },
    #[clap(hide = true)]
    HealthJson,
}

#[derive(Parser, Clone)]
pub enum IdeAction {
    #[clap(about = "List all")]
    List,
    #[clap(about = "Show one")]
    Show { name: String },
}

// ─── Handler ──────────────────────────────────────────────────────────────────

pub fn handle_doctor_command(args: &DoctorCommands, b00t_path: &str) -> Result<()> {
    match args {
        DoctorCommands::Check { json, probe } => {
            let results: Vec<Value> = all_deps()
                .into_iter()
                .filter(|d| {
                    probe.as_ref().map_or(true, |p| {
                        d["id"].as_str().map_or(false, |id| id.contains(p))
                    })
                })
                .collect();

            // Phase 1: well-formedness — count datums that pass prove_by_type()
            let store = HashMapStore::from_path(b00t_path).unwrap_or_default();
            let total_datums = store.len();
            let (provable, broken): (Vec<_>, Vec<_>) =
                store.iter().partition(|d| d.datum.prove_by_type().is_ok());
            // Phase 2: coherence — cross-datum reference validation
            let ref_errors = store.validate_references();
            let self_deps: Vec<_> = ref_errors
                .iter()
                .filter(|e| matches!(e, ReferenceError::SelfDependency { .. }))
                .collect();
            let empty_deps: Vec<_> = ref_errors
                .iter()
                .filter(|e| matches!(e, ReferenceError::EmptyDependency { .. }))
                .collect();

            if *json {
                let datum_report = json!({
                    "total": total_datums,
                    "provable": provable.len(),
                    "broken": broken.iter().map(|d| {
                        let err = d.datum.prove_by_type().unwrap_err();
                        json!({"key": d.key.as_ref(), "error": err.to_string()})
                    }).collect::<Vec<_>>(),
                    "reference_errors": ref_errors.iter().map(|e| e.to_string()).collect::<Vec<_>>(),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "system_deps": results,
                        "datum_store": datum_report
                    }))?
                );
            } else {
                println!("🥾 b00t doctor — dependency check\n");
                for r in &results {
                    let ok = r["pass"].as_bool().unwrap_or(false);
                    let ms = r["latency_ms"].as_u64().unwrap_or(0);
                    println!(
                        "  {}  {}  {}ms  {}",
                        if ok { "✅" } else { "❌" },
                        r["id"].as_str().unwrap_or("?"),
                        ms,
                        r["detail"].as_str().unwrap_or("")
                    );
                }
                let ok = results
                    .iter()
                    .filter(|r| r["pass"].as_bool().unwrap_or(false))
                    .count();
                println!("\n  {}/{} satisfied", ok, results.len());

                println!("\n🗄️  datum store — {b00t_path}");
                println!("  {}/{} datums provable", provable.len(), total_datums);
                for d in &broken {
                    let err = d.datum.prove_by_type().unwrap_err();
                    println!("  ❌ {}: {}", d.key.as_ref(), err);
                }
                if !self_deps.is_empty() || !empty_deps.is_empty() {
                    println!(
                        "  ⚠️  reference errors: {} self-deps, {} empty deps",
                        self_deps.len(),
                        empty_deps.len()
                    );
                    for e in self_deps.iter().chain(empty_deps.iter()) {
                        println!("     {e}");
                    }
                } else {
                    println!("  ✅ store coherence: no self-deps or empty deps");
                }
            }
            Ok(())
        }
        DoctorCommands::Setup {
            role,
            target,
            json,
            dry_run,
        } => {
            let roles: Vec<String> = role
                .as_deref()
                .unwrap_or("worker")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let composite = compose_roles(&roles, b00t_path)?;
            if *json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "roles": roles, "composite": {
                            "agents": composite.agents, "cli": composite.cli, "mcps": composite.mcps,
                            "skills": composite.skills, "compliance": composite.compliance
                        }, "dry_run": dry_run
                    }))?
                );
                return Ok(());
            }
            println!("🥾 b00t doctor setup — roles: {}", roles.join(", "));
            println!("  Skills: {}", composite.skills.join(", "));
            println!("  CLI tools: {}", composite.cli.join(", "));
            println!("  MCP servers: {}", composite.mcps.join(", "));
            println!("\n  🔧 CLI check:");
            for cli in &composite.cli {
                let name = cli.trim_end_matches(".cli");
                let (ok, d) = sh(&format!(
                    "which {} 2>/dev/null && {} --version 2>/dev/null | head -1 || echo MISSING",
                    name, name
                ));
                println!(
                    "    {} {}: {}",
                    if ok { "✅" } else { "❌" },
                    name,
                    d.trim()
                );
            }
            println!("\n  🔌 MCP datums:");
            for mcp in &composite.mcps {
                let name = mcp.trim_end_matches(".mcp");
                let p = PathBuf::from(b00t_path).join(format!("{}.mcp.toml", name));
                println!(
                    "    {} {}: {}",
                    if p.exists() { "✅" } else { "❌" },
                    name,
                    p.display()
                );
            }
            if !*dry_run {
                let idelist: Vec<&str> = if target.as_deref().unwrap_or("all") == "all" {
                    vec!["vscode", "claudecode", "geminicli", "copilot"]
                } else {
                    vec![target.as_deref().unwrap_or("all")]
                };
                for ide in &idelist {
                    println!("\n  📡 Installing into {}:", ide);
                    for r in &install_role_mcps(&composite, ide) {
                        println!("    {}", r);
                    }
                }
            }
            Ok(())
        }
        DoctorCommands::Env { json } => {
            let doc = generate_env_doc(b00t_path);
            if *json {
                println!("{}", serde_json::to_string_pretty(&doc)?);
            } else {
                println!("🥾 b00t doctor env — local environment\n");
                println!("  Host: {}", doc["hostname"].as_str().unwrap_or("?"));
                println!("  OS: {}", doc["os"].as_str().unwrap_or("?"));
                println!(
                    "  RAM: {} | Disk: {}",
                    doc["memory"].as_str().unwrap_or("?"),
                    doc["disk"].as_str().unwrap_or("?")
                );
                println!("  Epoch: {}", doc["epoch"].as_str().unwrap_or("?"));
                println!("\n  Deps:");
                for d in doc["deps"].as_array().unwrap_or(&vec![]) {
                    let ok = d["pass"].as_bool().unwrap_or(false);
                    println!(
                        "    {}  {}: {}",
                        if ok { "●" } else { "○" },
                        d["id"].as_str().unwrap_or("?"),
                        d["detail"].as_str().unwrap_or("")
                    );
                }
            }
            Ok(())
        }
        DoctorCommands::Ide { cmd } => {
            let ides = vec![
                list_ide_mcp("vscode"),
                list_ide_mcp("claudecode"),
                list_ide_mcp("geminicli"),
                list_ide_mcp("copilot"),
            ];
            match cmd.as_ref().unwrap_or(&IdeAction::List) {
                IdeAction::List => println!("{}", serde_json::to_string_pretty(&ides)?),
                IdeAction::Show { name } => {
                    println!("{}", serde_json::to_string_pretty(&list_ide_mcp(name))?)
                }
            }
            Ok(())
        }
        DoctorCommands::HealthJson => {
            let results = all_deps();
            let ok = results
                .iter()
                .filter(|r| r["pass"].as_bool().unwrap_or(false))
                .count();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "total": results.len(), "passed": ok, "failed": results.len() - ok, "probes": results
                }))?
            );
            Ok(())
        }
    }
}
pub fn health_json() -> serde_json::Value {
    let results = all_deps();
    let ok = results
        .iter()
        .filter(|r| r["pass"].as_bool().unwrap_or(false))
        .count();
    serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "total": results.len(), "passed": ok, "failed": results.len() - ok, "probes": results
    })
}

/// Check 1: b00t-cli binary exists and reports version
fn check_b00t_cli() -> Value {
    let exists = Command::new("which")
        .arg("b00t-cli")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let version = if exists {
        Command::new("b00t-cli")
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout)
                        .or_else(|_| String::from_utf8(o.stderr))
                        .ok()
                } else {
                    None
                }
            })
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    } else {
        String::new()
    };

    json!({
        "check": "b00t-cli binary",
        "status": if exists { "ok" } else { "fail" },
        "detail": if exists { format!("v{}", version) } else { "not found in PATH".to_string() }
    })
}

/// Check 2: _b00t_ directory exists with datums
fn check_b00t_dir(b00t_path: &str, fix: bool) -> Value {
    let expanded = shellexpand::tilde(b00t_path).to_string();
    let path = PathBuf::from(&expanded);

    let exists = path.exists();
    if !exists && fix {
        let _ = std::fs::create_dir_all(&path);
    }

    // Count datum files (.toml)
    let datum_count = if path.exists() {
        match std::fs::read_dir(&path) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext == "toml" || ext == "tomllm" || ext == "tomllmd")
                        .unwrap_or(false)
                })
                .count(),
            Err(_) => 0,
        }
    } else {
        0
    };

    let exists_now = path.exists();
    json!({
        "check": "_b00t_/ directory",
        "status": if exists_now && datum_count > 0 { "ok" } else if exists_now { "warn" } else { "fail" },
        "detail": format!("{} datums at {}", datum_count, path.display())
    })
}

/// Check 3: .opencode/ directory exists with skills
fn check_opencode_dir(fix: bool) -> Value {
    let path = PathBuf::from(".opencode");
    let exists = path.exists();
    if !exists && fix {
        let _ = std::fs::create_dir_all(path.join("skills"));
        let _ = std::fs::create_dir_all(path.join("context"));
    }

    let skills_count = if path.join("skills").exists() {
        match std::fs::read_dir(path.join("skills")) {
            Ok(entries) => entries.filter_map(|e| e.ok()).count(),
            Err(_) => 0,
        }
    } else {
        0
    };

    let exists_now = path.exists();
    json!({
        "check": ".opencode/ directory",
        "status": if exists_now { "ok" } else { "fail" },
        "detail": format!("{} skills", skills_count)
    })
}

/// Check 4: Focus schema datum exists
fn check_focus_schema(b00t_path: &str) -> Value {
    let expanded = shellexpand::tilde(b00t_path).to_string();
    let path = PathBuf::from(&expanded).join("focus.schema.tomllmd");

    let exists = path.exists();
    json!({
        "check": "focus schema datum",
        "status": if exists { "ok" } else { "fail" },
        "detail": if exists {
            format!("found at {}", path.display())
        } else {
            format!("not found at {}", path.display())
        }
    })
}

/// Check 5: ledgrrr-mcp / ledgerr-mcp service status
fn check_ledgrrr_service() -> Value {
    let output = Command::new("systemctl")
        .args(["--user", "is-active", "ledgrrr-mcp"])
        .output();

    match output {
        Ok(o) => {
            let status = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let active = status == "active" || o.status.success();
            json!({
                "check": "ledgrrr-mcp service", // alt: ledgerr-mcp
                "status": if active { "ok" } else { "fail" },
                "detail": if active { "active".to_string() } else { status }
            })
        }
        Err(e) => {
            json!({
                "check": "ledgrrr-mcp service", // alt: ledgerr-mcp
                "status": "fail",
                "detail": format!("systemctl not available: {}", e)
            })
        }
    }
}

/// Check 6: Local model endpoint reachable
fn check_model_endpoint() -> Value {
    // Use curl (preferred) to avoid tokio runtime panic (#[tokio::main]); reqwest fallback commented below
    let reachable = std::process::Command::new("curl")
        .args([
            "-s",
            "--max-time",
            "3",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "http://localhost:8001/v1/models",
        ])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "200")
        // alt: reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(5)).build()...
        .unwrap_or(false);

    json!({
        "check": "model endpoint (localhost:8001)",
        "status": if reachable { "ok" } else { "fail" },
        "detail": if reachable {
            "reachable".to_string()
        } else {
            "not reachable (is vllm/ch0nky running?)".to_string()
        }
    })
}

/// Check 7: .b00t/fsl/ directory exists for FSL examples
fn check_fsl_dir(fix: bool) -> Value {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
    let path = home.join(".b00t").join("fsl");
    let expanded = shellexpand::tilde(&path.to_string_lossy()).to_string();
    let path = PathBuf::from(&expanded);

    let exists = path.exists();
    if !exists && fix {
        let _ = std::fs::create_dir_all(&path);
    }

    let exists_now = path.exists();
    json!({
        "check": ".b00t/fsl/ directory",
        "status": if exists_now { "ok" } else { "fail" },
        "detail": if exists_now {
            format!("exists at {}", path.display())
        } else {
            "not found".to_string()
        }
    })
}
