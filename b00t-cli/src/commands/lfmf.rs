use anyhow::Result;
use b00t_c0re_lib::LfmfSystem;
use tiktoken_rs::o200k_base;

/// Ensure B00T_LEARN_DIR points into the provided path unless explicitly overridden.
fn ensure_learn_dir(path: &str) -> Result<()> {
    if std::env::var("B00T_LEARN_DIR").is_err() {
        let learn_dir = crate::get_expanded_path(path)?
            .join("learn")
            .to_string_lossy()
            .to_string();
        unsafe {
            std::env::set_var("B00T_LEARN_DIR", &learn_dir);
        }
    }
    Ok(())
}

/// Handle LFMF advice retrieval — print prior lessons for a tool to stdout.
///
/// Kaizen agent-fix-first: consult prior lessons BEFORE attempting a fix so
/// agents benefit from past failures. Diagnostics go to stderr so stdout can
/// be captured cleanly in bash: `ADVICE=$(b00t-cli lfmf advice runpod)`.
///
/// With `query`: similarity-ranked matches via `LfmfSystem::get_advice`.
/// Without: all recorded lessons via `LfmfSystem::list_lessons`.
pub async fn handle_lfmf_advice(path: &str, tool: &str, query: Option<&str>) -> Result<()> {
    ensure_learn_dir(path)?;

    let config = LfmfSystem::load_config(path)?;
    let mut lfmf_system = LfmfSystem::new(config);

    // 🤓 Vector DB init is deliberately skipped: advice must be deterministic
    // and clean on stdout for bash capture. Without initialize() the grok
    // client stays None and LfmfSystem uses its filesystem fallback
    // (~/.b00t/learn/<tool>.md), which is authoritative for recorded lessons.
    let lessons = match query {
        Some(q) => lfmf_system.get_advice(tool, q, Some(5)).await?,
        None => lfmf_system.list_lessons(tool, Some(10)).await?,
    };

    for lesson in lessons.into_iter().filter(|l| !l.trim().is_empty()) {
        println!("{}", lesson);
    }
    Ok(())
}

/// Handle LFMF (Lessons From My Failures) recording
/// Uses shared LFMF system from b00t-c0re-lib for consistency
pub async fn handle_lfmf(path: &str, tool: &str, lesson: &str, scope: &str) -> Result<()> {
    // Ensure lessons write into the provided path unless explicitly overridden
    ensure_learn_dir(path)?;

    // Expect lesson in "<topic>: <body>" format
    let parts: Vec<&str> = lesson.splitn(2, ':').map(|s| s.trim()).collect();
    if parts.len() != 2 {
        anyhow::bail!("Lesson must be in '<topic>: <body>' format. See --help for examples.");
    }
    let topic = parts[0];
    let body = parts[1];

    // Token count enforcement (using tiktoken, not words)
    // 🤓: This enforces limits using OpenAI tiktoken, not word count. See src/commands/tiktoken.rs for details.
    let bpe = o200k_base().map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {e}"))?;
    let topic_tokens = bpe.encode_with_special_tokens(topic).len();
    let body_tokens = bpe.encode_with_special_tokens(body).len();
    if topic_tokens > 25 {
        anyhow::bail!(
            "Topic must be <25 tokens (OpenAI tiktoken, not words). Yours: {}. See --help for guidance.",
            topic_tokens
        );
    }
    if body_tokens > 250 {
        anyhow::bail!(
            "Body must be <250 tokens (OpenAI tiktoken, not words). Yours: {}. See --help for guidance.",
            body_tokens
        );
    }
    if topic.is_empty() || body.is_empty() {
        anyhow::bail!("Topic and body must not be empty. See --help for examples.");
    }
    // Affirmative style check (simple heuristic)
    if body.to_lowercase().contains("don't") || body.to_lowercase().contains("never") {
        println!(
            "⚠️ Please use positive, affirmative style (e.g., 'Do X for Y benefit'). See --help for examples."
        );
    }

    // Use shared LFMF system for recording (now in async function)
    let config = LfmfSystem::load_config(path)?;
    let mut lfmf_system = LfmfSystem::new(config);

    // Try to initialize vector database (non-fatal if fails)
    if let Err(e) = lfmf_system.initialize().await {
        println!(
            "⚠️ Vector database unavailable: {}. Lesson will be saved to filesystem only.",
            e
        );
    }

    // Record the lesson using shared system
    // Scope handling: currently only memoized, extend LfmfSystem for future
    println!("Scope: {}", scope);
    lfmf_system.record_lesson(tool, lesson).await?;

    println!("✅ Lesson recorded for {}: {}", tool, topic);
    Ok(())
}
