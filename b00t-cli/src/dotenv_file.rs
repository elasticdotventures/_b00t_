//! A tiny `.env` reader for `b00t agent invoke`.
//!
//! The invoked agent's child process gets its environment from here: inherited process env, then the local `.env`
//! (the project's own settings, so a stale inherited variable does not silently win), then the agent datum's own
//! `[b00t.env]`. Same ".env overrides the process env" convention as `session_memory`. Values are never printed.
//!
//! Parser drafted by the local Qwen3.8 via pi (docs/PATTERN-pi-subagents.md in kr0ki); two bugs were found in triage
//! (trailing space before an inline comment, a quoted value followed by a comment) and fixed here with tests.

use std::collections::HashMap;
use std::path::Path;

/// Read `<dir>/.env`. A missing or unreadable file is an empty map: `.env` is optional.
pub fn load_dotenv(dir: &Path) -> HashMap<String, String> {
    std::fs::read_to_string(dir.join(".env"))
        .map(|text| parse_dotenv(&text))
        .unwrap_or_default()
}

/// Inherited env < local `.env` < the agent datum's own env.
pub fn merged_env(
    mut process: HashMap<String, String>,
    dotenv: &HashMap<String, String>,
    agent: Option<&HashMap<String, String>>,
) -> HashMap<String, String> {
    process.extend(dotenv.iter().map(|(k, v)| (k.clone(), v.clone())));
    if let Some(agent) = agent {
        process.extend(agent.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    process
}

pub fn parse_dotenv(contents: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for raw_line in contents.lines() {
        let line = raw_line.trim_end_matches('\r').trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let rest = if line.starts_with("export ") {
            line["export ".len()..].trim_start()
        } else {
            line
        };
        let eq = match rest.find('=') {
            Some(p) => p,
            None => continue,
        };
        let key = rest[..eq].trim();
        if !is_valid_key(key) {
            continue;
        }
        let value = parse_value(&rest[eq + 1..]);
        out.insert(key.to_string(), value);
    }
    out
}

fn is_valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn parse_value(raw: &str) -> String {
    let trimmed = raw.trim();
    // A quoted value ends at its matching closing quote, so `KEY="a b" # note` keeps `a b`.
    // (Single quotes are verbatim; double quotes honour \" so an escaped quote does not close the value.)
    if let Some(quote @ ('"' | '\'')) = trimmed.chars().next() {
        let mut escaped = false;
        for (i, c) in trimmed.char_indices().skip(1) {
            if quote == '"' && escaped {
                escaped = false;
            } else if quote == '"' && c == '\\' {
                escaped = true;
            } else if c == quote {
                let inner = &trimmed[1..i];
                return if quote == '"' {
                    unescape_double(inner)
                } else {
                    inner.to_string()
                };
            }
        }
        // no closing quote: treat the whole thing as an unquoted literal below
    }
    match trimmed.find(" #") {
        Some(p) => trimmed[..p].trim_end().to_string(),
        None => trimmed.to_string(),
    }
}

fn unescape_double(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_assignment() {
        let m = parse_dotenv("KEY=value");
        assert_eq!(m.get("KEY"), Some(&"value".to_string()));
    }

    #[test]
    fn blank_and_comment_lines() {
        let m = parse_dotenv("\n\n# comment\nKEY=val\n");
        assert_eq!(m.get("KEY"), Some(&"val".to_string()));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn export_prefix() {
        let m = parse_dotenv("export KEY = value");
        assert_eq!(m.get("KEY"), Some(&"value".to_string()));
    }

    #[test]
    fn invalid_key_skipped() {
        let m = parse_dotenv("1BAD=x\nA-B=x\nOK=y");
        assert!(m.get("1BAD").is_none());
        assert!(m.get("A-B").is_none());
        assert_eq!(m.get("OK"), Some(&"y".to_string()));
    }

    #[test]
    fn no_equals_skipped() {
        let m = parse_dotenv("NOEQUALS\nKEY=val");
        assert!(m.get("NOEQUALS").is_none());
        assert_eq!(m.get("KEY"), Some(&"val".to_string()));
    }

    #[test]
    fn split_on_first_equals() {
        let m = parse_dotenv("KEY=a=b=c");
        assert_eq!(m.get("KEY"), Some(&"a=b=c".to_string()));
    }

    #[test]
    fn double_quotes_verbatim() {
        let m = parse_dotenv("KEY=\"hello world\"");
        assert_eq!(m.get("KEY"), Some(&"hello world".to_string()));
    }

    #[test]
    fn single_quotes_verbatim() {
        let m = parse_dotenv("KEY='hello world'");
        assert_eq!(m.get("KEY"), Some(&"hello world".to_string()));
    }

    #[test]
    fn escapes_in_double_quotes() {
        let m = parse_dotenv("KEY=\"line1\\nline2\\\"quote\\\\back\"");
        assert_eq!(m.get("KEY"), Some(&"line1\nline2\"quote\\back".to_string()));
    }

    #[test]
    fn no_escapes_in_single_quotes() {
        let m = parse_dotenv("KEY='a\\nb'");
        assert_eq!(m.get("KEY"), Some(&"a\\nb".to_string()));
    }

    #[test]
    fn inline_comment_stripped() {
        let m = parse_dotenv("KEY=value # comment");
        assert_eq!(m.get("KEY"), Some(&"value".to_string()));
    }

    #[test]
    fn hash_in_url_kept() {
        let m = parse_dotenv("URL=http://h/x#y");
        assert_eq!(m.get("URL"), Some(&"http://h/x#y".to_string()));
    }

    #[test]
    fn quoted_value_contains_hash() {
        let m = parse_dotenv("KEY=\"val # not comment\"");
        assert_eq!(m.get("KEY"), Some(&"val # not comment".to_string()));
    }

    #[test]
    fn crlf_handling() {
        let m = parse_dotenv("KEY=val\r\n");
        assert_eq!(m.get("KEY"), Some(&"val".to_string()));
    }

    #[test]
    fn duplicate_keys_last_wins() {
        let m = parse_dotenv("KEY=first\nKEY=second");
        assert_eq!(m.get("KEY"), Some(&"second".to_string()));
    }

    #[test]
    fn empty_value() {
        let m = parse_dotenv("KEY=");
        assert_eq!(m.get("KEY"), Some(&"".to_string()));
    }

    #[test]
    fn no_variable_expansion() {
        let m = parse_dotenv("A=base\nB=$A/path");
        assert_eq!(m.get("B"), Some(&"$A/path".to_string()));
    }

    #[test]
    fn openai_url_parsed_exactly() {
        let m = parse_dotenv("OPENAI_API_URL=http://127.0.0.1:8002/v1");
        assert_eq!(
            m.get("OPENAI_API_URL"),
            Some(&"http://127.0.0.1:8002/v1".to_string())
        );
    }

    #[test]
    fn kr0ki_origins_parsed_exactly() {
        let m = parse_dotenv(
            "KR0KI_STORYB00K_ALLOWED_ORIGINS=http://localhost:8787,http://192.168.1.137:8787",
        );
        assert_eq!(
            m.get("KR0KI_STORYB00K_ALLOWED_ORIGINS"),
            Some(&"http://localhost:8787,http://192.168.1.137:8787".to_string())
        );
    }

    #[test]
    fn whitespace_around_key_and_unquoted_value() {
        let m = parse_dotenv("  KEY  =  value  ");
        assert_eq!(m.get("KEY"), Some(&"value".to_string()));
    }

    #[test]
    fn spaces_before_an_inline_comment_are_not_part_of_the_value() {
        assert_eq!(
            parse_dotenv("KEY=value   # note").get("KEY"),
            Some(&"value".to_string())
        );
    }

    #[test]
    fn a_quoted_value_may_be_followed_by_a_comment() {
        assert_eq!(
            parse_dotenv("KEY=\"a b\" # note").get("KEY"),
            Some(&"a b".to_string())
        );
        assert_eq!(
            parse_dotenv("KEY='a b' # note").get("KEY"),
            Some(&"a b".to_string())
        );
        assert_eq!(
            parse_dotenv("KEY=\"say \\\"hi\\\"\" # n").get("KEY"),
            Some(&"say \"hi\"".to_string())
        );
    }

    #[test]
    fn an_unterminated_quote_is_kept_literally() {
        assert_eq!(
            parse_dotenv("KEY=\"oops").get("KEY"),
            Some(&"\"oops".to_string())
        );
    }

    #[test]
    fn precedence_is_process_then_dotenv_then_agent_config() {
        let process = HashMap::from([
            ("A".to_string(), "proc".to_string()),
            ("B".to_string(), "proc".to_string()),
            ("C".to_string(), "proc".to_string()),
        ]);
        let dotenv = HashMap::from([
            ("B".to_string(), "dotenv".to_string()),
            ("C".to_string(), "dotenv".to_string()),
        ]);
        let agent = HashMap::from([("C".to_string(), "agent".to_string())]);
        let env = merged_env(process, &dotenv, Some(&agent));
        assert_eq!(env["A"], "proc");
        assert_eq!(env["B"], "dotenv"); // the project's .env beats a stale inherited variable
        assert_eq!(env["C"], "agent"); // an explicit agent config beats both
    }

    #[test]
    fn a_missing_dotenv_is_empty_not_an_error() {
        let dir = std::env::temp_dir().join(format!("b00t-dotenv-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(load_dotenv(&dir).is_empty());
        std::fs::write(
            dir.join(".env"),
            "OPENAI_API_URL=http://127.0.0.1:8002/v1\n",
        )
        .unwrap();
        assert_eq!(
            load_dotenv(&dir)["OPENAI_API_URL"],
            "http://127.0.0.1:8002/v1"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
