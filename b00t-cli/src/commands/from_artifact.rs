//! `b00t datum from-artifact` — E3: auto-datum generation from arbitrary artifacts.
//!
//! Extracts text from any file (PDF, Markdown, code, etc.) via kreuzberg, then
//! generates a `.tomllmd` datum stub enriched with the extracted content.
//! If `B00T_SM0L_ENDPOINT` is set, the sm0l oracle generates the datum; otherwise
//! a structured template is used.
//!
//! # Sub-task decomposition
//! - ST-A: kreuzberg text extraction (subprocess Python call)
//! - ST-B: datum content generation (sm0l oracle OR template)
//! - ST-C: write .tomllmd output (file or stdout)
//!
//! # Usage
//! ```bash
//! b00t datum from-artifact --path README.md
//! b00t datum from-artifact --path spec.pdf --topic iso-27001 --output-dir ~/.b00t/_b00t_/datums
//! b00t datum from-artifact --path api-schema.json --format json
//! ```

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

// ── ST-A: kreuzberg extraction ────────────────────────────────────────────────

/// Inline Python script that runs kreuzberg.extract_file and prints extracted text.
/// Gracefully handles missing kreuzberg with a clear error message.
const KREUZBERG_EXTRACT_SCRIPT: &str = r#"
import asyncio, sys
path = sys.argv[1]
try:
    from kreuzberg import extract_file
except ImportError:
    print("ERROR: kreuzberg not installed — run: uv pip install kreuzberg", file=sys.stderr)
    sys.exit(1)
async def main():
    result = await extract_file(path)
    content = getattr(result, 'content', None) or getattr(result, 'text', None) or ''
    print(content)
asyncio.run(main())
"#;

/// Extract plain text from `path` via kreuzberg (Python subprocess).
/// Returns extracted text. If kreuzberg is not installed, returns a clear error.
pub fn extract_text_from_artifact(path: &Path) -> Result<String> {
    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("path is not valid UTF-8"))?;

    let output = std::process::Command::new("python3")
        .args(["-c", KREUZBERG_EXTRACT_SCRIPT, path_str])
        .output()
        .context("failed to run python3 — is Python 3 installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("kreuzberg extraction failed: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ── ST-B: datum content generation ───────────────────────────────────────────

/// Infer a b00t:map tier from text length (proxy for content richness).
fn infer_tier(text: &str) -> &'static str {
    let words = text.split_whitespace().count();
    if words < 200 {
        "sm0l"
    } else if words < 2000 {
        "ch0nky"
    } else {
        "frontier"
    }
}

/// Infer complexity score from extracted text length.
fn infer_complexity(text: &str) -> u8 {
    let words = text.split_whitespace().count();
    match words {
        0..=99 => 1,
        100..=499 => 3,
        500..=1999 => 5,
        2000..=4999 => 7,
        _ => 9,
    }
}

/// Generate a `.tomllmd` datum from an artifact topic + extracted text.
///
/// If `sm0l_endpoint` is set, posts to the sm0l oracle for LLM-generated content.
/// Falls back to a structured template if the endpoint is absent or errors.
pub fn generate_datum_from_artifact(
    topic: &str,
    extracted_text: &str,
    source_path: &str,
    sm0l_endpoint: Option<&str>,
) -> String {
    if let Some(endpoint) = sm0l_endpoint {
        match call_sm0l_oracle(endpoint, topic, extracted_text) {
            Ok(content) => return content,
            Err(e) => eprintln!("warn: sm0l oracle failed ({e}), using template"),
        }
    }
    template_datum(topic, extracted_text, source_path)
}

/// Template-based datum generation when sm0l oracle is unavailable.
fn template_datum(topic: &str, extracted_text: &str, source_path: &str) -> String {
    let today = chrono::Local::now().format("%Y-%m-%d");
    let tier = infer_tier(extracted_text);
    let complexity = infer_complexity(extracted_text);
    let words = extracted_text.split_whitespace().count();

    // Summarise: first 3 non-empty lines of extracted text
    let summary_lines: Vec<&str> = extracted_text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(3)
        .collect();
    let summary = if summary_lines.is_empty() {
        format!("Auto-generated from {source_path}")
    } else {
        summary_lines.join(" · ")
    };

    // Truncate to a safe TOML single-line value
    let hint = if summary.len() > 120 {
        format!("{}…", &summary[..120])
    } else {
        summary.clone()
    };

    // Embed first 500 chars of extracted text as a TOML comment block
    let excerpt: String = extracted_text.chars().take(500).collect();
    let excerpt_lines: Vec<String> = excerpt
        .lines()
        .map(|l| format!("# {l}"))
        .collect();
    let excerpt_block = excerpt_lines.join("\n");

    format!(
        r#"# Auto-generated datum from artifact: {source_path}
# Generated by: b00t datum from-artifact on {today}
# Extracted words: {words}
# Enrich with: b00t learn {topic}
#
# --- Extracted excerpt (first 500 chars) ---
{excerpt_block}
# ---

[b00t]
name = "{topic}"
type = "skill"
hint = "{hint}"
auto_generated = true
auto_generated_date = "{today}"
auto_generated_source = "{source_path}"

# b00t:map v1
# summary: {summary_short}
# tags: auto-artifact, kreuzberg, from-artifact
# tier: {tier}
# cmds: b00t learn {topic}
# complexity: {complexity}
"#,
        summary_short = if summary.len() > 80 {
            format!("{}…", &summary[..80])
        } else {
            summary.clone()
        },
    )
}

/// Call sm0l oracle via HTTP POST to generate datum content from extracted text.
/// Expects the endpoint to accept `{ "prompt": "...", "max_tokens": N }` and return
/// `{ "text": "..." }` (OpenAI-compatible completion API).
fn call_sm0l_oracle(endpoint: &str, topic: &str, extracted_text: &str) -> Result<String> {
    let excerpt: String = extracted_text.chars().take(2000).collect();
    let prompt = format!(
        "Generate a valid b00t datum .tomllmd file for the topic '{topic}'. \
         The following text was extracted from an artifact related to this topic:\n\n{excerpt}\n\n\
         Output ONLY the TOML content with # b00t:map v1 tail-map. No explanation."
    );

    let body = serde_json::json!({
        "prompt": prompt,
        "max_tokens": 512,
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("build reqwest client")?;

    let resp = client
        .post(endpoint)
        .json(&body)
        .send()
        .context("sm0l oracle POST")?;

    if !resp.status().is_success() {
        anyhow::bail!("sm0l returned HTTP {}", resp.status());
    }

    let json: serde_json::Value = resp.json().context("parse sm0l response")?;
    let text = json
        .get("text")
        .or_else(|| json.get("content"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("sm0l response missing 'text' field"))?;

    Ok(text.to_string())
}

// ── ST-C: write output ────────────────────────────────────────────────────────

/// Write generated datum to `<output_dir>/FROM-ARTIFACT-<safe_topic>.tomllmd`.
pub fn write_artifact_datum(output_dir: &Path, topic: &str, content: &str) -> Result<PathBuf> {
    let safe = topic.replace(['/', '\\', ' ', ':', '.'], "-");
    std::fs::create_dir_all(output_dir).context("create output dir")?;
    let path = output_dir.join(format!("FROM-ARTIFACT-{safe}.tomllmd"));
    std::fs::write(&path, content).context("write datum file")?;
    Ok(path)
}

// ── CLI interface ──────────────────────────────────────────────────────────────

#[derive(clap::Parser, Clone, Debug)]
pub struct FromArtifactArgs {
    #[clap(long, help = "Path to the artifact file (PDF, Markdown, code, etc.)")]
    pub path: PathBuf,

    #[clap(long, help = "Topic / datum name (defaults to file stem)")]
    pub topic: Option<String>,

    #[clap(long, help = "Output directory (default: stdout)")]
    pub output_dir: Option<PathBuf>,

    #[clap(long, default_value = "toml", help = "Output format: toml | json")]
    pub format: String,

    #[clap(
        long,
        env = "B00T_SM0L_ENDPOINT",
        help = "sm0l oracle HTTP endpoint for LLM-generated datums"
    )]
    pub sm0l_endpoint: Option<String>,
}

pub fn handle_from_artifact(args: &FromArtifactArgs) -> Result<()> {
    let path = &args.path;

    if !path.exists() {
        anyhow::bail!("artifact not found: {}", path.display());
    }

    let topic = args.topic.clone().unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    });

    eprintln!("[from-artifact] extracting: {}", path.display());
    let extracted = extract_text_from_artifact(path)?;
    let word_count = extracted.split_whitespace().count();
    eprintln!("[from-artifact] extracted {word_count} words from {}", path.display());

    let source_path = path.to_string_lossy().to_string();
    let content = generate_datum_from_artifact(
        &topic,
        &extracted,
        &source_path,
        args.sm0l_endpoint.as_deref(),
    );

    match &args.output_dir {
        Some(dir) => {
            let written = write_artifact_datum(dir, &topic, &content)?;
            match args.format.as_str() {
                "json" => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "topic": topic,
                            "source": source_path,
                            "word_count": word_count,
                            "output": written.to_string_lossy(),
                        }))?
                    );
                }
                _ => {
                    println!("[from_artifact]");
                    println!("topic = {topic:?}");
                    println!("source = {source_path:?}");
                    println!("word_count = {word_count}");
                    println!("output = {:?}", written.display());
                }
            }
        }
        None => {
            // No output dir → print datum to stdout
            print!("{content}");
        }
    }

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn infer_tier_short_text() {
        assert_eq!(infer_tier("hello world"), "sm0l");
    }

    #[test]
    fn infer_tier_medium_text() {
        let text = "word ".repeat(500);
        assert_eq!(infer_tier(&text), "ch0nky");
    }

    #[test]
    fn infer_tier_long_text() {
        let text = "word ".repeat(3000);
        assert_eq!(infer_tier(&text), "frontier");
    }

    #[test]
    fn infer_complexity_short() {
        assert_eq!(infer_complexity("hello"), 1);
        let t = "word ".repeat(50);
        assert_eq!(infer_complexity(&t), 1);
    }

    #[test]
    fn infer_complexity_medium() {
        let t = "word ".repeat(300);
        assert_eq!(infer_complexity(&t), 3);
    }

    #[test]
    fn template_datum_contains_required_fields() {
        let content = template_datum("rust-async.skill", "async await tokio futures", "/tmp/spec.pdf");
        assert!(content.contains("[b00t]"), "TOML section");
        assert!(content.contains("rust-async.skill"), "topic name");
        assert!(content.contains("auto_generated = true"), "auto_generated flag");
        assert!(content.contains("# b00t:map v1"), "tail-map present");
        assert!(content.contains("# tier:"), "tier field");
        assert!(content.contains("# complexity:"), "complexity field");
        assert!(content.contains("/tmp/spec.pdf"), "source path recorded");
    }

    #[test]
    fn template_datum_excerpt_present() {
        let text = "This is important content about the topic.";
        let content = template_datum("test-topic", text, "test.md");
        // Excerpt is embedded as comment lines
        assert!(content.contains("# This is important content"));
    }

    #[test]
    fn write_artifact_datum_creates_file() {
        let dir = TempDir::new().unwrap();
        let content = "[b00t]\nname = \"test\"\ntype = \"skill\"\nhint = \"test\"\n";
        let path = write_artifact_datum(dir.path(), "my.topic", content).unwrap();
        assert!(path.exists(), "file created");
        assert!(path.to_string_lossy().contains("FROM-ARTIFACT-"));
        let read_back = std::fs::read_to_string(&path).unwrap();
        assert_eq!(read_back, content);
    }

    #[test]
    fn extract_text_returns_error_without_python() {
        // Test that the function doesn't panic even if kreuzberg errors
        // (the process may succeed or fail depending on environment)
        let path = PathBuf::from("/nonexistent/file.pdf");
        let result = extract_text_from_artifact(&path);
        // We expect an error (file not found or kreuzberg error) — not a panic
        assert!(result.is_err(), "non-existent file should error");
    }
}
