// k0mmand3r/src/parser_stages.rs
//
// Named parser stages for K0mmand3rStage guard hooks.
//
// The k0mmand3r winnow parser emits stage events as it progresses through
// the parsing phases. External guard systems (HiveGuard, b00t hive) register
// callbacks at specific stages to intercept, warn, block, or redirect commands
// at parse time rather than post-hoc.
//
// Stages are called in order during KmdLine::parse():
//   PreParse → PreVerb → PostVerb → PreParams → PostParams → PreContent → PostContent → PostParse
//
// Each stage receives the current parse state (verb, params, content so far).
// A guard returning StageAction::Allow continues parsing.
// StageAction::Block stops parsing and returns the block result immediately.
//
// 🔗参 b00t-cli/src/hive.rs: K0mmand3rStageGuard

use std::cell::RefCell;
use std::collections::HashMap;

/// Named parsing stages in order of execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ParseStage {
    /// Before any parsing begins
    PreParse,
    /// After identifying the leading /
    PreVerb,
    /// After extracting the verb name
    PostVerb,
    /// Before parameter parsing
    PreParams,
    /// After all parameters collected
    PostParams,
    /// Before content parsing
    PreContent,
    /// After content parsed
    PostContent,
    /// After full KmdLine constructed
    PostParse,
}

impl ParseStage {
    /// All stages in execution order
    pub const ALL: &'static [ParseStage] = &[
        ParseStage::PreParse,
        ParseStage::PreVerb,
        ParseStage::PostVerb,
        ParseStage::PreParams,
        ParseStage::PostParams,
        ParseStage::PreContent,
        ParseStage::PostContent,
        ParseStage::PostParse,
    ];

    /// Parse a stage name from string (case-insensitive, ignores hyphens/underscores)
    pub fn from_name(name: &str) -> Option<ParseStage> {
        let normalized = name.to_lowercase().replace('-', "").replace('_', "");
        match normalized.as_str() {
            "preparse" | "pre" => Some(ParseStage::PreParse),
            "preverb" => Some(ParseStage::PreVerb),
            "postverb" => Some(ParseStage::PostVerb),
            "preparams" | "preparam" => Some(ParseStage::PreParams),
            "postparams" | "postparam" => Some(ParseStage::PostParams),
            "precontent" => Some(ParseStage::PreContent),
            "postcontent" => Some(ParseStage::PostContent),
            "postparse" | "post" => Some(ParseStage::PostParse),
            _ => None,
        }
    }
}

impl std::fmt::Display for ParseStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseStage::PreParse => write!(f, "pre_parse"),
            ParseStage::PreVerb => write!(f, "pre_verb"),
            ParseStage::PostVerb => write!(f, "post_verb"),
            ParseStage::PreParams => write!(f, "pre_params"),
            ParseStage::PostParams => write!(f, "post_params"),
            ParseStage::PreContent => write!(f, "pre_content"),
            ParseStage::PostContent => write!(f, "post_content"),
            ParseStage::PostParse => write!(f, "post_parse"),
        }
    }
}

/// State snapshot passed to stage guards.
#[derive(Debug, Clone, Default)]
pub struct ParseState {
    pub verb: Option<String>,
    pub params: Vec<(String, String)>,
    pub content: Option<String>,
    pub raw_input: String,
}

/// Action a stage guard can return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageAction {
    /// Continue parsing normally
    Allow,
    /// Skip remaining stages, return this result
    Block { message: String },
}

/// Type-erased guard callback registered at a specific parser stage.
/// Takes the current parse state and returns an action decision.
type StageGuardFn = Box<dyn Fn(&ParseState) -> StageAction + Send>;

/// Global parser stage guard registry.
///
/// Guards register for one or more stages. During parsing, the k0mmand3r
/// parser emits each stage by calling run_stage(), which iterates all
/// registered guards at that stage and checks for blocks.
///
/// Thread-local to avoid synchronization overhead — parsers are typically
/// used from a single thread.
thread_local! {
    static STAGE_GUARDS: RefCell<HashMap<ParseStage, Vec<StageGuardFn>>> =
        RefCell::new(HashMap::new());
}

/// Register a guard callback at a specific parser stage.
/// Guards at the same stage are invoked in registration order; the first
/// `StageAction::Block` short-circuits remaining guards at that stage.
/// To remove guards (e.g., in tests), use `clear_guards()`.
pub fn register_stage_guard(stage: ParseStage, guard: StageGuardFn) {
    STAGE_GUARDS.with(|guards| {
        let mut map = guards.borrow_mut();
        map.entry(stage).or_default().push(guard);
    });
}

/// Run all guards registered at a given stage.
/// Returns StageAction::Block if any guard blocks, otherwise Allow.
pub fn run_stage(stage: ParseStage, state: &ParseState) -> StageAction {
    STAGE_GUARDS.with(|guards| {
        let map = guards.borrow();
        if let Some(callbacks) = map.get(&stage) {
            for cb in callbacks {
                let result = cb(state);
                if let StageAction::Block { .. } = &result {
                    return result;
                }
            }
        }
        StageAction::Allow
    })
}

/// Clear all registered guards (for testing).
pub fn clear_guards() {
    STAGE_GUARDS.with(|guards| {
        guards.borrow_mut().clear();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_from_name() {
        assert_eq!(
            ParseStage::from_name("pre_parse"),
            Some(ParseStage::PreParse)
        );
        assert_eq!(
            ParseStage::from_name("pre-parse"),
            Some(ParseStage::PreParse)
        );
        assert_eq!(
            ParseStage::from_name("preparse"),
            Some(ParseStage::PreParse)
        );
        assert_eq!(
            ParseStage::from_name("post_params"),
            Some(ParseStage::PostParams)
        );
        assert_eq!(ParseStage::from_name("unknown_stage"), None);
    }

    #[test]
    fn test_display_stage() {
        assert_eq!(ParseStage::PreParse.to_string(), "pre_parse");
        assert_eq!(ParseStage::PostContent.to_string(), "post_content");
    }

    #[test]
    fn test_register_and_run_allow() {
        clear_guards();
        register_stage_guard(ParseStage::PreParse, Box::new(|_| StageAction::Allow));
        let state = ParseState::default();
        assert_eq!(run_stage(ParseStage::PreParse, &state), StageAction::Allow);
    }

    #[test]
    fn test_register_and_run_block() {
        clear_guards();
        register_stage_guard(
            ParseStage::PreVerb,
            Box::new(|_| StageAction::Block {
                message: "blocked by test".to_string(),
            }),
        );
        let state = ParseState::default();
        let result = run_stage(ParseStage::PreVerb, &state);
        assert_eq!(
            result,
            StageAction::Block {
                message: "blocked by test".to_string()
            }
        );
    }

    #[test]
    fn test_block_stops_early() {
        clear_guards();
        // Second guard should not be reached if first blocks
        register_stage_guard(
            ParseStage::PreParse,
            Box::new(|_| StageAction::Block {
                message: "first blocks".to_string(),
            }),
        );
        register_stage_guard(
            ParseStage::PreParse,
            Box::new(|_| StageAction::Block {
                message: "should not fire".to_string(),
            }),
        );
        let state = ParseState::default();
        let result = run_stage(ParseStage::PreParse, &state);
        assert_eq!(
            result,
            StageAction::Block {
                message: "first blocks".to_string()
            }
        );
    }

    #[test]
    fn test_no_guards_returns_allow() {
        clear_guards();
        let state = ParseState::default();
        assert_eq!(run_stage(ParseStage::PreParse, &state), StageAction::Allow);
    }

    #[test]
    fn test_all_stages_have_names() {
        for stage in ParseStage::ALL {
            let name = stage.to_string();
            assert!(
                ParseStage::from_name(&name).is_some(),
                "stage {} should roundtrip",
                name
            );
        }
    }
}
