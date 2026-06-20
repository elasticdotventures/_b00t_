use crate::datum_utils::{self, DatumFilter};
use crate::DatumType;
use anyhow::{Context, Result};
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

    #[clap(about = "Semantic datum search via irontology-mcp (#200)")]
    SemanticSearch {
        #[clap(help = "Natural language query")]
        query: String,

        #[clap(long, help = "Re-index all datums before searching")]
        rebuild: bool,

        #[clap(long, help = "Max results", default_value = "5")]
        limit: usize,
    },

    #[clap(about = "Validate a datum TOML file against BootDatum schema")]
    Validate {
        #[clap(help = "Path to .toml file or datum key (e.g., mold.cli)")]
        target: String,

        #[clap(long, help = "check extension/type consistency (e.g. .cli suffix vs type=cli)")]
        strict: bool,
    },

    #[clap(about = "Emit a minimal valid datum TOML for the given type")]
    Scaffold {
        #[clap(help = "Datum type (cli, mcp, ai, docker, hardware, …)")]
        datum_type: String,

        #[clap(help = "Datum name")]
        name: String,
    },

    #[clap(about = "Create a delegated task ticket for datum creation (bypasses future datum-write guard)")]
    Delegate {
        #[clap(help = "Datum type (cli, mcp, ai, docker, hardware, …)")]
        datum_type: String,

        #[clap(help = "Datum name")]
        name: String,

        #[clap(long, help = "Additional description / rationale")]
        description: Option<String>,

        #[clap(long, help = "Install method hint (apt, cargo, pip, curl, …)")]
        install_method: Option<String>,
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
        DatumCommands::SemanticSearch {
            query,
            rebuild,
            limit,
        } => {
            // 🤓 Handle::block_on panics if called from within an async runtime context.
            //    Spawn a fresh OS thread with its own current_thread runtime to bridge
            //    the sync dispatch → async GrokClient calls.
            let path_owned = path.to_string();
            let query_owned = query.to_string();
            let rebuild = *rebuild;
            let limit = *limit;
            std::thread::spawn(move || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio build")
                    .block_on(handle_semantic_search(
                        &path_owned,
                        &query_owned,
                        rebuild,
                        limit,
                    ))
            })
            .join()
            .map_err(|_| anyhow::anyhow!("semantic-search thread panicked"))?
        }
        DatumCommands::Validate { target, strict } => handle_validate(path, target, *strict),
        DatumCommands::Scaffold { datum_type, name } => handle_scaffold(datum_type, name),
        DatumCommands::Delegate { datum_type, name, description, install_method } => {
            handle_delegate(datum_type, name, description.as_deref(), install_method.as_deref())
        }
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

    if datum.status.is_some()
        || datum.enabled.is_some()
        || datum.status_msg.is_some()
        || datum.replacement.is_some()
        || !datum.git_attributes.is_empty()
    {
        println!("## Operational Metadata");
        println!();
        if let Some(status) = &datum.status {
            println!("**Status:** {}", status);
        }
        if let Some(enabled) = datum.enabled {
            println!("**Enabled:** {}", enabled);
        }
        if let Some(message) = &datum.status_msg {
            println!("**Status Message:** {}", message);
        }
        if let Some(replacement) = &datum.replacement {
            println!("**Replacement:** {}", replacement);
        }
        let extra_attrs: std::collections::BTreeMap<_, _> = datum
            .git_attributes
            .iter()
            .filter(|(key, _)| {
                !matches!(
                    key.as_str(),
                    "status" | "enabled" | "status_msg" | "replacement"
                )
            })
            .collect();
        if !extra_attrs.is_empty() {
            println!();
            println!("**Git Attributes:**");
            for (key, value) in extra_attrs {
                println!("- b00t.{}: {}", key, value);
            }
        }
        println!();
    }

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
    if let Some(install) = datum.install_command() {
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
        Some(crate::DatumType::Ai) => "fas fa-brain".to_string(),
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

    // Parse --types into DatumFilter.datum_types (all comma-separated entries)
    let datum_types: Vec<crate::DatumType> = type_filter
        .map(|t| {
            t.split(',')
                .filter_map(|s| parse_datum_type(s.trim().to_lowercase().as_str()))
                .collect()
        })
        .unwrap_or_default();

    let filter = DatumFilter {
        needs_any_env,
        needs_all_env,
        require_constraints: require.to_vec(),
        datum_types,
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
        println!(
            "{:<30} {:<12} {}",
            "KEY",
            "TYPE",
            if explain { "REASON / OK" } else { "HINT" }
        );
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
            .retain(|e| kept_keys.contains(e.from.as_str()) && kept_keys.contains(e.to.as_str()));
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

fn handle_neighbors(b00t_path: &str, datum_key: &str, depth: usize, direction: &str) -> Result<()> {
    let graph = datum_utils::build_datum_graph(b00t_path, None)?;
    let neighbors = datum_utils::graph_neighbors(&graph, datum_key, depth, direction);

    if neighbors.is_empty() {
        println!(
            "No neighbors found for '{}' (depth={}, direction={})",
            datum_key, depth, direction
        );
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

// ── #200: semantic datum search ───────────────────────────────────────────────

async fn handle_semantic_search(
    b00t_path: &str,
    query: &str,
    rebuild: bool,
    limit: usize,
) -> Result<()> {
    use b00t_c0re_lib::grok::GrokClient;

    let mut client = GrokClient::new();

    // Try irontology; fall back to regex search if unavailable
    // 🤓 GROK_BACKEND defaults to irontology (set in GrokBackend::from_env)
    match client.initialize().await {
        Ok(()) => {
            if rebuild {
                let count =
                    datum_utils::index_datums_into_irontology(b00t_path, &mut client).await?;
                println!("📚 Indexed {} datums into irontology", count);
            }

            let result = client.ask(query, None, Some(limit)).await?;
            if result.results.is_empty() {
                println!(
                    "No semantic results for '{}'. Try --rebuild to index datums first.",
                    query
                );
            } else {
                println!("{:<30} {}", "KEY/SOURCE", "CONTENT PREVIEW");
                println!("{}", "-".repeat(72));
                for r in &result.results {
                    let source = r.source.as_deref().unwrap_or(&r.id);
                    println!("{:<30} {}", truncate(source, 29), truncate(&r.content, 40),);
                }
                println!("\n{} result(s)", result.results.len());
            }
        }
        Err(e) => {
            // Graceful degradation: fall back to regex search
            eprintln!(
                "⚠️ irontology-mcp unavailable ({}), falling back to regex search",
                e
            );
            handle_search(b00t_path, query, None, "table", None)?;
        }
    }

    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn truncate(s: &str, max: usize) -> String {
    // Avoid slicing `&str` at arbitrary byte indices, which can panic for UTF-8.
    if max == 0 {
        return String::new();
    }

    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else {
        let take = max.saturating_sub(1);
        let mut result: String = s.chars().take(take).collect();
        result.push('…');
        result
    }
}

fn parse_datum_type(s: &str) -> Option<crate::DatumType> {
    crate::DatumType::from_type_token(s)
}

/// Known TOML keys in [b00t] section, derived from BootDatum struct fields.
/// 🤓 single source of truth: update this when BootDatum adds/removes fields.
///    `type` maps to BootDatum::datum_type (serde rename).
const KNOWN_B00T_KEYS: &[&str] = &[
    "name", "type", "status", "enabled", "status_msg", "replacement",
    "git_attributes",
    "desires", "auto_install", "hint", "skills", "compliance",
    "install", "update", "version", "version_regex", "requires_sudo",
    "command", "args", "vsix_id", "script",
    "image", "docker_args", "oci_uri", "resource_path",
    "chart_path", "namespace", "values_file",
    "keywords", "package_name", "env", "require", "aliases",
    "depends_on", "unlocks", "gate", "url", "branch", "clone_path",
    "entangled_agents", "entangled_cli", "entangled_mcp",
    "entangled_ai_models", "entangled_apis", "entangled_docker",
    "entangled_k8s", "channel_prefix",
    "learn", "lfmf_category",
    // nested sections (valid as [b00t.X] tables)
    "ansible", "k0mmand3r", "knowledge", "mcp",
    "hook_detect", "hook_install", "hook_learn", "hook_uninstall", "hook_update",
    "job", "skill", "justfile", "stack", "orchestration",
    "uninstall", "provides", "implements", "members", "type_tags",
    "usage", "dsn", "protocol",
];

/// Validate a datum file against BootDatum schema.
fn handle_validate(datum_path: &str, target: &str, strict: bool) -> Result<()> {
    let expanded = shellexpand::tilde(datum_path);
    let dir = std::path::Path::new(expanded.as_ref());

    // Resolve target to a file path
    let file_path = if target.ends_with(".toml") || target.ends_with(".tomllm") || target.ends_with(".tomllmd") {
        // Direct file path
        let p = std::path::Path::new(target);
        if p.is_absolute() { p.to_path_buf() } else { dir.join(target) }
    } else if target.contains('.') {
        // datum key like "mold.cli" — resolve via get_config
        let config_result = crate::get_config(target, datum_path);
        let (_config, filename) = match config_result {
            Ok(c) => c,
            Err(e) => anyhow::bail!("datum '{}' not found at {}: {}", target, datum_path, e),
        };
        dir.join(&filename)
    } else {
        anyhow::bail!("specify datum key (e.g. mold.cli), file path, or type.name format");
    };

    if !file_path.exists() {
        anyhow::bail!("file not found: {}", file_path.display());
    }

    let filename = file_path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    println!("==> {}", file_path.display());
    println!();

    // Parse TOML into raw value for field inspection
    let content = std::fs::read_to_string(&file_path).context("cannot read file")?;
    let raw: toml::Value = toml::from_str(&content).context("invalid TOML")?;

    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Check [b00t] section exists
    let b00t_table = match raw.get("b00t") {
        Some(toml::Value::Table(t)) => t,
        Some(_) => { errors.push("[b00t] must be a TOML table".into()); return print_validation_result(&errors, &warnings); }
        None => { errors.push("missing [b00t] section".into()); return print_validation_result(&errors, &warnings); }
    };

    // Required fields
    if b00t_table.get("name").is_none() {
        errors.push("missing required field: name".into());
    }
    if b00t_table.get("hint").is_none() {
        errors.push("missing required field: hint".into());
    }

    // Check for unknown keys
    for key in b00t_table.keys() {
        if !KNOWN_B00T_KEYS.contains(&key.as_str()) {
            warnings.push(format!("unknown field: b00t.{} (not in BootDatum schema)", key));
        }
    }

    // If strict: check extension ↔ type consistency
    if strict {
        if let Some(toml::Value::String(type_str)) = b00t_table.get("type") {
            let declared = DatumType::from_type_token(type_str);
            let from_ext = DatumType::from_filename(filename);
            if declared != Some(from_ext) && from_ext != DatumType::Unknown {
                warnings.push(format!(
                    "type={} but filename suggests {:?} (extension .{})",
                    type_str,
                    from_ext,
                    from_ext.base_suffix().trim_start_matches('.')
                ));
            }
        }
    }

    // Validate version_regex compiles
    if let Some(toml::Value::String(re)) = b00t_table.get("version_regex") {
        if regex::Regex::new(re).is_err() {
            errors.push(format!("invalid version_regex: '{}'", re));
        }
    }

    print_validation_result(&errors, &warnings)
}

fn print_validation_result(errors: &[String], warnings: &[String]) -> Result<()> {
    if errors.is_empty() && warnings.is_empty() {
        println!("valid — no issues found");
        return Ok(());
    }
    for e in errors {
        println!("  ERROR: {}", e);
    }
    for w in warnings {
        println!("  WARN: {}", w);
    }
    if !errors.is_empty() {
        anyhow::bail!("{} error(s), {} warning(s)", errors.len(), warnings.len());
    }
    println!("ok ({} warning(s))", warnings.len());
    Ok(())
}

/// Emit a minimal valid datum TOML skeleton.
fn handle_scaffold(datum_type: &str, name: &str) -> Result<()> {
    let dt = DatumType::from_type_token(datum_type)
        .or_else(|| DatumType::from_type_token(&datum_type.to_lowercase()))
        .context(format!("unknown datum type: '{}' (use: cli, mcp, ai, model, docker, hardware, …)", datum_type))?;

    // Reverse-dot for model sub-type: name.model.ai.tomllmd
    let is_model = datum_type == "model" || datum_type == "ai_model";
    let (filename, toml_type) = if is_model {
        (format!("{}.model.ai.tomllmd", name), "ai")
    } else {
        (format!("{}{}", name, dt.file_extension()), datum_type)
    };

    println!("# {}.{} — {} datum", name, datum_type, dt.base_suffix().trim_start_matches('.'));
    println!("# Generated: {}", chrono::Utc::now().to_rfc3339());
    println!("# File: {}", filename);
    println!("# Install with: b00t-cli install {}", filename.trim_end_matches(".tomllmd").trim_end_matches(".toml"));
    println!();
    println!("[b00t]");
    println!("name        = \"{}\"", name);
    println!("type        = \"{}\"", toml_type);
    println!("desires     = \"1.0.0\"");
    println!("hint        = \"TODO: one-line description\"");
    println!();

    // Type-specific skeleton fields
    match dt {
        DatumType::Cli | DatumType::Apt | DatumType::Nix => {
            println!("# install     = \"TODO: install command\"");
            println!("# version     = \"{} --version\"", name);
            println!("# version_regex = '(\\\\d+\\\\.\\\\d+\\\\.\\\\d+)'");
        }
        DatumType::Mcp => {
            println!("# command     = \"TODO: binary to execute\"");
            println!("# args        = [\"-y\", \"@scope/package\"]");
        }
        DatumType::Ai if is_model => {
            println!("# [model]");
            println!("# architecture  = \"transformer\"");
            println!("# provider      = \"huggingface\"");
            println!("# size          = \"large\"");
            println!("# capabilities  = [\"chat\", \"code\"]");
            println!("# litellm_model = \"huggingface/org/model-name\"");
            println!("# huggingface_id = \"org/model-name\"");
            println!("# context_window = 4096");
            println!("# [model.parameters]");
            println!("# max_tokens    = 4096");
            println!("# temperature   = 0.7");
            println!("# [model.metadata]");
            println!("# family        = \"model-family\"");
            println!("# quant          = \"Q4_K_M\"");
        }
        DatumType::Docker => {
            println!("# image       = \"org/image:tag\"");
            println!("# docker_args = [\"-p\", \"8080:8080\"]");
        }
        DatumType::Hardware => {
            println!("# [b00t.hardware]");
            println!("# vendor      = \"VendorName\"");
            println!("# soc         = \"SoC-Model\"");
            println!("# driver_pkg  = \"driver-package\"");
        }
        _ => {
            println!("# TODO: add type-specific fields");
        }
    }
    println!();
    println!("# Optional fields:");
    println!("# depends_on  = [\"prereq.cli\"]");
    println!("# unlocks     = [\"mcp__*\"]");
    println!("# env         = {{ KEY = \"VALUE\" }}");

    Ok(())
}

/// Create a delegated task ticket for datum creation.
/// Bypasses the future datum-write guard by routing through b00t task system.
fn handle_delegate(
    datum_type: &str,
    name: &str,
    description: Option<&str>,
    install_method: Option<&str>,
) -> Result<()> {
    use std::process::Command;

    let dt = DatumType::from_type_token(datum_type)
        .or_else(|| DatumType::from_type_token(&datum_type.to_lowercase()))
        .context(format!("unknown datum type: '{}'", datum_type))?;

    // Reverse-dot for model sub-type (matches handle_scaffold)
    let is_model = datum_type == "model" || datum_type == "ai_model";
    let (filename, datum_key) = if is_model {
        (format!("{}.model.ai.tomllmd", name), format!("{}.ai", name))
    } else {
        (format!("{}{}", name, dt.file_extension()), format!("{}.{}", name, datum_type))
    };
    let type_label = if is_model { "ai" } else { datum_type };
    let scaffold_type = datum_type;

    let desc = format!(
        "datum: create {datum_key}\n\
         type: {type_label}\n\
         name: {name}\n\
         file: {filename}\n\
         install_method: {install}\n\
         rationale: {rationale}\n\
         ---\n\
         Validation:\n\
         1. b00t datum scaffold {scaffold_type} {name} > _b00t_/{filename}\n\
         2. Edit _b00t_/{filename}: fill hint, install, version, version_regex\n\
         3. b00t datum validate {datum_key} --strict\n\
         4. b00t-cli . {name}  # verify version detection\n\
         5. cp _b00t_/{filename} $_B00T_Path/  # deploy to active path",
        datum_key = datum_key,
        type_label = type_label,
        name = name,
        filename = filename,
        install = install_method.unwrap_or("unknown"),
        rationale = description.unwrap_or("agent-requested datum creation"),
        scaffold_type = scaffold_type,
    );

    let method_hint = install_method.map(|m| format!(" install_method={}", m)).unwrap_or_default();
    let title = format!("datum: create {datum_key} ({type_label} datum{method_hint})");

    println!("# Datum Delegation Ticket");
    println!();
    println!("type:        {}", type_label);
    println!("name:        {}", name);
    println!("key:         {}", datum_key);
    println!("file:        {}", filename);
    if let Some(m) = install_method {
        println!("install:     {}", m);
    }
    println!();

    println!("## Scaffold (for reference)");
    println!();
    handle_scaffold(datum_type, name)?;

    println!();
    println!("## Creating b00t task…");
    let output = Command::new("b00t-cli")
        .args([
            "task", "add",
            &title,
            "--description", &desc,
            "--tags", "datum,registration",
            "--priority", "2",
            "--criteria", &format!("b00t datum validate {} --strict passes", datum_key),
            "--criteria", &format!("b00t-cli . {} returns version match", name),
        ])
        .output()
        .context("failed to create b00t task")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("b00t task add failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    println!("{}", stdout.trim());
    println!();
    println!("Delegated. Next task: `b00t task next`");

    Ok(())
}
