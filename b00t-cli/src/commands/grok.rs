use anyhow::{Context, Result};
use b00t_c0re_lib::{DatumNode, DualGrokClient, GrokBackend, IrontologyBridgeClient};
use clap::Subcommand;
use std::{fs, io::Read, path::PathBuf};
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
    },
}

pub async fn handle_grok_command(command: GrokCommands) -> Result<()> {
    match command {
        GrokCommands::Digest { topic, content, rag } => {
            let backend = GrokBackend::from_flag(rag.as_deref())?;
            match backend {
                GrokBackend::Both | GrokBackend::Irontology | GrokBackend::Raglite => {
                    handle_dual_digest(&topic, &content, backend).await
                }
            }
        }
        GrokCommands::Ask { query, topic, limit, rag } => {
            let backend = GrokBackend::from_flag(rag.as_deref())?;
            handle_dual_ask(&query, topic.as_deref(), limit, backend).await
        }
        GrokCommands::Learn { source, content, topic, rag } => {
            let backend = GrokBackend::from_flag(rag.as_deref())?;
            let content_str = content.as_deref().unwrap_or("");
            handle_dual_learn(source.as_deref(), content_str, topic.as_deref(), backend).await
        }
        GrokCommands::Assimilate { topic, content, file, class, tags, ingest } => {
            handle_assimilate(&topic, content.as_deref(), file.as_deref(), &class, &tags, ingest)
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
        return Err(anyhow::anyhow!("--topic is required when using --rag=raglite"));
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
    let topic = topic
        .ok_or_else(|| anyhow::anyhow!("--topic is required for grok learn"))?;

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
) -> Result<()> {
    // Resolve content: inline | file | stdin
    let content = match (content_inline, file) {
        (Some(c), _) => c.to_string(),
        (None, Some(f)) => fs::read_to_string(f)
            .with_context(|| format!("reading file {}", f.display()))?,
        (None, None) => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            buf
        }
    };

    if content.trim().is_empty() {
        return Err(anyhow::anyhow!("Content is empty — nothing to assimilate"));
    }

    println!("🔮 Assimilating {} bytes for topic '{}'...", content.len(), topic);

    // Step 1: Store content as git blob (content NOT in filesystem)
    let git_hash = store_as_git_blob(&content)?;
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

    let tags_toml = tags
        .iter()
        .map(|t| format!("\"{}\"", t))
        .collect::<Vec<_>>()
        .join(", ");

    let datum_toml = format!(
        r#"# b00t datum — generated by `b00t grok assimilate`
# 🤓 Content lives ONLY in git object store. NOT in filesystem.
#    Retrieve: git -C ~/.b00t cat-file blob {git_hash}
#    Validate: git -C ~/.b00t cat-file -e {git_hash}

[datum]
topic     = "{topic}"
class     = "{class}"
tags      = [{tags_toml}]
git_hash  = "{git_hash}"
datum_id  = "{datum_id}"
created   = "{created}"

[validation]
validate  = "git -C ~/.b00t cat-file -e {git_hash}"
# 🤓 This validate command is executed by `b00t datum validate` as part of pre-release checks

# b00t:map v1
# summary: {topic} datum — assimilated content stored as git blob {git_hash_short}
# tags: {tags_str}
# tier: sm0l
# cmds: git -C ~/.b00t cat-file blob {git_hash}
# complexity: 1
"#,
        git_hash = git_hash,
        topic = topic,
        class = class,
        tags_toml = tags_toml,
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
                Ok(r) => println!("  🕸️  Irontology: {} ({} facts)", r.subject_prefix, r.facts_stored),
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

    println!("✅ Assimilation complete — content in git object store, datum at {}", datum_name);
    Ok(())
}

/// Store content as a git blob object — returns the SHA1 hash
/// 🤓 Uses `git -C <repo_root> hash-object -w --stdin` to ensure
///    the blob lands in the correct repo's object store
fn store_as_git_blob(content: &str) -> Result<String> {
    let repo_root = find_git_root()?;
    let output = std::process::Command::new("git")
        .args(["-C", repo_root.to_str().unwrap_or("."), "hash-object", "-w", "--stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("spawning git hash-object")?
        .wait_with_output_and_stdin(content.as_bytes())?;

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
        .args(["-C", repo_root.to_str().unwrap_or("."), "cat-file", "-e", hash])
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

// ── git stdin helper ──────────────────────────────────────────────────────────

trait ChildExt {
    fn wait_with_output_and_stdin(self, stdin_bytes: &[u8]) -> Result<std::process::Output>;
}

impl ChildExt for std::process::Child {
    fn wait_with_output_and_stdin(mut self, stdin_bytes: &[u8]) -> Result<std::process::Output> {
        use std::io::Write;
        if let Some(mut stdin) = self.stdin.take() {
            stdin.write_all(stdin_bytes).context("writing to git stdin")?;
        }
        self.wait_with_output().context("waiting for git")
    }
}
