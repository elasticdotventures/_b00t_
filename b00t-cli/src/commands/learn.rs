//! Unified learn command - intelligent knowledge management
//!
//! Combines LFMF lessons, curated docs, man pages, and RAG into one command

use anyhow::{Context, Result};
use b00t_c0re_lib::{DisplayOpts, GrokClient, KnowledgeSource, LfmfSystem, ManPage};
use clap::Parser;
use reqwest;
use std::fs;
use tiktoken_rs::o200k_base;
use toml::{self, Value};

use crate::datum_cli::CliDatum;
use crate::traits::DatumChecker;
use crate::{BootDatum, DatumType, get_config, get_expanded_path};

/// Arguments for the unified learn command.
///
/// Combines LFMF lessons, curated docs, man pages, and RAG into a single interface.
/// Supports recording lessons, searching, displaying knowledge, and RAG operations.
#[derive(Parser, Debug, Clone)]
pub struct LearnArgs {
    /// Topic to learn about (e.g., git, rust, just)
    #[arg(help = "Topic to learn about")]
    pub topic: Option<String>,

    // 🤓 --topic=<value> alias: MCP tools pass named flags; positional is for CLI users.
    //    Both are accepted; --topic wins if both somehow provided (MCP compat).
    #[arg(long = "topic", hide = true, conflicts_with = "topic")]
    pub topic_flag: Option<String>,

    // Display modifiers
    #[arg(long, help = "Force display man page")]
    pub man: bool,

    #[arg(long, help = "Show table of contents only")]
    pub toc: bool,

    #[arg(long, help = "Jump to specific section number")]
    pub section: Option<usize>,

    #[arg(long, help = "Concise token-optimized output")]
    pub concise: bool,

    // Record lesson (replaces lfmf)
    #[arg(long, help = "Record lesson: '<topic>: <body>'")]
    pub record: Option<String>,

    #[arg(long, help = "Record globally (default: repo)")]
    pub global: bool,

    // Search lessons (replaces advice)
    #[arg(long, help = "Search lessons: '<query>' or 'list'")]
    pub search: Option<String>,

    #[arg(long, help = "Max search results", default_value = "5")]
    pub limit: usize,

    // RAG operations (from grok)
    #[arg(long, help = "Digest content to RAG")]
    pub digest: Option<String>,

    #[arg(long, help = "Query RAG knowledgebase")]
    pub ask: Option<String>,

    // Unified capability routing
    #[arg(long, help = "List capabilities by type: skill|role|all")]
    pub list: bool,

    #[arg(long = "capability-type", help = "Filter by capability type")]
    pub capability_type: Option<String>,
}

pub async fn handle_learn(path: &str, args: LearnArgs) -> Result<()> {
    // 🤓 merge positional topic + --topic flag; --topic wins (MCP compat: passes named flags)
    let topic_val = args.topic_flag.or(args.topic);

    // Record lesson
    if let Some(ref lesson) = args.record {
        return handle_record(path, topic_val.as_deref(), &lesson, args.global).await;
    }

    // Search lessons
    if let Some(ref query) = args.search {
        return handle_search(path, topic_val.as_deref(), &query, args.limit).await;
    }

    // Digest to RAG — if content looks like a URL, fetch via Markdown-for-Agents first
    if let Some(ref content) = args.digest {
        let resolved = if content.starts_with("http://") || content.starts_with("https://") {
            fetch_as_markdown(content).await?
        } else {
            content.clone()
        };
        return handle_digest(path, topic_val.as_deref(), &resolved).await;
    }

    // Query RAG
    if let Some(ref query) = args.ask {
        return handle_ask(path, topic_val.as_deref(), &query, args.limit).await;
    }

    // Unified capability routing: --capability-type=skill|role|all
    if args.list || args.capability_type.is_some() {
        return handle_capability_list(path, args.list, args.capability_type.as_deref()).await;
    }

    // Default: display knowledge
    let topic =
        topic_val.ok_or_else(|| anyhow::anyhow!("Topic required. Use: b00t learn <topic>"))?;

    handle_display(
        path,
        &topic,
        DisplayOpts {
            force_man: args.man,
            toc_only: args.toc,
            section: args.section,
            concise: args.concise,
        },
    )
    .await
}

async fn handle_capability_list(path: &str, _list: bool, filter_type: Option<&str>) -> Result<()> {
    let registry_path = get_registry_path(path)?;
    if !registry_path.exists() {
        println!("No capability registry found. Run: b00t registry sync");
        return Ok(());
    }

    let content = fs::read_to_string(&registry_path)?;
    // Strip comment lines and empty lines for parsing
    let content_filtered: String = content
        .lines()
        .map(|l| l.trim_end())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    let registry: Value = match toml::from_str(&content_filtered) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "Warning: Failed to parse registry: {}. Run: b00t registry sync",
                e
            );
            return Ok(());
        }
    };

    let cap_type = filter_type.unwrap_or("all");

    fn get_desc(val: &toml::map::Map<String, toml::Value>) -> String {
        val.get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }

    fn get_tags_map(val: &toml::map::Map<String, toml::Value>) -> Vec<String> {
        val.get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    let capabilities = registry.get("capabilities");

    let show_all = cap_type == "all";
    let show_skills = show_all || cap_type == "skill";
    let show_roles = show_all || cap_type == "role";
    let show_datums = show_all || cap_type == "datum";
    let show_mcp = show_all || cap_type == "mcp";

    // Helper to get a section table
    fn get_section<'a>(
        registry: &'a toml::Value,
        section: &str,
    ) -> Option<&'a toml::map::Map<String, toml::Value>> {
        registry.get(section).and_then(|v| v.as_table())
    }

    if show_skills {
        if let Some(skills_map) = get_section(&registry, "skills") {
            println!("## Skills");
            for (name, val) in skills_map {
                if let Some(val_map) = val.as_table() {
                    let desc = get_desc(val_map);
                    let tags_vec = get_tags_map(val_map);
                    let tag_str = if tags_vec.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", tags_vec.join(", "))
                    };
                    println!("  • {} — {}{}", name, desc, tag_str);
                }
            }
            println!();
        }
    }

    if show_roles {
        if let Some(roles_map) = get_section(&registry, "roles") {
            println!("## Roles");
            for (name, val) in roles_map {
                if let Some(val_map) = val.as_table() {
                    let desc = get_desc(val_map);
                    let deps: Vec<String> = val_map
                        .get("depends_on")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    let dep_str = if deps.is_empty() {
                        String::new()
                    } else {
                        format!(" (→ {})", deps.join(", "))
                    };
                    println!("  • {}{} — {}", name, dep_str, desc);
                }
            }
            println!();
        }
    }

    if show_datums {
        if let Some(datums_map) = get_section(&registry, "datums") {
            println!("## Datums");
            for (name, val) in datums_map {
                if let Some(val_map) = val.as_table() {
                    let desc = get_desc(val_map);
                    let tags_vec = get_tags_map(val_map);
                    let tag_str = if tags_vec.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", tags_vec.join(", "))
                    };
                    println!("  • {} — {}{}", name, desc, tag_str);
                }
            }
            println!();
        }
    }

    if show_mcp {
        if let Some(mcps_map) = get_section(&registry, "mcp") {
            println!("## MCP Servers");
            for (name, val) in mcps_map {
                if let Some(val_map) = val.as_table() {
                    let desc = get_desc(val_map);
                    println!("  • {} — {}", name, desc);
                }
            }
            println!();
        }
    }

    println!("Run: b00t learn <name> to load a capability");
    println!("Run: b00t learn --capability-type=<type> to filter");

    Ok(())
}

fn get_registry_path(path: &str) -> Result<std::path::PathBuf> {
    let expanded = get_expanded_path(path)?;
    Ok(expanded.join("capability-registry.toml"))
}

async fn handle_display(path: &str, topic: &str, opts: DisplayOpts) -> Result<()> {
    let knowledge = KnowledgeSource::gather(topic, path).await?;
    let datum_insight = gather_datum_insight(path, topic);

    // Auto-create datum if man page exists but no datum
    if knowledge.man_page.is_some() && !datum_exists(path, topic)? {
        if let Some(ref man) = knowledge.man_page {
            create_datum_from_man(path, topic, man)?;
            println!("✅ Auto-created datum: _b00t_/{}.cli.toml\n", topic);
        }
    }

    // Check if any knowledge exists
    if !knowledge.has_knowledge() && datum_insight.is_none() {
        anyhow::bail!(
            "No knowledge found for '{}'. Try:\n  • b00t learn {} --record \"<topic>: <body>\"\n  • b00t learn {} --man (if man page exists)",
            topic,
            topic,
            topic
        );
    }

    knowledge.display(&opts)?;
    if let Some(insight) = datum_insight {
        display_datum_insight(&insight);
    }

    // Run hook_learn if present in datum
    if let Ok((config, _)) = crate::get_config(topic, path) {
        if let Some(script) = config.b00t.hook_learn {
            use crate::hook_engine::{HookResult, run_hook};
            match run_hook(&script) {
                HookResult::Ok => {}
                HookResult::Info(msg) | HookResult::Warn(msg) => println!("{}", msg),
                HookResult::Missing(msg) => println!("⚠️  {}", msg),
                HookResult::Redirect(_) => {}
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct DatumInsight {
    topic: String,
    selected_key: String,
    selected_file: String,
    datum_type: DatumType,
    hint: String,
    current_version: Option<String>,
    desired_version: Option<String>,
    version_emoji: Option<&'static str>,
    install_source: Option<&'static str>,
    references: Vec<(String, String)>,
    usages: Vec<(String, String)>,
    variants: Vec<String>,
}

fn gather_datum_insight(path: &str, topic: &str) -> Option<DatumInsight> {
    let variants = datum_variants(path, topic).ok()?;
    if variants.is_empty() {
        return None;
    }

    let selected_key = preferred_learn_variant(&variants);
    let (config, filename) = get_config(&selected_key, path).ok()?;
    let datum = config.b00t;
    let datum_type = datum.get_datum_type(Some(&filename));
    let version_probe = cli_version_probe(path, &selected_key, &datum);

    Some(DatumInsight {
        topic: topic.to_string(),
        selected_key,
        selected_file: filename,
        datum_type,
        hint: datum.hint.clone(),
        current_version: version_probe.as_ref().and_then(|v| v.0.clone()),
        desired_version: version_probe.as_ref().and_then(|v| v.1.clone()),
        version_emoji: version_probe.as_ref().map(|v| v.2),
        install_source: install_source_label(&datum),
        references: datum
            .references
            .unwrap_or_default()
            .into_iter()
            .map(|r| (r.name, r.url))
            .collect(),
        usages: datum
            .usage
            .unwrap_or_default()
            .into_iter()
            .map(|u| (u.description, u.command))
            .collect(),
        variants,
    })
}

fn cli_version_probe(
    path: &str,
    selected_key: &str,
    datum: &BootDatum,
) -> Option<(Option<String>, Option<String>, &'static str)> {
    let cli_key = if selected_key.ends_with(".cli") {
        Some(selected_key.to_string())
    } else if datum.datum_type == Some(DatumType::Cli) {
        Some(selected_key.to_string())
    } else {
        None
    }?;

    let cli = CliDatum::from_config(&cli_key, path).ok()?;
    let status = cli.version_status();
    Some((cli.current_version(), cli.desired_version(), status.emoji()))
}

fn install_source_label(datum: &BootDatum) -> Option<&'static str> {
    let install = datum.install_command()?;
    if install.contains("opencode.ai/install") {
        Some("upstream CLI installer")
    } else if install.contains("opencode-ai@latest") {
        Some("npm package opencode-ai")
    } else if install.contains("pnpm") {
        Some("pnpm package install")
    } else {
        Some("custom install command")
    }
}

fn preferred_learn_variant(variants: &[String]) -> String {
    variants
        .iter()
        .find(|v| v.ends_with(".cli"))
        .cloned()
        .unwrap_or_else(|| variants[0].clone())
}

fn datum_variants(path: &str, topic: &str) -> Result<Vec<String>> {
    let expanded = get_expanded_path(path)?;
    let mut variants = Vec::new();

    if let Ok(entries) = fs::read_dir(expanded) {
        for entry in entries {
            let entry = entry?;
            let file_name = match entry.file_name().to_str() {
                Some(name) => name.to_string(),
                None => continue,
            };
            if let Some(key) = datum_variant_key_for_topic(topic, &file_name) {
                variants.push(key);
            }
        }
    }

    variants.sort();
    variants.dedup();
    Ok(variants)
}

fn datum_variant_key_for_topic(topic: &str, file_name: &str) -> Option<String> {
    for ext in [".tomllmd", ".tomllm", ".toml"] {
        let Some(stem) = file_name.strip_suffix(ext) else {
            continue;
        };
        if stem == topic || !stem.starts_with(&format!("{topic}.")) {
            continue;
        }
        if stem.ends_with(".stack") {
            continue;
        }
        return Some(stem.to_string());
    }
    None
}

fn display_datum_insight(insight: &DatumInsight) {
    println!("## 🧩 Datum Inquiry\n");
    println!("- Topic: `{}`", insight.topic);
    println!("- Selected datum: `{}`", insight.selected_key);
    println!(
        "- Type: `{}` from `{}`",
        insight.datum_type.base_suffix().trim_start_matches('.'),
        insight.selected_file
    );
    println!("- Hint: {}", insight.hint);

    if let Some(source) = insight.install_source {
        println!("- Install source: {}", source);
    }

    if let Some(current) = &insight.current_version {
        let status = insight.version_emoji.unwrap_or("");
        if let Some(desired) = &insight.desired_version {
            println!(
                "- Version: {} current=`{}` desired=`{}`",
                status, current, desired
            );
        } else {
            println!("- Version: {} current=`{}`", status, current);
        }
    } else if let Some(desired) = &insight.desired_version {
        println!("- Version: desired=`{}` current=not-detected", desired);
    }

    if !insight.variants.is_empty() {
        println!("- Variants: {}", insight.variants.join(", "));
    }

    if !insight.references.is_empty() {
        println!("\n### References\n");
        for (name, url) in insight.references.iter().take(4) {
            println!("- {} — {}", name, url);
        }
    }

    if !insight.usages.is_empty() {
        println!("\n### Executable Skills\n");
        for (description, command) in insight.usages.iter().take(4) {
            println!("- {}: `{}`", description, command);
        }
    }

    println!();
}

async fn handle_record(path: &str, topic: Option<&str>, lesson: &str, global: bool) -> Result<()> {
    let topic = topic.ok_or_else(|| anyhow::anyhow!("Topic required for recording lesson"))?;

    // Parse "<topic>: <body>" format
    let parts: Vec<&str> = lesson.splitn(2, ':').map(|s| s.trim()).collect();
    if parts.len() != 2 {
        anyhow::bail!(
            "Lesson must be in '<topic>: <body>' format.\n\nExample:\n  b00t learn git --record \"atomic commits: Commit small, focused changes for easier review\""
        );
    }

    let lesson_topic = parts[0];
    let body = parts[1];

    // Token count enforcement (using tiktoken)
    let bpe = o200k_base().map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {e}"))?;
    let topic_tokens = bpe.encode_with_special_tokens(lesson_topic).len();
    let body_tokens = bpe.encode_with_special_tokens(body).len();

    if topic_tokens > 25 {
        anyhow::bail!(
            "Topic must be <25 tokens (OpenAI tiktoken). Yours: {}.",
            topic_tokens
        );
    }

    if body_tokens > 250 {
        anyhow::bail!(
            "Body must be <250 tokens (OpenAI tiktoken). Yours: {}.",
            body_tokens
        );
    }

    if lesson_topic.is_empty() || body.is_empty() {
        anyhow::bail!("Topic and body must not be empty.");
    }

    // Affirmative style check
    if body.to_lowercase().contains("don't") || body.to_lowercase().contains("never") {
        println!("⚠️  Consider using positive, affirmative style (e.g., 'Do X for Y benefit').\n");
    }

    // Use LFMF system for recording
    let config = LfmfSystem::load_config(path)?;
    let mut lfmf_system = LfmfSystem::new(config);

    // Set datum lookup for category resolution
    let lookup = crate::datum_utils::B00tDatumLookup::new(path.to_string());
    lfmf_system.set_datum_lookup(lookup);

    // Try to initialize vector database (non-fatal if fails)
    if let Err(e) = lfmf_system.initialize().await {
        println!(
            "⚠️  Vector database unavailable: {}. Lesson will be saved to filesystem only.",
            e
        );
    }

    let scope = if global { "global" } else { "repo" };
    println!("Scope: {}", scope);

    lfmf_system
        .record_lesson(topic, lesson)
        .await
        .context("Failed to record lesson")?;

    println!("✅ Recorded lesson for '{}': {}", topic, lesson_topic);
    println!("\nView: b00t learn {} --search list", topic);

    Ok(())
}

async fn handle_search(path: &str, topic: Option<&str>, query: &str, limit: usize) -> Result<()> {
    let topic = topic.ok_or_else(|| anyhow::anyhow!("Topic required for searching lessons"))?;

    let config = LfmfSystem::load_config(path)?;
    let mut lfmf_system = LfmfSystem::new(config);

    let lookup = crate::datum_utils::B00tDatumLookup::new(path.to_string());
    lfmf_system.set_datum_lookup(lookup);

    // Initialize vector DB (non-fatal if fails)
    if let Err(e) = lfmf_system.initialize().await {
        println!(
            "🔄 Vector database unavailable ({}), using filesystem fallback",
            e
        );
    }

    let results = if query.eq_ignore_ascii_case("list") {
        // List all lessons
        lfmf_system.list_lessons(topic, Some(limit)).await?
    } else {
        // Search lessons using get_advice
        lfmf_system.get_advice(topic, query, Some(limit)).await?
    };

    if results.is_empty() {
        println!("No lessons found for '{}'", topic);
        println!(
            "\nRecord one: b00t learn {} --record \"<topic>: <body>\"",
            topic
        );
        return Ok(());
    }

    println!("## Lessons for '{}' ({} total)\n", topic, results.len());
    for (idx, lesson) in results.iter().enumerate() {
        println!("{}. {}", idx + 1, lesson);
    }

    Ok(())
}

/// Fetch a URL preferring `text/markdown` (Cloudflare Markdown-for-Agents / content negotiation).
/// Sends `Accept: text/markdown, text/html;q=0.9` — servers that support it return markdown
/// directly (80% token reduction vs HTML). Falls back to raw body for non-markdown responses.
/// Logs `x-markdown-tokens` hint when available.
async fn fetch_as_markdown(url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("b00t-cli/learn (Mozilla/5.0 compatible; AI agent)")
        .build()
        .context("Failed to build HTTP client")?;

    let resp = client
        .get(url)
        .header(
            reqwest::header::ACCEPT,
            "text/markdown, text/html;q=0.9, */*;q=0.8",
        )
        .send()
        .await
        .with_context(|| format!("Failed to fetch {}", url))?;

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    if let Some(tokens) = resp
        .headers()
        .get("x-markdown-tokens")
        .and_then(|v| v.to_str().ok())
    {
        eprintln!("📄 Markdown-for-Agents: ~{} tokens ({})", tokens, url);
    } else if content_type.contains("text/markdown") {
        eprintln!("📄 text/markdown received from {}", url);
    } else {
        eprintln!(
            "📄 Fetched {} ({}) — no markdown negotiation",
            url, content_type
        );
    }

    resp.text()
        .await
        .with_context(|| format!("Failed to read response body from {}", url))
}

async fn handle_digest(_path: &str, topic: Option<&str>, content: &str) -> Result<()> {
    let topic = topic
        .ok_or_else(|| anyhow::anyhow!("Topic required for digesting content to RAG"))?
        .to_string();

    let client = GrokClient::new();

    client
        .digest(&topic, content)
        .await
        .context("Failed to digest content")?;

    println!("✅ Digested content to RAG under topic '{}'", topic);
    println!("\nQuery: b00t learn {} --ask \"<question>\"", topic);

    Ok(())
}

async fn handle_ask(_path: &str, topic: Option<&str>, query: &str, limit: usize) -> Result<()> {
    let client = GrokClient::new();

    let results = client
        .ask(query, topic, Some(limit))
        .await
        .context("Failed to query RAG")?;

    if results.results.is_empty() {
        println!("No results found for query: '{}'", query);
        return Ok(());
    }

    println!("## RAG Results for '{}'\n", query);
    for (idx, chunk) in results.results.iter().enumerate() {
        println!(
            "{}. (topic: {})\n   {}\n",
            idx + 1,
            chunk.topic,
            chunk.content.lines().next().unwrap_or("")
        );
    }

    Ok(())
}

/// Check if datum exists for topic
fn datum_exists(path: &str, topic: &str) -> Result<bool> {
    let datum_path = datum_path(path, topic)?;
    Ok(datum_path.exists())
}

/// Create datum from man page
fn create_datum_from_man(path: &str, topic: &str, man: &ManPage) -> Result<()> {
    let datum_content = man.to_datum_toml();
    let datum_path = datum_path(path, topic)?;
    fs::write(&datum_path, datum_content).context("Failed to write datum file")?;
    Ok(())
}

fn datum_path(path: &str, topic: &str) -> Result<std::path::PathBuf> {
    Ok(crate::get_expanded_path(path)?.join(format!("{}.cli.toml", topic)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn datum_path_expands_home_directory() {
        let home = dirs::home_dir().expect("expected home directory");
        let path = datum_path("~/.b00t/_b00t_", "git").expect("expected expanded datum path");

        assert_eq!(path, home.join(".b00t/_b00t_/git.cli.toml"));
    }

    #[test]
    fn datum_variant_key_for_topic_extracts_typed_variants() {
        assert_eq!(
            datum_variant_key_for_topic("opencode", "opencode.cli.toml"),
            Some("opencode.cli".to_string())
        );
        assert_eq!(
            datum_variant_key_for_topic("opencode", "opencode.agent.tomllmd"),
            Some("opencode.agent".to_string())
        );
        assert_eq!(
            datum_variant_key_for_topic("opencode", "other.cli.toml"),
            None
        );
    }

    #[test]
    fn preferred_learn_variant_prefers_cli() {
        let variants = vec!["opencode.agent".to_string(), "opencode.cli".to_string()];
        assert_eq!(preferred_learn_variant(&variants), "opencode.cli");
    }

    #[test]
    fn datum_variants_lists_topic_variants() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("opencode.cli.toml"),
            "[b00t]\nname=\"opencode\"\ntype=\"cli\"\nhint=\"x\"\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("opencode.agent.toml"),
            "[b00t]\nname=\"opencode\"\ntype=\"agent\"\nhint=\"x\"\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("other.cli.toml"),
            "[b00t]\nname=\"other\"\ntype=\"cli\"\nhint=\"x\"\n",
        )
        .unwrap();

        let variants = datum_variants(dir.path().to_str().unwrap(), "opencode").unwrap();
        assert_eq!(
            variants,
            vec!["opencode.agent".to_string(), "opencode.cli".to_string()]
        );
    }
}
