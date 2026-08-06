//! Bridge between b00t-cli's HiveGuard system and b00t-c0re-gov's governance gate system.
//!
//! This module provides a conversion layer from the existing HiveGuard types
//! to b00t-c0re-gov's `GateResult`, enabling agents to call
//! `GovernanceGate::check()` and receive a `GateResult` that maps HiveGuard's
//! Allow/Warn/Block semantics into the governance crate's Allow/Deny/Hook model.
//!
//! ## Contract
//!
//! | HiveGuard Result       | GateResult Mapping        | Notes                                |
//! |------------------------|---------------------------|--------------------------------------|
//! | `GuardResult::Allow`   | `GateResult::Allow`       | Pass-through                         |
//! | `GuardResult::Warn`    | `GateResult::Allow`       | Warning metadata logged via tracing  |
//! | `GuardResult::Block`   | `GateResult::Deny`        | Reason = guard message               |
//! | K0mmand3rStage pattern | `GateResult::Hook`        | Stage guard → Event hook             |
//! | Rhai timer/schedule    | `GateResult::Hook`        | Timer keyword detected in Rhai expr  |
//!
//! ### Agent Runtime Contract
//!
//! `GateResult::Hook` means the caller (agent runtime) should:
//! 1. Snapshot the current agent context
//! 2. Register the hook with the governance scheduler
//! 3. Go productive (yield to other tasks)
//! 4. When the hook fires, resume with the continuation context
//!
//! This enables timer-based governance checks (e.g., "re-check this guard in 5s")
//! and event-driven patterns (e.g., "re-check when k0mmand3r reaches pre_tokenize").

use crate::hive::{
    GuardContext, GuardPattern, GuardResult, HiveGuard, check_guards as hive_check_guards,
};
use b00t_c0re_gov::types::{GateResult, HookToken, HookType};
use chrono::Utc;
use uuid::Uuid;

/// Convert a single `GuardResult` (from HiveGuard evaluation) into the
/// governance crate's `GateResult`.
///
/// This is the primary bridge function. Call it after `hive_check_guards()`
/// to get a `GateResult` suitable for agent runtime dispatch.
pub fn guard_result_to_gate_result(result: GuardResult) -> GateResult {
    match result {
        GuardResult::Allow => GateResult::Allow,
        GuardResult::Warn {
            message,
            redirect: _,
        } => {
            // Warn becomes Allow with warning metadata surfaced through tracing
            tracing::warn!(
                guard_message = %message,
                "guard warning issued; proceeding (warn → Allow)",
            );
            GateResult::Allow
        }
        GuardResult::Block { message } => GateResult::Deny {
            reason: message,
            escalation_path: None,
        },
    }
}

/// Run the HiveGuard chain and return a `GateResult`.
///
/// Wraps `check_guards()` from `hive.rs`, mapping the result into
/// the governance crate's `GateResult` type.
///
/// This is the simplest integration point: replaces direct calls to
/// `check_guards()` with `check_guards_gate_result()` when the caller
/// wants a `GateResult` instead of a `GuardResult`.
pub fn check_guards_gate_result(
    command: &str,
    guards: &[HiveGuard],
    context: &GuardContext,
) -> GateResult {
    let result = hive_check_guards(command, guards, context);
    guard_result_to_gate_result(result)
}

/// Detect if a guard pattern represents a timer, event, or lifecycle hook
/// that should return `GateResult::Hook` rather than a static Allow/Deny.
///
/// Hook-eligible patterns:
/// - `K0mmand3rStage` guards — they intercept at specific parser lifecycle stages
/// - `RhaiExpr` guards whose expression contains timer/schedule/defer keywords
pub fn detect_hook_pattern(guard: &HiveGuard) -> Option<HookToken> {
    match &guard.pattern {
        GuardPattern::K0mmand3rStage(stage) => {
            let stage_name = stage.stage.clone();
            Some(HookToken {
                id: Uuid::new_v4(),
                hook_type: HookType::Event(stage_name),
                created_at: Utc::now(),
                ttl_ms: None,
                description: guard
                    .message
                    .clone()
                    .unwrap_or_else(|| "k0mmand3r stage guard hook".to_string()),
            })
        }
        GuardPattern::RhaiExpr(expr) => {
            let lower = expr.rhai.to_lowercase();
            if lower.contains("timer")
                || lower.contains("schedule")
                || lower.contains("defer")
                || lower.contains("delay")
            {
                Some(HookToken {
                    id: Uuid::new_v4(),
                    hook_type: HookType::TimerMs(5000), // default 5-second deferral
                    created_at: Utc::now(),
                    ttl_ms: Some(30_000), // 30-second TTL
                    description: guard
                        .message
                        .clone()
                        .unwrap_or_else(|| "rhai timer/schedule guard hook".to_string()),
                })
            } else {
                None
            }
        }
        GuardPattern::JsonRegexPattern(_) => None,
    }
}

/// Comprehensive guard check that also detects hook patterns.
///
/// 1. Iterates all guards in order
/// 2. For each matching guard:
///    a. If it's a hook pattern → return `GateResult::Hook`
///    b. Otherwise → run `check_guards()` and convert to `GateResult`
/// 3. If no guard matched, scan for any hook-eligible guard and return `Hook`
/// 4. If nothing found, return `GateResult::Allow`
pub fn check_guards_with_hooks(
    command: &str,
    guards: &[HiveGuard],
    context: &GuardContext,
) -> GateResult {
    // First pass: check guards normally, intercepting hook patterns
    for guard in guards {
        if guard.pattern.matches(command, context) {
            // If this guard is a hook pattern, return Hook
            if let Some(hook) = detect_hook_pattern(guard) {
                return GateResult::Hook(hook);
            }
            // Otherwise, evaluate it normally
            let result = hive_check_guards(command, &[guard.clone()], context);
            return guard_result_to_gate_result(result);
        }
    }

    // Second pass: check if any guard is a hook pattern that should fire
    // independently of command matching (e.g., timer guards, event hooks)
    for guard in guards {
        if let Some(hook) = detect_hook_pattern(guard) {
            return GateResult::Hook(hook);
        }
    }

    GateResult::Allow
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hive::{
        GuardContext, GuardPattern, HiveGuard, HiveGuardAction, K0mmand3rStageGuard, RhaiGuardExpr,
    };

    #[test]
    fn test_allow_maps_to_allow() {
        let result = GuardResult::Allow;
        let result = guard_result_to_gate_result(result);
        assert!(
            matches!(result, GateResult::Allow),
            "expected Allow, got {:?}",
            result
        );
    }

    #[test]
    fn test_warn_maps_to_allow() {
        let result = GuardResult::Warn {
            message: "use uv instead of pip".to_string(),
            redirect: Some("uv pip install".to_string()),
        };
        // Warn → Allow, but tracing warning is emitted
        let result = guard_result_to_gate_result(result);
        assert!(
            matches!(result, GateResult::Allow),
            "expected Allow, got {:?}",
            result
        );
    }

    #[test]
    fn test_block_maps_to_deny() {
        let result = GuardResult::Block {
            message: "rm -rf / is dangerous".to_string(),
        };
        match guard_result_to_gate_result(result) {
            GateResult::Deny {
                reason,
                escalation_path,
            } => {
                assert_eq!(reason, "rm -rf / is dangerous");
                assert!(escalation_path.is_none());
            }
            other => panic!("expected Deny, got {:?}", other),
        }
    }

    #[test]
    fn test_k0mmand3r_stage_detected_as_hook() {
        let guard = HiveGuard {
            pattern: GuardPattern::K0mmand3rStage(K0mmand3rStageGuard {
                stage: "pre_tokenize".to_string(),
            }),
            action: HiveGuardAction::Warn,
            message: Some("stage guard hook".to_string()),
            redirect: None,
            repeat_threshold: None,
        };
        let hook = detect_hook_pattern(&guard);
        assert!(
            hook.is_some(),
            "K0mmand3rStage should be detected as a hook pattern"
        );
        let token = hook.unwrap();
        assert!(
            matches!(token.hook_type, HookType::Event(_)),
            "expected Event hook type, got {:?}",
            token.hook_type
        );
    }

    #[test]
    fn test_regex_guard_not_detected_as_hook() {
        let guard = HiveGuard {
            pattern: GuardPattern::JsonRegexPattern("pip install".to_string()),
            action: HiveGuardAction::Warn,
            message: Some("use uv".to_string()),
            redirect: Some("uv pip install".to_string()),
            repeat_threshold: None,
        };
        assert!(
            detect_hook_pattern(&guard).is_none(),
            "JsonRegexPattern should NOT be detected as a hook"
        );
    }

    #[test]
    fn test_rhai_timer_detected_as_hook() {
        let guard = HiveGuard {
            pattern: GuardPattern::RhaiExpr(RhaiGuardExpr {
                rhai: "cmd.contains('timer')".to_string(),
            }),
            action: HiveGuardAction::Warn,
            message: Some("timer-based guard".to_string()),
            redirect: None,
            repeat_threshold: None,
        };
        let hook = detect_hook_pattern(&guard);
        assert!(
            hook.is_some(),
            "RhaiExpr with 'timer' keyword should be a hook pattern"
        );
        let token = hook.unwrap();
        assert!(
            matches!(token.hook_type, HookType::TimerMs(_)),
            "expected TimerMs hook type, got {:?}",
            token.hook_type
        );
    }

    #[test]
    fn test_rhai_no_timer_not_detected_as_hook() {
        let guard = HiveGuard {
            pattern: GuardPattern::RhaiExpr(RhaiGuardExpr {
                rhai: "cmd.contains('pip')".to_string(),
            }),
            action: HiveGuardAction::Warn,
            message: Some("pip guard".to_string()),
            redirect: None,
            repeat_threshold: None,
        };
        assert!(
            detect_hook_pattern(&guard).is_none(),
            "RhaiExpr without timer keywords should NOT be a hook"
        );
    }

    #[test]
    fn test_check_guards_gate_result_allow() {
        let guards = vec![HiveGuard {
            pattern: GuardPattern::JsonRegexPattern("pip install".to_string()),
            action: HiveGuardAction::Warn,
            message: Some("use uv".to_string()),
            redirect: Some("uv pip install".to_string()),
            repeat_threshold: None,
        }];
        let ctx = GuardContext::default();
        let result = check_guards_gate_result("cargo build", &guards, &ctx);
        assert!(
            matches!(result, GateResult::Allow),
            "expected Allow, got {:?}",
            result
        );
    }

    #[test]
    fn test_check_guards_gate_result_block() {
        let guards = vec![HiveGuard {
            pattern: GuardPattern::JsonRegexPattern("rm -rf /".to_string()),
            action: HiveGuardAction::Block,
            message: Some("🚫 blocked".to_string()),
            redirect: None,
            repeat_threshold: None,
        }];
        let ctx = GuardContext::default();
        let result = check_guards_gate_result("rm -rf /", &guards, &ctx);
        match result {
            GateResult::Deny { reason, .. } => {
                assert_eq!(reason, "🚫 blocked");
            }
            other => panic!("expected Deny, got {:?}", other),
        }
    }

    #[test]
    fn test_check_guards_gate_result_warn_is_allow() {
        let guards = vec![HiveGuard {
            pattern: GuardPattern::JsonRegexPattern("pip install".to_string()),
            action: HiveGuardAction::Warn,
            message: Some("🦨 use uv pip install".to_string()),
            redirect: Some("uv pip install".to_string()),
            repeat_threshold: None,
        }];
        let ctx = GuardContext::default();
        let result = check_guards_gate_result("pip install requests", &guards, &ctx);
        assert!(
            matches!(result, GateResult::Allow),
            "expected Allow, got {:?}",
            result
        );
    }

    #[test]
    fn test_check_guards_with_hooks_k0mmand3r_stage() {
        let guards = vec![HiveGuard {
            pattern: GuardPattern::K0mmand3rStage(K0mmand3rStageGuard {
                stage: "pre_tokenize".to_string(),
            }),
            action: HiveGuardAction::Warn,
            message: Some("stage guard".to_string()),
            redirect: None,
            repeat_threshold: None,
        }];
        let ctx = GuardContext::default();
        let result = check_guards_with_hooks("any command", &guards, &ctx);
        match result {
            GateResult::Hook(token) => {
                assert!(matches!(token.hook_type, HookType::Event(_)));
            }
            other => panic!("expected Hook, got {:?}", other),
        }
    }

    #[test]
    fn test_check_guards_with_hooks_allow() {
        let guards = vec![HiveGuard {
            pattern: GuardPattern::JsonRegexPattern("pip install".to_string()),
            action: HiveGuardAction::Warn,
            message: Some("use uv".to_string()),
            redirect: Some("uv pip install".to_string()),
            repeat_threshold: None,
        }];
        let ctx = GuardContext::default();
        let result = check_guards_with_hooks("cargo build", &guards, &ctx);
        assert!(
            matches!(result, GateResult::Allow),
            "expected Allow, got {:?}",
            result
        );
    }

    #[test]
    fn test_check_guards_with_hooks_block() {
        let guards = vec![HiveGuard {
            pattern: GuardPattern::JsonRegexPattern("rm -rf /".to_string()),
            action: HiveGuardAction::Block,
            message: Some("🚫 blocked".to_string()),
            redirect: None,
            repeat_threshold: None,
        }];
        let ctx = GuardContext::default();
        let result = check_guards_with_hooks("rm -rf /", &guards, &ctx);
        match result {
            GateResult::Deny { reason, .. } => {
                assert_eq!(reason, "🚫 blocked");
            }
            other => panic!("expected Deny, got {:?}", other),
        }
    }

    #[test]
    fn test_with_hooks_prefers_matched_guard_over_second_pass_hook() {
        // A JsonRegexPattern guard that matches should take priority over
        // an unmatched K0mmand3rStage hook
        let guards = vec![
            HiveGuard {
                pattern: GuardPattern::JsonRegexPattern("pip install".to_string()),
                action: HiveGuardAction::Block,
                message: Some("pip blocked".to_string()),
                redirect: None,
                repeat_threshold: None,
            },
            HiveGuard {
                pattern: GuardPattern::K0mmand3rStage(K0mmand3rStageGuard {
                    stage: "pre_tokenize".to_string(),
                }),
                action: HiveGuardAction::Warn,
                message: Some("stage guard".to_string()),
                redirect: None,
                repeat_threshold: None,
            },
        ];
        let ctx = GuardContext::default();
        // pip install matches the first guard (Block), not the stage hook
        let result = check_guards_with_hooks("pip install flask", &guards, &ctx);
        match result {
            GateResult::Deny { reason, .. } => {
                assert_eq!(reason, "pip blocked");
            }
            other => panic!("expected Deny from first guard, got {:?}", other),
        }
    }
}
