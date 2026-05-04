//! `b00t doctor` — system diagnostics for b00t infrastructure
//!
//! # Usage
//! ```bash
//! b00t-cli doctor check                  # run diagnostics
//! b00t-cli doctor check --fix            # create missing directories
//! b00t-cli doctor check --json           # emit JSON report
//! ```

use anyhow::Result;
use clap::Parser;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;

#[derive(Parser, Clone)]
pub enum DoctorCommands {
    #[clap(about = "Run system diagnostics")]
    Check {
        #[clap(long, help = "Fix issues when possible")]
        fix: bool,
        #[clap(long, help = "Emit JSON report")]
        json: bool,
    },
}

pub fn handle_doctor_command(args: &DoctorCommands, b00t_path: &str) -> Result<()> {
    match args {
        DoctorCommands::Check { fix, json } => {
            let mut results: Vec<Value> = Vec::new();

            // 1. b00t-cli binary exists and version
            results.push(check_b00t_cli());

            // 2. _b00t_ directory exists with datums
            results.push(check_b00t_dir(b00t_path, *fix));

            // 3. .opencode/ directory exists with skills
            results.push(check_opencode_dir(*fix));

            // 4. Focus schema datum exists (_b00t_/focus.schema.tomllmd)
            results.push(check_focus_schema(b00t_path));

            // 5. ledgrrr-mcp service status
            results.push(check_ledgrrr_service());

            // 6. Local model endpoint reachable
            results.push(check_model_endpoint());

            // 7. .b00t/fsl/ directory exists for FSL examples
            results.push(check_fsl_dir(*fix));

            if *json {
                println!("{}", serde_json::to_string_pretty(&results)?);
            } else {
                println!("🥾 b00t doctor — system diagnostics\n");
                let ok_count = results.iter().filter(|r| r["status"] == "ok").count();
                for r in &results {
                    let ok = r["status"] == "ok";
                    let icon = if ok { "✅" } else { "❌" };
                    let name = r["check"].as_str().unwrap_or("");
                    println!("{}  {}", icon, name);
                    if let Some(detail) = r["detail"].as_str() {
                        if !detail.is_empty() {
                            println!("       {}", detail);
                        }
                    }
                }
                println!(
                    "\n{}/{} checks passed",
                    ok_count,
                    results.len()
                );
            }

            Ok(())
        }
    }
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

/// Check 5: ledgrrr-mcp service status
fn check_ledgrrr_service() -> Value {
    let output = Command::new("systemctl")
        .args(["--user", "is-active", "ledgrrr-mcp"])
        .output();

    match output {
        Ok(o) => {
            let status = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let active = status == "active" || o.status.success();
            json!({
                "check": "ledgrrr-mcp service",
                "status": if active { "ok" } else { "fail" },
                "detail": if active { "active".to_string() } else { status }
            })
        }
        Err(e) => {
            json!({
                "check": "ledgrrr-mcp service",
                "status": "fail",
                "detail": format!("systemctl not available: {}", e)
            })
        }
    }
}

/// Check 6: Local model endpoint reachable
fn check_model_endpoint() -> Value {
    // Use curl instead of reqwest::blocking to avoid tokio runtime panic (#[tokio::main])
    let reachable = std::process::Command::new("curl")
        .args(["-s", "--max-time", "3", "-o", "/dev/null", "-w", "%{http_code}", "http://localhost:8001/v1/models"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "200")
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
