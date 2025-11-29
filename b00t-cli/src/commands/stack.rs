use anyhow::{Context, Result, anyhow};
use clap::Parser;
use regex::Regex;
use std::collections::HashMap;

use crate::datum_stack::StackDatum;
use crate::dependency_resolver::DependencyResolver;
use crate::traits::DatumCrdDisplay;
use crate::{
    BootDatum, ansible::AnsibleConfig, ansible::run_playbook, get_config, get_expanded_path,
};

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
    #[clap(
        about = "Generate k8s CRD from stack",
        long_about = "Generate a Kubernetes Custom Resource Definition YAML from a stack.\n\nThis enables MBSE-based stack → pod transformation for AI/ML pipelines.\n\nExamples:\n  b00t-cli stack to-crd llm-inference-pipeline\n  b00t-cli stack to-crd llm-inference-pipeline --output stack.yaml\n  b00t-cli stack to-crd llm-inference-pipeline --pod-only"
    )]
    ToCrd {
        #[clap(help = "Stack name")]
        name: String,
        #[clap(long, short = 'o', help = "Output file (default: stdout)")]
        output: Option<String>,
        #[clap(long, help = "Generate only pod spec (not full CRD)")]
        pod_only: bool,
    },
    #[clap(
        about = "Convert stack to k8s manifests via kompose",
        long_about = "Convert stack to Kubernetes manifests using kompose.\n\nFlow: Stack → docker-compose → kompose → k8s manifests + orchestration metadata\n\nThis uses real container images from docker members, eliminating guesswork.\n\nExamples:\n  b00t-cli stack to-k8s postgres-dev-stack\n  b00t-cli stack to-k8s postgres-dev-stack --output-dir ./k8s\n  b00t-cli stack to-k8s llm-pipeline --enhance"
    )]
    ToK8s {
        #[clap(help = "Stack name")]
        name: String,
        #[clap(
            long,
            help = "Output directory for k8s manifests (default: ./<stack>-k8s)"
        )]
        output_dir: Option<String>,
        #[clap(
            long,
            help = "Enhance with b00t orchestration metadata (GPU affinity, budget)"
        )]
        enhance: bool,
    },
    #[clap(
        about = "Run an Ansible playbook from stack context",
        long_about = "Runs either a raw playbook path or a datum-backed playbook through the shared Ansible helper.\n\nExamples:\n  b00t-cli stack ansible --run script ansible/playbooks/k0s_kata.yaml -- -i inventory\n  b00t-cli stack ansible --run datum k0s -- k0s_role=worker"
    )]
    Ansible {
        #[clap(
            long,
            help = "Specify whether to run a script path or stack datum (script|datum)",
            value_parser = ["script", "datum"]
        )]
        run: String,
        #[clap(help = "Playbook path or datum name")]
        target: String,
        #[clap(
            last = true,
            help = "Parameters forwarded to ansible-playbook (extra args or key=value vars)"
        )]
        params: Vec<String>,
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
            StackCommands::ToCrd {
                name,
                output,
                pod_only,
            } => generate_crd(name, path, output.as_deref(), *pod_only),
            StackCommands::ToK8s {
                name,
                output_dir,
                enhance,
            } => generate_k8s_via_kompose(name, path, output_dir.as_deref(), *enhance),
            StackCommands::Ansible {
                run,
                target,
                params,
            } => run_stack_ansible(run, target, params, path),
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
                    eprintln!("  ❌ {} (error: {})", stack_path.display(), e);
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
        if name.contains('*') || name.contains('?') {
            return install_datums_by_pattern(name, path, dry_run);
        }
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

fn install_datums_by_pattern(pattern: &str, path: &str, dry_run: bool) -> Result<()> {
    let available_datums = load_all_datums(path)?;
    let regex = build_pattern_regex(pattern)?;
    let matched_keys: Vec<String> = available_datums
        .keys()
        .filter(|key| regex.is_match(key))
        .cloned()
        .collect();

    if matched_keys.is_empty() {
        anyhow::bail!("No datums match pattern '{}'", pattern);
    }

    let datum_refs: Vec<&BootDatum> = available_datums.values().collect();
    let resolver = DependencyResolver::new(datum_refs);
    let install_order = resolver.resolve_many(&matched_keys)?;

    println!(
        "📦 Installing datums matching pattern '{}' ({} items):",
        pattern,
        install_order.len()
    );
    for (idx, member) in install_order.iter().enumerate() {
        println!("   {}. {}", idx + 1, member);
    }

    if dry_run {
        return Ok(());
    }

    for member in install_order {
        if let Some(datum) = available_datums.get(&member) {
            run_stack_ansible("datum", &datum.name, &[], path)?;
        }
    }

    Ok(())
}

fn build_pattern_regex(pattern: &str) -> Result<Regex> {
    let escaped = regex::escape(pattern);
    let regex_str = format!("^{}$", escaped.replace(r"\*", ".*").replace(r"\?", "."));
    let regex = Regex::new(&regex_str).context("Failed to compile datum pattern")?;
    Ok(regex)
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

/// Generate k8s CRD from stack (MBSE stack → pod transformation)
fn generate_crd(name: &str, path: &str, output_file: Option<&str>, pod_only: bool) -> Result<()> {
    let stack_path = get_expanded_path(path)?.join(format!("{}.stack.toml", name));

    if !stack_path.exists() {
        anyhow::bail!("Stack '{}' not found at {}", name, stack_path.display());
    }

    let stack = StackDatum::from_file(&stack_path)?;

    let yaml = if pod_only {
        stack.to_pod_spec().context("Failed to generate pod spec")?
    } else {
        stack
            .to_crd_template()
            .context("Failed to generate CRD template")?
    };

    if let Some(output_path) = output_file {
        std::fs::write(output_path, &yaml)
            .context(format!("Failed to write to {}", output_path))?;

        if pod_only {
            println!("✅ Generated pod spec: {}", output_path);
        } else {
            println!("✅ Generated k8s CRD: {}", output_path);
        }

        // Show resource requirements summary
        let reqs = stack.get_resource_requirements();
        if reqs.cpu.is_some() || reqs.memory.is_some() || reqs.gpu_count.is_some() {
            println!("\n📊 Resource Requirements:");
            if let Some(cpu) = &reqs.cpu {
                println!("   CPU: {}", cpu);
            }
            if let Some(memory) = &reqs.memory {
                println!("   Memory: {}", memory);
            }
            if let Some(gpu_count) = reqs.gpu_count {
                println!(
                    "   GPU: {} x {}",
                    gpu_count,
                    reqs.gpu_type.as_deref().unwrap_or("nvidia.com/gpu")
                );
            }
        }

        // Show affinity strategy
        let affinity = stack.get_affinity_rules();
        if let Some(batch_group) = &affinity.gpu_batch_group {
            println!("\n🔗 GPU Batch Group: {}", batch_group);
            println!("   Strategy: {:?}", affinity.strategy);
        }

        // Show budget constraints
        if let Some(budget) = stack.get_budget_constraints() {
            println!("\n💰 Budget Constraints:");
            println!(
                "   Daily Limit: {:.2} {}",
                budget.daily_limit, budget.currency
            );
            println!(
                "   Cost per Job: {:.2} {}",
                budget.cost_per_job, budget.currency
            );
            println!("   On Exceeded: {}", budget.on_exceeded);
        }
    } else {
        println!("{}", yaml);
    }

    Ok(())
}

/// Convert stack to k8s manifests via kompose (stack → compose → k8s)
fn generate_k8s_via_kompose(
    name: &str,
    path: &str,
    output_dir: Option<&str>,
    enhance: bool,
) -> Result<()> {
    let stack_path = get_expanded_path(path)?.join(format!("{}.stack.toml", name));

    if !stack_path.exists() {
        anyhow::bail!("Stack '{}' not found at {}", name, stack_path.display());
    }

    let stack = StackDatum::from_file(&stack_path)?;
    let available_datums = load_all_datums(path)?;

    println!(
        "🔄 Converting stack '{}' to k8s manifests via kompose...",
        name
    );

    // Step 1: Generate docker-compose.yml
    println!("   1/3 Generating docker-compose.yml...");
    let compose_yaml = stack.generate_docker_compose(&available_datums)?;

    // Write to temp file
    let temp_compose = format!("/tmp/{}-compose.yml", name);
    std::fs::write(&temp_compose, &compose_yaml)
        .context("Failed to write temporary docker-compose.yml")?;

    // Step 2: Run kompose convert
    println!("   2/3 Running kompose convert...");

    // Check if kompose is installed
    let kompose_check = std::process::Command::new("kompose")
        .arg("version")
        .output();

    if kompose_check.is_err() {
        anyhow::bail!(
            "kompose not found. Install with: b00t cli install kompose\n\
             Or download from: https://github.com/kubernetes/kompose"
        );
    }

    let output_directory = output_dir
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("./{}-k8s", name));

    // Create output directory
    std::fs::create_dir_all(&output_directory).context(format!(
        "Failed to create output directory: {}",
        output_directory
    ))?;

    // Run kompose convert
    let kompose_result = std::process::Command::new("kompose")
        .arg("convert")
        .arg("-f")
        .arg(&temp_compose)
        .arg("-o")
        .arg(&output_directory)
        .output()
        .context("Failed to run kompose convert")?;

    if !kompose_result.status.success() {
        let stderr = String::from_utf8_lossy(&kompose_result.stderr);
        anyhow::bail!("kompose convert failed: {}", stderr);
    }

    // Step 3: Enhance with orchestration metadata (if requested)
    if enhance {
        println!("   3/3 Enhancing with b00t orchestration metadata...");
        enhance_k8s_manifests(&stack, &output_directory)?;
    } else {
        println!("   3/3 Skipping enhancement (use --enhance to add orchestration metadata)");
    }

    println!("\n✅ Generated k8s manifests in: {}", output_directory);

    // Show what was generated
    let manifest_files: Vec<_> = std::fs::read_dir(&output_directory)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s == "yaml" || s == "yml")
                .unwrap_or(false)
        })
        .collect();

    println!("\n📄 Generated files ({}):", manifest_files.len());
    for file in &manifest_files {
        println!("   - {}", file.file_name().to_string_lossy());
    }

    // Show resource requirements if enhanced
    if enhance {
        let reqs = stack.get_resource_requirements();
        if reqs.cpu.is_some() || reqs.memory.is_some() || reqs.gpu_count.is_some() {
            println!("\n📊 Enhanced with Resource Requirements:");
            if let Some(cpu) = &reqs.cpu {
                println!("   CPU: {}", cpu);
            }
            if let Some(memory) = &reqs.memory {
                println!("   Memory: {}", memory);
            }
            if let Some(gpu_count) = reqs.gpu_count {
                println!(
                    "   GPU: {} x {}",
                    gpu_count,
                    reqs.gpu_type.as_deref().unwrap_or("nvidia.com/gpu")
                );
            }
        }

        let affinity = stack.get_affinity_rules();
        if affinity.gpu_batch_group.is_some() {
            println!("\n🔗 Enhanced with GPU Batch Affinity");
        }

        if let Some(budget) = stack.get_budget_constraints() {
            println!("\n💰 Enhanced with Budget Constraints:");
            println!(
                "   Daily Limit: {:.2} {}",
                budget.daily_limit, budget.currency
            );
        }
    }

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_compose);

    println!("\n💡 Next steps:");
    println!("   kubectl apply -f {}", output_directory);
    if !enhance {
        println!("   Or run with --enhance to add GPU affinity and budget metadata");
    }

    Ok(())
}

fn run_stack_ansible(run: &str, target: &str, params: &[String], path: &str) -> Result<()> {
    let workspace = get_expanded_path(path)?;
    let mut config = AnsibleConfig::default();

    if run == "datum" {
        let (cfg, _) = get_config(target, path).map_err(|e| {
            anyhow!(
                "Failed to load datum '{target}': {e}",
                target = target,
                e = e
            )
        })?;
        config = cfg
            .b00t
            .ansible
            .ok_or_else(|| anyhow!("Datum '{target}' has no [ansible] section", target = target))?;
    } else {
        config.playbook = target.to_string();
    }

    let mut extra_args = config.extra_args.take().unwrap_or_default();
    let mut extra_vars = config.extra_vars.take().unwrap_or_default();

    for param in params {
        if let Some((key, value)) = param.split_once('=') {
            extra_vars.insert(key.to_string(), value.to_string());
        } else {
            extra_args.push(param.clone());
        }
    }

    if !extra_args.is_empty() {
        config.extra_args = Some(extra_args);
    }

    if !extra_vars.is_empty() {
        config.extra_vars = Some(extra_vars);
    }

    run_playbook(&config, Some(workspace.as_path()))
}

/// Enhance kompose-generated k8s manifests with b00t orchestration metadata
fn enhance_k8s_manifests(stack: &StackDatum, output_dir: &str) -> Result<()> {
    use serde_yaml::Value;

    let budget = stack.get_budget_constraints();
    let affinity = stack.get_affinity_rules();

    // Find all deployment YAMLs
    for entry in std::fs::read_dir(output_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) != Some("yaml") {
            continue;
        }

        // Check if it's a Deployment
        let content = std::fs::read_to_string(&path)?;
        if !content.contains("kind: Deployment") {
            continue;
        }

        // Parse YAML
        let mut doc: Value = serde_yaml::from_str(&content)?;

        // Add budget annotations to metadata
        if let Some(budget_constraints) = &budget {
            if let Some(metadata) = doc.get_mut("metadata") {
                if let Some(metadata_map) = metadata.as_mapping_mut() {
                    let annotations_key = Value::String("annotations".to_string());

                    // Get or create annotations map
                    let annotations = metadata_map
                        .entry(annotations_key)
                        .or_insert_with(|| Value::Mapping(Default::default()));

                    if let Some(annot_map) = annotations.as_mapping_mut() {
                        annot_map.insert(
                            Value::String("b00t.io/budget-daily-limit".to_string()),
                            Value::String(budget_constraints.daily_limit.to_string()),
                        );
                        annot_map.insert(
                            Value::String("b00t.io/budget-cost-per-job".to_string()),
                            Value::String(budget_constraints.cost_per_job.to_string()),
                        );
                        annot_map.insert(
                            Value::String("b00t.io/budget-currency".to_string()),
                            Value::String(budget_constraints.currency.clone()),
                        );
                    }
                }
            }
        }

        // Add GPU batch group label to pod template
        if let Some(batch_group) = &affinity.gpu_batch_group {
            if let Some(spec) = doc.get_mut("spec") {
                if let Some(template) = spec.get_mut("template") {
                    if let Some(template_metadata) = template.get_mut("metadata") {
                        if let Some(metadata_map) = template_metadata.as_mapping_mut() {
                            let labels_key = Value::String("labels".to_string());

                            // Get or create labels map
                            let labels = metadata_map
                                .entry(labels_key)
                                .or_insert_with(|| Value::Mapping(Default::default()));

                            if let Some(labels_map) = labels.as_mapping_mut() {
                                labels_map.insert(
                                    Value::String("b00t.io/gpu-batch-group".to_string()),
                                    Value::String(batch_group.clone()),
                                );
                            }
                        }
                    }
                }
            }
        }

        // Write back
        let output = serde_yaml::to_string(&doc)?;
        std::fs::write(&path, output)?;
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
                            let datum_type = datum
                                .datum_type
                                .as_ref()
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
