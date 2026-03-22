use crate::datum_utils::{self, DatumFilter};
use anyhow::Result;
use clap::Parser;
use std::collections::HashMap;

#[derive(Parser, Debug)]
pub enum DatumCommands {
    #[clap(about = "Show comprehensive datum information")]
    Show {
        #[clap(help = "Datum name to show (e.g., just, rust, docker)")]
        name: String,
    },

    #[clap(about = "Generate JSTree-compatible JSON from datums")]
    Tree {
        #[clap(long, help = "Output file (default: stdout)")]
        output: Option<String>,

        #[clap(long, help = "Group by datum type")]
        group_by_type: bool,

        #[clap(long, help = "Include only specific types (comma-separated)")]
        types: Option<String>,
    },

    #[clap(about = "Search datums by regex/literal pattern (#198)")]
    Search {
        #[clap(help = "Pattern (regex or literal substring)")]
        pattern: String,

        #[clap(long, help = "Filter by type (cli, mcp, ai, k8s, …)")]
        types: Option<String>,

        #[clap(long, help = "Output format: table|json", default_value = "table")]
        format: String,

        #[clap(long, help = "Max recursion depth")]
        depth: Option<usize>,
    },

    #[clap(about = "Filter datums by constraints + availability (#199)")]
    Filter {
        #[clap(long, help = "Only datums with all env vars set (any)")]
        needs_any_env: bool,

        #[clap(long, help = "Only datums with all env vars set (all)")]
        needs_all_env: bool,

        #[clap(
            long,
            help = "Require OS or CMD: OS:linux, CMD:docker",
            value_name = "CONSTRAINT"
        )]
        require: Vec<String>,

        #[clap(long, help = "Filter by type (cli, mcp, ai, k8s, …)")]
        types: Option<String>,

        #[clap(long, help = "Show why datums were filtered out")]
        explain: bool,

        #[clap(long, help = "Output format: table|json", default_value = "table")]
        format: String,
    },

    #[clap(about = "Export datum ontology as DOT or JSON graph (#201)")]
    Graph {
        #[clap(long, help = "Output format: dot|json", default_value = "dot", value_parser = ["dot", "json"])]
        format: String,

        #[clap(long, help = "Output file (default: stdout)")]
        output: Option<String>,

        #[clap(long, help = "Filter by type (cli, mcp, ai, k8s, …)")]
        types: Option<String>,

        #[clap(long, help = "Max recursion depth")]
        depth: Option<usize>,
    },

    #[clap(about = "Query datum graph neighbors (#201)")]
    Neighbors {
        #[clap(help = "Datum key (e.g., rust.cli)")]
        datum: String,

        #[clap(long, help = "BFS depth", default_value = "1")]
        depth: usize,

        #[clap(long, help = "Edge direction: in|out|both", default_value = "both", value_parser = ["in", "out", "both"])]
        direction: String,
    },
}

pub fn handle_datum_command(path: &str, datum_command: &DatumCommands) -> Result<()> {
    match datum_command {
        DatumCommands::Show { name } => handle_show(path, name),
        DatumCommands::Tree {
            output,
            group_by_type,
            types,
        } => handle_tree(path, output.as_deref(), *group_by_type, types.as_deref()),
        DatumCommands::Search {
            pattern,
            types,
            format,
            depth,
        } => handle_search(path, pattern, types.as_deref(), format, *depth),
        DatumCommands::Filter {
            needs_any_env,
            needs_all_env,
            require,
            types,
            explain,
            format,
        } => handle_filter(
            path,
            *needs_any_env,
            *needs_all_env,
            require,
            types.as_deref(),
            *explain,
            format,
        ),
        DatumCommands::Graph {
            format,
            output,
            types,
            depth,
        } => handle_graph(path, format, output.as_deref(), types.as_deref(), *depth),
        DatumCommands::Neighbors {
            datum,
            depth,
            direction,
        } => handle_neighbors(path, datum, *depth, direction),
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
            .unwrap_or(&crate::DatumType::Unknown)
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
        println!("View lessons with: `b00t learn {} --search list`", category);
        println!(
            "Record lessons with: `b00t learn {} --record \"<topic>: <solution>\"`",
            category
        );
        println!();
    }

    Ok(())
}

fn handle_tree(
    b00t_path: &str,
    output_file: Option<&str>,
    group_by_type: bool,
    types_filter: Option<&str>,
) -> Result<()> {
    // Load all datums
    let datums = datum_utils::get_all_datums(b00t_path)?;

    // Filter by types if specified
    let filtered_datums: HashMap<String, crate::BootDatum> = if let Some(types_str) = types_filter {
        let types: Vec<String> = types_str
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .collect();
        datums
            .into_iter()
            .filter(|(_, datum)| {
                if let Some(dtype) = &datum.datum_type {
                    let dtype_str = format!("{:?}", dtype).to_lowercase();
                    types.iter().any(|t| dtype_str.contains(t))
                } else {
                    false
                }
            })
            .collect()
    } else {
        datums
    };

    // Generate JSTree structure
    let tree_data = if group_by_type {
        generate_grouped_tree(&filtered_datums)?
    } else {
        generate_flat_tree(&filtered_datums)?
    };

    let json_output = serde_json::to_string_pretty(&tree_data)?;

    // Output to file or stdout
    if let Some(output_path) = output_file {
        std::fs::write(output_path, json_output)?;
        println!("✅ JSTree JSON written to: {}", output_path);
    } else {
        println!("{}", json_output);
    }

    Ok(())
}

fn generate_flat_tree(datums: &HashMap<String, crate::BootDatum>) -> Result<serde_json::Value> {
    use serde_json::json;

    let mut nodes = Vec::new();

    for (name, datum) in datums {
        let node = json!({
            "id": format!("datum-{}", name),
            "text": name,
            "type": "leaf",
            "data-nsd-label-uuid": format!("{}-uuid", name),
            "a_attr": {
                "onclick": format!("window.location='datums/{}.html';", name)
            },
            "icon": get_icon_for_type(&datum.datum_type),
            "li_attr": {
                "title": datum.hint
            }
        });
        nodes.push(node);
    }

    // Sort by name
    nodes.sort_by(|a, b| {
        let a_text = a["text"].as_str().unwrap_or("");
        let b_text = b["text"].as_str().unwrap_or("");
        a_text.cmp(b_text)
    });

    Ok(json!(nodes))
}

fn generate_grouped_tree(datums: &HashMap<String, crate::BootDatum>) -> Result<serde_json::Value> {
    use serde_json::json;
    use std::collections::BTreeMap;

    // Group by type
    let mut groups: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();

    for (name, datum) in datums {
        let type_name = if let Some(dtype) = &datum.datum_type {
            format!("{:?}", dtype)
        } else {
            "Unknown".to_string()
        };

        let node = json!({
            "id": format!("datum-{}", name),
            "text": name,
            "type": "leaf",
            "data-nsd-label-uuid": format!("{}-uuid", name),
            "a_attr": {
                "onclick": format!("window.location='datums/{}.html';", name)
            },
            "icon": get_icon_for_type(&datum.datum_type),
            "li_attr": {
                "title": datum.hint
            }
        });

        groups.entry(type_name).or_insert_with(Vec::new).push(node);
    }

    // Build tree with type groups
    let mut root_nodes = Vec::new();

    for (type_name, children) in groups {
        let group_node = json!({
            "id": format!("type-{}", type_name.to_lowercase()),
            "text": type_name,
            "children": children,
            "icon": get_icon_for_type_group(&type_name)
        });
        root_nodes.push(group_node);
    }

    Ok(json!(root_nodes))
}

fn get_icon_for_type(dtype: &Option<crate::DatumType>) -> String {
    match dtype {
        Some(crate::DatumType::Cli) => "fas fa-terminal".to_string(),
        Some(crate::DatumType::Mcp) => "fas fa-plug".to_string(),
        Some(crate::DatumType::Ai) => "fas fa-robot".to_string(),
        Some(crate::DatumType::AiModel) => "fas fa-brain".to_string(),
        Some(crate::DatumType::K8s) => "fas fa-dharmachakra".to_string(),
        Some(crate::DatumType::Job) => "fas fa-tasks".to_string(),
        Some(crate::DatumType::Stack) => "fas fa-layer-group".to_string(),
        Some(crate::DatumType::Agent) => "fas fa-user-secret".to_string(),
        _ => "jstree-file".to_string(),
    }
}

fn get_icon_for_type_group(type_name: &str) -> String {
    match type_name {
        "Cli" => "fas fa-folder".to_string(),
        "Mcp" => "fas fa-folder-open".to_string(),
        "Ai" => "fas fa-folder".to_string(),
        "K8s" => "fas fa-folder".to_string(),
        _ => "fas fa-folder".to_string(),
    }
}

// ── #198: datum search ────────────────────────────────────────────────────────

fn handle_search(
    b00t_path: &str,
    pattern: &str,
    type_filter: Option<&str>,
    format: &str,
    depth: Option<usize>,
) -> Result<()> {
    let results = datum_utils::search_datums(b00t_path, pattern, type_filter, depth)?;

    if results.is_empty() {
        println!("No datums matching '{}'", pattern);
        return Ok(());
    }

    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        // table output
        println!("{:<30} {:<12} {:<14} {}", "KEY", "TYPE", "MATCH", "HINT");
        println!("{}", "-".repeat(80));
        for r in &results {
            let type_str = r
                .datum_type
                .as_ref()
                .map(|t| format!("{:?}", t))
                .unwrap_or_else(|| "?".to_string());
            let match_str = r.match_reason.as_deref().unwrap_or("-");
            println!(
                "{:<30} {:<12} {:<14} {}",
                truncate(&r.key, 29),
                truncate(&type_str, 11),
                truncate(match_str, 13),
                truncate(&r.hint, 40),
            );
        }
        println!("\n{} datum(s) found", results.len());
    }

    Ok(())
}

// ── #199: datum filter ────────────────────────────────────────────────────────

fn handle_filter(
    b00t_path: &str,
    needs_any_env: bool,
    needs_all_env: bool,
    require: &[String],
    type_filter: Option<&str>,
    explain: bool,
    format: &str,
) -> Result<()> {
    let all_datums = datum_utils::get_all_datums(b00t_path)?;

    // Parse --types into DatumFilter.datum_type (first type only for now)
    let datum_type = type_filter.and_then(|t| {
        let lower = t.split(',').next().unwrap_or("").trim().to_lowercase();
        parse_datum_type(&lower)
    });

    let filter = DatumFilter {
        needs_any_env,
        needs_all_env,
        require_constraints: require.to_vec(),
        datum_type,
        ..Default::default()
    };

    let results = datum_utils::filter_datums(all_datums, &filter, explain);

    if results.is_empty() {
        println!("No datums satisfy the given constraints.");
        return Ok(());
    }

    if format == "json" {
        // Serialize as array of {key, name, type, hint, reason?}
        let arr: Vec<serde_json::Value> = results
            .iter()
            .map(|(key, datum, reason)| {
                let mut obj = serde_json::json!({
                    "key": key,
                    "name": datum.name,
                    "type": datum.datum_type.as_ref().map(|t| format!("{:?}", t)),
                    "hint": datum.hint,
                });
                if let Some(r) = reason {
                    obj["filtered_reason"] = serde_json::json!(r);
                }
                obj
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr)?);
    } else {
        println!("{:<30} {:<12} {}", "KEY", "TYPE", if explain { "REASON / OK" } else { "HINT" });
        println!("{}", "-".repeat(72));
        for (key, datum, reason) in &results {
            let type_str = datum
                .datum_type
                .as_ref()
                .map(|t| format!("{:?}", t))
                .unwrap_or_else(|| "?".to_string());
            let info = reason.as_deref().unwrap_or(&datum.hint);
            println!(
                "{:<30} {:<12} {}",
                truncate(key, 29),
                truncate(&type_str, 11),
                truncate(info, 40),
            );
        }
        let pass: usize = results.iter().filter(|(_, _, r)| r.is_none()).count();
        println!("\n{} passing / {} total", pass, results.len());
    }

    Ok(())
}

// ── #201: datum graph + neighbors ────────────────────────────────────────────

fn handle_graph(
    b00t_path: &str,
    format: &str,
    output_file: Option<&str>,
    type_filter: Option<&str>,
    depth: Option<usize>,
) -> Result<()> {
    let mut graph = datum_utils::build_datum_graph(b00t_path, depth)?;

    // Apply type filter to nodes + edges
    if let Some(types_str) = type_filter {
        let filters: Vec<String> = types_str
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .collect();
        graph.nodes.retain(|n| {
            let t = n
                .datum_type
                .as_ref()
                .map(|t| format!("{:?}", t).to_lowercase())
                .unwrap_or_default();
            filters.iter().any(|f| t.contains(f.as_str()))
        });
        let kept_keys: std::collections::HashSet<&str> =
            graph.nodes.iter().map(|n| n.key.as_str()).collect();
        graph
            .edges
            .retain(|e| kept_keys.contains(e.from.as_str()) || kept_keys.contains(e.to.as_str()));
    }

    let content = if format == "json" {
        serde_json::to_string_pretty(&graph)?
    } else {
        datum_utils::graph_to_dot(&graph)
    };

    if let Some(path) = output_file {
        std::fs::write(path, &content)?;
        println!("✅ Graph written to: {}", path);
    } else {
        println!("{}", content);
    }

    Ok(())
}

fn handle_neighbors(
    b00t_path: &str,
    datum_key: &str,
    depth: usize,
    direction: &str,
) -> Result<()> {
    let graph = datum_utils::build_datum_graph(b00t_path, None)?;
    let neighbors = datum_utils::graph_neighbors(&graph, datum_key, depth, direction);

    if neighbors.is_empty() {
        println!("No neighbors found for '{}' (depth={}, direction={})", datum_key, depth, direction);
        return Ok(());
    }

    println!("{:<30} {:<30} {}", "FROM", "TO", "EDGE_TYPE");
    println!("{}", "-".repeat(72));
    for edge in &neighbors {
        println!(
            "{:<30} {:<30} {}",
            truncate(&edge.from, 29),
            truncate(&edge.to, 29),
            edge.edge_type,
        );
    }
    println!("\n{} edge(s)", neighbors.len());

    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

fn parse_datum_type(s: &str) -> Option<crate::DatumType> {
    match s {
        "cli" => Some(crate::DatumType::Cli),
        "mcp" => Some(crate::DatumType::Mcp),
        "ai" => Some(crate::DatumType::Ai),
        "aimodel" | "ai_model" | "ai-model" => Some(crate::DatumType::AiModel),
        "k8s" | "kubernetes" => Some(crate::DatumType::K8s),
        "job" => Some(crate::DatumType::Job),
        "stack" => Some(crate::DatumType::Stack),
        "agent" => Some(crate::DatumType::Agent),
        _ => None,
    }
}
