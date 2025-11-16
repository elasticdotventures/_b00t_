use crate::datum_utils;
use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
pub enum DatumCommands {
    #[clap(about = "Show comprehensive datum information")]
    Show {
        #[clap(help = "Datum name to show (e.g., just, rust, docker)")]
        name: String,
    },
}

pub fn handle_datum_command(path: &str, datum_command: &DatumCommands) -> Result<()> {
    match datum_command {
        DatumCommands::Show { name } => handle_show(path, name),
    }
}

fn handle_show(b00t_path: &str, datum_name: &str) -> Result<()> {
    // Find the datum
    let datum = datum_utils::find_datum_by_pattern(b00t_path, datum_name)?
        .ok_or_else(|| anyhow::anyhow!("Datum '{}' not found", datum_name))?;

    println!("# Datum: {}", datum.name);
    println!();

    // Basic info
    println!(
        "**Type:** {:?}",
        datum
            .datum_type
            .as_ref()
            .unwrap_or(&b00t_cli::DatumType::Unknown)
    );
    println!("**Hint:** {}", datum.hint);
    println!();

    // LFMF category
    if let Some(category) = &datum.lfmf_category {
        println!("**LFMF Category:** {}", category);
        println!();
    }

    // Learn content
    if let Some(learn_meta) = &datum.learn {
        println!("## Learn");
        println!();
        if let Some(topic) = &learn_meta.topic {
            println!("**Topic:** {}", topic);

            // Try to load and display learn content
            if let Ok(Some(content)) = datum_utils::get_datum_learn_content(b00t_path, &datum) {
                println!();
                println!("---");
                println!();
                // Display first 20 lines of content
                let lines: Vec<&str> = content.lines().take(20).collect();
                println!("{}", lines.join("\n"));
                if content.lines().count() > 20 {
                    println!();
                    println!("... ({} more lines)", content.lines().count() - 20);
                }
            }
        } else if let Some(inline) = &learn_meta.inline {
            println!("{}", inline);
        }
        println!();
    }

    // Usage examples
    if let Some(usage_examples) = &datum.usage {
        println!("## Usage Examples");
        println!();
        for (idx, example) in usage_examples.iter().enumerate() {
            println!("{}. **{}**", idx + 1, example.description);
            println!("   ```bash");
            println!("   {}", example.command);
            println!("   ```");
            if let Some(output) = &example.output {
                println!("   Output:");
                println!("   ```");
                println!("   {}", output);
                println!("   ```");
            }
            println!();
        }
    }

    // Dependencies
    if let Some(deps) = &datum.depends_on {
        println!("## Dependencies");
        println!();
        for dep in deps {
            println!("- {}", dep);
        }
        println!();
    }

    // Environment variables
    if let Some(env) = &datum.env {
        println!("## Environment Variables");
        println!();
        for (key, value) in env {
            println!("- `{}`: {}", key, value);
        }
        println!();
    }

    // Installation
    if let Some(install) = &datum.install {
        println!("## Installation");
        println!();
        println!("```bash");
        println!("{}", install);
        println!("```");
        println!();
    }

    // Version info
    if let Some(version_cmd) = &datum.version {
        println!("## Version Check");
        println!();
        println!("```bash");
        println!("{}", version_cmd);
        println!("```");
        if let Some(desired) = &datum.desires {
            println!("Desired version: {}", desired);
        }
        println!();
    }

    // LFMF lessons if category is set
    if let Some(category) = &datum.lfmf_category {
        println!("## LFMF Lessons ({})", category);
        println!();
        println!("View lessons with: `b00t advice {} list`", category);
        println!(
            "Record lessons with: `b00t lfmf {} \"<topic>: <solution>\"`",
            category
        );
        println!();
    }

    Ok(())
}
