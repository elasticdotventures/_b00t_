use anyhow::Result;
use b00t_c0re_lib::learn::record_lesson;
use tiktoken_rs::o200k_base;

pub fn handle_lfmf(path: &str, tool: &str, lesson: &str) -> Result<()> {
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
        anyhow::bail!("Topic must be <25 tokens (OpenAI tiktoken, not words). Yours: {}. See --help for guidance.", topic_tokens);
    }
    if body_tokens > 250 {
        anyhow::bail!("Body must be <250 tokens (OpenAI tiktoken, not words). Yours: {}. See --help for guidance.", body_tokens);
    }
    if topic.is_empty() || body.is_empty() {
        anyhow::bail!("Topic and body must not be empty. See --help for examples.");
    }
    // Affirmative style check (simple heuristic)
    if body.to_lowercase().contains("don't") || body.to_lowercase().contains("never") {
        println!("⚠️ Please use positive, affirmative style (e.g., 'Do X for Y benefit'). See --help for examples.");
    }

    // Compose lesson as "<topic>: <body>"
    let formatted_lesson = format!("{}: {}", topic, body);
    record_lesson(path, tool, &formatted_lesson)?;
    println!("✅ Lesson recorded for {}: {}", tool, topic);
    Ok(())
}
