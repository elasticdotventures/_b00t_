//! `b00t is <name>` — stateful system-normal checklist gate.
//! Phase 1 of CONOPS-system-normal.md: no persistence, implicit-AND only.

use crate::checklist::{ChecklistDisposition, ChecklistFile, list_checklists};
use anyhow::{Result, bail};

pub fn execute(path: &str, name: Option<&str>, as_json: bool, explain: bool) -> Result<()> {
    let base = shellexpand::tilde(path).to_string();
    let base_dir = std::path::Path::new(&base);

    let Some(name) = name else {
        let found = list_checklists(base_dir);
        if as_json {
            println!("{}", serde_json::to_string_pretty(&found)?);
        } else if found.is_empty() {
            println!("No checklists found in {}", base_dir.display());
        } else {
            println!("Available checklists in {}:", base_dir.display());
            for f in &found {
                println!("  {f}");
            }
        }
        return Ok(());
    };

    let file_path = base_dir.join(format!("{name}.checklist.toml"));
    if !file_path.exists() {
        bail!(
            "checklist not found: {} (looked for {})",
            name,
            file_path.display()
        );
    }

    let checklist = ChecklistFile::load(&file_path)?;
    let result = checklist.evaluate(&base);
    let exit_code = result.disposition.exit_code();

    if as_json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        for outcome in &result.outcomes {
            let icon = match outcome.status {
                "satisfied" => "✅",
                "violated" => "❌",
                _ => "❓",
            };
            println!("  {icon} {}", outcome.id);
            if explain {
                if let Some(reason) = &outcome.reason {
                    println!("      ↳ {reason}");
                }
            }
        }
        match &result.disposition {
            ChecklistDisposition::Satisfied => {
                println!("✅ {}: system normal", result.name);
            }
            ChecklistDisposition::Violated { failing } => {
                println!("❌ {}: {} check(s) failed", result.name, failing.len());
                if explain {
                    for f in failing {
                        println!("   - {f}");
                    }
                }
            }
            ChecklistDisposition::Unknown { undetermined } => {
                println!(
                    "❓ {}: {} check(s) undetermined",
                    result.name,
                    undetermined.len()
                );
                if explain {
                    for u in undetermined {
                        println!("   - {u}");
                    }
                }
            }
        }
    }

    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}
