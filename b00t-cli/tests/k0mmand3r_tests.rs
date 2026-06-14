// b00t-cli/tests/k0mmand3r_tests.rs
// Comprehensive integration tests for K0mmand3r typed commands.
// Uses Rust 2024 edition (workspace-level).

use b00t_cli::k0mmand3r::*;
use std::collections::BTreeMap;

// ============================================================================
// Section 1: K0mmand3rCmd::parse — all variants
// ============================================================================

#[test]
fn test_parse_negotiate_positional() {
    let cmd = K0mmand3rCmd::parse("/negotiate blessing observe-infra").unwrap();
    match cmd {
        K0mmand3rCmd::Negotiate {
            resource,
            id,
            modifiers,
        } => {
            assert_eq!(resource, "blessing");
            assert_eq!(id, "observe-infra");
            assert!(modifiers.is_empty(), "expected no extra modifiers");
        }
        _ => panic!("Expected Negotiate variant"),
    }
}

#[test]
fn test_parse_negotiate_blessing_modifier() {
    let cmd = K0mmand3rCmd::parse("/negotiate blessing:observe-infra").unwrap();
    match cmd {
        K0mmand3rCmd::Negotiate { resource, id, .. } => {
            assert_eq!(resource, "blessing");
            assert_eq!(id, "observe-infra");
        }
        _ => panic!("Expected Negotiate variant"),
    }
}

#[test]
fn test_parse_negotiate_resource_modifier() {
    let cmd = K0mmand3rCmd::parse("/negotiate resource:my-resource").unwrap();
    match cmd {
        K0mmand3rCmd::Negotiate { resource, id, .. } => {
            assert_eq!(resource, "resource");
            assert_eq!(id, "my-resource");
        }
        _ => panic!("Expected Negotiate variant"),
    }
}

#[test]
fn test_parse_negotiate_with_extra_modifiers() {
    let cmd = K0mmand3rCmd::parse("/negotiate blessing:deploy scope=global ttl=3600").unwrap();
    match cmd {
        K0mmand3rCmd::Negotiate {
            resource,
            id,
            modifiers,
        } => {
            assert_eq!(resource, "blessing");
            assert_eq!(id, "deploy");
            assert_eq!(modifiers.get("scope").map(String::as_str), Some("global"));
            assert_eq!(modifiers.get("ttl").map(String::as_str), Some("3600"));
        }
        _ => panic!("Expected Negotiate variant"),
    }
}

#[test]
fn test_parse_negotiate_insufficient_args() {
    let err = K0mmand3rCmd::parse("/negotiate").unwrap_err();
    assert!(err.contains("Usage"), "error should mention usage");
}

#[test]
fn test_parse_vote_yes() {
    let cmd = K0mmand3rCmd::parse("/vote prop-42 yes").unwrap();
    match cmd {
        K0mmand3rCmd::Vote {
            proposal,
            choice,
            reason,
        } => {
            assert_eq!(proposal, "prop-42");
            assert_eq!(choice, b00t_ipc::VoteChoice::Yes);
            assert_eq!(reason, None);
        }
        _ => panic!("Expected Vote variant"),
    }
}

#[test]
fn test_parse_vote_yes_shorthand() {
    let cmd = K0mmand3rCmd::parse("/vote prop-42 y").unwrap();
    match cmd {
        K0mmand3rCmd::Vote {
            proposal,
            choice,
            reason,
        } => {
            assert_eq!(proposal, "prop-42");
            assert_eq!(choice, b00t_ipc::VoteChoice::Yes);
            assert_eq!(reason, None);
        }
        _ => panic!("Expected Vote variant"),
    }
}

#[test]
fn test_parse_vote_no() {
    let cmd = K0mmand3rCmd::parse("/vote prop-42 no").unwrap();
    match cmd {
        K0mmand3rCmd::Vote {
            proposal, choice, ..
        } => {
            assert_eq!(proposal, "prop-42");
            assert_eq!(choice, b00t_ipc::VoteChoice::No);
        }
        _ => panic!("Expected Vote variant"),
    }
}

#[test]
fn test_parse_vote_no_shorthand() {
    let cmd = K0mmand3rCmd::parse("/vote prop-42 n").unwrap();
    match cmd {
        K0mmand3rCmd::Vote {
            proposal, choice, ..
        } => {
            assert_eq!(proposal, "prop-42");
            assert_eq!(choice, b00t_ipc::VoteChoice::No);
        }
        _ => panic!("Expected Vote variant"),
    }
}

#[test]
fn test_parse_vote_abstain() {
    let cmd = K0mmand3rCmd::parse("/vote prop-42 abstain").unwrap();
    match cmd {
        K0mmand3rCmd::Vote {
            proposal, choice, ..
        } => {
            assert_eq!(proposal, "prop-42");
            assert_eq!(choice, b00t_ipc::VoteChoice::Abstain);
        }
        _ => panic!("Expected Vote variant"),
    }
}

#[test]
fn test_parse_vote_abstain_shorthand() {
    let cmd = K0mmand3rCmd::parse("/vote prop-42 a").unwrap();
    match cmd {
        K0mmand3rCmd::Vote {
            proposal, choice, ..
        } => {
            assert_eq!(proposal, "prop-42");
            assert_eq!(choice, b00t_ipc::VoteChoice::Abstain);
        }
        _ => panic!("Expected Vote variant"),
    }
}

#[test]
fn test_parse_vote_with_reason() {
    // NOTE: "with" is a modifier keyword so avoid it in reason text
    let cmd = K0mmand3rCmd::parse("/vote prop-42 yes because it aligns our goals").unwrap();
    match cmd {
        K0mmand3rCmd::Vote {
            proposal,
            choice,
            reason,
        } => {
            assert_eq!(proposal, "prop-42");
            assert_eq!(choice, b00t_ipc::VoteChoice::Yes);
            assert_eq!(reason, Some("because it aligns our goals".to_string()));
        }
        _ => panic!("Expected Vote variant"),
    }
}

#[test]
fn test_parse_vote_with_modifiers() {
    // Use proposal= / choice= modifier syntax (avoid "on" which captures rest)
    let cmd = K0mmand3rCmd::parse("/vote proposal=prop-42 choice=yes reason=good").unwrap();
    match cmd {
        K0mmand3rCmd::Vote {
            proposal,
            choice,
            reason,
        } => {
            assert_eq!(proposal, "prop-42");
            assert_eq!(choice, b00t_ipc::VoteChoice::Yes);
            assert_eq!(reason, Some("good".to_string()));
        }
        _ => panic!("Expected Vote variant"),
    }
}

#[test]
fn test_parse_vote_invalid_choice() {
    let err = K0mmand3rCmd::parse("/vote prop-42 maybe").unwrap_err();
    assert!(err.contains("yes, no, or abstain"));
}

#[test]
fn test_parse_vote_missing_choice() {
    let err = K0mmand3rCmd::parse("/vote prop-42").unwrap_err();
    assert!(err.contains("Missing vote choice") || err.contains("Usage"));
}

#[test]
fn test_parse_delegate_positional() {
    let cmd = K0mmand3rCmd::parse("/delegate b00t-sandbox 5000").unwrap();
    match cmd {
        K0mmand3rCmd::Delegate { agent, budget } => {
            assert_eq!(agent, "b00t-sandbox");
            assert_eq!(budget, 5000);
        }
        _ => panic!("Expected Delegate variant"),
    }
}

#[test]
fn test_parse_delegate_with_modifiers() {
    // Avoid "to" keyword — use agent: and budget: directly
    let cmd = K0mmand3rCmd::parse("/delegate agent:executive budget:10000").unwrap();
    match cmd {
        K0mmand3rCmd::Delegate { agent, budget } => {
            assert_eq!(agent, "executive");
            assert_eq!(budget, 10000);
        }
        _ => panic!("Expected Delegate variant"),
    }
}

#[test]
fn test_parse_delegate_missing_budget() {
    let err = K0mmand3rCmd::parse("/delegate b00t-sandbox").unwrap_err();
    assert!(err.contains("budget") || err.contains("Missing") || err.contains("Usage"));
}

#[test]
fn test_parse_delegate_non_numeric_budget() {
    let err = K0mmand3rCmd::parse("/delegate b00t-sandbox lots").unwrap_err();
    assert!(err.contains("number") || err.contains("Budget"));
}

#[test]
fn test_parse_loop_goal_only() {
    let cmd = K0mmand3rCmd::parse("/loop goal:deploy").unwrap();
    match cmd {
        K0mmand3rCmd::Loop { spec } => {
            assert_eq!(spec.goal, "deploy");
            assert!(spec.metric.is_empty());
            assert!(spec.verify.is_empty());
        }
        _ => panic!("Expected Loop variant"),
    }
}

#[test]
fn test_parse_loop_full_spec() {
    let cmd = K0mmand3rCmd::parse(
        "/loop goal:deploy metric:uptime verify:healthcheck max:5 guard:has-blessing scope:global",
    )
    .unwrap();
    match cmd {
        K0mmand3rCmd::Loop { spec } => {
            assert_eq!(spec.goal, "deploy");
            assert_eq!(spec.metric, "uptime");
            assert_eq!(spec.verify, "healthcheck");
            assert_eq!(spec.max, Some(5));
            assert_eq!(spec.guard, Some("has-blessing".to_string()));
            assert_eq!(spec.scope, Some("global".to_string()));
        }
        _ => panic!("Expected Loop variant"),
    }
}

#[test]
fn test_parse_loop_missing_goal() {
    let err = K0mmand3rCmd::parse("/loop metric:uptime").unwrap_err();
    assert!(err.contains("goal"), "error: {err}");
}

#[test]
fn test_parse_crew_form() {
    let cmd = K0mmand3rCmd::parse("/crew form alice bob charlie").unwrap();
    match cmd {
        K0mmand3rCmd::Crew { action, members } => {
            assert_eq!(action, CrewAction::Form);
            assert_eq!(members, vec!["alice", "bob", "charlie"]);
        }
        _ => panic!("Expected Crew variant"),
    }
}

#[test]
fn test_parse_crew_join() {
    let cmd = K0mmand3rCmd::parse("/crew join existing-crew").unwrap();
    match cmd {
        K0mmand3rCmd::Crew { action, members } => {
            assert_eq!(action, CrewAction::Join);
            assert_eq!(members, vec!["existing-crew"]);
        }
        _ => panic!("Expected Crew variant"),
    }
}

#[test]
fn test_parse_crew_leave() {
    let cmd = K0mmand3rCmd::parse("/crew leave old-crew").unwrap();
    match cmd {
        K0mmand3rCmd::Crew { action, members } => {
            assert_eq!(action, CrewAction::Leave);
            assert_eq!(members, vec!["old-crew"]);
        }
        _ => panic!("Expected Crew variant"),
    }
}

#[test]
fn test_parse_crew_with_modifiers() {
    // Use action=form syntax to avoid "action" being consumed as positional
    let cmd = K0mmand3rCmd::parse("/crew action=form members=alice,bob").unwrap();
    match cmd {
        K0mmand3rCmd::Crew { action, members } => {
            assert_eq!(action, CrewAction::Form);
            assert_eq!(members, vec!["alice", "bob"]);
        }
        _ => panic!("Expected Crew variant"),
    }
}

#[test]
fn test_parse_crew_unknown_action() {
    let err = K0mmand3rCmd::parse("/crew disband").unwrap_err();
    assert!(err.contains("action") || err.contains("Unknown"));
}

#[test]
fn test_parse_crew_missing_action() {
    let err = K0mmand3rCmd::parse("/crew").unwrap_err();
    assert!(err.contains("Usage") || err.contains("action"));
}

#[test]
fn test_parse_handshake_positional() {
    let cmd = K0mmand3rCmd::parse("/handshake executive").unwrap();
    match cmd {
        K0mmand3rCmd::Handshake { agent, proposal } => {
            assert_eq!(agent, "executive");
            assert_eq!(proposal, None);
        }
        _ => panic!("Expected Handshake variant"),
    }
}

#[test]
fn test_parse_handshake_with_proposal() {
    let cmd = K0mmand3rCmd::parse("/handshake observer propose alliance").unwrap();
    match cmd {
        K0mmand3rCmd::Handshake { agent, proposal } => {
            assert_eq!(agent, "observer");
            assert_eq!(proposal, Some("propose alliance".to_string()));
        }
        _ => panic!("Expected Handshake variant"),
    }
}

#[test]
fn test_parse_handshake_with_modifier() {
    // Avoid "to" keyword — use agent: directly
    let cmd = K0mmand3rCmd::parse("/handshake agent:executive proposal=secret-deal").unwrap();
    match cmd {
        K0mmand3rCmd::Handshake { agent, proposal } => {
            assert_eq!(agent, "executive");
            assert_eq!(proposal, Some("secret-deal".to_string()));
        }
        _ => panic!("Expected Handshake variant"),
    }
}

#[test]
fn test_parse_handshake_missing_agent() {
    let err = K0mmand3rCmd::parse("/handshake").unwrap_err();
    assert!(err.contains("agent") || err.contains("Usage"));
}

#[test]
fn test_parse_status() {
    let cmd = K0mmand3rCmd::parse("/status").unwrap();
    assert_eq!(cmd, K0mmand3rCmd::Status);
}

#[test]
fn test_parse_status_with_extra() {
    // Extra tokens after /status are ignored by the parser
    let cmd = K0mmand3rCmd::parse("/status from agent:executive").unwrap();
    assert_eq!(cmd, K0mmand3rCmd::Status);
}

#[test]
fn test_parse_propose() {
    let cmd = K0mmand3rCmd::parse("/propose Let us build a new feature").unwrap();
    match cmd {
        K0mmand3rCmd::Propose { description } => {
            assert_eq!(description, "Let us build a new feature");
        }
        _ => panic!("Expected Propose variant"),
    }
}

#[test]
fn test_parse_propose_with_modifier() {
    let cmd = K0mmand3rCmd::parse("/propose description=refactor-the-database").unwrap();
    match cmd {
        K0mmand3rCmd::Propose { description } => {
            assert_eq!(description, "refactor-the-database");
        }
        _ => panic!("Expected Propose variant"),
    }
}

#[test]
fn test_parse_propose_empty() {
    let cmd = K0mmand3rCmd::parse("/propose").unwrap();
    match cmd {
        K0mmand3rCmd::Propose { description } => {
            assert!(description.is_empty(), "empty proposal is accepted");
        }
        _ => panic!("Expected Propose variant"),
    }
}

#[test]
fn test_parse_ahoy_full() {
    let cmd = K0mmand3rCmd::parse("/ahoy rust-dev 500 rust,cli,api Build CLI tools").unwrap();
    match cmd {
        K0mmand3rCmd::Ahoy {
            role,
            budget,
            skills,
            description,
        } => {
            assert_eq!(role, "rust-dev");
            assert_eq!(budget, 500);
            assert_eq!(skills, vec!["rust", "cli", "api"]);
            assert_eq!(description, "Build CLI tools");
        }
        _ => panic!("Expected Ahoy variant"),
    }
}

#[test]
fn test_parse_ahoy_via_modifiers() {
    // Avoid quotes which are literal in shell-split tokens
    let cmd = K0mmand3rCmd::parse(
        "/ahoy role=rust-dev budget=1000 skills=rust,cli description=Build-tools",
    )
    .unwrap();
    match cmd {
        K0mmand3rCmd::Ahoy {
            role,
            budget,
            skills,
            ..
        } => {
            assert_eq!(role, "rust-dev");
            assert_eq!(budget, 1000);
            assert_eq!(skills, vec!["rust", "cli"]);
        }
        _ => panic!("Expected Ahoy variant"),
    }
}

#[test]
fn test_parse_ahoy_missing_role() {
    let err = K0mmand3rCmd::parse("/ahoy").unwrap_err();
    assert!(err.contains("Usage") || err.contains("role"));
}

#[test]
fn test_parse_ahoy_missing_budget() {
    let err = K0mmand3rCmd::parse("/ahoy rust-dev").unwrap_err();
    assert!(err.contains("budget") || err.contains("budget"));
}

#[test]
fn test_parse_ahoy_non_numeric_budget() {
    let err = K0mmand3rCmd::parse("/ahoy rust-dev lots rust").unwrap_err();
    assert!(err.contains("number") || err.contains("Budget"));
}

#[test]
fn test_parse_ahoy_no_skills() {
    // Skills are optional — default empty
    let cmd = K0mmand3rCmd::parse("/ahoy rust-dev 500").unwrap();
    match cmd {
        K0mmand3rCmd::Ahoy {
            role,
            budget,
            skills,
            ..
        } => {
            assert_eq!(role, "rust-dev");
            assert_eq!(budget, 500);
            assert!(skills.is_empty());
        }
        _ => panic!("Expected Ahoy variant"),
    }
}

#[test]
fn test_parse_apply_positional() {
    let cmd = K0mmand3rCmd::parse("/apply ahoy-42 I am the best for this").unwrap();
    match cmd {
        K0mmand3rCmd::Apply { ahoy_id, pitch } => {
            assert_eq!(ahoy_id, "ahoy-42");
            assert_eq!(pitch, "I am the best for this");
        }
        _ => panic!("Expected Apply variant"),
    }
}

#[test]
fn test_parse_apply_modifier() {
    let cmd = K0mmand3rCmd::parse("/apply ahoy_id=ahoy-99 pitch=hire-me").unwrap();
    match cmd {
        K0mmand3rCmd::Apply { ahoy_id, pitch } => {
            assert_eq!(ahoy_id, "ahoy-99");
            assert_eq!(pitch, "hire-me");
        }
        _ => panic!("Expected Apply variant"),
    }
}

#[test]
fn test_parse_apply_minimal() {
    let cmd = K0mmand3rCmd::parse("/apply ahoy-42").unwrap();
    match cmd {
        K0mmand3rCmd::Apply { ahoy_id, pitch } => {
            assert_eq!(ahoy_id, "ahoy-42");
            assert!(pitch.is_empty(), "pitch may be empty");
        }
        _ => panic!("Expected Apply variant"),
    }
}

#[test]
fn test_parse_apply_missing_ahoy_id() {
    let err = K0mmand3rCmd::parse("/apply").unwrap_err();
    assert!(err.contains("Usage") || err.contains("ahoy_id"));
}

#[test]
fn test_parse_award_positional() {
    let cmd = K0mmand3rCmd::parse("/award ahoy-42 alice").unwrap();
    match cmd {
        K0mmand3rCmd::Award { ahoy_id, winner } => {
            assert_eq!(ahoy_id, "ahoy-42");
            assert_eq!(winner, "alice");
        }
        _ => panic!("Expected Award variant"),
    }
}

#[test]
fn test_parse_award_modifier() {
    let cmd = K0mmand3rCmd::parse("/award ahoy_id=ahoy-7 winner=bob").unwrap();
    match cmd {
        K0mmand3rCmd::Award { ahoy_id, winner } => {
            assert_eq!(ahoy_id, "ahoy-7");
            assert_eq!(winner, "bob");
        }
        _ => panic!("Expected Award variant"),
    }
}

#[test]
fn test_parse_award_missing_ahoy_id() {
    let err = K0mmand3rCmd::parse("/award").unwrap_err();
    assert!(err.contains("Usage") || err.contains("ahoy_id"));
}

#[test]
fn test_parse_award_missing_winner() {
    let err = K0mmand3rCmd::parse("/award ahoy-42").unwrap_err();
    assert!(err.contains("winner") || err.contains("Missing"));
}

// ============================================================================
// Section 2: K0mmand::parse (legacy slash-command format) + validate
// ============================================================================

#[test]
fn test_k0mmand_parse_negotiate() {
    let cmd = K0mmand::parse("/negotiate blessing:observe-infrastructure").unwrap();
    assert_eq!(cmd.verb, "negotiate");
    assert_eq!(cmd.object, "blessing:observe-infrastructure");
    // K0mmand::parse also pushes key:value tokens into modifiers map
    assert!(cmd.modifiers.contains_key("blessing"));
}

#[test]
fn test_k0mmand_parse_vote() {
    let cmd = K0mmand::parse("/vote on blessing:execute-transition-safely").unwrap();
    assert_eq!(cmd.verb, "vote");
    assert_eq!(cmd.object, "blessing:execute-transition-safely");
}

#[test]
fn test_k0mmand_parse_delegate() {
    let cmd = K0mmand::parse("/delegate step:apply-transitions to agent:b00t-sandbox").unwrap();
    assert_eq!(cmd.verb, "delegate");
    assert_eq!(cmd.object, "step:apply-transitions");
    assert!(cmd.modifiers.get("to").is_some());
}

#[test]
fn test_k0mmand_parse_status() {
    let cmd = K0mmand::parse("/status from agent:executive").unwrap();
    assert_eq!(cmd.verb, "status");
    assert_eq!(cmd.object, "agent:executive");
}

#[test]
fn test_k0mmand_parse_handshake() {
    let cmd = K0mmand::parse("/handshake challenge:abc123").unwrap();
    assert_eq!(cmd.verb, "handshake");
    assert_eq!(cmd.object, "challenge:abc123");
}

#[test]
fn test_k0mmand_parse_crew() {
    let cmd = K0mmand::parse("/crew from agent:captain").unwrap();
    assert_eq!(cmd.verb, "crew");
    assert_eq!(cmd.object, "agent:captain");
}

#[test]
fn test_k0mmand_parse_no_slash() {
    let err = K0mmand::parse("negotiate blessing:test").unwrap_err();
    assert!(
        err.contains("/"),
        "error should mention slash prefix: {err}"
    );
}

#[test]
fn test_k0mmand_parse_empty() {
    let err = K0mmand::parse("/").unwrap_err();
    assert!(err.contains("Empty"), "error: {err}");
}

#[test]
fn test_k0mmand_parse_empty_string() {
    let err = K0mmand::parse("").unwrap_err();
    assert!(
        err.contains("/"),
        "should reject empty string without slash"
    );
}

#[test]
fn test_k0mmand_parse_modifiers_key_value() {
    let cmd = K0mmand::parse("/negotiate blessing:foo budget=3000 ttl=600").unwrap();
    assert_eq!(cmd.verb, "negotiate");
    assert_eq!(
        cmd.modifiers.get("budget").map(String::as_str),
        Some("3000")
    );
    assert_eq!(cmd.modifiers.get("ttl").map(String::as_str), Some("600"));
}

#[test]
fn test_k0mmand_validate_known_verbs() {
    let verbs = [
        "negotiate",
        "vote",
        "delegate",
        "status",
        "handshake",
        "crew",
    ];
    for verb in &verbs {
        let cmd = K0mmand {
            verb: verb.to_string(),
            object: "test:value".to_string(),
            modifiers: BTreeMap::new(),
        };
        assert!(cmd.validate().is_ok(), "verb '{verb}' should be valid");
    }
}

#[test]
fn test_k0mmand_validate_unknown_verb() {
    let cmd = K0mmand {
        verb: "foobar".to_string(),
        object: "test:value".to_string(),
        modifiers: BTreeMap::new(),
    };
    let err = cmd.validate().unwrap_err();
    assert!(err.contains("Unknown") || err.contains("foobar"));
}

// ============================================================================
// Section 3: LoopSpec::from_tokens with goal/metric/verify syntax
// ============================================================================

#[test]
fn test_loopspec_from_tokens_positional_pipe() {
    let positional = vec!["goal:build|metric:compile-time|verify:tests-pass|max:10"];
    let modifiers = BTreeMap::new();
    let spec = LoopSpec::from_tokens(&positional, &modifiers).unwrap();
    assert_eq!(spec.goal, "build");
    assert_eq!(spec.metric, "compile-time");
    assert_eq!(spec.verify, "tests-pass");
    assert_eq!(spec.max, Some(10));
    assert_eq!(spec.guard, None);
    assert_eq!(spec.scope, None);
    assert_eq!(spec.direction, None);
}

#[test]
fn test_loopspec_from_tokens_all_fields() {
    let positional = vec![
        "goal:deploy|metric:latency|verify:health|guard:has-blessing|max:5|scope:us-east|direction:forward",
    ];
    let modifiers = BTreeMap::new();
    let spec = LoopSpec::from_tokens(&positional, &modifiers).unwrap();
    assert_eq!(spec.goal, "deploy");
    assert_eq!(spec.metric, "latency");
    assert_eq!(spec.verify, "health");
    assert_eq!(spec.guard, Some("has-blessing".to_string()));
    assert_eq!(spec.max, Some(5));
    assert_eq!(spec.scope, Some("us-east".to_string()));
    assert_eq!(spec.direction, Some("forward".to_string()));
}

#[test]
fn test_loopspec_from_tokens_minimal() {
    let positional = vec!["goal:deploy"];
    let modifiers = BTreeMap::new();
    let spec = LoopSpec::from_tokens(&positional, &modifiers).unwrap();
    assert_eq!(spec.goal, "deploy");
    assert!(spec.metric.is_empty());
    assert!(spec.verify.is_empty());
}

#[test]
fn test_loopspec_from_tokens_unknown_key_ignored() {
    let positional = vec!["goal:build|unknown:value|metric:time"];
    let modifiers = BTreeMap::new();
    let spec = LoopSpec::from_tokens(&positional, &modifiers).unwrap();
    assert_eq!(spec.goal, "build");
    assert_eq!(spec.metric, "time");
    // unknown:value should be silently ignored
}

#[test]
fn test_loopspec_from_tokens_modifiers_fallback() {
    let positional: Vec<&str> = vec![];
    let mut modifiers = BTreeMap::new();
    modifiers.insert("goal".to_string(), "deploy".to_string());
    modifiers.insert("metric".to_string(), "latency".to_string());
    modifiers.insert("verify".to_string(), "healthcheck".to_string());
    modifiers.insert("guard".to_string(), "budget<100".to_string());
    modifiers.insert("max".to_string(), "3".to_string());
    modifiers.insert("scope".to_string(), "global".to_string());
    modifiers.insert("direction".to_string(), "reverse".to_string());
    let spec = LoopSpec::from_tokens(&positional, &modifiers).unwrap();
    assert_eq!(spec.goal, "deploy");
    assert_eq!(spec.metric, "latency");
    assert_eq!(spec.verify, "healthcheck");
    assert_eq!(spec.guard, Some("budget<100".to_string()));
    assert_eq!(spec.max, Some(3));
    assert_eq!(spec.scope, Some("global".to_string()));
    assert_eq!(spec.direction, Some("reverse".to_string()));
}

#[test]
fn test_loopspec_from_tokens_modifiers_partial() {
    let positional: Vec<&str> = vec![];
    let mut modifiers = BTreeMap::new();
    modifiers.insert("goal".to_string(), "deploy".to_string());
    // Only goal is required; metric/verify can be empty
    let spec = LoopSpec::from_tokens(&positional, &modifiers).unwrap();
    assert_eq!(spec.goal, "deploy");
    assert!(spec.metric.is_empty());
    assert!(spec.verify.is_empty());
}

#[test]
fn test_loopspec_from_tokens_modifiers_invalid_max() {
    let positional: Vec<&str> = vec![];
    let mut modifiers = BTreeMap::new();
    modifiers.insert("goal".to_string(), "deploy".to_string());
    modifiers.insert("max".to_string(), "not-a-number".to_string());
    let spec = LoopSpec::from_tokens(&positional, &modifiers).unwrap();
    assert_eq!(spec.goal, "deploy");
    // Invalid max silently yields None (v.parse().ok())
    assert_eq!(spec.max, None);
}

#[test]
fn test_loopspec_from_tokens_missing_goal_positional() {
    let positional = vec!["metric:uptime|verify:health"];
    let modifiers = BTreeMap::new();
    let err = LoopSpec::from_tokens(&positional, &modifiers).unwrap_err();
    assert!(err.contains("goal"), "error: {err}");
}

#[test]
fn test_loopspec_from_tokens_missing_goal_modifiers() {
    let positional: Vec<&str> = vec![];
    let modifiers = BTreeMap::new();
    let err = LoopSpec::from_tokens(&positional, &modifiers).unwrap_err();
    assert!(err.contains("goal"), "error: {err}");
}

#[test]
fn test_loopspec_from_tokens_mixed_key_format() {
    // Positional takes priority; modifiers ignored if positional has a segment
    let positional = vec!["goal:positional-goal"];
    let mut modifiers = BTreeMap::new();
    modifiers.insert("goal".to_string(), "modifier-goal".to_string());
    let spec = LoopSpec::from_tokens(&positional, &modifiers).unwrap();
    assert_eq!(spec.goal, "positional-goal");
}

// ============================================================================
// Section 4: Handshake parsing — challenge/response semantics
// ============================================================================

#[test]
fn test_handshake_basic() {
    let cmd = K0mmand3rCmd::parse("/handshake alice").unwrap();
    match cmd {
        K0mmand3rCmd::Handshake { agent, proposal } => {
            assert_eq!(agent, "alice");
            assert_eq!(proposal, None);
        }
        _ => panic!("Expected Handshake"),
    }
}

#[test]
fn test_handshake_challenge_response() {
    // Avoid "to" keyword — use "respond-challenge" as one token
    let cmd = K0mmand3rCmd::parse("/handshake bob respond-challenge xyz789").unwrap();
    match cmd {
        K0mmand3rCmd::Handshake { agent, proposal } => {
            assert_eq!(agent, "bob");
            assert_eq!(proposal, Some("respond-challenge xyz789".to_string()));
        }
        _ => panic!("Expected Handshake"),
    }
}

#[test]
fn test_handshake_verb_object() {
    let cmd = K0mmand3rCmd::parse("/handshake agent:observer").unwrap();
    assert_eq!(cmd.verb(), "handshake");
    assert_eq!(cmd.object(), "agent:observer");
}

#[test]
fn test_handshake_verb_on_unknown() {
    let cmd = K0mmand3rCmd::parse("/handshake charlie").unwrap();
    assert_eq!(cmd.verb(), "handshake");
    assert!(
        cmd.object().contains("charlie") || cmd.object().contains("agent"),
        "object: {}",
        cmd.object()
    );
}

// ============================================================================
// Section 5: Edge cases (empty input, unknown verbs, malformed tokens)
// ============================================================================

#[test]
fn test_parse_empty_string() {
    let err = K0mmand3rCmd::parse("").unwrap_err();
    assert!(err.contains("must start with /"), "error: {err}");
}

#[test]
fn test_parse_whitespace_only() {
    let err = K0mmand3rCmd::parse("   ").unwrap_err();
    assert!(err.contains("must start with /"), "error: {err}");
}

#[test]
fn test_parse_only_slash() {
    let err = K0mmand3rCmd::parse("/").unwrap_err();
    assert!(err.contains("Empty"), "error: {err}");
}

#[test]
fn test_parse_slash_with_spaces() {
    let err = K0mmand3rCmd::parse("/  ").unwrap_err();
    assert!(err.contains("Empty"), "error: {err}");
}

#[test]
fn test_parse_unknown_verb() {
    let cmd = K0mmand3rCmd::parse("/foobar something extra").unwrap();
    match cmd {
        K0mmand3rCmd::Unknown { raw } => {
            assert_eq!(raw, "/foobar something extra");
        }
        _ => panic!("Expected Unknown variant"),
    }
}

#[test]
fn test_parse_unknown_verb_only() {
    let cmd = K0mmand3rCmd::parse("/garbage").unwrap();
    match cmd {
        K0mmand3rCmd::Unknown { raw } => {
            assert_eq!(raw, "/garbage");
        }
        _ => panic!("Expected Unknown variant"),
    }
}

#[test]
fn test_parse_unknown_verb_verb_method() {
    let cmd = K0mmand3rCmd::parse("/weird-command arg1 arg2").unwrap();
    assert_eq!(cmd.verb(), "weird-command");
}

#[test]
fn test_parse_unknown_verb_object_method() {
    let cmd = K0mmand3rCmd::parse("/weird-command arg1 arg2").unwrap();
    // Unknown stores the raw string in object()
    assert!(
        cmd.object().contains("/weird-command arg1 arg2") || cmd.object().contains("weird-command")
    );
}

#[test]
fn test_parse_all_modifiers_no_positional() {
    // All key=value or key:value tokens, no bare words for negotiate
    let err =
        K0mmand3rCmd::parse("/negotiate key=value-with-dashes another-key=val_with_underscores")
            .unwrap_err();
    // Since there are no positional/bare tokens, parser can't extract resource/id
    assert!(err.contains("Usage"), "expected error: {err}");
}

#[test]
fn test_parse_empty_modifier_value() {
    // Token with colon but empty value
    let cmd = K0mmand3rCmd::parse("/handshake alice proposal:").unwrap();
    match cmd {
        K0mmand3rCmd::Handshake { agent, proposal } => {
            assert_eq!(agent, "alice");
            // proposal: becomes a modifier with empty string value
            // Since positional.len() == 1, modifiers.remove("proposal") = Some("")
            assert_eq!(proposal, Some("".to_string()));
        }
        _ => panic!("Expected Handshake"),
    }
}

#[test]
fn test_parse_very_long_input() {
    let long_str = "a".repeat(1000);
    let cmd_str = format!("/propose {long_str}");
    let cmd = K0mmand3rCmd::parse(&cmd_str).unwrap();
    match cmd {
        K0mmand3rCmd::Propose { description } => {
            assert_eq!(description.len(), 1000);
            assert_eq!(description, long_str);
        }
        _ => panic!("Expected Propose"),
    }
}

#[test]
fn test_parse_repeated_modifier_keys() {
    // When same modifier key appears multiple times, last one wins (BTreeMap insert)
    // Avoid "to" keyword — use agent: and budget: directly
    let cmd = K0mmand3rCmd::parse("/delegate agent:bob budget:100 budget:200").unwrap();
    match cmd {
        K0mmand3rCmd::Delegate { agent, budget } => {
            assert_eq!(agent, "bob");
            // budget:200 overwrites budget:100
            assert_eq!(budget, 200);
        }
        _ => panic!("Expected Delegate"),
    }
}

#[test]
fn test_parse_only_modifiers_no_positional() {
    // All key=value or key:value tokens, no bare words
    let cmd = K0mmand3rCmd::parse("/vote proposal=p-1 choice=yes reason=ok").unwrap();
    match cmd {
        K0mmand3rCmd::Vote {
            proposal,
            choice,
            reason,
        } => {
            assert_eq!(proposal, "p-1");
            assert_eq!(choice, b00t_ipc::VoteChoice::Yes);
            assert_eq!(reason, Some("ok".to_string()));
        }
        _ => panic!("Expected Vote"),
    }
}

// ============================================================================
// Section 6: K0mmand3rCmd::verb() and object() consistency
// ============================================================================

#[test]
fn test_verb_returns_correct_string_for_all_variants() {
    let tests: Vec<(K0mmand3rCmd, &str)> = vec![
        (
            K0mmand3rCmd::Negotiate {
                resource: "blessing".into(),
                id: "x".into(),
                modifiers: BTreeMap::new(),
            },
            "negotiate",
        ),
        (
            K0mmand3rCmd::Vote {
                proposal: "p".into(),
                choice: b00t_ipc::VoteChoice::Yes,
                reason: None,
            },
            "vote",
        ),
        (
            K0mmand3rCmd::Delegate {
                agent: "a".into(),
                budget: 100,
            },
            "delegate",
        ),
        (
            K0mmand3rCmd::Loop {
                spec: LoopSpec {
                    goal: "g".into(),
                    metric: String::new(),
                    verify: String::new(),
                    guard: None,
                    max: None,
                    scope: None,
                    direction: None,
                },
            },
            "loop",
        ),
        (
            K0mmand3rCmd::Handshake {
                agent: "a".into(),
                proposal: None,
            },
            "handshake",
        ),
        (
            K0mmand3rCmd::Crew {
                action: CrewAction::Form,
                members: vec![],
            },
            "crew",
        ),
        (K0mmand3rCmd::Status, "status"),
        (
            K0mmand3rCmd::Propose {
                description: "d".into(),
            },
            "propose",
        ),
        (
            K0mmand3rCmd::Ahoy {
                role: "r".into(),
                budget: 0,
                skills: vec![],
                description: String::new(),
            },
            "ahoy",
        ),
        (
            K0mmand3rCmd::Apply {
                ahoy_id: "a".into(),
                pitch: String::new(),
            },
            "apply",
        ),
        (
            K0mmand3rCmd::Award {
                ahoy_id: "a".into(),
                winner: "w".into(),
            },
            "award",
        ),
        (
            K0mmand3rCmd::Unknown {
                raw: "/bogus".into(),
            },
            "bogus",
        ),
    ];

    for (cmd, expected_verb) in tests {
        assert_eq!(cmd.verb(), expected_verb, "verb mismatch for variant");
    }
}

#[test]
fn test_object_returns_non_empty_for_all_variants() {
    let cases: Vec<K0mmand3rCmd> = vec![
        K0mmand3rCmd::Negotiate {
            resource: "blessing".into(),
            id: "deploy".into(),
            modifiers: BTreeMap::new(),
        },
        K0mmand3rCmd::Vote {
            proposal: "prop-1".into(),
            choice: b00t_ipc::VoteChoice::Yes,
            reason: None,
        },
        K0mmand3rCmd::Delegate {
            agent: "executive".into(),
            budget: 5000,
        },
        K0mmand3rCmd::Loop {
            spec: LoopSpec {
                goal: "deploy".into(),
                metric: "uptime".into(),
                verify: "health".into(),
                guard: None,
                max: None,
                scope: None,
                direction: None,
            },
        },
        K0mmand3rCmd::Handshake {
            agent: "observer".into(),
            proposal: None,
        },
        K0mmand3rCmd::Crew {
            action: CrewAction::Form,
            members: vec!["alice".into(), "bob".into()],
        },
        K0mmand3rCmd::Status,
        K0mmand3rCmd::Propose {
            description: "build stuff".into(),
        },
        K0mmand3rCmd::Ahoy {
            role: "rust-dev".into(),
            budget: 500,
            skills: vec!["rust".into()],
            description: "build".into(),
        },
        K0mmand3rCmd::Apply {
            ahoy_id: "ahoy-1".into(),
            pitch: "me".into(),
        },
        K0mmand3rCmd::Award {
            ahoy_id: "ahoy-1".into(),
            winner: "alice".into(),
        },
    ];

    for cmd in cases {
        let obj = cmd.object();
        assert!(!obj.is_empty(), "object() should not be empty for {cmd:?}");
    }
}

#[test]
fn test_roundtrip_status() {
    // Status roundtrips: parse /status -> verb() returns "status"
    let cmd = K0mmand3rCmd::parse("/status").unwrap();
    assert_eq!(cmd.verb(), "status");
    assert_eq!(cmd.object(), "status");
    assert_eq!(cmd, K0mmand3rCmd::Status);
}
