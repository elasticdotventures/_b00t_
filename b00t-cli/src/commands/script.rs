//! RHAI script execution commands for b00t-cli

use anyhow::{Context, Result};
use b00t_c0re_lib::{B00tContext, RhaiEngine};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Clone)]
pub enum ScriptCommands {
    #[clap(about = "Run a RHAI script")]
    Run {
        #[clap(help = "Path to RHAI script file or script name from scripts directory")]
        script: String,
        #[clap(help = "Arguments to pass to the script", num_args = 0..)]
        args: Vec<String>,
    },
    #[clap(about = "List available RHAI scripts")]
    List,
    #[clap(about = "Evaluate RHAI code directly")]
    Eval {
        #[clap(help = "RHAI code to evaluate")]
        code: String,
    },
}

pub fn handle_script_command(script_command: ScriptCommands) -> Result<()> {
    let context = B00tContext::current()?;
    let engine = RhaiEngine::new(context)?;

    match script_command {
        ScriptCommands::Run { script, args: _ } => {
            run_script(&engine, &script)?;
        }
        ScriptCommands::List => {
            list_scripts(&engine)?;
        }
        ScriptCommands::Eval { code } => {
            eval_code(&engine, &code)?;
        }
    }

    Ok(())
}

fn run_script(engine: &RhaiEngine, script_path: &str) -> Result<()> {
    let path = resolve_script_path(engine, script_path)?;

    println!("🚀 Executing RHAI script: {}", path.display());

    let result = engine
        .execute_file(&path)
        .with_context(|| format!("Failed to execute script: {}", path.display()))?;

    // Print result if it's not empty/unit
    if !result.is_unit() {
        println!("📤 Script result: {:?}", result);
    }

    println!("✅ Script execution completed");
    Ok(())
}

fn list_scripts(engine: &RhaiEngine) -> Result<()> {
    let scripts = engine.list_scripts()?;

    if scripts.is_empty() {
        println!(
            "📁 No RHAI scripts found in: {}",
            engine.scripts_dir().display()
        );
        println!("💡 Create scripts in: ~/.dotfiles/_b00t_/scripts/");
        return Ok(());
    }

    println!("📋 Available RHAI scripts:");
    for script in scripts {
        let name = script
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let path = script.strip_prefix(engine.scripts_dir()).unwrap_or(&script);
        println!("  • {} ({})", name, path.display());
    }

    Ok(())
}

fn eval_code(engine: &RhaiEngine, code: &str) -> Result<()> {
    println!("🔍 Evaluating RHAI code...");

    let result = engine
        .execute_script(code)
        .with_context(|| "Failed to evaluate RHAI code")?;

    println!("📤 Result: {:?}", result);
    Ok(())
}

fn resolve_script_path(engine: &RhaiEngine, script_path: &str) -> Result<PathBuf> {
    let path = PathBuf::from(script_path);

    // If it's an absolute path or contains path separators, use as-is
    if path.is_absolute() || script_path.contains('/') {
        if path.exists() {
            return Ok(path);
        } else {
            return Err(anyhow::anyhow!("Script file not found: {}", script_path));
        }
    }

    // Otherwise, look in the scripts directory
    let scripts_dir = engine.scripts_dir();

    // Try with .rhai extension if not provided
    let script_with_ext = if path.extension().is_none() {
        format!("{}.rhai", script_path)
    } else {
        script_path.to_string()
    };

    let script_in_dir = scripts_dir.join(&script_with_ext);
    if script_in_dir.exists() {
        return Ok(script_in_dir);
    }

    // Try without extension in scripts dir
    let script_no_ext = scripts_dir.join(script_path);
    if script_no_ext.exists() {
        return Ok(script_no_ext);
    }

    Err(anyhow::anyhow!(
        "Script not found: {} (looked in {} and current directory)",
        script_path,
        scripts_dir.display()
    ))
}
