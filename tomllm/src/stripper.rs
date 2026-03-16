//! Comment stripping for .tomllm files
//!
//! TOML natively ignores `#` comments during parsing — this module extracts them
//! BEFORE parsing so they can be associated with their adjacent keys/sections.
//! Use `strip()` when passing config values downstream to save tokens.

/// Strip all `#` comment lines from a .tomllm string → pure TOML
///
/// Output is valid TOML parseable by any standard TOML parser.
/// Inline comments (after values on same line) are also stripped.
pub fn strip(input: &str) -> String {
    input
        .lines()
        .map(strip_line)
        .filter(|l| !l.trim().is_empty() || l.is_empty()) // preserve blank lines for readability
        .collect::<Vec<_>>()
        .join("\n")
}

/// Strip `#` comment from a single line, respecting quoted strings
fn strip_line(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let mut in_string = false;
    let mut escape_next = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if escape_next {
            result.push(c);
            escape_next = false;
            i += 1;
            continue;
        }

        if c == '\\' && in_string {
            result.push(c);
            escape_next = true;
            i += 1;
            continue;
        }

        if c == '"' {
            in_string = !in_string;
            result.push(c);
            i += 1;
            continue;
        }

        if c == '#' && !in_string {
            // comment starts here — trim trailing whitespace and stop
            break;
        }

        result.push(c);
        i += 1;
    }

    // trim trailing whitespace left by inline comment removal
    result.trim_end().to_string()
}

/// Extract all comment lines from a .tomllm string
/// Returns (line_number, comment_text) pairs (0-indexed)
pub fn extract_comments(input: &str) -> Vec<(usize, String)> {
    input
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                let comment = trimmed.trim_start_matches('#').trim().to_string();
                Some((i, comment))
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_comment_lines() {
        let input = r#"
# 🤓 use uv pip, never pip directly
package_manager = "uv"

# section comment
[toolchain]
python = "3.12"  # inline comment
"#;
        let stripped = strip(input);
        assert!(!stripped.contains("🤓"));
        assert!(!stripped.contains("section comment"));
        assert!(!stripped.contains("inline comment"));
        assert!(stripped.contains("package_manager"));
        assert!(stripped.contains("python"));
        // must parse as valid TOML
        let parsed: toml::Value = toml::from_str(&stripped).expect("stripped should be valid TOML");
        assert!(parsed.get("package_manager").is_some());
    }

    #[test]
    fn test_strip_preserves_quoted_hash() {
        let input = r#"pattern = "http://example.com/#anchor""#;
        let stripped = strip_line(input);
        assert_eq!(stripped, input); // hash inside string must not be stripped
    }

    #[test]
    fn test_extract_comments() {
        let input = "# first\nkey = \"value\"\n# second\n";
        let comments = extract_comments(input);
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0], (0, "first".to_string()));
        assert_eq!(comments[1], (2, "second".to_string()));
    }
}
