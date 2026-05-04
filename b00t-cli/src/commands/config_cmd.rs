//! `b00t config` — configuration inspection and environment variable emission.
//!
//! Subcommands:
//! - `env`: Emit shell env vars for model endpoints and known paths.

use anyhow::Result;
use clap::Parser;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Clone)]
pub enum ConfigCommands {
    #[clap(about = "Emit shell env vars for model endpoints and paths")]
    Env {
        #[clap(long, help = "Output as JSON instead of shell exports")]
        json: bool,
        #[clap(long, help = "Output for direnv (.envrc format)")]
        direnv: bool,
    },
    #[clap(about = "Scaffold a new b00t project in the current directory")]
    Init {
        #[clap(long, help = "Project directory (default: current dir)")]
        dir: Option<PathBuf>,
        #[clap(long, help = "Force overwrite existing files")]
        force: bool,
    },
}

pub async fn handle_config_command(cmd: &ConfigCommands, path: &str) -> Result<()> {
    match cmd {
        ConfigCommands::Env { json, direnv } => handle_env(path, *json, *direnv).await,
        ConfigCommands::Init { dir, force } => handle_init(dir.clone(), *force).await,
    }
}

async fn handle_env(path: &str, json_output: bool, direnv_mode: bool) -> Result<()> {
    let mut vars: BTreeMap<String, String> = BTreeMap::new();

    // 1. Active model endpoints from `b00t model served`
    match crate::model_manager::list_served_models(path).await {
        Ok(endpoints) => {
            for ep in &endpoints {
                let label = ep
                    .source_models
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string())
                    .to_uppercase();
                vars.insert(format!("MODEL_ENDPOINT_{}", label), ep.base_url.clone());
                if let Some(port) = ep.port {
                    vars.insert(format!("MODEL_{}_PORT", label), port.to_string());
                }
            }
        }
        Err(e) => {
            eprintln!("[config env] warning: cannot query model endpoints: {e}");
        }
    }

    // 2. Known env vars — emit if set in the environment
    for key in &["OPENCODE_MODEL", "B00T_ROLE"] {
        if let Ok(val) = std::env::var(key) {
            vars.insert(key.to_string(), val);
        }
    }

    // 3. Always emit _B00T_PATH (default to the cli --path / env var)
    let b00t_path = std::env::var("_B00T_PATH")
        .unwrap_or_else(|_| path.to_string());
    vars.insert("_B00T_PATH".to_string(), b00t_path);

    // 4. Emit
    if json_output {
        println!("{}", serde_json::to_string_pretty(&vars)?);
    } else {
        let separator = if direnv_mode { "" } else { "export " };
        for (key, value) in &vars {
            println!("{}{}={}", separator, key, shell_quote(value));
        }
    }

    Ok(())
}

/// Scaffold a new b00t project in the specified directory.
async fn handle_init(dir: Option<PathBuf>, force: bool) -> Result<()> {
    let project_dir = dir
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    fs::create_dir_all(&project_dir)?;

    let mut created: Vec<String> = Vec::new();

    // 1. _b00t_/ dir with .gitkeep
    let b00t_dir = project_dir.join("_b00t_");
    if !b00t_dir.exists() || force {
        fs::create_dir_all(&b00t_dir)?;
        let gitkeep = b00t_dir.join(".gitkeep");
        if !gitkeep.exists() || force {
            fs::write(&gitkeep, "")?;
        }
        created.push("_b00t_/".to_string());
    }

    // 2. .opencode/skills/ dir with .gitkeep
    let skills_dir = project_dir.join(".opencode").join("skills");
    if !skills_dir.exists() || force {
        fs::create_dir_all(&skills_dir)?;
        let gitkeep = skills_dir.join(".gitkeep");
        if !gitkeep.exists() || force {
            fs::write(&gitkeep, "")?;
        }
        created.push(".opencode/skills/".to_string());
    }

    // 3. .opencode/context/ dir
    let context_dir = project_dir.join(".opencode").join("context");
    if !context_dir.exists() || force {
        fs::create_dir_all(&context_dir)?;
        created.push(".opencode/context/".to_string());
    }

    // 4. AGENTS.md with basic b00t header
    let agents_path = project_dir.join("AGENTS.md");
    if !agents_path.exists() || force {
        let agents_content = r#"# 🍰 b00t Agent Configuration

This file provides guidance to b00t agents working in this project.

## Overview

This project uses the b00t framework for agentic development workflows.
Refer to the main AGENTS.md in the dotfiles repository for full protocol documentation.

## Project-Specific Instructions

- Follow existing code patterns and conventions
- Write tests for all new functionality
- Use the justfile for common tasks
"#;
        fs::write(&agents_path, agents_content)?;
        created.push("AGENTS.md".to_string());
    }

    // 5. justfile with basic test recipe
    let just_path = project_dir.join("justfile");
    if !just_path.exists() || force {
        let just_content = r#"# b00t project justfile
# Common tasks

test:
    cargo test

build:
    cargo build

check:
    cargo check

lint:
    cargo clippy

clean:
    cargo clean
"#;
        fs::write(&just_path, just_content)?;
        created.push("justfile".to_string());
    }

    // Summary
    println!("✅ Created b00t project scaffold in {}", project_dir.display());
    for item in &created {
        println!("  📁 {}", item);
    }

    Ok(())
}

/// Simple Bourne-shell quoting: if the value contains only safe chars,
/// emit it bare; otherwise single-quote it, handling embedded single quotes
/// via the `'\''` idiom.
fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./:@".contains(c))
    {
        value.to_string()
    } else {
        let mut quoted = String::from("'");
        for ch in value.chars() {
            if ch == '\'' {
                quoted.push_str("'\"'\"'");
            } else {
                quoted.push(ch);
            }
        }
        quoted.push('\'');
        quoted
    }
}
