use anyhow::{Context, Result};
use b00t_c0re_lib::{DatumNode, DualGrokClient, GrokBackend, IrontologyBridgeClient};
use clap::Subcommand;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::{fs, io::Read, io::Write as IoWrite, path::PathBuf};
use uuid::Uuid;

#[derive(Subcommand, Clone)]
pub enum GrokCommands {
    /// Digest content into chunks about a topic
    ///
    /// Default: fan-out to both RAGLight + Irontology backends.
    ///
    /// Examples:
    ///   b00t grok digest -t rust "Rust ensures memory safety"
    ///   b00t grok digest -t python "duck typing" --rag=raglite
    ///   b00t grok digest -t rust "ownership" --rag=irontology
    Digest {
        /// Topic to digest content about (must be a known b00t datum topic)
        #[arg(short, long)]
        topic: String,
        /// Content to digest (positional)
        content: String,
        /// Backend: raglite, irontology, or both (default: both)
        #[arg(
            long = "rag",
            value_name = "BACKEND",
            num_args = 0..=1,
            default_missing_value = "both",
            help = "Backend: raglite | irontology | both (default: both)"
        )]
        rag: Option<String>,
    },
    /// Ask questions and search the knowledgebase
    ///
    /// Examples:
    ///   b00t grok ask "memory safety patterns" -t rust
    ///   b00t grok ask "error handling" -t rust --limit 5
    ///   b00t grok ask "async" -t rust --rag=irontology
    Ask {
        /// Query to search for
        query: String,
        /// Optional topic to filter by
        #[arg(short, long)]
        topic: Option<String>,
        /// Maximum results to return (default: 10)
        #[arg(long)]
        limit: Option<usize>,
        /// Backend: raglite, irontology, or both (default: both)
        #[arg(
            long = "rag",
            value_name = "BACKEND",
            num_args = 0..=1,
            default_missing_value = "both",
            help = "Backend: raglite | irontology | both (default: both)"
        )]
        rag: Option<String>,
    },
    /// Learn from URLs or files
    ///
    /// Examples:
    ///   b00t grok learn -s "notes.md" "$(cat notes.md)" -t rust
    ///   b00t grok learn "Rust is a systems programming language" -t rust
    Learn {
        /// Source URL or file path
        #[arg(short, long)]
        source: Option<String>,
        /// Content to learn from (positional; required unless --source is given)
        #[arg(required_unless_present = "source")]
        content: Option<String>,
        /// Topic to associate with ingested content
        #[arg(short, long)]
        topic: Option<String>,
        /// Backend: raglite, irontology, or both (default: both)
        #[arg(
            long = "rag",
            value_name = "BACKEND",
            num_args = 0..=1,
            default_missing_value = "both",
            help = "Backend: raglite | irontology | both (default: both)"
        )]
        rag: Option<String>,
    },
    /// Assimilate content: LLM-distill → store as git blob → write datum TOML
    ///
    /// Content is stored ONLY in git object store (not filesystem).
    /// Datum TOML written to _b00t_/<topic>-<uuid>.datum.toml with git_hash field.
    /// Validate: git cat-file -e <git_hash>
    ///
    /// Examples:
    ///   b00t grok assimilate -t rust "Rust ownership prevents data races"
    ///   b00t grok assimilate -t rust --file notes.md
    ///   b00t grok assimilate -t rust --distill "long article content..."
    ///   b00t grok assimilate -t rust "long article content..."
    Assimilate {
        /// Topic for the datum (must be a known b00t datum topic)
        #[arg(short, long)]
        topic: String,
        /// Content to assimilate (inline)
        content: Option<String>,
        /// Source file to read content from
        #[arg(long, value_name = "FILE")]
        file: Option<PathBuf>,
        /// OWL class label for the datum (default: Concept)
        #[arg(long, default_value = "Concept")]
        class: String,
        /// Tags to attach (comma-separated)
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Also ingest into grok backends after storing git blob (use --ingest=false to disable)
        #[arg(long, default_value_t = true, num_args = 0..=1)]
        ingest: bool,
        /// Canonical source URL — required for datum validity
        #[arg(long, help = "Canonical source URL (required for datum validity)")]
        source_url: Option<String>,
        /// Enable enhanced crawl: sm0l concept extraction + link following + indexing
        #[arg(long, help = "Enable enhanced assimilation pipeline")]
        enhanced: bool,
        /// Crawl depth when --enhanced (0 = no crawl, default 2)
        #[arg(long, help = "Max crawl depth for link following")]
        depth: Option<u32>,
        /// Index targets for concept ingestion (comma-sep: grafeo,raglite,codebase-memory)
        #[arg(long, value_delimiter = ',', help = "Index targets to ingest into")]
        index_target: Vec<String>,
        /// Fork/register this datum into the b00tyverse vendor tree
        #[arg(long, default_value_t = false)]
        b00tyverse: bool,
    },
}

pub async fn handle_grok_command(command: GrokCommands) -> Result<()> {
    match command {
        GrokCommands::Digest {
            topic,
            content,
            rag,
        } => {
            let backend = GrokBackend::from_flag(rag.as_deref())?;
            match backend {
                GrokBackend::Both
                | GrokBackend::Irontology
                | GrokBackend::Raglite => {
                    handle_dual_digest(&topic, &content, backend).await
                }
                GrokBackend::CodebaseMemory => Err(anyhow::anyhow!(
                    "--rag=codebase-memory is not supported for digest; use ask/search queries"
                )),
            }
        }
        GrokCommands::Ask {
            query,
            topic,
            limit,
            rag,
        } => {
            let backend = GrokBackend::from_flag(rag.as_deref())?;
            handle_dual_ask(&query, topic.as_deref(), limit, backend).await
        }
        GrokCommands::Learn {
            source,
            content,
            topic,
            rag,
        } => {
            let backend = GrokBackend::from_flag(rag.as_deref())?;
            let content_str = match (content.as_deref(), source.as_deref()) {
                (Some(c), _) => c.to_string(),
                (None, Some(src)) => {
                    let path = std::path::Path::new(src);
                    if path.exists() {
                        fs::read_to_string(path)
                            .with_context(|| format!("reading source file {}", src))?
                    } else {
                        return Err(anyhow::anyhow!(
                            "Source '{}' not found — URL fetching not yet implemented",
                            src
                        ));
                    }
                }
                (None, None) => unreachable!("Either content or --source must be provided"),
            };
            handle_dual_learn(source.as_deref(), &content_str, topic.as_deref(), backend).await
        }
        GrokCommands::Assimilate {
            topic,
            content,
            file,
            class,
            tags,
            ingest,
            source_url,
            enhanced,
            depth,
            index_target,
            b00tyverse,
        } => {
            handle_assimilate(
                &topic,
                content.as_deref(),
                file.as_deref(),
                &class,
                &tags,
                ingest,
                source_url.as_deref(),
                enhanced,
                depth,
                &index_target,
                b00tyverse,
            )
            .await
        }
    }
}

// ── Dual-backend handlers ─────────────────────────────────────────────────────

async fn handle_dual_digest(topic: &str, content: &str, backend: GrokBackend) -> Result<()> {
    println!(
        "🧠 [{}] digesting content for topic '{}'...",
        backend.display_name(),
        topic
    );

    let mut client = DualGrokClient::new();
    let result = client.ingest(topic, content, backend).await?;

    println!("✅ Digested to [{}]:", result.backend);
    if let Some(job_id) = &result.raglite_job_id {
        println!("  📄 RAGLight job_id: {}", job_id);
        println!("  ⌛ RAGLight indexing runs asynchronously");
    }
    if let Some(subject) = &result.irontology_subject {
        println!("  🕸️  Irontology subject: {}", subject);
    }
    for warn in &result.warnings {
        eprintln!("  ⚠️  {}", warn);
    }
    if !result.raglite_ok && !result.irontology_ok {
        return Err(anyhow::anyhow!("Both backends failed — see warnings above"));
    }
    Ok(())
}

async fn handle_dual_ask(
    query: &str,
    topic: Option<&str>,
    limit: Option<usize>,
    backend: GrokBackend,
) -> Result<()> {
    // --rag=raglite requires --topic; --rag=irontology without topic queries all topics
    if matches!(backend, GrokBackend::Raglite) && topic.is_none() {
        return Err(anyhow::anyhow!(
            "--topic is required when using --rag=raglite"
        ));
    }

    println!(
        "🔍 [{}] searching for '{}' {}",
        backend.display_name(),
        query,
        topic.map(|t| format!("(topic: {})", t)).unwrap_or_default()
    );

    let client = DualGrokClient::new();
    let result = client.query(query, topic, limit, backend).await?;

    println!("📊 Found {} results:", result.total_found);
    for (i, item) in result.items.iter().enumerate() {
        println!("\n{}. [{}] topic: {}", i + 1, item.backend, item.topic);
        let preview: String = item.content.chars().take(120).collect();
        println!("   💬 {}", preview);
        if !item.tags.is_empty() {
            println!("   🏷️  {}", item.tags.join(", "));
        }
    }
    for warn in &result.warnings {
        eprintln!("  ⚠️  {}", warn);
    }
    let control_sink = b00t_c0re_lib::default_control_event_sink();
    for event in &result.control_events {
        let receipt = control_sink.emit(event);
        eprintln!(
            "  {} {} -> {} [{}] {}",
            event.action_code, event.source, event.target, event.log_ref, receipt.message
        );
    }
    if result.total_found == 0 && !result.warnings.is_empty() {
        eprintln!("❌ No results — all backends returned warnings");
    }
    Ok(())
}

async fn handle_dual_learn(
    source: Option<&str>,
    content: &str,
    topic: Option<&str>,
    backend: GrokBackend,
) -> Result<()> {
    let topic = topic.ok_or_else(|| anyhow::anyhow!("--topic is required for grok learn"))?;

    let source_str = source.unwrap_or("direct_input");
    println!(
        "📚 [{}] learning topic '{}' from '{}'...",
        backend.display_name(),
        topic,
        source_str
    );

    let mut client = DualGrokClient::new();
    let result = client.ingest(topic, content, backend).await?;

    println!("✅ Learned into [{}]:", result.backend);
    if result.raglite_ok {
        println!(
            "  📄 RAGLight job_id: {}",
            result.raglite_job_id.as_deref().unwrap_or("queued")
        );
    }
    if result.irontology_ok {
        println!(
            "  🕸️  Irontology: {}",
            result.irontology_subject.as_deref().unwrap_or("stored")
        );
    }
    for warn in &result.warnings {
        eprintln!("  ⚠️  {}", warn);
    }
    if !result.raglite_ok && !result.irontology_ok {
        return Err(anyhow::anyhow!("Both backends failed — see warnings above"));
    }
    Ok(())
}

// ── Assimilate: git-blob datum storage ───────────────────────────────────────

async fn handle_assimilate(
    topic: &str,
    content_inline: Option<&str>,
    file: Option<&std::path::Path>,
    class: &str,
    tags: &[String],
    ingest: bool,
    source_url: Option<&str>,
    enhanced: bool,
    depth: Option<u32>,
    index_target: &[String],
    b00tyverse: bool,
) -> Result<()> {
    // Warn early if no source_url — datum will lack deduplication anchor
    if source_url.is_none() {
        eprintln!(
            "⚠️  No --source-url provided. Datum will be invalid (source required for deduplication)."
        );
    }

    // ── Type-detect: GitHub repo URL → polyseme handler ────────────────────
    if let Some(url) = source_url {
        if let Some(parsed) = parse_github_repo_url(url) {
            eprintln!("  🔍 detected GitHub repo: {}/{}", parsed.owner, parsed.repo);
            assimilate_github_repo(&parsed, topic, tags).unwrap_or_else(|e| {
                eprintln!("  ⚠️  polyseme scaffold failed: {e}");
            });
        }
    }

    // ── b00tyverse registration: datum + vendor submodule ──────────────────
    if b00tyverse {
        register_b00tyverse(topic, class, tags, source_url).unwrap_or_else(|e| {
            eprintln!("  ⚠️  b00tyverse registration failed: {e}");
        });
    }

    // ── Enhanced assimilation pipeline ──────────────────────────────────────
    //
    // When --enhanced is set, run the full pipeline:
    // 1. ContentRouter fetches/parses the source (URL or file)
    // 2. ConceptExtractor extracts concepts + links via sm0l model
    // 3. CrawlEngine follows links (depth-limited, same-origin)
    // 4. IndexDispatcher ingests into configured targets
    //
    // The primary content is enriched with concepts and used for datum storage.
    let mut enriched_tags: Vec<String> = tags.to_vec();
    let mut enhanced_content: Option<String> = None;

    if enhanced {
        let source = source_url
            .map(|s| s.to_string())
            .or_else(|| content_inline.map(|s| s.to_string()))
            .or_else(|| file.map(|f| f.display().to_string()))
            .ok_or_else(|| {
                anyhow::anyhow!("--enhanced requires a source: use --source-url, content, or --file")
            })?;

        let config = crate::assimilate::EnhancedConfig {
            max_depth: depth.unwrap_or(2),
            index_targets: index_target.to_vec(),
            ..Default::default()
        };

        let docs = crate::assimilate::run_enhanced(&source, topic, &config).await?;

        // Collect concept names as tags for the datum
        if let Some(primary) = docs.first() {
            for concept in &primary.extraction.concepts {
                let tag = if concept.is_advanced {
                    format!("advanced:{}", concept.name)
                } else {
                    concept.name.clone()
                };
                if !enriched_tags.contains(&tag) {
                    enriched_tags.push(tag);
                }
            }
        }

        eprintln!(
            "→ {} concept tag(s) extracted for datum",
            enriched_tags.len() - tags.len()
        );

        // If no inline/file content was provided, use the primary doc's text
        // from the enhanced pipeline for the git-blob datum.
        if content_inline.is_none() && file.is_none() {
            if let Some(primary) = docs.first() {
                enhanced_content = Some(primary.content.text.clone());
            }
        }
    }

    // Resolve content: enhanced-pipeline | inline | file | stdin
    let content = if let Some(ec) = enhanced_content {
        ec
    } else {
        match (content_inline, file) {
            (Some(c), _) => c.to_string(),
            (None, Some(f)) => {
                fs::read_to_string(f).with_context(|| format!("reading file {}", f.display()))?
            }
            (None, None) => {
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)?;
                buf
            }
        }
    };

    if content.trim().is_empty() {
        return Err(anyhow::anyhow!("Content is empty — nothing to assimilate"));
    }

    println!(
        "🔮 Assimilating {} bytes for topic '{}'...",
        content.len(),
        topic
    );

    // Step 1: Gzip-compress content before storing as git blob
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(content.as_bytes())
        .context("gzip encoding content")?;
    let compressed = encoder.finish().context("finalizing gzip stream")?;

    println!(
        "📦 Compressed {} → {} bytes (gzip)",
        content.len(),
        compressed.len()
    );

    // Step 2: Store compressed bytes as git blob (content NOT in filesystem)
    let git_hash = store_as_git_blob_bytes(&compressed)?;
    println!("📦 git blob: {}", git_hash);

    // Step 2: Validate the blob is accessible
    validate_git_blob(&git_hash)?;
    println!("✅ Validated: git cat-file -e {}", &git_hash[..12]);

    // Step 3: Write datum TOML to _b00t_/ — references hash, NOT content
    let datum_id = Uuid::new_v4();
    let datum_name = format!(
        "{}-{}.datum.toml",
        sanitize_for_filename(topic),
        &datum_id.to_string()[..8]
    );
    let datum_path = find_b00t_dir()?.join(&datum_name);

    let tags_toml = enriched_tags
        .iter()
        .map(|t| {
            let escaped = t.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{}\"", escaped)
        })
        .collect::<Vec<_>>()
        .join(", ");

    let source_url_toml = source_url.unwrap_or("");
    let datum_toml = format!(
        r#"# b00t datum — generated by `b00t grok assimilate`
# 🤓 Content lives ONLY in git object store. NOT in filesystem. Blob is gzip-compressed.
#    Retrieve: git -C ~/.b00t cat-file blob {git_hash} | gunzip
#    Validate: git -C ~/.b00t cat-file -e {git_hash}

[datum]
topic      = "{topic}"
class      = "{class}"
tags       = [{tags_toml}]
source_url = "{source_url_toml}"
encoding   = "gzip"
git_hash   = "{git_hash}"
datum_id   = "{datum_id}"
created    = "{created}"

[validation]
validate  = "git -C ~/.b00t cat-file -e {git_hash}"
# 🤓 Decompress: git -C ~/.b00t cat-file blob {git_hash} | gunzip
# 🤓 Blob lives in user's ~/.b00t git store; ~ is expanded by b00t datum validate
# 🤓 This validate command is executed by `b00t datum validate` as part of pre-release checks

# b00t:map v1
# summary: {topic} datum — assimilated content stored as gzip git blob {git_hash_short}
# tags: {tags_str}
# tier: sm0l
# cmds: git -C ~/.b00t cat-file blob {git_hash} | gunzip
# complexity: 1
"#,
        git_hash = git_hash,
        topic = topic,
        class = class,
        tags_toml = tags_toml,
        source_url_toml = source_url_toml,
        datum_id = datum_id,
        created = chrono::Utc::now().to_rfc3339(),
        git_hash_short = &git_hash[..12],
        tags_str = tags.join(", "),
    );

    fs::write(&datum_path, &datum_toml)
        .with_context(|| format!("writing datum TOML to {}", datum_path.display()))?;
    println!("📝 Datum written: {}", datum_path.display());
    println!("   git_hash = {}", git_hash);

    // Step 4: Optionally ingest into grok backends
    if ingest {
        println!("🔄 Ingesting into grok backends (both)...");
        let datum_node = DatumNode {
            topic: topic.to_string(),
            class: class.to_string(),
            content: content.clone(),
            tags: tags.to_vec(),
            predicates: vec![("storedAt".to_string(), format!("git:{}", git_hash))],
        };

        // Irontology ingest
        match IrontologyBridgeClient::new("b00t-grok") {
            Ok(iron) => match iron.ingest(&datum_node).await {
                Ok(r) => println!(
                    "  🕸️  Irontology: {} ({} facts)",
                    r.subject_prefix, r.facts_stored
                ),
                Err(e) => eprintln!("  ⚠️  Irontology ingest: {}", e),
            },
            Err(e) => eprintln!("  ⚠️  Irontology unavailable: {}", e),
        }

        // RAGLight ingest via DualGrokClient
        let mut dual = DualGrokClient::new();
        match dual.ingest(topic, &content, GrokBackend::Raglite).await {
            Ok(r) => {
                if r.raglite_ok {
                    println!(
                        "  📄 RAGLight job_id: {}",
                        r.raglite_job_id.as_deref().unwrap_or("queued")
                    );
                }
                for w in &r.warnings {
                    eprintln!("  ⚠️  {}", w);
                }
            }
            Err(e) => eprintln!("  ⚠️  RAGLight ingest: {}", e),
        }
    }

    println!(
        "✅ Assimilation complete — content in git object store, datum at {}",
        datum_name
    );
    Ok(())
}

// ── b00tyverse: vendor-tree registration ─────────────────────────────────────

/// Register a datum into the b00tyverse vendor tree (`--b00tyverse`).
///
/// 1. Writes `_b00t_/datums/<topic>.<class>.tomllm` with the full b00t datum
///    schema, including a `[b00t.b00tyverse]` section pointing at the
///    PromptExecution fork and the `vendor/<repo>` path.
/// 2. Best-effort: `git submodule add <source_url> vendor/<repo>` when the
///    source is a GitHub repo and the vendor path does not exist yet.
///
/// Non-fatal by contract: the caller continues content assimilation on error.
fn register_b00tyverse(
    topic: &str,
    class: &str,
    tags: &[String],
    source_url: Option<&str>,
) -> Result<()> {
    let datums_dir = find_b00t_dir()?.join("datums");
    fs::create_dir_all(&datums_dir)
        .with_context(|| format!("creating {}", datums_dir.display()))?;

    let parsed = source_url.and_then(parse_github_repo_url);
    let (fork_url, vendor_path) = match &parsed {
        Some(p) => (
            format!("https://github.com/PromptExecution/{}", p.repo),
            format!("vendor/{}", p.repo),
        ),
        None => (
            String::new(),
            format!("vendor/{}", sanitize_for_filename(topic)),
        ),
    };

    let datum_path = datums_dir.join(format!(
        "{}.{}.tomllm",
        sanitize_for_filename(topic),
        class.to_lowercase()
    ));

    if datum_path.exists() {
        eprintln!(
            "  ⏭️  b00tyverse datum exists: {} — leaving in place",
            datum_path.display()
        );
    } else {
        let src = source_url.unwrap_or("");
        let tags_str = tags.join(", ");
        let datum = format!(
            r#"[b00t]
name = "{topic}"
type = "{class}"
hint = "{topic} — assimilated into the b00tyverse vendor tree"

[b00t.source]
url = "{src}"

[b00t.b00tyverse]
fork_url = "{fork_url}"
vendor_path = "{vendor_path}"
status = "assimilated"

# b00t:map v1
# summary: {topic} — b00tyverse datum generated by `b00t grok assimilate --b00tyverse`
# tags: {tags_str}
# tier: ch0nky
# cmds: b00t learn {topic}
# complexity: 3
"#,
        );
        fs::write(&datum_path, datum)
            .with_context(|| format!("writing {}", datum_path.display()))?;
        eprintln!("  ✅ b00tyverse datum: {}", datum_path.display());
    }

    // Best-effort git submodule add for GitHub sources
    if let (Some(p), Some(url)) = (&parsed, source_url) {
        let repo_root = find_git_root()?;
        let vendor_abs = repo_root.join("vendor").join(&p.repo);
        if vendor_abs.exists() {
            eprintln!(
                "  ⏭️  {} exists — skipping submodule add",
                vendor_abs.display()
            );
        } else {
            eprintln!("  → git submodule add {url} vendor/{}", p.repo);
            let output = std::process::Command::new("git")
                .args([
                    "-C",
                    repo_root.to_str().unwrap_or("."),
                    "submodule",
                    "add",
                    url,
                    &format!("vendor/{}", p.repo),
                ])
                .output()
                .context("spawning git submodule add")?;
            if output.status.success() {
                eprintln!("  ✅ submodule: vendor/{}", p.repo);
            } else {
                eprintln!(
                    "  ⚠️  git submodule add failed ({}): {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
        }
    }

    Ok(())
}

/// Store content as a git blob object — returns the SHA1 hash
/// 🤓 Uses `git -C <repo_root> hash-object -w --stdin` to ensure
///    the blob lands in the correct repo's object store
fn store_as_git_blob(content: &str) -> Result<String> {
    store_as_git_blob_bytes(content.as_bytes())
}

/// Store raw bytes as a git blob object — returns the SHA1 hash
/// 🤓 Used for pre-compressed (gzip) content blobs
fn store_as_git_blob_bytes(bytes: &[u8]) -> Result<String> {
    let repo_root = find_git_root()?;
    let output = std::process::Command::new("git")
        .args([
            "-C",
            repo_root.to_str().unwrap_or("."),
            "hash-object",
            "-w",
            "--stdin",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("spawning git hash-object")?
        .wait_with_output_and_stdin(bytes)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("git hash-object failed: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Confirm the blob is retrievable — panics are a pre-release failure
fn validate_git_blob(hash: &str) -> Result<()> {
    let repo_root = find_git_root()?;
    let status = std::process::Command::new("git")
        .args([
            "-C",
            repo_root.to_str().unwrap_or("."),
            "cat-file",
            "-e",
            hash,
        ])
        .status()
        .context("git cat-file -e")?;
    if !status.success() {
        return Err(anyhow::anyhow!(
            "git cat-file -e {} failed — blob not found in object store",
            hash
        ));
    }
    Ok(())
}

fn find_git_root() -> Result<PathBuf> {
    // Prefer ~/.b00t as canonical git root for datum storage
    let b00t = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("no home dir"))?
        .join(".b00t");
    if b00t.join(".git").exists() {
        return Ok(b00t);
    }
    // Fallback: CWD git root
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("git rev-parse --show-toplevel")?;
    if output.status.success() {
        return Ok(PathBuf::from(
            String::from_utf8_lossy(&output.stdout).trim().to_string(),
        ));
    }
    Err(anyhow::anyhow!("Not inside a git repository"))
}

fn find_b00t_dir() -> Result<PathBuf> {
    let b00t = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("no home dir"))?
        .join(".b00t")
        .join("_b00t_");
    if b00t.exists() {
        return Ok(b00t);
    }
    Err(anyhow::anyhow!(
        "_b00t_ directory not found at {:?} — run `b00t up` first",
        b00t
    ))
}

fn sanitize_for_filename(input: &str) -> String {
    let mut s: String = input
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    s = s.trim_matches('_').to_string();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    if s.is_empty() { "topic".to_string() } else { s }
}

// ── GitHub repo type detection for polyseme scaffolding ──────────────────────

struct ParsedRepo {
    owner: String,
    repo: String,
}

/// Repo language with auto-detected install/version commands (#581).
#[derive(Debug, Clone, PartialEq)]
enum RepoLanguage {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Unknown(String),
}

impl RepoLanguage {
    fn from_gh_api(owner: &str, repo: &str) -> Self {
        let output = std::process::Command::new("gh")
            .args(["api", &format!("repos/{owner}/{repo}"), "--jq", ".language"])
            .output();
        let lang = output
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default()
            .trim()
            .to_lowercase();
        match lang.as_str() {
            "rust" => Self::Rust,
            "python" => Self::Python,
            "javascript" => Self::JavaScript,
            "typescript" => Self::TypeScript,
            "go" => Self::Go,
            "" => Self::Unknown("unknown".into()),
            other => Self::Unknown(other.into()),
        }
    }

    fn install_cmd(&self, repo: &str, owner: &str) -> String {
        match self {
            Self::Rust => format!("install = \"cargo install {repo}\""),
            // 🤓 b00t command guard: `pip install` is skunked — always `uv pip install`
            Self::Python => format!("install = \"uv pip install {repo}\""),
            Self::JavaScript | Self::TypeScript => format!("install = \"npm install -g {repo}\""),
            Self::Go => format!("install = \"go install github.com/{owner}/{repo}@latest\""),
            Self::Unknown(lang) => format!("# install = \"TODO: install {repo} ({lang})\""),
        }
    }

    fn version_cmd(&self, repo: &str) -> String {
        match self {
            Self::Rust | Self::Python | Self::JavaScript | Self::TypeScript => {
                format!("version = \"{repo} --version\"")
            }
            Self::Go => format!("version = \"{repo} version\""),
            Self::Unknown(_) => format!("# version = \"TODO: version check for {repo}\""),
        }
    }
}

impl std::fmt::Display for RepoLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rust => write!(f, "Rust"),
            Self::Python => write!(f, "Python"),
            Self::JavaScript => write!(f, "JavaScript"),
            Self::TypeScript => write!(f, "TypeScript"),
            Self::Go => write!(f, "Go"),
            Self::Unknown(s) => write!(f, "{s}"),
        }
    }
}

/// Detect install command and version check from repo language (#581).
fn detect_cli_defaults(owner: &str, repo: &str) -> (String, String) {
    let lang = RepoLanguage::from_gh_api(owner, repo);
    (lang.install_cmd(repo, owner), lang.version_cmd(repo))
}
fn parse_github_repo_url(url: &str) -> Option<ParsedRepo> {
    let stripped = url
        .trim_end_matches('/')
        .trim_end_matches(".git");
    // Match: https://github.com/OWNER/REPO with nothing after
    if let Some(rest) = stripped.strip_prefix("https://github.com/")
        .or_else(|| stripped.strip_prefix("http://github.com/"))
    {
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() == 2 {
            return Some(ParsedRepo {
                owner: parts[0].to_string(),
                repo: parts[1].to_string(),
            });
        }
    }
    None
}

/// Auto-scaffold datum for a GitHub repo.
/// Creates a `.cli.toml` stub for unambiguous names. Only creates a
/// `.polyseme.tomllmd` when multiple artifacts claim the same name from
/// different canonical sources (e.g. "bubblewrap" = sandbox container + Android app).
/// Non-fatal: errors are returned but the caller continues with content assimilation.
fn assimilate_github_repo(parsed: &ParsedRepo, topic: &str, _tags: &[String]) -> anyhow::Result<()> {
    use crate::{PolysemeRef, UnifiedConfig};

    let canonical = format!("github:{}/{}", parsed.owner, parsed.repo);
    let description = format!("{} — {}/{}", parsed.repo, parsed.owner, parsed.repo);
    let repo_url = format!("https://github.com/{}/{}", parsed.owner, parsed.repo);
    let b00t_dir = find_b00t_dir()?;

    let cli_path = b00t_dir.join(format!("{}.cli.toml", topic));
    let poly_path = b00t_dir.join(format!("{}.polyseme.tomllmd", topic));

    // Detect ambiguity: an existing .cli.toml with a DIFFERENT source URL means
    // this name is already claimed by another artifact. If no .cli.toml exists
    // at all, or the existing one is from the same source, it's unambiguous.
    let existing_source = if cli_path.exists() {
        std::fs::read_to_string(&cli_path).ok()
            .and_then(|c| {
                c.lines()
                    .find(|l| l.contains("Assimilated from:"))
                    .or_else(|| c.lines().find(|l| l.contains("# source_url")))
                    .map(|l| l.to_string())
            })
    } else {
        None
    };

    let is_ambiguous = poly_path.exists()
        || existing_source.as_ref()
            .map(|s| !s.contains(&canonical) && !s.contains(&format!("{}/{}", parsed.owner, parsed.repo)))
            .unwrap_or(false);

    let (detected_install, detected_version) = detect_cli_defaults(&parsed.owner, &parsed.repo);

    if is_ambiguous {
        // ── Polyseme path (name collision) ──
        let concrete_name = format!("{}-{}", topic, parsed.owner);

        let mut polyseme_datum: crate::BootDatum = if poly_path.exists() {
            let content = std::fs::read_to_string(&poly_path)?;
            toml::from_str::<UnifiedConfig>(&content)?.b00t
        } else {
            crate::BootDatum {
                name: topic.to_string(),
                datum_type: Some(crate::DatumType::Unknown),
                hint: format!("Polyseme datum — '{topic}' resolves to multiple artifacts"),
                ..Default::default()
            }
        };

        let mut poly_cfg = polyseme_datum.polyseme.clone().unwrap_or_default();
        let mut refs = poly_cfg.refs.unwrap_or_default();
        let mut sources = poly_cfg.sources.unwrap_or_default();

        if !refs.iter().any(|r| r.canonical == canonical) {
            refs.push(PolysemeRef {
                name: concrete_name.clone(),
                canonical: canonical.clone(),
                datum: format!("{}.cli", concrete_name),
                description: description.clone(),
            });
        }
        if !sources.iter().any(|s| s == &repo_url) {
            sources.push(repo_url);
        }

        poly_cfg.refs = Some(refs);
        poly_cfg.sources = Some(sources);
        polyseme_datum.polyseme = Some(poly_cfg);

        let unified = UnifiedConfig { b00t: polyseme_datum, service_contract: vec![], env: None, sections: None };
        std::fs::write(&poly_path, format!("{}\n", toml::to_string_pretty(&unified)?))?;
        eprintln!("  ✅ polyseme: {}", poly_path.display());

        // Scaffold concrete CLI datum under polyseme
        let cli_concrete = b00t_dir.join(format!("{}.cli.toml", concrete_name));
        if !cli_concrete.exists() {
            let scaffold = format!(
                r#"# {concrete_name}.cli — cli datum (polyseme ref of {topic})
# Assimilated from: {canonical}

[b00t]
name        = "{concrete_name}"
type        = "cli"
hint        = "{description} — {owner}/{repo}"

{detected_install}
{detected_version}
# version_regex = '(\\d+\\.\\d+\\.\\d+)'

# b00t:map v1
# summary: concrete datum for {canonical}
# tags: {concrete_name}, {owner}, {repo}
# tier: ch0nky
# cmds: b00t install {concrete_name}
# complexity: 1
"#,
                owner = parsed.owner,
                repo = parsed.repo,
            );
            std::fs::write(&cli_concrete, scaffold)?;
            eprintln!("  ✅ scaffold: {}", cli_concrete.display());
        }
    } else {
        // ── Unambiguous — create .cli.toml directly ──
        if !cli_path.exists() {
            let scaffold = format!(
                r#"# {topic}.cli — cli datum
# Assimilated from: {canonical}

[b00t]
name        = "{topic}"
type        = "cli"
hint        = "{description} — {owner}/{repo}"

{detected_install}
{detected_version}
# version_regex = '(\\d+\\.\\d+\\.\\d+)'

# b00t:map v1
# summary: datum for {canonical} — scaffolded by grok assimilate
# tags: {topic}, {owner}, {repo}
# tier: ch0nky
# cmds: b00t install {topic}
# complexity: 1
"#,
                owner = parsed.owner,
                repo = parsed.repo,
            );
            std::fs::write(&cli_path, scaffold)?;
            eprintln!("  ✅ cli scaffold: {}", cli_path.display());
        } else {
            eprintln!("  ⏭️  {topic}.cli.toml exists — skipping scaffold");
        }
    }

    Ok(())
}

// ── git stdin helper ──────────────────────────────────────────────────────────

trait ChildExt {
    fn wait_with_output_and_stdin(self, stdin_bytes: &[u8]) -> Result<std::process::Output>;
}

impl ChildExt for std::process::Child {
    fn wait_with_output_and_stdin(mut self, stdin_bytes: &[u8]) -> Result<std::process::Output> {
        use std::io::Write;
        if let Some(mut stdin) = self.stdin.take() {
            stdin
                .write_all(stdin_bytes)
                .context("writing to git stdin")?;
        }
        self.wait_with_output().context("waiting for git")
    }
}
