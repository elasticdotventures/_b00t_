//! Delegation macro expansion for @tier: shorthand in `b00t hive run`.
//!
//! Expands `@tier[opt-task-id]: <cmd>` → `b00t agent delegate <tier> '<cmd>'
//! --output-contract='<canonical>'`
//!
//! Output contracts are hardcoded here until model-routing.tomllm is implemented
//! (PRD-ARCH-003 sub-task 2). The TODO marker below is the load point.
//!
//! PRD-ARCH-003: Delegation Shorthand Macros

use crate::otel::{self, MetricEvent};
use crate::postel;

/// Result of parsing a `@tier[task]:` prefix.
#[derive(Debug, Clone, PartialEq)]
pub struct DelegationMacro {
    /// Canonical tier name: sm0l | ch0nky | frontier
    pub tier: String,
    /// Optional task ID from `@sm0l[42]:` syntax
    pub task_id: Option<String>,
    /// The command string after the prefix
    pub command: String,
    /// Canonical output contract for this tier
    pub output_contract: String,
}

/// Output contracts per tier.
/// TODO(PRD-ARCH-003#2): load from _b00t_/model-routing.tomllm at runtime.
fn output_contract(tier: &str) -> &'static str {
    match tier {
        "sm0l" => "PASS|FAIL:<5lines>",
        "ch0nky" => "diff+test",
        "frontier" => "decision+rationale",
        _ => "PASS|FAIL",
    }
}

/// Canonical tier names. Postel: accept aliases, normalize.
fn canonicalize_tier(raw: &str) -> Option<&'static str> {
    match raw.to_lowercase().as_str() {
        "sm0l" | "small" | "haiku" | "mini" => Some("sm0l"),
        "ch0nky" | "chunky" | "medium" | "sonnet" | "coder" => Some("ch0nky"),
        "frontier" | "opus" | "large" | "gpt4" | "claude" => Some("frontier"),
        _ => None,
    }
}

/// Try to parse `@tier[task-id]: command` from the start of `input`.
///
/// Returns `Some(DelegationMacro)` if the prefix matches, `None` otherwise.
/// Emits a Postel hint when a non-canonical tier alias is used.
/// Emits a warning (and returns `None`) for unrecognized `@foo:` prefixes.
pub fn parse_delegation_prefix(input: &str) -> Option<DelegationMacro> {
    let input = input.trim();
    if !input.starts_with('@') {
        return None;
    }

    // Match @tier[optional-task]: command
    // Regex equivalent: ^@([a-zA-Z0-9_]+)(?:\[([^\]]+)\])?:\s*(.+)$
    let rest = &input[1..]; // skip '@'

    // Find the ':' that ends the prefix
    let colon_pos = rest.find(':')?;
    let prefix_part = &rest[..colon_pos];
    let cmd_part = rest[colon_pos + 1..].trim();

    if cmd_part.is_empty() {
        eprintln!("⚠️  b00t: @{prefix_part}: has no command — skipping delegation expansion");
        return None;
    }

    // Parse optional [task-id] from prefix_part: "sm0l" or "sm0l[42]"
    let (tier_raw, task_id) = if let Some(bracket_start) = prefix_part.find('[') {
        let bracket_end = prefix_part.find(']').unwrap_or(prefix_part.len());
        let tier = &prefix_part[..bracket_start];
        let task = prefix_part[bracket_start + 1..bracket_end].to_owned();
        (tier, if task.is_empty() { None } else { Some(task) })
    } else {
        (prefix_part, None)
    };

    match canonicalize_tier(tier_raw) {
        Some(canonical) => {
            if tier_raw.to_lowercase() != canonical {
                postel::hint(
                    tier_raw,
                    canonical,
                    &format!("@{canonical}: {cmd_part}"),
                    "canonical tier name",
                );
            }
            let contract = output_contract(canonical).to_owned();
            otel::record(MetricEvent::CommandRun {
                cmd: format!("@{canonical}: {cmd_part}"),
                tier: Some(canonical.to_owned()),
                duration_ms: None,
            });
            Some(DelegationMacro {
                tier: canonical.to_owned(),
                task_id,
                command: cmd_part.to_owned(),
                output_contract: contract,
            })
        }
        None => {
            // Unrecognized @foo: prefix → warn, pass through unchanged
            eprintln!(
                "⚠️  b00t: unknown delegation tier '@{tier_raw}:' — \
                 known tiers: sm0l, ch0nky, frontier; \
                 treating as literal command"
            );
            None
        }
    }
}

/// Expand a `DelegationMacro` to its full CLI invocation string.
///
/// `@sm0l: cargo test` → `b00t agent delegate sm0l 'cargo test'
///     --output-contract='PASS|FAIL:<5lines>'`
pub fn expand(m: &DelegationMacro) -> String {
    let mut parts = vec![
        "b00t".to_owned(),
        "agent".to_owned(),
        "delegate".to_owned(),
        m.tier.clone(),
        format!("'{}'", m.command.replace('\'', "\\'")),
        format!("--output-contract='{}'", m.output_contract),
    ];
    if let Some(task) = &m.task_id {
        parts.push(format!("--task={task}"));
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sm0l_prefix() {
        let m = parse_delegation_prefix("@sm0l: cargo test -p b00t-cli").unwrap();
        assert_eq!(m.tier, "sm0l");
        assert_eq!(m.command, "cargo test -p b00t-cli");
        assert_eq!(m.output_contract, "PASS|FAIL:<5lines>");
        assert!(m.task_id.is_none());
    }

    #[test]
    fn test_parse_ch0nky_prefix() {
        let m = parse_delegation_prefix("@ch0nky: implement OKR datum schema").unwrap();
        assert_eq!(m.tier, "ch0nky");
        assert_eq!(m.output_contract, "diff+test");
    }

    #[test]
    fn test_parse_frontier_prefix() {
        let m = parse_delegation_prefix("@frontier: evaluate security model").unwrap();
        assert_eq!(m.tier, "frontier");
        assert_eq!(m.output_contract, "decision+rationale");
    }

    #[test]
    fn test_parse_task_id_variant() {
        let m = parse_delegation_prefix("@sm0l[42]: cargo test").unwrap();
        assert_eq!(m.tier, "sm0l");
        assert_eq!(m.task_id, Some("42".to_owned()));
        assert_eq!(m.command, "cargo test");
    }

    #[test]
    fn test_no_prefix_returns_none() {
        assert!(parse_delegation_prefix("cargo test").is_none());
        assert!(parse_delegation_prefix("git push --force").is_none());
    }

    #[test]
    fn test_empty_command_returns_none() {
        assert!(parse_delegation_prefix("@sm0l:").is_none());
        assert!(parse_delegation_prefix("@sm0l:   ").is_none());
    }

    #[test]
    fn test_unknown_tier_returns_none() {
        // Unknown tier → warn + return None (passthrough)
        assert!(parse_delegation_prefix("@foo: some command").is_none());
    }

    #[test]
    fn test_expand_without_task() {
        let m = DelegationMacro {
            tier: "sm0l".into(),
            task_id: None,
            command: "cargo test".into(),
            output_contract: "PASS|FAIL:<5lines>".into(),
        };
        let expanded = expand(&m);
        assert!(expanded.contains("b00t agent delegate sm0l"));
        assert!(expanded.contains("--output-contract='PASS|FAIL:<5lines>'"));
    }

    #[test]
    fn test_expand_with_task() {
        let m = DelegationMacro {
            tier: "sm0l".into(),
            task_id: Some("42".into()),
            command: "cargo test".into(),
            output_contract: "PASS|FAIL:<5lines>".into(),
        };
        let expanded = expand(&m);
        assert!(expanded.contains("--task=42"));
    }

    #[test]
    fn test_postel_alias_sm0l() {
        // "small" is a Postel alias for "sm0l"
        let m = parse_delegation_prefix("@small: echo hi");
        assert!(m.is_some());
        assert_eq!(m.unwrap().tier, "sm0l");
    }
}
