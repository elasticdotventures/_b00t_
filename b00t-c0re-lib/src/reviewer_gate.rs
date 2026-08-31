//! #1106/#1101: shared "reviewer gate" primitive — the documented-but-
//! unbuilt karpathy/deepwiki 3-gate autolearn pattern's Research gate,
//! generalized to two verdict vocabularies over one dispatch path.
//!
//! A sm0l reviewer model sits between Orient and Act, judging either:
//! - **Relevance** (#1106): is this skill/datum's content relevant to the
//!   current goal, before its full body is consumed? `RELEVANT|SKIP:<reason>`.
//! - **Disclosure** (#1101): is this content safe to write to a shared,
//!   global, or public-facing scope? `SAFE|SENSITIVE:<reason>`.
//!
//! Both issues explicitly require failing OPEN (permissive, with a loud
//! warning) when the reviewer backend is unreachable or returns something
//! unparseable — alpha tooling should degrade visibly, not silently block
//! normal work. Callers that need a stricter default (e.g. #1101's "SENSITIVE
//! blocks by default") implement that policy themselves by matching on
//! [`GateVerdict::Block`] vs the other variants; this module only reports
//! what the reviewer actually said (or that it couldn't be asked).
//!
//! Dispatch substrate: `sm0l_dispatch::dispatch()`, NOT `grok ask`. Per
//! `_b00t_/learn/grok.md`, `grok ask`'s default irontology backend fails
//! silently (empty results, not an error) when no HelixDB server is
//! running — a poor foundation for a gate whose whole job is to fail
//! *loudly* when unavailable. `sm0l_dispatch::SmolEndpoint::discover()`
//! already implements the fail-open priority chain both issues want
//! (env override → local ch0nky/sm0l ports → HF Inference API → Err), is
//! already live (used by `commands::learn`'s DWIW path), and is fully
//! independent of HelixDB.

use crate::sm0l_dispatch::{self, SmolBehavior, SmolConfig};

/// Which judgment this gate call is making.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateMode {
    /// #1106: is the reviewed content relevant to `goal`?
    Relevance { goal: String },
    /// #1101: is the reviewed content safe to disclose publicly?
    Disclosure,
}

/// Outcome of a gate call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateVerdict {
    /// `RELEVANT` / `SAFE`.
    Pass,
    /// `SKIP:<reason>` / `SENSITIVE:<reason>` — a real, parsed verdict from
    /// the reviewer, not an availability failure.
    Block { reason: String },
    /// The reviewer backend was unreachable, or returned something that
    /// didn't parse as either verdict token. Callers should treat this as
    /// permissive (proceed) per both issues' fail-open requirement, while
    /// surfacing `warning` to the operator — this is NOT the same as a
    /// reviewed-and-safe [`GateVerdict::Pass`], and callers that want to
    /// distinguish "reviewed" from "review unavailable" can match on this
    /// variant specifically.
    Unavailable { warning: String },
}

impl GateVerdict {
    /// True for `Pass` and `Unavailable` (fail-open) — false only for a
    /// genuine, parsed `Block` verdict.
    pub fn allows_proceeding(&self) -> bool {
        !matches!(self, GateVerdict::Block { .. })
    }
}

/// Ask the reviewer gate to judge `content` under `mode`. Fails open (with
/// a loud `eprintln!`) on any dispatch or parse failure — never panics,
/// never silently blocks.
pub fn gate_verdict(content: &str, mode: &GateMode) -> GateVerdict {
    let behavior = match mode {
        GateMode::Relevance { goal } => SmolBehavior::RelevanceGate { goal: goal.clone() },
        GateMode::Disclosure => SmolBehavior::DisclosureGate,
    };
    let config = SmolConfig::default();
    match sm0l_dispatch::dispatch(&behavior, &config, content, None, 32_000) {
        Ok(output) => parse_verdict(mode, output.result.as_deref().unwrap_or("")),
        Err(e) => {
            let warning = format!("reviewer gate unavailable ({e}) — failing open");
            eprintln!("⚠️  {warning}");
            GateVerdict::Unavailable { warning }
        }
    }
}

fn parse_verdict(mode: &GateMode, raw: &str) -> GateVerdict {
    let trimmed = raw.trim();
    let (pass_token, block_prefix) = match mode {
        GateMode::Relevance { .. } => ("RELEVANT", "SKIP:"),
        GateMode::Disclosure => ("SAFE", "SENSITIVE:"),
    };

    if trimmed.eq_ignore_ascii_case(pass_token) {
        return GateVerdict::Pass;
    }
    if let Some(reason) = strip_prefix_ci(trimmed, block_prefix) {
        return GateVerdict::Block { reason: reason.trim().to_string() };
    }

    let warning = format!("reviewer gate returned unparseable verdict '{trimmed}' — failing open");
    eprintln!("⚠️  {warning}");
    GateVerdict::Unavailable { warning }
}

/// Case-insensitive `str::strip_prefix`, since sm0l models don't always
/// respect exact-case instructions.
fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_verdict_relevance_pass() {
        assert_eq!(
            parse_verdict(&GateMode::Relevance { goal: "x".into() }, "RELEVANT"),
            GateVerdict::Pass
        );
        assert_eq!(
            parse_verdict(&GateMode::Relevance { goal: "x".into() }, "relevant"),
            GateVerdict::Pass,
            "reviewer verdicts should be matched case-insensitively"
        );
    }

    #[test]
    fn parse_verdict_relevance_block() {
        assert_eq!(
            parse_verdict(&GateMode::Relevance { goal: "x".into() }, "SKIP:not on topic"),
            GateVerdict::Block { reason: "not on topic".to_string() }
        );
    }

    #[test]
    fn parse_verdict_disclosure_pass_and_block() {
        assert_eq!(
            parse_verdict(&GateMode::Disclosure, "SAFE"),
            GateVerdict::Pass
        );
        assert_eq!(
            parse_verdict(&GateMode::Disclosure, "SENSITIVE:leaks internal hostname"),
            GateVerdict::Block { reason: "leaks internal hostname".to_string() }
        );
    }

    #[test]
    fn parse_verdict_unparseable_fails_open() {
        let v = parse_verdict(&GateMode::Relevance { goal: "x".into() }, "uh, maybe?");
        assert!(matches!(v, GateVerdict::Unavailable { .. }));
        assert!(v.allows_proceeding());
    }

    #[test]
    fn parse_verdict_empty_fails_open() {
        let v = parse_verdict(&GateMode::Disclosure, "");
        assert!(matches!(v, GateVerdict::Unavailable { .. }));
    }

    #[test]
    fn allows_proceeding_is_false_only_for_block() {
        assert!(GateVerdict::Pass.allows_proceeding());
        assert!(GateVerdict::Unavailable { warning: "x".into() }.allows_proceeding());
        assert!(!GateVerdict::Block { reason: "x".into() }.allows_proceeding());
    }

    /// `gate_verdict()` itself, with no sm0l endpoint reachable in this
    /// test environment, must fail open rather than error/panic — this is
    /// the end-to-end path a real caller (skill activate, lfmf --global)
    /// exercises when the reviewer backend genuinely isn't running.
    #[test]
    fn gate_verdict_fails_open_when_no_endpoint_reachable() {
        // Best-effort: only meaningful when no B00T_AI_SM0L_BASE / local
        // sm0l port / HF_TOKEN happens to be configured in the test
        // environment. If one IS configured, this just exercises the real
        // dispatch path instead, which is also fine to run.
        let verdict = gate_verdict("some content", &GateMode::Disclosure);
        assert!(verdict.allows_proceeding());
    }
}
