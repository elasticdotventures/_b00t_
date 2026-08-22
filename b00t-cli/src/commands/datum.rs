use crate::DatumType;
use crate::datum_store::{DatumStore, HashMapStore};
use crate::datum_utils::{self, DatumFilter};
use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashMap;
use ufo_types::{Disposition, IsoAuditable, Satisfies, SatisfiesResult, Stereotyped, UfoStereotype};

#[derive(Parser, Debug)]
pub enum DatumCommands {
    #[clap(about = "Show comprehensive datum information")]
    Show {
        #[clap(help = "Datum name to show (e.g., just, rust, docker)")]
        name: String,
        #[clap(
            long,
            help = "Authorize via a scoped agent token (k8s TokenReview + role-shard-datum RoleBinding check) instead of ambient trust"
        )]
        as_agent_token: Option<String>,
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
        target: Option<String>,

        #[clap(
            long,
            help = "check extension/type consistency (e.g. .cli suffix vs type=cli)"
        )]
        strict: bool,

        #[clap(long, help = "validate cross-datum graph references")]
        graph: bool,
    },

    #[clap(about = "Emit a minimal valid datum TOML for the given type")]
    Scaffold {
        #[clap(help = "Datum type (cli, mcp, ai, docker, hardware, …)")]
        datum_type: String,

        #[clap(help = "Datum name")]
        name: String,
    },

    #[clap(
        about = "Create a delegated task ticket for datum creation (bypasses future datum-write guard)"
    )]
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

    #[clap(about = "Resolve an abstract interface from a datum with token substitution")]
    Call {
        #[clap(help = "Datum key (e.g., gh.cli)")]
        datum: String,

        #[clap(help = "Abstract interface name (e.g., pr_comment, pr_review)")]
        interface: String,

        #[clap(
            long,
            help = "Token substitutions: key=value (repeatable)",
            value_name = "KEY=VALUE"
        )]
        token: Vec<String>,

        #[clap(long, help = "Execute the resolved command instead of printing")]
        exec: bool,
    },

    #[clap(about = "Calibrate tier assignments in b00t:map tail-maps via telemetry (E2)")]
    Calibrate(crate::commands::calibrate::CalibrateArgs),

    #[clap(about = "Generate a datum from an artifact via kreuzberg extraction (E3)")]
    FromArtifact(crate::commands::from_artifact::FromArtifactArgs),

    #[clap(
        about = "Generate wrkflw-ci-build.yml for a GithubActionsCompatible repo (CodeRunnerActionProcess local gate)"
    )]
    GenWrkflw {
        #[clap(
            help = "Path to repo root (defaults to current directory)",
            default_value = "."
        )]
        repo_path: String,

        #[clap(
            long,
            help = "Write wrkflw-ci-build.yml to .github/workflows/ instead of printing"
        )]
        write: bool,
    },

    #[clap(about = "Health-check a datum's gates + maintenance check_command (#694)")]
    HealthCheck {
        #[clap(long, help = "Specific datum name to check")]
        name: Option<String>,

        #[clap(long, help = "Check all datums")]
        all: bool,
    },

    #[clap(about = "Aggregate pass/warn/fail health report across all datums + store status (#694)")]
    HealthReport {
        #[clap(long, help = "Output format: table|json", default_value = "table")]
        format: String,
    },

    #[clap(about = "Govern a datum (or all datums): prove + gate + hook, report health (#696)")]
    Govern {
        #[clap(help = "Datum name (omit with --all)")]
        name: Option<String>,

        #[clap(long, help = "Govern all datums, skipping status=disabled")]
        all: bool,

        #[clap(long, help = "Output format: table|json", default_value = "table")]
        format: String,
    },
}

pub async fn handle_datum_command(path: &str, datum_command: &DatumCommands) -> Result<()> {
    match datum_command {
        DatumCommands::Show { name, as_agent_token } => {
            handle_show(path, name, as_agent_token.as_deref()).await
        }
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
        DatumCommands::Validate { target, strict, graph } => {
            if *graph {
                handle_validate_graph(path)
            } else {
                let Some(target) = target else {
                    anyhow::bail!("specify datum key, file path, or --graph");
                };
                handle_validate(path, target, *strict)
            }
        }
        DatumCommands::Scaffold { datum_type, name } => handle_scaffold(datum_type, name),
        DatumCommands::Delegate {
            datum_type,
            name,
            description,
            install_method,
        } => handle_delegate(
            datum_type,
            name,
            description.as_deref(),
            install_method.as_deref(),
        ),
        DatumCommands::Call {
            datum,
            interface,
            token,
            exec,
        } => handle_call(path, datum, interface, token, *exec),
        DatumCommands::Calibrate(args) => crate::commands::calibrate::handle_calibrate(path, args),
        DatumCommands::FromArtifact(args) => {
            crate::commands::from_artifact::handle_from_artifact(args)
        }
        DatumCommands::GenWrkflw { repo_path, write } => handle_gen_wrkflw(repo_path, *write),
        DatumCommands::HealthCheck { name, all } => {
            handle_health_check(path, name.as_deref(), *all)
        }
        DatumCommands::HealthReport { format } => handle_health_report(path, format),
        DatumCommands::Govern { name, all, format } => {
            handle_govern(path, name.as_deref(), *all, format)
        }
    }
}

async fn handle_show(b00t_path: &str, datum_name: &str, as_agent_token: Option<&str>) -> Result<()> {
    // Component 4 (agent-scoped token pilot): when --as-agent-token is
    // given, gate the read path on a k8s TokenReview + role-shard-datum
    // RoleBinding check rather than ambient trust. See
    // docs/superpowers/specs/2026-08-22-agent-scoped-token-issuance.md.
    if let Some(token) = as_agent_token {
        crate::agent_token::authorize_datum_shard_token(token)
            .await
            .context("datum show --as-agent-token")?;
    }

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
    "name",
    "type",
    "status",
    "enabled",
    "status_msg",
    "replacement",
    "git_attributes",
    "desires",
    "auto_install",
    "hint",
    "skills",
    "compliance",
    "install",
    "update",
    "version",
    "version_regex",
    "requires_sudo",
    "command",
    "args",
    "vsix_id",
    "script",
    "image",
    "docker_args",
    "oci_uri",
    "resource_path",
    "chart_path",
    "namespace",
    "values_file",
    "keywords",
    "package_name",
    "env",
    "require",
    "aliases",
    "depends_on",
    "unlocks",
    "gate",
    "url",
    "branch",
    "clone_path",
    "entangled_agents",
    "entangled_cli",
    "entangled_mcp",
    "entangled_ai_models",
    "entangled_apis",
    "entangled_docker",
    "entangled_k8s",
    "channel_prefix",
    "learn",
    "lfmf_category",
    // nested sections (valid as [b00t.X] tables)
    "ansible",
    "k0mmand3r",
    "knowledge",
    "mcp",
    "hook_detect",
    "hook_install",
    "hook_learn",
    "hook_uninstall",
    "hook_update",
    "job",
    "skill",
    "justfile",
    "stack",
    "orchestration",
    "uninstall",
    "provides",
    "implements",
    "members",
    "type_tags",
    "usage",
    "dsn",
    "protocol",
];

/// Validate a datum file against BootDatum schema.
fn handle_validate(datum_path: &str, target: &str, strict: bool) -> Result<()> {
    let expanded = shellexpand::tilde(datum_path);
    let dir = std::path::Path::new(expanded.as_ref());

    // Resolve target to a file path
    let file_path =
        if target.ends_with(".toml") || target.ends_with(".tomllm") || target.ends_with(".tomllmd")
        {
            // Direct file path
            let p = std::path::Path::new(target);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                dir.join(target)
            }
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

    // Check [b00t] section exists
    let b00t_table = match raw.get("b00t") {
        Some(toml::Value::Table(t)) => t.clone(),
        Some(_) => {
            let errors = vec!["[b00t] must be a TOML table".to_string()];
            return print_validation_result(&errors, &[]);
        }
        None => {
            let errors = vec!["missing [b00t] section".to_string()];
            return print_validation_result(&errors, &[]);
        }
    };

    let outcome = compute_datum_validation(&b00t_table, filename, strict);
    print_validation_result(&outcome.errors, &outcome.warnings)?;

    // Route through the real Satisfies<C> / evidence-sink path (#927) —
    // additive to the print-based UX above, which is unchanged.
    let subject = DatumTomlSubject {
        raw: &b00t_table,
        filename,
    };
    let constraint = BootDatumSchemaConstraint { strict };
    let result = subject.satisfies(&constraint);

    let _ = crate::commands::evidence::record_is_a(target, &subject.ufo_stereotype().to_string());
    for iso_id in constraint.iso_standard_ids() {
        let _ = crate::commands::evidence::record_audited_by(target, &iso_id);
    }

    if matches!(result.disposition, Disposition::Unknown) {
        println!("  UNKNOWN: cannot verify type/extension consistency (unrecognized extension)");
        if strict {
            anyhow::bail!(
                "--strict: type/extension consistency is undecidable for '{}' (unrecognized extension)",
                filename
            );
        }
    }

    Ok(())
}

/// Pure outcome of validating a `[b00t]` TOML table against the `BootDatum`
/// schema — same checks `handle_validate` has always run, extracted so both
/// the print-based UX and the `Satisfies<BootDatumSchemaConstraint>` impl
/// below can share one implementation.
struct DatumValidationOutcome {
    errors: Vec<String>,
    warnings: Vec<String>,
    /// True exactly when the strict extension↔type consistency check could
    /// not be decided because the filename's extension is not a recognized
    /// datum type (`DatumType::from_filename` returned `Unknown`). Prior to
    /// #927 this case was silently skipped — neither an error nor a
    /// warning — which is the bug this issue fixes: it's now surfaced as a
    /// distinguishable `Disposition::Unknown`, not silently folded into
    /// "valid".
    extension_check_undecidable: bool,
}

/// Same required-field / unknown-key / extension-consistency / version_regex
/// checks `handle_validate` has always run — extracted to a pure function so
/// it can feed both the existing print-based UX and the new
/// `Satisfies<BootDatumSchemaConstraint>` impl.
fn compute_datum_validation(
    b00t_table: &toml::value::Table,
    filename: &str,
    strict: bool,
) -> DatumValidationOutcome {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut extension_check_undecidable = false;

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
            warnings.push(format!(
                "unknown field: b00t.{} (not in BootDatum schema)",
                key
            ));
        }
    }

    // If strict: check extension ↔ type consistency
    if strict {
        if let Some(toml::Value::String(type_str)) = b00t_table.get("type") {
            let declared = DatumType::from_type_token(type_str);
            let from_ext = DatumType::from_filename(filename);
            if from_ext == DatumType::Unknown {
                // #927 fix: previously silently skipped — no error, no
                // warning. Unrecognized extension means we genuinely cannot
                // decide whether type and extension are consistent.
                extension_check_undecidable = true;
            } else if declared != Some(from_ext) {
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

    DatumValidationOutcome {
        errors,
        warnings,
        extension_check_undecidable,
    }
}

/// `[b00t]` TOML table + originating filename — the subject evaluated
/// against `BootDatumSchemaConstraint`.
pub struct DatumTomlSubject<'a> {
    pub raw: &'a toml::value::Table,
    pub filename: &'a str,
}

/// Constraint: a `[b00t]` TOML table must conform to the `BootDatum` schema
/// (required fields present, extension↔type consistent when `strict`,
/// `version_regex` compiles).
pub struct BootDatumSchemaConstraint {
    pub strict: bool,
}

impl IsoAuditable for BootDatumSchemaConstraint {
    fn iso_standard_ids(&self) -> Vec<String> {
        vec!["b00t:BootDatumSchema".into()]
    }
}

impl Stereotyped for DatumTomlSubject<'_> {
    /// Resolves the declared `type` field through `DatumType`'s lattice
    /// (#925/#926 single source of truth) — same pattern as
    /// `ontology.rs`'s sparql "type" arm and `BootDatum`'s own
    /// `Stereotyped` impl. Missing/unrecognized type degrades to
    /// `Kind("Unknown")`, never panics.
    fn ufo_stereotype(&self) -> UfoStereotype {
        let type_str = match self.raw.get("type") {
            Some(toml::Value::String(s)) => s.as_str(),
            _ => "",
        };
        DatumType::from_type_token(type_str)
            .unwrap_or(DatumType::Unknown)
            .ufo_stereotype()
    }
}

impl Satisfies<BootDatumSchemaConstraint> for DatumTomlSubject<'_> {
    fn satisfies(&self, c: &BootDatumSchemaConstraint) -> SatisfiesResult {
        let outcome = compute_datum_validation(self.raw, self.filename, c.strict);
        if !outcome.errors.is_empty() {
            return SatisfiesResult::violated(outcome.errors.join("; "), 1.0);
        }
        if outcome.extension_check_undecidable {
            return SatisfiesResult::unknown(0.5);
        }
        SatisfiesResult::satisfied(1.0)
    }
}

fn handle_validate_graph(datum_path: &str) -> Result<()> {
    let store = HashMapStore::from_path(datum_path)?;
    let errors = store.validate_references();

    if errors.is_empty() {
        println!("datum graph: valid ({} datums)", store.len());
        return Ok(());
    }

    for error in &errors {
        eprintln!("  ERROR: {error}");
    }
    anyhow::bail!("datum graph validation failed: {} reference error(s)", errors.len());
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
        .context(format!(
            "unknown datum type: '{}' (use: cli, mcp, ai, model, docker, hardware, …)",
            datum_type
        ))?;

    // Reverse-dot for model sub-type: name.model.ai.tomllmd
    let is_model = datum_type == "model" || datum_type == "ai_model";
    let (filename, toml_type) = if is_model {
        (format!("{}.model.ai.tomllmd", name), "ai")
    } else {
        (format!("{}{}", name, dt.file_extension()), datum_type)
    };

    println!(
        "# {}.{} — {} datum",
        name,
        datum_type,
        dt.base_suffix().trim_start_matches('.')
    );
    println!("# Generated: {}", chrono::Utc::now().to_rfc3339());
    println!("# File: {}", filename);
    println!(
        "# Install with: b00t-cli install {}",
        filename
            .trim_end_matches(".tomllmd")
            .trim_end_matches(".toml")
    );
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
        DatumType::Repo => {
            println!("# [b00t.source]");
            println!("# repo        = \"https://github.com/org/{}\"", name);
            println!("# vendor_path = \"vendor/{}\"", name);
            println!();
            println!("# GithubActionsCompatible — declare CodeRunnerActionProcess providers");
            println!("# (run `b00t datum gen-wrkflw <path>` to generate wrkflw-ci-build.yml)");
            println!("[b00t.services.action_runner]");
            println!("local  = \"wrkflw\"");
            println!("remote = \"github-actions\"");
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
        (
            format!("{}{}", name, dt.file_extension()),
            format!("{}.{}", name, datum_type),
        )
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

    let method_hint = install_method
        .map(|m| format!(" install_method={}", m))
        .unwrap_or_default();
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
            "task",
            "add",
            &title,
            "--description",
            &desc,
            "--tags",
            "datum,registration",
            "--priority",
            "2",
            "--criteria",
            &format!("b00t datum validate {} --strict passes", datum_key),
            "--criteria",
            &format!("b00t-cli . {} returns version match", name),
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

/// Resolve an abstract interface from a datum TOML with token substitution.
///
/// Reads `[b00t.abstracts.<interface>]` from the datum file, selects the
/// appropriate backend (prefers `gh_cli` over `curl_rest`), substitutes
/// `{token}` placeholders, and either prints or executes the command.
fn handle_call(
    b00t_path: &str,
    datum_key: &str,
    interface: &str,
    tokens: &[String],
    exec: bool,
) -> Result<()> {
    // Find the datum file path
    let all = datum_utils::get_all_datums_with_paths(b00t_path, None)?;
    let file_path = all
        .iter()
        .find(|(name, _)| name.as_str() == datum_key || name.ends_with(&format!(".{}", datum_key)))
        .map(|(_, (_, path))| path.clone())
        .or_else(|| {
            all.iter()
                .find(|(name, _)| name.contains(datum_key))
                .map(|(_, (_, path))| path.clone())
        })
        .ok_or_else(|| anyhow::anyhow!("Datum '{}' not found in {}", datum_key, b00t_path))?;

    let file_path = std::path::PathBuf::from(&file_path);

    // Parse the raw TOML to access [b00t.abstracts] (not yet in BootDatum struct)
    let content = std::fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read datum file {}", file_path.display()))?;

    let root: toml::Value = toml::from_str(&content)
        .with_context(|| format!("Failed to parse TOML in {}", file_path.display()))?;

    // Navigate: b00t -> abstracts -> <interface>
    let abstracts = root
        .get("b00t")
        .and_then(|b| b.get("abstracts"))
        .and_then(|a| a.get(interface))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Abstract interface '{}' not found in datum '{}'. Available: check `b00t datum show {}`",
                interface, datum_key, datum_key
            )
        })?;

    // Select backend: github_mcp (if MCP server running) -> gh_cli -> curl_rest
    let template = if abstracts.get("github_mcp").is_some() {
        if github_mcp_running() {
            abstracts.get("github_mcp")
        } else {
            abstracts.get("gh_cli")
        }
    } else {
        abstracts.get("gh_cli")
    }
    .or_else(|| abstracts.get("curl_rest"))
    .and_then(|v| v.as_str())
    .ok_or_else(|| {
        anyhow::anyhow!(
            "No backend template found for '{}' in datum '{}'",
            interface,
            datum_key
        )
    })?;

    // Parse token substitutions: key=value -> {key} -> value
    let token_map: HashMap<&str, &str> = tokens.iter().filter_map(|t| t.split_once('=')).collect();

    if exec {
        let argv = resolve_command_argv(template, &token_map)?;
        if argv_has_unresolved_placeholders(&argv) {
            anyhow::bail!(
                "Unresolved tokens remain: check template for unmatched {{placeholders}}"
            );
        }
        let (program, args) = argv
            .split_first()
            .ok_or_else(|| anyhow::anyhow!("Resolved command is empty"))?;
        eprintln!("▶ {}", argv.join(" "));
        let status = std::process::Command::new(program)
            .args(args)
            .status()
            .with_context(|| format!("Failed to execute: {}", argv.join(" ")))?;
        if !status.success() {
            anyhow::bail!("Command exited with status: {}", status);
        }
    } else {
        let resolved = substitute_tokens(template, &token_map);
        if resolved.contains('{') && resolved.contains('}') {
            eprintln!(
                "⚠️  Unresolved tokens remain: check template for unmatched {{placeholders}}"
            );
        }
        println!("{}", resolved);
    }

    Ok(())
}

// ─── gen-wrkflw ──────────────────────────────────────────────────────────────

/// Detect build shape and emit a wrkflw-ci-build.yml (CodeRunnerActionProcess local gate).
/// Follows the l3dg3rr pattern: workflow_dispatch only, emulation-friendly.
fn handle_gen_wrkflw(repo_path: &str, write: bool) -> Result<()> {
    use std::path::Path;

    let root = Path::new(repo_path)
        .canonicalize()
        .with_context(|| format!("cannot resolve path: {repo_path}"))?;

    let workflows_dir = root.join(".github/workflows");
    anyhow::ensure!(
        workflows_dir.exists(),
        "no .github/workflows/ found at {} — not a GithubActionsCompatible repo",
        root.display()
    );

    // Check for existing wrkflw workflow
    let out_path = workflows_dir.join("wrkflw-ci-build.yml");
    if out_path.exists() && !write {
        println!(
            "# wrkflw-ci-build.yml already exists at {}",
            out_path.display()
        );
        println!("# Use --write to overwrite.");
        return Ok(());
    }

    // Detect build shape
    let is_rust = root.join("Cargo.toml").exists();
    let is_node = root.join("package.json").exists();
    let is_python = root.join("pyproject.toml").exists() || root.join("setup.py").exists();
    let repo_name = root.file_name().and_then(|n| n.to_str()).unwrap_or("repo");

    let build_steps = if is_rust {
        "      - name: Build\n        run: cargo build --workspace\n\n      - name: Test\n        run: cargo test --workspace"
    } else if is_node {
        "      - name: Install\n        run: npm ci\n\n      - name: Build\n        run: npm run build\n\n      - name: Test\n        run: npm test"
    } else if is_python {
        "      - name: Install\n        run: uv pip install -e .[dev]\n\n      - name: Test\n        run: uv run pytest"
    } else {
        "      - name: Build\n        run: echo 'TODO: add build steps'"
    };

    let header = format!(
        "# wrkflw-ci-build: local CI gate for {repo_name} (emulation mode, no Docker)\n\
         # Run with: wrkflw run .github/workflows/wrkflw-ci-build.yml\n\
         # Or:       just ci-local\n\
         #\n\
         # Follows l3dg3rr wrkflw pattern: workflow_dispatch only, emulation-friendly.\n\
         # GithubActionsCompatible / CodeRunnerActionProcess (see WRKFLW.tomllmd)\n\
         name: wrkflw-ci-build\n"
    );
    let body = concat!(
        "\non:\n",
        "  workflow_dispatch:\n",
        "\nenv:\n",
        "  CARGO_TERM_COLOR: always\n",
        "  RUST_BACKTRACE: \"1\"\n",
        "\ndefaults:\n",
        "  run:\n",
        "    shell: bash\n",
        "\njobs:\n",
        "  build:\n",
        "    name: \"Local CI gate\"\n",
        "    runs-on: ubuntu-latest\n",
        "    steps:\n",
        "      - uses: actions/checkout@v4\n\n",
    );
    let content = format!("{header}{body}{build_steps}\n");

    if write {
        std::fs::write(&out_path, &content)
            .with_context(|| format!("write {}", out_path.display()))?;
        println!("✓ wrote {}", out_path.display());
        println!(
            "  Add to justfile: just ci-local = wrkflw run .github/workflows/wrkflw-ci-build.yml"
        );
        println!("  Declare in datum: [b00t.services.action_runner] local = \"wrkflw\"");
    } else {
        println!("# Preview — run with --write to create .github/workflows/wrkflw-ci-build.yml");
        println!("{content}");
    }

    Ok(())
}

// ─── govern (#696) ───────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
struct PhaseResult {
    passed: bool,
    detail: String,
}

#[derive(serde::Serialize)]
struct GovernReport {
    datum: String,
    proved: PhaseResult,
    gated: Vec<crate::gates::GateResult>,
    hooked: PhaseResult,
    status: Option<String>,
    healthy: bool,
}

/// Run all three governance phases (prove, gate, hook) against a single datum
/// and summarize the result. Never panics: proof/hook failures are captured
/// as `passed: false` phase results rather than propagated as errors.
fn govern_one(datum: &crate::BootDatum, path: &str) -> GovernReport {
    let proved = match datum.prove_by_type() {
        Ok(()) => PhaseResult {
            passed: true,
            detail: "proof passed".to_string(),
        },
        Err(e) => PhaseResult {
            passed: false,
            detail: e.to_string(),
        },
    };

    let gated: Vec<crate::gates::GateResult> = datum
        .gate
        .as_ref()
        .map(|gates| crate::gates::evaluate_gates(gates, path))
        .unwrap_or_default();
    let gates_passed = gated.iter().all(|g| g.passed);

    let hooked = match datum.hook_detect.as_deref() {
        Some(script) if !script.trim().is_empty() => match crate::hook_engine::run_hook(script) {
            crate::hook_engine::HookResult::Ok => PhaseResult {
                passed: true,
                detail: "hook ok".to_string(),
            },
            crate::hook_engine::HookResult::Warn(msg) => PhaseResult {
                passed: true,
                detail: format!("warn: {msg}"),
            },
            crate::hook_engine::HookResult::Info(msg) => PhaseResult {
                passed: true,
                detail: msg,
            },
            crate::hook_engine::HookResult::Missing(msg) => PhaseResult {
                passed: false,
                detail: format!("missing: {msg}"),
            },
            crate::hook_engine::HookResult::Redirect(name) => PhaseResult {
                passed: false,
                detail: format!("redirect: {name}"),
            },
        },
        _ => PhaseResult {
            passed: true,
            detail: "no hook_detect".to_string(),
        },
    };

    let healthy = proved.passed && gates_passed && hooked.passed;

    GovernReport {
        datum: datum.name.clone(),
        proved,
        gated,
        hooked,
        status: datum.status.clone(),
        healthy,
    }
}

fn print_govern_report_table(report: &GovernReport) {
    let overall = if report.healthy { "✅ healthy" } else { "❌ unhealthy" };
    println!("# {} — {}", report.datum, overall);
    println!(
        "  proved: {} — {}",
        if report.proved.passed { "ok" } else { "FAIL" },
        report.proved.detail
    );
    if report.gated.is_empty() {
        println!("  gated:  (no gates)");
    } else {
        for gate in &report.gated {
            println!(
                "  gated:  {} — {}",
                if gate.passed { "ok" } else { "FAIL" },
                gate.reason
            );
        }
    }
    println!(
        "  hooked: {} — {}",
        if report.hooked.passed { "ok" } else { "FAIL" },
        report.hooked.detail
    );
    if let Some(status) = &report.status {
        println!("  status: {status}");
    }
    println!();
}

/// Govern every datum under `path`, skipping entries with `status = "disabled"`.
/// Results are sorted by datum key for deterministic output.
fn govern_all(path: &str) -> Result<Vec<GovernReport>> {
    let all_datums = datum_utils::get_all_datums(path)?;
    let mut keys: Vec<&String> = all_datums.keys().collect();
    keys.sort();
    Ok(keys
        .into_iter()
        .filter_map(|key| {
            let datum = all_datums.get(key)?;
            if datum.status.as_deref() == Some("disabled") {
                return None;
            }
            Some(govern_one(datum, path))
        })
        .collect())
}

/// `b00t datum govern <name>` / `b00t datum govern --all`.
///
/// Runs prove_by_type + evaluate_gates + hook_detect against a single named
/// datum, or every datum under `path` (skipping `status = "disabled"` entries
/// when `--all` is given) and reports per-datum health.
fn handle_govern(path: &str, name: Option<&str>, all: bool, format: &str) -> Result<()> {
    let reports: Vec<GovernReport> = if all {
        govern_all(path)?
    } else {
        let datum_name = name.ok_or_else(|| {
            anyhow::anyhow!("datum govern requires a name, or pass --all to govern every datum")
        })?;
        let datum = datum_utils::find_datum_by_pattern(path, datum_name)?
            .ok_or_else(|| anyhow::anyhow!("Datum '{}' not found", datum_name))?;
        vec![govern_one(&datum, path)]
    };

    if format == "json" {
        println!("{}", serde_json::to_string_pretty(&reports)?);
    } else if reports.is_empty() {
        println!("No datums to govern.");
    } else {
        for report in &reports {
            print_govern_report_table(report);
        }
        let healthy_count = reports.iter().filter(|r| r.healthy).count();
        println!("{} healthy / {} total", healthy_count, reports.len());
    }

    Ok(())
}

fn github_mcp_running() -> bool {
    std::process::Command::new("b00t")
        .args(["mcp", "list", "--search", "github"])
        .output()
        .map(|output| {
            output.status.success() && String::from_utf8_lossy(&output.stdout).contains("Running")
        })
        .unwrap_or(false)
}

fn resolve_command_argv(template: &str, token_map: &HashMap<&str, &str>) -> Result<Vec<String>> {
    if template.contains('\n') {
        anyhow::bail!(
            "Refusing to execute multi-line backend template without a shell; use a single-command backend such as gh_cli"
        );
    }

    let argv = shlex::split(template)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse backend template as argv"))?;
    if argv.is_empty() {
        anyhow::bail!("Backend template resolved to an empty command");
    }

    Ok(argv
        .into_iter()
        .map(|arg| substitute_tokens(&arg, token_map))
        .collect())
}

fn substitute_tokens(input: &str, token_map: &HashMap<&str, &str>) -> String {
    let mut resolved = input.to_string();
    for (key, value) in token_map {
        let placeholder = format!("{{{}}}", key);
        resolved = resolved.replace(&placeholder, value);
    }
    resolved
}

fn argv_has_unresolved_placeholders(argv: &[String]) -> bool {
    argv.iter()
        .any(|arg| arg.contains('{') && arg.contains('}'))
}

#[cfg(test)]
mod call_tests {
    use super::*;

    #[test]
    fn resolve_command_argv_keeps_shell_metacharacters_inside_token_arg() {
        let mut token_map = HashMap::new();
        token_map.insert("pr", "523");
        token_map.insert("body", "x'; touch /tmp/pwn; #");

        let argv = resolve_command_argv("gh pr comment {pr} --body '{body}'", &token_map).unwrap();

        assert_eq!(
            argv,
            vec![
                "gh".to_string(),
                "pr".to_string(),
                "comment".to_string(),
                "523".to_string(),
                "--body".to_string(),
                "x'; touch /tmp/pwn; #".to_string(),
            ]
        );
        assert!(!argv.iter().any(|arg| arg == "touch" || arg == "/tmp/pwn"));
    }

    #[test]
    fn resolve_command_argv_rejects_multiline_shell_backend() {
        let token_map = HashMap::new();
        let err = resolve_command_argv("PR={pr}\necho $PR", &token_map).unwrap_err();

        assert!(
            err.to_string()
                .contains("Refusing to execute multi-line backend template")
        );
    }
}

// ── Health check / health report (#694) ─────────────────────────────────────
//
// Health-checks a BootDatum by evaluating its `gate` preconditions (hard
// fail — the datum is not usable) and, if configured, its
// `maintenance.check_command` (soft warn — the datum works but its version
// check is failing / stale). Reuses `InstallerEngine::run_check` for command
// execution rather than reinventing shell-exec.

#[derive(serde::Serialize, Debug, Clone, PartialEq)]
enum HealthState {
    Pass,
    Warn,
    Fail,
}

impl std::fmt::Display for HealthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            HealthState::Pass => "pass",
            HealthState::Warn => "warn",
            HealthState::Fail => "fail",
        };
        write!(f, "{s}")
    }
}

/// Serializable mirror of `crate::gates::GateResult`, which does not derive
/// `Serialize` itself. gates.rs is out of scope for #694, so we copy the two
/// fields we need rather than modifying it.
#[derive(serde::Serialize, Debug, Clone, PartialEq)]
struct GateResultSummary {
    passed: bool,
    reason: String,
}

impl From<&crate::gates::GateResult> for GateResultSummary {
    fn from(g: &crate::gates::GateResult) -> Self {
        Self {
            passed: g.passed,
            reason: g.reason.clone(),
        }
    }
}

#[derive(serde::Serialize, Debug, Clone, PartialEq)]
struct DatumHealth {
    datum: String,
    state: HealthState,
    reason: String,
    last_checked: Option<String>,
    gate_results: Vec<GateResultSummary>,
}

/// Health-check a single datum: gate preconditions first (hard Fail — reason
/// comes from the failing gate's hint/message), then its
/// `maintenance.check_command` if configured (Warn on nonzero exit / exec
/// error), else Pass with no maintenance configured.
fn health_check_one(datum: &crate::BootDatum, path: &str) -> DatumHealth {
    let gate_results: Vec<GateResultSummary> = datum
        .gate
        .as_deref()
        .map(|gates| crate::gates::evaluate_gates(gates, path))
        .unwrap_or_default()
        .iter()
        .map(GateResultSummary::from)
        .collect();

    if let Some(failed) = gate_results.iter().find(|g| !g.passed) {
        return DatumHealth {
            datum: datum.name.clone(),
            state: HealthState::Fail,
            reason: failed.reason.clone(),
            last_checked: None,
            gate_results,
        };
    }

    if let Some(maintenance) = &datum.maintenance {
        if let Some(check_command) = &maintenance.check_command {
            let check = crate::install::capability::CapabilityCheck {
                name: datum.name.clone(),
                check_command: check_command.clone(),
                remediation: String::new(),
                required: false,
                vendor_datum: None,
            };
            let result = crate::install::capability::InstallerEngine::run_check(&check);
            let now = Some(chrono::Utc::now().to_rfc3339());
            return match result.status {
                crate::install::capability::CheckStatus::Pass => DatumHealth {
                    datum: datum.name.clone(),
                    state: HealthState::Pass,
                    reason: "maintenance check_command passed".to_string(),
                    last_checked: now,
                    gate_results,
                },
                _ => DatumHealth {
                    datum: datum.name.clone(),
                    state: HealthState::Warn,
                    reason: result
                        .error
                        .unwrap_or_else(|| "maintenance check_command failed".to_string()),
                    last_checked: now,
                    gate_results,
                },
            };
        }
    }

    DatumHealth {
        datum: datum.name.clone(),
        state: HealthState::Pass,
        reason: "no maintenance check configured".to_string(),
        last_checked: None,
        gate_results,
    }
}

fn print_datum_health(health: &DatumHealth) {
    let icon = match health.state {
        HealthState::Pass => "✅",
        HealthState::Warn => "⚠️ ",
        HealthState::Fail => "❌",
    };
    println!(
        "{icon} {} [{}] — {}",
        health.datum, health.state, health.reason
    );
}

fn handle_health_check(path: &str, name: Option<&str>, all: bool) -> Result<()> {
    if !all && name.is_none() {
        anyhow::bail!("datum health-check requires --name <datum> or --all");
    }

    let datums = datum_utils::get_all_datums(path)?;

    if all {
        for datum in datums.values() {
            let health = health_check_one(datum, path);
            print_datum_health(&health);
        }
        return Ok(());
    }

    let target = name.expect("checked above: name or all must be set");
    let datum = datums
        .get(target)
        .or_else(|| datums.values().find(|d| d.name == target))
        .with_context(|| format!("datum '{target}' not found"))?;
    let health = health_check_one(datum, path);
    print_datum_health(&health);
    Ok(())
}

fn handle_health_report(path: &str, format: &str) -> Result<()> {
    let datums = datum_utils::get_all_datums(path)?;

    let mut results: Vec<DatumHealth> = datums
        .values()
        .map(|datum| health_check_one(datum, path))
        .collect();
    results.sort_by(|a, b| a.datum.cmp(&b.datum));

    let pass = results
        .iter()
        .filter(|r| r.state == HealthState::Pass)
        .count();
    let warn = results
        .iter()
        .filter(|r| r.state == HealthState::Warn)
        .count();
    let fail = results
        .iter()
        .filter(|r| r.state == HealthState::Fail)
        .count();
    let (store_objects, store_bytes) = b00t_c0re_lib::store::status();

    match format {
        "json" => {
            let report = serde_json::json!({
                "datums": results,
                "summary": {
                    "pass": pass,
                    "warn": warn,
                    "fail": fail,
                    "total": results.len(),
                },
                "store": {
                    "objects": store_objects,
                    "bytes": store_bytes,
                },
            });
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        _ => {
            for health in &results {
                print_datum_health(health);
            }
            println!(
                "\n── health-report summary ──\n  pass: {pass}  warn: {warn}  fail: {fail}  total: {}\n  store: {store_objects} objects, {store_bytes} bytes",
                results.len()
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_health_datum(name: &str) -> crate::BootDatum {
        crate::BootDatum {
            name: name.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn health_check_one_passing_maintenance_command_yields_pass() {
        let mut datum = make_health_datum("healthy-cli");
        datum.maintenance = Some(crate::MaintenanceConfig {
            check_interval_days: Some(7),
            check_command: Some("true".to_string()),
            version_source: None,
            check_regex: None,
        });

        let health = health_check_one(&datum, "/tmp");

        assert_eq!(health.state, HealthState::Pass);
        assert_eq!(health.datum, "healthy-cli");
    }

    #[test]
    fn health_check_one_no_gate_no_maintenance_yields_pass() {
        // Most real datums have neither a [gate] nor a [maintenance] section —
        // this is the common-case default and should be a clean Pass, not an
        // accidental Fail/Warn from an empty gate list or missing check_command.
        let datum = make_health_datum("plain-cli");

        let health = health_check_one(&datum, "/tmp");

        assert_eq!(health.state, HealthState::Pass);
        assert_eq!(health.reason, "no maintenance check configured");
        assert!(health.gate_results.is_empty());
        assert!(health.last_checked.is_none());
    }

    #[test]
    fn health_check_one_failing_gate_yields_fail_with_gate_reason() {
        let mut datum = make_health_datum("gated-cli");
        datum.gate = Some(vec![crate::GateSpec {
            command: Some("definitely-not-a-real-command-xyz".to_string()),
            file: None,
            env: None,
            rhai: None,
            knowledge_backend: None,
            hint: Some("install definitely-not-a-real-command-xyz first".to_string()),
        }]);

        let health = health_check_one(&datum, "/tmp");

        // health_check_one always maps a failing gate to Fail (never Warn) —
        // asserting the exact state (rather than Fail|Warn) so a future
        // regression that muddles gate-fail vs. maintenance-warn is caught.
        assert_eq!(health.state, HealthState::Fail);
        assert!(health.reason.contains("definitely-not-a-real-command-xyz"));
        assert!(!health.gate_results.is_empty());
        assert!(!health.gate_results[0].passed);
    }

    #[test]
    fn health_report_aggregates_pass_warn_fail_counts() {
        let mut pass_datum = make_health_datum("pass-datum");
        pass_datum.maintenance = Some(crate::MaintenanceConfig {
            check_interval_days: Some(1),
            check_command: Some("true".to_string()),
            version_source: None,
            check_regex: None,
        });

        let mut warn_datum = make_health_datum("warn-datum");
        warn_datum.maintenance = Some(crate::MaintenanceConfig {
            check_interval_days: Some(1),
            check_command: Some("false".to_string()),
            version_source: None,
            check_regex: None,
        });

        let mut fail_datum = make_health_datum("fail-datum");
        fail_datum.gate = Some(vec![crate::GateSpec {
            command: Some("definitely-not-a-real-command-xyz".to_string()),
            file: None,
            env: None,
            rhai: None,
            knowledge_backend: None,
            hint: None,
        }]);

        let results: Vec<DatumHealth> = vec![&pass_datum, &warn_datum, &fail_datum]
            .into_iter()
            .map(|d| health_check_one(d, "/tmp"))
            .collect();

        let pass = results
            .iter()
            .filter(|r| r.state == HealthState::Pass)
            .count();
        let warn = results
            .iter()
            .filter(|r| r.state == HealthState::Warn)
            .count();
        let fail = results
            .iter()
            .filter(|r| r.state == HealthState::Fail)
            .count();

        assert_eq!(pass, 1, "expected exactly one Pass datum");
        assert_eq!(warn, 1, "expected exactly one Warn datum");
        assert_eq!(fail, 1, "expected exactly one Fail datum");
    }
}

#[cfg(test)]
mod govern_tests {
    use super::*;
    use tempfile::TempDir;

    /// Write a minimal `<name>.<type>.toml` datum fixture into `dir`.
    /// `extra` is inserted verbatim into the `[b00t]` table (e.g. `version = "..."`,
    /// `status = "disabled"`, or a trailing `[[b00t.gate]]` block).
    fn write_fixture(dir: &TempDir, name: &str, datum_type: &str, extra: &str) {
        let content = format!(
            "[b00t]\nname = \"{name}\"\ntype = \"{datum_type}\"\nhint = \"test datum {name}\"\n{extra}\n"
        );
        let filename = format!("{name}.{datum_type}.toml");
        std::fs::write(dir.path().join(filename), content).unwrap();
    }

    #[test]
    fn govern_one_healthy_cli_datum_passes() {
        let temp_dir = TempDir::new().unwrap();
        // File gate resolves relative to the datum directory (path is a dir here).
        std::fs::write(temp_dir.path().join("marker.txt"), "present").unwrap();
        write_fixture(
            &temp_dir,
            "healthy-cli",
            "cli",
            "version = \"healthy-cli --version\"\n\n[[b00t.gate]]\nfile = \"marker.txt\"\n",
        );

        let path = temp_dir.path().to_str().unwrap();
        let datum = datum_utils::find_datum_by_pattern(path, "healthy-cli")
            .unwrap()
            .expect("fixture datum should be found");

        let report = govern_one(&datum, path);

        assert!(report.proved.passed, "proof should pass: {}", report.proved.detail);
        assert_eq!(report.gated.len(), 1);
        assert!(report.gated[0].passed, "gate should pass: {}", report.gated[0].reason);
        assert!(report.hooked.passed, "no hook_detect should default to passed");
        assert!(report.healthy, "datum with passing proof/gate/hook should be healthy");
    }

    #[test]
    fn govern_one_missing_env_gate_marks_unhealthy() {
        let temp_dir = TempDir::new().unwrap();
        write_fixture(
            &temp_dir,
            "gated-cli",
            "cli",
            "version = \"gated-cli --version\"\n\n[[b00t.gate]]\nenv = \"B00T_GOVERN_TEST_MISSING_VAR_9d8f3a\"\n",
        );

        let path = temp_dir.path().to_str().unwrap();
        let datum = datum_utils::find_datum_by_pattern(path, "gated-cli")
            .unwrap()
            .expect("fixture datum should be found");

        // Ensure the gate's env var really is unset in this process.
        assert!(std::env::var("B00T_GOVERN_TEST_MISSING_VAR_9d8f3a").is_err());

        let report = govern_one(&datum, path);

        assert_eq!(report.gated.len(), 1);
        assert!(
            !report.gated[0].passed,
            "gate on a missing env var should fail"
        );
        assert!(!report.healthy, "unhealthy gate should make the datum unhealthy");
    }

    #[test]
    fn govern_all_skips_disabled_datums() {
        let temp_dir = TempDir::new().unwrap();
        write_fixture(&temp_dir, "active-cli", "cli", "version = \"active-cli --version\"\n");
        write_fixture(
            &temp_dir,
            "disabled-cli",
            "cli",
            "version = \"disabled-cli --version\"\nstatus = \"disabled\"\n",
        );

        let path = temp_dir.path().to_str().unwrap();
        let reports = govern_all(path).unwrap();

        assert!(
            reports.iter().any(|r| r.datum == "active-cli"),
            "active datum should be present in the report list"
        );
        assert!(
            !reports.iter().any(|r| r.datum == "disabled-cli"),
            "disabled datum should be skipped from --all governance"
        );
    }
}

#[cfg(test)]
mod datum_toml_subject_satisfies_tests {
    use super::*;

    fn table_from_toml(toml_str: &str) -> toml::value::Table {
        let value: toml::Value = toml::from_str(toml_str).unwrap();
        match value {
            toml::Value::Table(t) => t,
            _ => panic!("expected a TOML table"),
        }
    }

    #[test]
    fn clean_datum_is_satisfied() {
        let table = table_from_toml(
            r#"
name = "clean-cli"
type = "cli"
hint = "a clean datum"
"#,
        );
        let subject = DatumTomlSubject {
            raw: &table,
            filename: "clean-cli.cli.toml",
        };
        let result = subject.satisfies(&BootDatumSchemaConstraint { strict: true });
        assert!(result.is_satisfied(), "expected Satisfied, got {:?}", result.disposition);
    }

    #[test]
    fn missing_required_field_is_violated() {
        // No `hint` field — a required field per KNOWN_B00T_KEYS / handle_validate.
        let table = table_from_toml(
            r#"
name = "incomplete-cli"
type = "cli"
"#,
        );
        let subject = DatumTomlSubject {
            raw: &table,
            filename: "incomplete-cli.cli.toml",
        };
        let result = subject.satisfies(&BootDatumSchemaConstraint { strict: false });
        match result.disposition {
            Disposition::Violated { reason } => {
                assert!(reason.contains("hint"), "reason: {reason}");
            }
            other => panic!("expected Violated, got {:?}", other),
        }
    }

    #[test]
    fn recognized_type_unrecognized_extension_strict_is_unknown() {
        // type="cli" is a recognized type, but the fixture filename's
        // extension is not a recognized datum-type suffix — #927's fix:
        // this must surface as Unknown, not be silently skipped as before.
        let table = table_from_toml(
            r#"
name = "weird-ext-cli"
type = "cli"
hint = "a datum with an unrecognized filename extension"
"#,
        );
        let subject = DatumTomlSubject {
            raw: &table,
            filename: "weird-ext-cli.some-made-up-extension-xyz",
        };
        let result = subject.satisfies(&BootDatumSchemaConstraint { strict: true });
        assert!(
            matches!(result.disposition, Disposition::Unknown),
            "expected Unknown, got {:?}",
            result.disposition
        );
    }

    #[test]
    fn non_strict_mode_never_produces_unknown_for_extension_mismatch() {
        // Same fixture as above, but strict=false: the extension check is
        // skipped entirely (not even attempted), so it's Satisfied.
        let table = table_from_toml(
            r#"
name = "weird-ext-cli"
type = "cli"
hint = "a datum with an unrecognized filename extension"
"#,
        );
        let subject = DatumTomlSubject {
            raw: &table,
            filename: "weird-ext-cli.some-made-up-extension-xyz",
        };
        let result = subject.satisfies(&BootDatumSchemaConstraint { strict: false });
        assert!(result.is_satisfied());
    }

    #[test]
    fn ufo_stereotype_delegates_to_datum_type_lattice() {
        let table = table_from_toml(
            r#"
name = "docker-thing"
type = "docker"
hint = "t"
"#,
        );
        let subject = DatumTomlSubject {
            raw: &table,
            filename: "docker-thing.docker.toml",
        };
        assert_eq!(
            subject.ufo_stereotype(),
            DatumType::Docker.ufo_stereotype()
        );
    }

    #[test]
    fn ufo_stereotype_degrades_to_unknown_for_missing_type() {
        let table = table_from_toml(
            r#"
name = "typeless"
hint = "t"
"#,
        );
        let subject = DatumTomlSubject {
            raw: &table,
            filename: "typeless.toml",
        };
        assert_eq!(
            subject.ufo_stereotype(),
            UfoStereotype::Kind("Unknown".into())
        );
    }
}

#[cfg(test)]
mod validate_evidence_integration_tests {
    use super::*;
    use crate::commands::evidence::{read_evidence, with_test_evidence_log_path};
    use tempfile::TempDir;

    /// Proves the #927 wiring is real, not just computed and discarded:
    /// evaluating a `DatumTomlSubject` against `BootDatumSchemaConstraint`
    /// and recording NS-9/NS-10 evidence really appends rows to
    /// `satisfies.jsonl` (via the same `record_is_a`/`record_audited_by`
    /// sink `handle_validate` itself calls), with `object` strings prefixed
    /// `"isA:"` and `"audited_by:"` respectively.
    #[test]
    fn satisfies_evaluation_records_isa_and_audited_by_evidence() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("satisfies.jsonl");

        with_test_evidence_log_path(log_path, || {
            let table = toml::from_str::<toml::Value>(
                r#"
name = "evidence-cli"
type = "cli"
hint = "exercises the evidence sink"
"#,
            )
            .unwrap();
            let table = match table {
                toml::Value::Table(t) => t,
                _ => unreachable!(),
            };

            let subject = DatumTomlSubject {
                raw: &table,
                filename: "evidence-cli.cli.toml",
            };
            let constraint = BootDatumSchemaConstraint { strict: true };
            let target = "evidence-cli.cli";

            let _result = subject.satisfies(&constraint);
            crate::commands::evidence::record_is_a(target, &subject.ufo_stereotype().to_string())
                .unwrap();
            for iso_id in constraint.iso_standard_ids() {
                crate::commands::evidence::record_audited_by(target, &iso_id).unwrap();
            }

            let records = read_evidence().unwrap();
            assert!(
                records
                    .iter()
                    .any(|r| r.subject == target
                        && r.object.as_str().map_or(false, |o| o.starts_with("isA:"))),
                "expected an isA: evidence record, got: {:?}",
                records
            );
            assert!(
                records.iter().any(|r| r.subject == target
                    && r
                        .object
                        .as_str()
                        .map_or(false, |o| o.starts_with("audited_by:"))),
                "expected an audited_by: evidence record, got: {:?}",
                records
            );
        });
    }
}
