use anyhow::Result;
use b00t_c0re_lib::LfmfSystem;
use b00t_c0re_lib::lfmf_telemetry::{
    LfmfAction, LfmfOutcome, LfmfTelemetryEvent, log_event, read_stats, telemetry_path,
};
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

/// A lesson parsed with salvage semantics.
///
/// 🤓 Meta-pattern: NEVER lose the payload. This CLI layer used to bail on
/// colon-free or over-long lessons while the storage layer (LfmfSystem::parse_lesson)
/// would have accepted them — an outer layer MUST NOT validate stricter than the
/// layer it fronts. Malformed input degrades to a salvage kind, never an error.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedLesson {
    pub topic: String,
    pub body: String,
    /// None = well-formed; Some(kind) = payload recovered (no_colon, url_colon,
    /// empty_part, topic_overflow, body_overflow) — kind feeds telemetry and the
    /// distillery marker.
    pub salvage: Option<String>,
}

/// Longest word-prefix of `text` that fits within `topic_max` tokens (≥1 word).
fn auto_topic(text: &str, count_tokens: &dyn Fn(&str) -> usize, topic_max: usize) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut end = words.len();
    while end > 1 && count_tokens(&words[..end].join(" ")) > topic_max {
        end -= 1;
    }
    words[..end].join(" ").trim_end_matches(':').to_string()
}

/// Parse a raw lesson with salvage-first semantics. Pure — token counting is
/// injected so tests can use word counts while production uses tiktoken.
pub fn parse_lesson_salvage(
    raw: &str,
    count_tokens: &dyn Fn(&str) -> usize,
    topic_max: usize,
    body_max: usize,
) -> ParsedLesson {
    let raw = raw.trim();

    let Some((topic_part, body_part)) = raw.split_once(':') else {
        // No colon — derive topic from the lesson itself, keep full payload as body.
        return ParsedLesson {
            topic: auto_topic(raw, count_tokens, topic_max),
            body: raw.to_string(),
            salvage: Some("no_colon".to_string()),
        };
    };
    let topic_part = topic_part.trim();
    let body_part = body_part.trim();

    // First colon belongs to a URL scheme (https://…) — not a topic separator.
    if body_part.starts_with("//") {
        return ParsedLesson {
            topic: auto_topic(raw, count_tokens, topic_max),
            body: raw.to_string(),
            salvage: Some("url_colon".to_string()),
        };
    }

    if topic_part.is_empty() {
        return ParsedLesson {
            topic: auto_topic(body_part, count_tokens, topic_max),
            body: body_part.to_string(),
            salvage: Some("empty_part".to_string()),
        };
    }
    if body_part.is_empty() {
        return ParsedLesson {
            topic: auto_topic(topic_part, count_tokens, topic_max),
            body: topic_part.trim_end_matches(':').to_string(),
            salvage: Some("empty_part".to_string()),
        };
    }

    if count_tokens(topic_part) > topic_max {
        // Truncated topic is derived metadata — the FULL raw becomes the body so
        // the overflow words survive (never lose the payload).
        return ParsedLesson {
            topic: auto_topic(topic_part, count_tokens, topic_max),
            body: raw.to_string(),
            salvage: Some("topic_overflow".to_string()),
        };
    }
    if count_tokens(body_part) > body_max {
        return ParsedLesson {
            topic: topic_part.to_string(),
            body: body_part.to_string(),
            salvage: Some("body_overflow".to_string()),
        };
    }

    ParsedLesson {
        topic: topic_part.to_string(),
        body: body_part.to_string(),
        salvage: None,
    }
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
    let lessons: Vec<String> = match query {
        Some(q) => lfmf_system.get_advice(tool, q, Some(5)).await?,
        None => lfmf_system.list_lessons(tool, Some(10)).await?,
    }
    .into_iter()
    .filter(|l| !l.trim().is_empty())
    .collect();

    if lessons.is_empty() {
        // A read miss — the moment lfmf failed to pay out. Count it.
        log_event(&LfmfTelemetryEvent::now(
            LfmfAction::Advice,
            tool,
            LfmfOutcome::NoResults,
            query.map(String::from),
        ));
        eprintln!("(no lessons recorded for '{}' yet — seed with: b00t lfmf {} \"<lesson>\")", tool, tool);
        return Ok(());
    }

    for lesson in &lessons {
        println!("{}", lesson);
    }
    log_event(&LfmfTelemetryEvent::now(
        LfmfAction::Advice,
        tool,
        LfmfOutcome::Ok,
        None,
    ));
    Ok(())
}

/// Handle LFMF (Lessons From My Failures) recording
/// Uses shared LFMF system from b00t-c0re-lib for consistency
pub async fn handle_lfmf(path: &str, tool: &str, lesson: &str, scope: &str) -> Result<()> {
    // Ensure lessons write into the provided path unless explicitly overridden
    ensure_learn_dir(path)?;

    let raw = lesson.trim();
    if raw.is_empty() {
        log_event(&LfmfTelemetryEvent::now(
            LfmfAction::Record,
            tool,
            LfmfOutcome::Error,
            Some("empty_lesson".to_string()),
        ));
        anyhow::bail!("Lesson must not be empty. See --help for examples.");
    }

    // Token accounting (OpenAI tiktoken, not words) drives salvage decisions.
    // 🤓 See src/commands/tiktoken.rs for details.
    let bpe = o200k_base().map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {e}"))?;
    let count_tokens = |s: &str| bpe.encode_with_special_tokens(s).len();
    let parsed = parse_lesson_salvage(raw, &count_tokens, 25, 250);

    // Affirmative style check (simple heuristic)
    if parsed.body.to_lowercase().contains("don't") || parsed.body.to_lowercase().contains("never")
    {
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

    // Scope handling: currently only memoized, extend LfmfSystem for future
    println!("Scope: {}", scope);

    // Storage format is "topic: body" — topic must stay colon-free or the core
    // parse_lesson re-split corrupts it (URL-derived topics contain ':').
    let safe_topic = parsed.topic.replace(':', "");
    let stored = match &parsed.salvage {
        // Salvage marker keeps entries greppable for the distillery (task #98).
        Some(kind) => format!("{}: {} <!-- salvaged:{} -->", safe_topic, parsed.body, kind),
        None => format!("{}: {}", safe_topic, parsed.body),
    };

    if let Err(e) = lfmf_system.record_lesson(tool, &stored).await {
        log_event(&LfmfTelemetryEvent::now(
            LfmfAction::Record,
            tool,
            LfmfOutcome::Error,
            Some("record_failed".to_string()),
        ));
        return Err(e);
    }

    match &parsed.salvage {
        Some(kind) => {
            eprintln!(
                "⚠️ lesson salvaged ({}) — payload recorded anyway; distillery will refine it later",
                kind
            );
            log_event(&LfmfTelemetryEvent::now(
                LfmfAction::Record,
                tool,
                LfmfOutcome::Salvaged,
                Some(kind.clone()),
            ));
        }
        None => log_event(&LfmfTelemetryEvent::now(
            LfmfAction::Record,
            tool,
            LfmfOutcome::Ok,
            None,
        )),
    }

    println!("✅ Lesson recorded for {}: {}", tool, parsed.topic);
    Ok(())
}

/// Print the lfmf hit/salvage/miss report from the telemetry JSONL.
/// `tool_filter = None` reports across all tools.
pub fn handle_lfmf_stats(tool_filter: Option<&str>) -> Result<()> {
    let path = telemetry_path();
    let stats = read_stats(&path, tool_filter);
    println!("telemetry: {}", path.display());
    if let Some(tool) = tool_filter {
        println!("filter: {}", tool);
    }
    print!("{}", stats);
    Ok(())
}
