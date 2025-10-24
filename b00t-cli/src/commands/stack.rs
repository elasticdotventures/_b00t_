use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashMap;
use std::path::Path;

use b00t_cli::datum_stack::StackDatum;
use b00t_cli::{BootDatum, get_expanded_path, UnifiedConfig};

#[derive(Parser)]
pub enum StackCommands {
    #[clap(
        about = "List all available software stacks",
        long_about = "List all software stacks defined in the _b00t_ directory.\n\nExamples:\n  b00t-cli stack list\n  b00t-cli stack list --json"
    )]
    List {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(
        about = "Show details about a specific stack",
        long_about = "Display detailed information about a software stack including members and dependencies.\n\nExamples:\n  b00t-cli stack show postgres-dev-stack\n  b00t-cli stack show ai-dev-stack --json"
    )]
    Show {
        #[clap(help = "Stack name")]
        name: String,
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(
        about = "Validate stack configuration and dependencies",
        long_about = "Validate that all stack members exist and dependencies are resolvable.\n\nExamples:\n  b00t-cli stack validate postgres-dev-stack\n  b00t-cli stack validate --all"
    )]
    Validate {
        #[clap(help = "Stack name (or use --all)")]
        name: Option<String>,
        #[clap(long, help = "Validate all stacks")]
        all: bool,
    },
    #[clap(
        about = "Install a software stack",
        long_about = "Install all members of a software stack in dependency order.\n\nExamples:\n  b00t-cli stack install postgres-dev-stack\n  b00t-cli stack install ai-dev-stack --dry-run"
    )]
    Install {
        #[clap(help = "Stack name")]
        name: String,
        #[clap(long, help = "Show what would be installed without installing")]
        dry_run: bool,
    },
    #[clap(
        about = "Generate docker-compose.yml from stack",
        long_about = "Generate a docker-compose.yml file from a Docker-based stack.\n\nExamples:\n  b00t-cli stack compose postgres-dev-stack\n  b00t-cli stack compose postgres-dev-stack --output docker-compose.yml"
    )]
    Compose {
        #[clap(help = "Stack name")]
        name: String,
        #[clap(long, short = 'o', help = "Output file (default: stdout)")]
        output: Option<String>,
    },
}

impl StackCommands {
    pub fn execute(&self, path: &str) -> Result<()> {
        match self {
            StackCommands::List { json } => list_stacks(path, *json),
            StackCommands::Show { name, json } => show_stack(name, path, *json),
            StackCommands::Validate { name, all } => {
                if *all {
                    validate_all_stacks(path)
                } else if let Some(stack_name) = name {
                    validate_stack(stack_name, path)
                } else {
                    anyhow::bail!("Must provide stack name or --all flag");
                }
            }
            StackCommands::Install { name, dry_run } => install_stack(name, path, *dry_run),
            StackCommands::Compose { name, output } => {
                generate_compose(name, path, output.as_deref())
            }
        }
    }
}

/// List all available stacks
fn list_stacks(path: &str, json_output: bool) -> Result<()> {
    let b00t_dir = get_expanded_path(path)?;
    let stack_paths = StackDatum::list_stacks(&b00t_dir)?;

    if stack_paths.is_empty() {
        if !json_output {
            println!("No stacks found in {}", b00t_dir.display());
            println!("Create a stack with a .stack.toml file in the _b00t_ directory");
        }
        return Ok(());
    }

    if json_output {
        let stacks: Vec<_> = stack_paths
            .iter()
            .filter_map(|path| {
                StackDatum::from_file(path).ok().map(|s| {
                    serde_json::json!({
                        "name": s.datum.name,
                        "hint": s.datum.hint,
                        "members": s.get_members(),
                        "path": path.display().to_string(),
                    })
                })
            })
            .collect();

        println!("{}", serde_json::to_string_pretty(&stacks)?);
    } else {
        println!("📦 Available Stacks in {}:\n", b00t_dir.display());

        for stack_path in stack_paths {
            match StackDatum::from_file(&stack_path) {
                Ok(stack) => {
                    println!("  {}", stack.get_summary());
                }
                Err(e) => {
                    eprintln!(
                        "  ❌ {} (error: {})",
                        stack_path.display(),
                        e
                    );
                }
            }
        }
        println!();
        println!("Use 'b00t-cli stack show <name>' for details");
        println!("Use 'b00t-cli stack install <name>' to install");
    }

    Ok(())
}

/// Show detailed information about a stack
fn show_stack(name: &str, path: &str, json_output: bool) -> Result<()> {
    let stack_path = get_expanded_path(path)?.join(format!("{}.stack.toml", name));

    if !stack_path.exists() {
        anyhow::bail!("Stack '{}' not found at {}", name, stack_path.display());
    }

    let stack = StackDatum::from_file(&stack_path)?;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&stack.datum)?);
    } else {
        println!("📦 Stack: {}", stack.datum.name);
        println!("   Hint: {}", stack.datum.hint);
        println!("\n📋 Members ({}):", stack.get_members().len());
        for member in stack.get_members() {
            println!("   - {}", member);
        }

        if let Some(env) = &stack.datum.env {
            println!("\n🔧 Environment Variables:");
            for (key, value) in env {
                println!("   {}={}", key, value);
            }
        }

        if let Some(deps) = &stack.datum.depends_on {
            if !deps.is_empty() {
                println!("\n⚙️  Dependencies:");
                for dep in deps {
                    println!("   - {}", dep);
                }
            }
        }

        println!("\n📍 Path: {}", stack_path.display());
    }

    Ok(())
}

/// Validate a single stack
fn validate_stack(name: &str, path: &str) -> Result<()> {
    let stack_path = get_expanded_path(path)?.join(format!("{}.stack.toml", name));

    if !stack_path.exists() {
        anyhow::bail!("Stack '{}' not found at {}", name, stack_path.display());
    }

    let stack = StackDatum::from_file(&stack_path)?;

    println!("🔍 Validating stack '{}'...", name);

    // Load all available datums
    let available_datums = load_all_datums(path)?;

    // Validate members exist
    let errors = stack.validate_members(&available_datums)?;

    if errors.is_empty() {
        println!("✅ Stack '{}' is valid", name);
        println!("   {} members found", stack.get_members().len());
        Ok(())
    } else {
        println!("❌ Stack '{}' has validation errors:", name);
        for error in errors {
            println!("   - {}", error);
        }
        anyhow::bail!("Stack validation failed");
    }
}

/// Validate all stacks
fn validate_all_stacks(path: &str) -> Result<()> {
    let b00t_dir = get_expanded_path(path)?;
    let stack_paths = StackDatum::list_stacks(&b00t_dir)?;

    if stack_paths.is_empty() {
        println!("No stacks found in {}", b00t_dir.display());
        return Ok(());
    }

    println!("🔍 Validating {} stacks...\n", stack_paths.len());

    let available_datums = load_all_datums(path)?;
    let mut total_errors = 0;

    for stack_path in stack_paths {
        match StackDatum::from_file(&stack_path) {
            Ok(stack) => {
                let errors = stack.validate_members(&available_datums)?;
                if errors.is_empty() {
                    println!("✅ {} - valid", stack.datum.name);
                } else {
                    println!("❌ {} - {} errors:", stack.datum.name, errors.len());
                    for error in &errors {
                        println!("     {}", error);
                    }
                    total_errors += errors.len();
                }
            }
            Err(e) => {
                println!("❌ {} - parse error: {}", stack_path.display(), e);
                total_errors += 1;
            }
        }
    }

    println!();
    if total_errors == 0 {
        println!("✅ All stacks are valid");
        Ok(())
    } else {
        anyhow::bail!("{} validation errors found", total_errors);
    }
}

/// Install a stack (placeholder - actual implementation would install members)
fn install_stack(name: &str, path: &str, dry_run: bool) -> Result<()> {
    let stack_path = get_expanded_path(path)?.join(format!("{}.stack.toml", name));

    if !stack_path.exists() {
        anyhow::bail!("Stack '{}' not found at {}", name, stack_path.display());
    }

    let stack = StackDatum::from_file(&stack_path)?;

    // Load available datums to build dependency resolver
    let available_datums = load_all_datums(path)?;

    // Build resolver with borrowed references
    let datum_refs: HashMap<String, &BootDatum> = available_datums
        .iter()
        .map(|(k, v)| (k.clone(), v))
        .collect();

    // Resolve installation order
    let install_order = stack.resolve_dependencies(&datum_refs)?;

    if dry_run {
        println!("🔍 Dry run: Would install {} in this order:", name);
        for (idx, member) in install_order.iter().enumerate() {
            println!("   {}. {}", idx + 1, member);
        }
        return Ok(());
    }

    println!("📦 Installing stack '{}'...", name);
    println!("   Installation order ({} items):", install_order.len());

    for (idx, member) in install_order.iter().enumerate() {
        println!("   {}. {}", idx + 1, member);
    }

    println!("\n⚠️  Actual installation not yet implemented");
    println!("   This would install each member using their install commands");

    Ok(())
}

/// Generate docker-compose.yml from stack
fn generate_compose(name: &str, path: &str, output_file: Option<&str>) -> Result<()> {
    let stack_path = get_expanded_path(path)?.join(format!("{}.stack.toml", name));

    if !stack_path.exists() {
        anyhow::bail!("Stack '{}' not found at {}", name, stack_path.display());
    }

    let stack = StackDatum::from_file(&stack_path)?;
    let available_datums = load_all_datums(path)?;

    let compose_yaml = stack.generate_docker_compose(&available_datums)?;

    if let Some(output_path) = output_file {
        std::fs::write(output_path, &compose_yaml)
            .context(format!("Failed to write to {}", output_path))?;
        println!("✅ Generated docker-compose.yml: {}", output_path);
    } else {
        println!("{}", compose_yaml);
    }

    Ok(())
}

/// Helper: Load all datums from _b00t_ directory
fn load_all_datums(path: &str) -> Result<HashMap<String, BootDatum>> {
    let mut datums = HashMap::new();
    let b00t_dir = get_expanded_path(path)?;

    if !b00t_dir.exists() {
        return Ok(datums);
    }

    for entry in std::fs::read_dir(&b00t_dir)? {
        let entry = entry?;
        let entry_path = entry.path();

        if entry_path.is_file() {
            if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                // Skip stack files themselves
                if file_name.ends_with(".stack.toml") {
                    continue;
                }

                // Load other datum types
                if file_name.ends_with(".toml") {
                    if let Ok(content) = std::fs::read_to_string(&entry_path) {
                        if let Ok(config) = toml::from_str::<crate::UnifiedConfig>(&content) {
                            let datum = config.b00t;
                            let datum_type = datum.datum_type.as_ref()
                                .map(|t| format!("{:?}", t).to_lowercase())
                                .unwrap_or_else(|| "unknown".to_string());
                            let key = format!("{}.{}", datum.name, datum_type);
                            datums.insert(key, datum);
                        }
                    }
                }
            }
        }
    }

    Ok(datums)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stack_commands_exist() {
        // Test list command
        let list_cmd = StackCommands::List { json: false };
        let result = list_cmd.execute("/tmp/nonexistent");
        assert!(result.is_ok()); // Should handle non-existent dir gracefully

        // Test validate command
        let validate_cmd = StackCommands::Validate {
            name: None,
            all: false,
        };
        let result = validate_cmd.execute("/tmp/nonexistent");
        assert!(result.is_err()); // Should error when no name or --all provided
    }
}
