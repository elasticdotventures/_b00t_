use crate::datum_justfile::JustfileDatum;
use crate::datum_utils::get_all_datums_with_paths;
use crate::just_ast::JustfileAst;
use crate::traits::{CliExecutor, DatumChecker};
use crate::{BootDatum, DatumType, get_expanded_path};
use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, ValueEnum)]
pub enum JustfileRegistryFormat {
    Lines,
    Args,
    Json,
}

#[derive(Debug, Subcommand)]
pub enum JustfileCommands {
    #[clap(about = "List registered justfile datums and recipe counts")]
    List {
        #[clap(long, help = "Emit structured JSON")]
        json: bool,
    },
    #[clap(about = "Query recipes for a registered justfile datum")]
    Query {
        #[clap(help = "Justfile datum name")]
        name: String,
        #[clap(long, help = "Filter recipe names/docs/dependencies by substring")]
        recipe: Option<String>,
        #[clap(long, help = "Emit structured JSON")]
        json: bool,
    },
    #[clap(about = "Run just AST validation for a registered justfile datum")]
    Validate {
        #[clap(help = "Justfile datum name")]
        name: String,
    },
    #[clap(about = "Emit the just --dump AST for a registered justfile datum")]
    Ast {
        #[clap(help = "Justfile datum name")]
        name: String,
    },
    #[clap(about = "Emit --allow arguments for strict just-mcp startup")]
    Registry {
        #[clap(value_enum, long, default_value = "args")]
        format: JustfileRegistryFormat,
    },
    #[clap(about = "Run a registered just recipe through JustfileDatum")]
    Run {
        #[clap(help = "Justfile datum name")]
        name: String,
        #[clap(help = "Recipe name and arguments")]
        args: Vec<String>,
    },
}

#[derive(Debug, Serialize)]
struct JustfileSummary {
    key: String,
    name: String,
    datum_file: String,
    justfile_path: String,
    mcp_server: Option<String>,
    recipe_groups: Vec<String>,
    recipe_count: usize,
    installed: bool,
    hint: String,
}

#[derive(Debug, Serialize)]
struct RecipeSummary {
    name: String,
    description: Option<String>,
    parameters: Vec<String>,
    dependencies: Vec<String>,
    private: bool,
}

pub fn discover_justfile_datums(b00t_path: &str) -> Result<Vec<(String, BootDatum, String)>> {
    let all = get_all_datums_with_paths(b00t_path, None)?;
    let mut justfiles: Vec<(String, BootDatum, String)> = all
        .into_iter()
        .filter(|(_, (datum, _))| datum.datum_type == Some(DatumType::Justfile))
        .map(|(key, (datum, file_path))| (key, datum, file_path))
        .collect();
    justfiles.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(justfiles)
}

pub fn discover_registered_justfile_paths(b00t_path: &str) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for (_, datum, file_path) in discover_justfile_datums(b00t_path)? {
        let base_dir = datum_project_base_dir(b00t_path, &file_path);
        let justfile = JustfileDatum::from_datum(datum, &base_dir)?;
        if justfile.justfile_path.exists() {
            paths.push(canonical_or_self(&justfile.justfile_path));
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

pub fn handle_justfile_command(cmd: &JustfileCommands, b00t_path: &str) -> Result<()> {
    match cmd {
        JustfileCommands::List { json } => {
            let summaries = discover_summaries(b00t_path)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&summaries)?);
            } else if summaries.is_empty() {
                println!("No justfile datums found.");
            } else {
                println!("Justfile datums:");
                for item in summaries {
                    println!(
                        "  {}  ({} recipes)  {}",
                        item.key, item.recipe_count, item.hint
                    );
                    println!("    {}", item.justfile_path);
                }
            }
        }
        JustfileCommands::Query { name, recipe, json } => {
            let justfile = resolve_justfile_datum(b00t_path, name)?;
            let needle = recipe.as_ref().map(|s| s.to_lowercase());
            let mut recipes: Vec<RecipeSummary> = justfile
                .list_commands()?
                .into_iter()
                .map(|cmd| RecipeSummary {
                    name: cmd.name,
                    description: cmd.description,
                    parameters: cmd
                        .parameters
                        .into_iter()
                        .map(|p| match p.default_value {
                            Some(default) => format!("{}={}", p.name, default),
                            None => p.name,
                        })
                        .collect(),
                    dependencies: cmd.dependencies,
                    private: cmd.private,
                })
                .filter(|r| {
                    let Some(needle) = &needle else {
                        return true;
                    };
                    r.name.to_lowercase().contains(needle)
                        || r.description
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(needle)
                        || r.dependencies
                            .iter()
                            .any(|d| d.to_lowercase().contains(needle))
                })
                .collect();
            recipes.sort_by(|a, b| a.name.cmp(&b.name));
            if *json {
                println!("{}", serde_json::to_string_pretty(&recipes)?);
            } else {
                for r in recipes {
                    let desc = r.description.unwrap_or_default();
                    println!("{}  {}", r.name, desc);
                }
            }
        }
        JustfileCommands::Validate { name } => {
            let justfile = resolve_justfile_datum(b00t_path, name)?;
            let ast = JustfileAst::load(&justfile.justfile_path)?;
            let warnings = ast.validate();
            if warnings.is_empty() {
                println!(
                    "PASS justfile validate: {} ({} recipes)",
                    justfile.justfile_path.display(),
                    ast.dump.recipes.len()
                );
            } else {
                println!(
                    "FAIL justfile validate: {}",
                    justfile.justfile_path.display()
                );
                for warning in warnings {
                    println!("{}", warning.trim());
                }
                std::process::exit(1);
            }
        }
        JustfileCommands::Ast { name } => {
            let justfile = resolve_justfile_datum(b00t_path, name)?;
            let ast = JustfileAst::load(&justfile.justfile_path)?;
            println!("{}", serde_json::to_string_pretty(&ast.dump)?);
        }
        JustfileCommands::Registry { format } => {
            let paths = discover_registered_justfile_paths(b00t_path)?;
            match format {
                JustfileRegistryFormat::Lines => {
                    for path in paths {
                        println!("{}", path.display());
                    }
                }
                JustfileRegistryFormat::Args => {
                    let args = paths
                        .iter()
                        .map(|p| format!("--allow {}", shell_quote(&p.display().to_string())))
                        .collect::<Vec<_>>()
                        .join(" ");
                    println!("{}", args);
                }
                JustfileRegistryFormat::Json => {
                    let paths = paths
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>();
                    println!("{}", serde_json::to_string_pretty(&paths)?);
                }
            }
        }
        JustfileCommands::Run { name, args } => {
            if args.is_empty() {
                anyhow::bail!("recipe name required");
            }
            let justfile = resolve_justfile_datum(b00t_path, name)?;
            let out = justfile.execute(args)?;
            print!("{}", out.value);
            if out.exit_code != 0 {
                std::process::exit(out.exit_code);
            }
        }
    }
    Ok(())
}

fn discover_summaries(b00t_path: &str) -> Result<Vec<JustfileSummary>> {
    let mut summaries = Vec::new();
    for (key, datum, file_path) in discover_justfile_datums(b00t_path)? {
        let base_dir = datum_project_base_dir(b00t_path, &file_path);
        let justfile = JustfileDatum::from_datum(datum.clone(), &base_dir)?;
        let recipe_count = justfile.list_commands().map(|cmds| cmds.len()).unwrap_or(0);
        let config = datum.justfile.clone().unwrap_or_default();
        summaries.push(JustfileSummary {
            key,
            name: datum.name.clone(),
            datum_file: file_path,
            justfile_path: justfile.justfile_path.display().to_string(),
            mcp_server: config.mcp_server,
            recipe_groups: config.recipe_groups.unwrap_or_default(),
            recipe_count,
            installed: justfile.is_installed(),
            hint: datum.hint.clone(),
        });
    }
    Ok(summaries)
}

fn resolve_justfile_datum(b00t_path: &str, name: &str) -> Result<JustfileDatum> {
    let justfiles = discover_justfile_datums(b00t_path)?;
    let (_, datum, file_path) = justfiles
        .into_iter()
        .find(|(key, datum, _)| key == name || datum.name == name)
        .ok_or_else(|| anyhow::anyhow!("no justfile datum named '{}'", name))?;
    let base_dir = datum_project_base_dir(b00t_path, &file_path);
    JustfileDatum::from_datum(datum, &base_dir)
}

fn datum_project_base_dir(b00t_path: &str, file_path: &str) -> PathBuf {
    match get_expanded_path(b00t_path) {
        Ok(path) if path.file_name().and_then(|s| s.to_str()) == Some("_b00t_") => path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf(),
        _ => datum_base_dir(file_path),
    }
}

fn datum_base_dir(file_path: &str) -> PathBuf {
    Path::new(file_path)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf()
}

fn canonical_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_registered_justfile(dir: &TempDir) {
        let b00t = dir.path().join("_b00t_");
        fs::create_dir_all(&b00t).unwrap();
        fs::write(dir.path().join("justfile"), "build:\n    echo build\n").unwrap();
        fs::write(
            b00t.join("demo.justfile.tomllm"),
            r#"[b00t]
name = "demo"
type = "justfile"
hint = "demo justfile"

[b00t.justfile]
path = "justfile"
mcp_server = "just-mcp"
recipe_groups = ["ci"]
"#,
        )
        .unwrap();
    }

    #[test]
    fn discover_registered_paths_comes_from_datums() {
        let dir = TempDir::new().unwrap();
        write_registered_justfile(&dir);
        let paths = discover_registered_justfile_paths(dir.path().join("_b00t_").to_str().unwrap())
            .unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths[0].ends_with("justfile"));
    }

    #[test]
    fn discover_justfile_datums_filters_by_type() {
        let dir = TempDir::new().unwrap();
        write_registered_justfile(&dir);
        let datums = discover_justfile_datums(dir.path().join("_b00t_").to_str().unwrap()).unwrap();
        assert_eq!(datums.len(), 1);
        assert_eq!(datums[0].1.name, "demo");
    }
}
