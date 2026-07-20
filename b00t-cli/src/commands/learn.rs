//! Unified learn command - intelligent knowledge management
//!
//! Combines LFMF lessons, curated docs, man pages, and RAG into one command

use anyhow::{Context, Result};
use b00t_c0re_lib::{
    DisplayOpts, GrokClient, KnowledgeSource, LfmfSystem, ManPage, lfmf::classify_init_failure,
};
use clap::Parser;
use std::fs;
use tiktoken_rs::o200k_base;
use toml::{self, Value};

use crate::get_expanded_path;

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

    // DWIW: emit MCP-optimized follow-up examples in results.
    // Also auto-enabled via B00T_MCP_CONTEXT=1 env var.
    #[arg(long, help = "Emit MCP follow-up examples in DWIW results")]
    pub mcp: bool,
}

pub async fn handle_learn(path: &str, args: LearnArgs) -> Result<()> {
    // 🤓 --role/--agent: scope this call's role context for downstream role-aware
    //    lookups (E1 skill-key evidence, RAG/grok query scoping) via the same
    //    _B00T_ROLE env var whoami's --role already relies on. Safety: this process
    //    is single-threaded through this point (before any spawned work reads the
    //    var), so a plain env write is sufficient — no async task has raced ahead.
    // Use agent from env if set; otherwise leave role unset
    if let Some(ref role) = std::env::var("_B00T_ROLE").ok() {
        unsafe {
            std::env::set_var("_B00T_ROLE", role);
        }
    }

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

    // Digest to RAG
    if let Some(ref content) = args.digest {
        return handle_digest(path, topic_val.as_deref(), &content).await;
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

    // E1: adaptive skip — if competency evidence exists, skip full load
    {
        let skill_key = if topic.contains('.') {
            topic.clone()
        } else {
            format!("{topic}.skill")
        };
        if let Ok(chain) = crate::commands::evidence::prove_skill(&skill_key) {
            if !chain.is_empty() {
                let latest = chain.last().unwrap();
                eprintln!(
                    "[learn:skip] {skill_key} already proven at {} — use --force to reload",
                    latest.timestamp
                );
                return Ok(());
            }
        }
    }

    handle_display(
        path,
        &topic,
        DisplayOpts {
            force_man: args.man,
            toc_only: args.toc,
            section: args.section,
            concise: args.concise,
        },
        args.mcp,
        args.limit,
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

    let _capabilities = registry.get("capabilities");

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

async fn handle_display(
    path: &str,
    topic: &str,
    opts: DisplayOpts,
    mcp: bool,
    limit: usize,
) -> Result<()> {
    let knowledge = KnowledgeSource::gather(topic, path).await?;

    // Auto-create datum if man page exists but no datum
    if knowledge.man_page.is_some() && !datum_exists(path, topic)? {
        if let Some(ref man) = knowledge.man_page {
            create_datum_from_man(path, topic, man)?;
            println!("✅ Auto-created datum: _b00t_/{}.cli.toml\n", topic);
        }
    }

    // DWIW: no curated datum → semantic search across all datums instead of hard-fail.
    // 🤓 LFMF always returns generic empty lessons → has_knowledge() stays true even for unknown
    //    topics; we gate on learn_content (actual datum soul page) being absent.
    let has_datum = knowledge.learn_content.is_some() || knowledge.man_page.is_some();
    if !has_datum {
        let mcp_ctx = mcp || std::env::var("B00T_MCP_CONTEXT").is_ok();
        return handle_dwiw(path, topic, mcp_ctx, limit).await;
    }

    knowledge.display(&opts)?;

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

/// DWIW fallback: no curated datum found → fanout query bus over all sources.
///
/// Pipeline:
///   1. Compile datum triples (BootDatum → SPO, one filesystem scan)
///   2. QueryBus::fanout → DatumSearchSource (keyword, w=3) + GraphAdjacencySource (horn FOL, w=2)
///   3. Collate by key: scores accumulate, best trust wins
///   4. Sm0l gate: if raw output > SM0L_TOKEN_THRESHOLD, summarize via sm0l_dispatch
///   5. Render: counts, trust grades, graph context, MCP examples (MCP ctx only)
///   6. Cache miss (0 results): queue research-soul task, warn about web trust
async fn handle_dwiw(path: &str, topic: &str, mcp_ctx: bool, limit: usize) -> Result<()> {
    use crate::datum_triples::compile_datum_triples;
    use crate::query_sources::{DatumSearchSource, GraphAdjacencySource};
    use b00t_c0re_lib::query_bus::QueryBus;
    use b00t_c0re_lib::query_bus::QueryContext;

    // Compile datum triples once — amortized across both sources
    let triples = compile_datum_triples(path).unwrap_or_default();
    let triple_count = triples.len();

    let ctx = QueryContext::new(topic, limit, triples);

    let bus = QueryBus::new()
        .with_source(DatumSearchSource::new(path))
        .with_source(GraphAdjacencySource::new(path, limit * 4));

    let ranked = bus.fanout(&ctx).await;

    if ranked.is_empty() {
        let exe = std::env::current_exe().ok();
        if let Some(bin) = exe {
            let _ = std::process::Command::new(&bin)
                .args(["task", "add", &format!("research-soul: {topic}")])
                .output();
        }
        println!("[learn:miss] no knowledge for '{topic}' (graph: {triple_count} triples)");
        println!("[learn:queued] research-soul: {topic}");
        println!("⚠️  web sources unverified — trust only [datum:user] grade results");
        anyhow::bail!("No knowledge found for '{topic}'. research-soul queued.");
    }

    let total = ranked.len();
    let shown = total.min(limit);

    // Sm0l gate: summarize if raw result bulk is large (>50 items or ≥8k token estimate).
    // 🤓 Rough estimate: each result line ≈ 20 tokens; 8k / 20 = 400 items before gate fires.
    //    We use 50 items as a practical gate (agent context is precious).
    const SM0L_ITEM_GATE: usize = 50;
    if total > SM0L_ITEM_GATE {
        let raw = ranked
            .iter()
            .map(|r| {
                format!(
                    "  {} [{}] (score={}) — {}",
                    r.key,
                    r.trust.as_str(),
                    r.score,
                    r.summary
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        use b00t_c0re_lib::sm0l_dispatch::{SmolBehavior, SmolConfig, SmolSession, dispatch};
        let session = SmolSession::new();
        let config = SmolConfig {
            max_output_lines: limit,
        };
        match dispatch(
            &SmolBehavior::Summarize,
            &config,
            &raw,
            Some(&session),
            32_000,
        ) {
            Ok(out) if out.result.is_some() => {
                println!(
                    "[learn:dwiw] {total} results for '{topic}' — sm0l summary (top {limit}):"
                );
                println!("[graph] {triple_count} triples compiled from datum graph");
                println!();
                println!("{}", out.result.unwrap());
                if mcp_ctx {
                    println!();
                    println!("[mcp:hint] raw results: b00t learn '{topic}' --limit={total}");
                }
                return Ok(());
            }
            _ => {
                // sm0l unavailable or empty result — fall through to standard display
            }
        }
    }

    println!("[learn:dwiw] {total} results for '{topic}'; showing top {shown}; --limit=N for more");
    println!("[graph] {triple_count} triples | sources: datum:search(w=3) graph:adjacency(w=2)");
    println!();

    for r in &ranked[..shown] {
        println!(
            "  {} [{}] (score={}) — {}",
            r.key,
            r.trust.as_str(),
            r.score,
            if r.summary.is_empty() {
                r.key.as_str()
            } else {
                r.summary.as_str()
            }
        );
        if let Some(ref reason) = r.match_reason {
            println!("    ↳ {reason}");
        }
    }

    if total > shown {
        println!(
            "\n  … {} more — use --limit={total} to see all",
            total - shown
        );
    }

    if mcp_ctx {
        if let Some(top) = ranked.first() {
            println!();
            println!("[mcp:hint] follow-up on top result '{}':", top.key);
            println!("  b00t learn --topic={}", top.key);
            println!("  b00t learn --topic={} --concise", top.key);
            println!("  b00t learn --topic={} --toc", top.key);
            println!("  b00t learn '{topic}' --limit={limit} --mcp  # expand this set");
        }
    }

    Ok(())
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

    // Try to initialize vector database; classify failure for actionable harness output
    if let Err(e) = lfmf_system.initialize().await {
        let (_, msg) = classify_init_failure(&e);
        println!("{}", msg);
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

    // Initialize vector DB; classify failure for actionable harness output
    if let Err(e) = lfmf_system.initialize().await {
        let (_, msg) = classify_init_failure(&e);
        println!("{}", msg);
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

    #[test]
    fn datum_path_expands_home_directory() {
        let home = dirs::home_dir().expect("expected home directory");
        let path = datum_path("~/.b00t/_b00t_", "git").expect("expected expanded datum path");

        assert_eq!(path, home.join(".b00t/_b00t_/git.cli.toml"));
    }
}
