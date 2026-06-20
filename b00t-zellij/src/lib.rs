//! # b00t-zellij — WebAssembly bindings for the Zellij interaction system
//!
//! Compiles to a `.wasm` binary for browser-based agents. Exports pure-logic
//! functions that the browser UI layer calls to classify actions, route through
//! the Eisenhower matrix, create menu items, and serialize KV entries — all
//! without any filesystem, subprocess, or environment-variable access.
//!
//! ## Architecture
//!
//! ```text
//! Browser JS  ──wasm_bindgen──►  b00t-zellij (.wasm)
//!                                    │
//!                                    ▼
//!                         local types (serde-compatible with b00t_c0re_lib)
//! ```
//!
//! On native targets the crate re-exports types from `b00t_c0re_lib`.
//! On `wasm32-unknown-unknown` it defines wire-compatible local types
//! because `b00t_c0re_lib` transitively depends on tokio→mio which does
//! not compile to that target. See `Cargo.toml` for details.
//!
//! ## Two-layer design
//!
//! Each export has two functions:
//! 1. An internal `_impl` function that returns `String` (JSON) — callable from
//!    native tests.
//! 2. A `#[wasm_bindgen]` public function that returns `JsValue` via
//!    `serde_wasm_bindgen::to_value` — only called in the browser.

use serde::Serialize;
use wasm_bindgen::prelude::*;

// ── Type imports (cfg-gated: native vs wasm32) ─────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
mod types {
    // On native, re-export from b00t-c0re-lib.
    pub use b00t_c0re_lib::{
        AgentAction, EisenhowerQuadrant, GateResult, InteractionMode, MenuItem, ZellijKvEntry,
    };
}

#[cfg(target_arch = "wasm32")]
mod types {
    // On WASM, define wire-compatible local types with identical serde repr.
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use std::str::FromStr;

    // ── InteractionMode ─────────────────────────────────────────────────

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "kebab-case")]
    pub enum InteractionMode {
        Menu,
        Confirm,
        Input,
        Subagent,
        Wizard,
    }

    impl InteractionMode {
        pub fn as_label(&self) -> &'static str {
            match self {
                InteractionMode::Menu => "Menu",
                InteractionMode::Confirm => "Confirm",
                InteractionMode::Input => "Input",
                InteractionMode::Subagent => "Subagent",
                InteractionMode::Wizard => "Wizard",
            }
        }
    }

    impl std::fmt::Display for InteractionMode {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.as_label())
        }
    }

    impl FromStr for InteractionMode {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.to_lowercase().as_str() {
                "menu" | "fzf-menu" | "fzf" => Ok(InteractionMode::Menu),
                "confirm" | "yesno" | "yes-no" | "y/n" => Ok(InteractionMode::Confirm),
                "input" | "text-input" | "text" | "prompt" => Ok(InteractionMode::Input),
                "subagent" | "sub-agent" | "report" | "subagent-report" => {
                    Ok(InteractionMode::Subagent)
                }
                "wizard" | "multi-step" | "multistep" => Ok(InteractionMode::Wizard),
                other => Err(format!(
                    "unknown interaction mode: '{other}'. \
                     Valid modes: menu, confirm, input, subagent, wizard"
                )),
            }
        }
    }

    // ── EisenhowerQuadrant ──────────────────────────────────────────────

    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "kebab-case")]
    pub enum EisenhowerQuadrant {
        UrgentImportant,
        NotUrgentImportant,
        UrgentNotImportant,
        NotUrgentNotImportant,
    }

    impl EisenhowerQuadrant {
        pub fn classify(urgent: bool, important: bool) -> Self {
            match (urgent, important) {
                (true, true) => EisenhowerQuadrant::UrgentImportant,
                (false, true) => EisenhowerQuadrant::NotUrgentImportant,
                (true, false) => EisenhowerQuadrant::UrgentNotImportant,
                (false, false) => EisenhowerQuadrant::NotUrgentNotImportant,
            }
        }

        pub fn as_label(&self) -> &'static str {
            match self {
                EisenhowerQuadrant::UrgentImportant => "Urgent & Important",
                EisenhowerQuadrant::NotUrgentImportant => "Not Urgent & Important",
                EisenhowerQuadrant::UrgentNotImportant => "Urgent & Not Important",
                EisenhowerQuadrant::NotUrgentNotImportant => "Not Urgent & Not Important",
            }
        }
    }

    impl std::fmt::Display for EisenhowerQuadrant {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.as_label())
        }
    }

    // ── GateResult ──────────────────────────────────────────────────────

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "lowercase")]
    pub enum GateResult {
        Allow,
        Deny,
        Hook,
    }

    impl GateResult {
        pub fn exit_code(&self) -> i32 {
            match self {
                GateResult::Allow => 0,
                GateResult::Deny => 1,
                GateResult::Hook => 2,
            }
        }

        pub fn as_label(&self) -> &'static str {
            match self {
                GateResult::Allow => "Allow",
                GateResult::Deny => "Deny",
                GateResult::Hook => "Hook",
            }
        }

        pub fn is_allowed(&self) -> bool {
            matches!(self, GateResult::Allow)
        }
    }

    impl std::fmt::Display for GateResult {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.as_label())
        }
    }

    // ── AgentAction ─────────────────────────────────────────────────────

    #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "kebab-case")]
    pub enum AgentAction {
        Approve,
        Reject,
        Delegate,
        Audit,
    }

    impl AgentAction {
        pub fn as_label(&self) -> &'static str {
            match self {
                AgentAction::Approve => "Approve",
                AgentAction::Reject => "Reject",
                AgentAction::Delegate => "Delegate",
                AgentAction::Audit => "Audit",
            }
        }
    }

    impl std::fmt::Display for AgentAction {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.as_label())
        }
    }

    // ── MenuItem ────────────────────────────────────────────────────────

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MenuItem {
        pub key: String,
        pub label: String,
        pub quadrant: EisenhowerQuadrant,
        pub action: AgentAction,
        #[serde(default)]
        pub description: Option<String>,
    }

    // ── ZellijKvEntry ───────────────────────────────────────────────────

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ZellijKvEntry {
        pub key: String,
        pub value: String,
        pub agent_id: String,
        pub session_id: String,
        pub created_at: DateTime<Utc>,
    }

    impl ZellijKvEntry {
        pub fn new(key: &str, value: &str, agent_id: &str, session_id: &str) -> Self {
            Self {
                key: key.to_string(),
                value: value.to_string(),
                agent_id: agent_id.to_string(),
                session_id: session_id.to_string(),
                created_at: Utc::now(),
            }
        }
    }
}

use types::*;

// ── wee_alloc for smaller WASM binary ────────────────────────────────────────

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// ── WASM start entry point ──────────────────────────────────────────────────

/// Called automatically when the WASM module is instantiated in the browser.
///
/// Sets up the global allocator (wee_alloc). In a full browser deployment,
/// you would also set a panic hook via `console_error_panic_hook::set_once()`
/// to get Rust panic messages in the browser console.
#[wasm_bindgen(start)]
pub fn wasm_start() {
    // The global allocator is already set via #[global_allocator].
    // For panic hooks in the browser, add `console_error_panic_hook`
    // as a dependency and call:
    //   console_error_panic_hook::set_once();
}

// ── Bridge helpers: String → JsValue for WASM exports ───────────────────────

/// Convert a JSON error string to a [`JsValue`].
///
/// Only called from `#[wasm_bindgen]` exports; never called on native targets.
fn err_to_js_value(message: &str) -> JsValue {
    JsValue::from_str(message)
}

// ── Export: interaction_mode_from_str ──────────────────────────────────────

/// **Internal implementation** — returns a JSON string.
///
/// Parse an interaction mode from a string identifier and return its JSON
/// representation.
fn interaction_mode_from_str_impl(s: &str) -> String {
    match s.parse::<InteractionMode>() {
        Ok(mode) => {
            #[derive(Serialize)]
            struct Output {
                mode: String,
            }
            serde_json::to_string(&Output {
                mode: mode.to_string(),
            })
            .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
        }
        Err(e) => format!("{{\"error\": \"{e}\"}}"),
    }
}

/// WASM export: parse an interaction mode from a string.
///
/// @param {string} s — Interaction mode string identifier
/// @returns {JsValue} JSON representation of the parsed mode
#[wasm_bindgen(js_name = interactionModeFromStr)]
pub fn interaction_mode_from_str(s: &str) -> JsValue {
    let json = interaction_mode_from_str_impl(s);
    err_to_js_value(&json)
}

// ── Export: eisenhower_route ────────────────────────────────────────────────

/// **Internal implementation** — returns a JSON string.
///
/// Route an action through the Eisenhower matrix and return the quadrant,
/// gate result, and human-readable reason.
fn eisenhower_route_impl(urgency: bool, importance: bool) -> String {
    let quadrant = EisenhowerQuadrant::classify(urgency, importance);
    let result = gate_result_for_quadrant(quadrant);
    let reason = hook_reason(quadrant);

    #[derive(Serialize)]
    struct Output {
        quadrant: String,
        result: String,
        reason: String,
    }

    serde_json::to_string(&Output {
        quadrant: serde_rename_quadrant(quadrant),
        result: result.to_string().to_lowercase(),
        reason: reason.to_string(),
    })
    .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
}

fn gate_result_for_quadrant(quadrant: EisenhowerQuadrant) -> GateResult {
    match quadrant {
        EisenhowerQuadrant::UrgentImportant => GateResult::Allow,
        EisenhowerQuadrant::NotUrgentImportant => GateResult::Hook,
        EisenhowerQuadrant::UrgentNotImportant => GateResult::Hook,
        EisenhowerQuadrant::NotUrgentNotImportant => GateResult::Deny,
    }
}

fn hook_reason(quadrant: EisenhowerQuadrant) -> &'static str {
    match quadrant {
        EisenhowerQuadrant::UrgentImportant => "confirm-required",
        EisenhowerQuadrant::NotUrgentImportant => "menu-interaction",
        EisenhowerQuadrant::UrgentNotImportant => "subagent-delegate",
        EisenhowerQuadrant::NotUrgentNotImportant => "eliminated",
    }
}

fn serde_rename_quadrant(quadrant: EisenhowerQuadrant) -> String {
    match quadrant {
        EisenhowerQuadrant::UrgentImportant => "urgent-important",
        EisenhowerQuadrant::NotUrgentImportant => "not-urgent-important",
        EisenhowerQuadrant::UrgentNotImportant => "urgent-not-important",
        EisenhowerQuadrant::NotUrgentNotImportant => "not-urgent-not-important",
    }
    .to_string()
}

/// WASM export: route an action through the Eisenhower matrix.
///
/// @param {boolean} urgency — Whether the action is urgent
/// @param {boolean} importance — Whether the action is important
/// @returns {JsValue} JSON: `{"quadrant": "...", "result": "...", "reason": "..."}`
#[wasm_bindgen(js_name = eisenhowerRoute)]
pub fn eisenhower_route(urgency: bool, importance: bool) -> JsValue {
    let json = eisenhower_route_impl(urgency, importance);
    err_to_js_value(&json)
}

// ── Export: create_menu_item ────────────────────────────────────────────────

/// **Internal implementation** — returns a JSON string.
fn create_menu_item_impl(key: &str, label: &str, quadrant: &str, action: &str) -> String {
    let quadrant = match parse_quadrant(quadrant) {
        Ok(q) => q,
        Err(e) => return format!("{{\"error\": \"{e}\"}}"),
    };
    let action = match parse_action(action) {
        Ok(a) => a,
        Err(e) => return format!("{{\"error\": \"{e}\"}}"),
    };

    let item = MenuItem {
        key: key.to_string(),
        label: label.to_string(),
        quadrant,
        action,
        description: None,
    };

    serde_json::to_string(&item).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
}

fn parse_quadrant(s: &str) -> Result<EisenhowerQuadrant, String> {
    let json_str = format!("\"{s}\"");
    serde_json::from_str::<EisenhowerQuadrant>(&json_str).map_err(|e| {
        format!(
            "invalid quadrant '{s}': {e}. Valid: urgent-important, \
             not-urgent-important, urgent-not-important, not-urgent-not-important"
        )
    })
}

fn parse_action(s: &str) -> Result<AgentAction, String> {
    let json_str = format!("\"{s}\"");
    serde_json::from_str::<AgentAction>(&json_str)
        .map_err(|e| format!("invalid action '{s}': {e}. Valid: approve, reject, delegate, audit"))
}

/// WASM export: create a menu item from individual fields.
///
/// @param {string} key — Short key (e.g., "build")
/// @param {string} label — Human-readable label (e.g., "Build Project")
/// @param {string} quadrant — Eisenhower quadrant in kebab-case
/// @param {string} action — Agent action in kebab-case
/// @returns {JsValue} JSON representation of the MenuItem, or error
#[wasm_bindgen(js_name = createMenuItem)]
pub fn create_menu_item(key: &str, label: &str, quadrant: &str, action: &str) -> JsValue {
    let json = create_menu_item_impl(key, label, quadrant, action);
    err_to_js_value(&json)
}

// ── Export: gate_decision_to_js ─────────────────────────────────────────────

/// **Internal implementation** — returns a JSON string.
fn gate_decision_to_js_impl(result: &str) -> String {
    let gate_result = match parse_gate_result(result) {
        Ok(r) => r,
        Err(e) => return format!("{{\"error\": \"{e}\"}}"),
    };

    #[derive(Serialize)]
    struct Output {
        result: String,
        exit_code: i32,
    }

    serde_json::to_string(&Output {
        result: gate_result.to_string().to_lowercase(),
        exit_code: gate_result.exit_code(),
    })
    .unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
}

fn parse_gate_result(s: &str) -> Result<GateResult, String> {
    let json_str = format!("\"{s}\"");
    if let Ok(r) = serde_json::from_str::<GateResult>(&json_str) {
        return Ok(r);
    }
    let lower = s.to_lowercase();
    let lower_json = format!("\"{lower}\"");
    serde_json::from_str::<GateResult>(&lower_json)
        .map_err(|e| format!("invalid gate result '{s}': {e}. Valid: allow, deny, hook"))
}

/// WASM export: parse a gate result string to JSON with exit code.
///
/// @param {string} result — Gate result string ("allow", "deny", "hook")
/// @returns {JsValue} JSON with result and POSIX exit_code
#[wasm_bindgen(js_name = gateDecisionToJs)]
pub fn gate_decision_to_js(result: &str) -> JsValue {
    let json = gate_decision_to_js_impl(result);
    err_to_js_value(&json)
}

// ── Export: kv_entry_new ────────────────────────────────────────────────────

/// **Internal implementation** — returns a JSON string.
fn kv_entry_new_impl(key: &str, value: &str) -> String {
    let entry = ZellijKvEntry::new(key, value, "wasm", "browser");
    serde_json::to_string(&entry).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"))
}

/// WASM export: create a Zellij-scoped KV entry.
///
/// @param {string} key — KV store key
/// @param {string} value — KV store value
/// @returns {JsValue} JSON representation of the ZellijKvEntry
#[wasm_bindgen(js_name = kvEntryNew)]
pub fn kv_entry_new(key: &str, value: &str) -> JsValue {
    let json = kv_entry_new_impl(key, value);
    err_to_js_value(&json)
}

// ── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_modes() {
        let cases = [
            ("menu", "Menu"),
            ("confirm", "Confirm"),
            ("input", "Input"),
            ("subagent", "Subagent"),
            ("wizard", "Wizard"),
        ];
        for (input, expected) in cases {
            let json = interaction_mode_from_str_impl(input);
            assert!(
                json.contains(expected),
                "Expected '{expected}' in output for input '{input}': {json}"
            );
        }
    }

    #[test]
    fn test_parse_invalid_mode() {
        let json = interaction_mode_from_str_impl("bogus");
        assert!(json.contains("error"), "Expected error: {json}");
    }

    #[test]
    fn test_urgent_important_allow() {
        let json = eisenhower_route_impl(true, true);
        assert!(json.contains("allow"), "Expected Allow: {json}");
        assert!(
            json.contains("urgent-important"),
            "Expected quadrant: {json}"
        );
    }

    #[test]
    fn test_not_urgent_important_hook_menu() {
        let json = eisenhower_route_impl(false, true);
        assert!(json.contains("hook"), "Expected Hook: {json}");
        assert!(
            json.contains("menu-interaction"),
            "Expected reason: {json}"
        );
    }

    #[test]
    fn test_urgent_not_important_hook_subagent() {
        let json = eisenhower_route_impl(true, false);
        assert!(json.contains("hook"), "Expected Hook: {json}");
        assert!(
            json.contains("subagent-delegate"),
            "Expected reason: {json}"
        );
    }

    #[test]
    fn test_not_urgent_not_important_deny() {
        let json = eisenhower_route_impl(false, false);
        assert!(json.contains("deny"), "Expected Deny: {json}");
        assert!(json.contains("eliminated"), "Expected reason: {json}");
    }

    #[test]
    fn test_all_four_quadrants() {
        let cases = [
            (true, true, "urgent-important", "allow", "confirm-required"),
            (
                false,
                true,
                "not-urgent-important",
                "hook",
                "menu-interaction",
            ),
            (
                true,
                false,
                "urgent-not-important",
                "hook",
                "subagent-delegate",
            ),
            (
                false,
                false,
                "not-urgent-not-important",
                "deny",
                "eliminated",
            ),
        ];

        for (urgency, importance, expected_quadrant, expected_result, expected_reason) in cases {
            let json = eisenhower_route_impl(urgency, importance);
            assert!(
                json.contains(expected_quadrant),
                "Expected quadrant {expected_quadrant}: {json}"
            );
            assert!(
                json.contains(expected_result),
                "Expected result {expected_result}: {json}"
            );
            assert!(
                json.contains(expected_reason),
                "Expected reason {expected_reason}: {json}"
            );
        }
    }

    #[test]
    fn test_create_valid_menu_item() {
        let json = create_menu_item_impl("build", "Build Project", "urgent-important", "approve");
        assert!(json.contains("build"), "Missing key: {json}");
        assert!(json.contains("Build Project"), "Missing label: {json}");
        assert!(
            json.contains("urgent-important"),
            "Missing quadrant: {json}"
        );
        assert!(json.contains("approve"), "Missing action: {json}");
    }

    #[test]
    fn test_create_menu_item_invalid_quadrant() {
        let json = create_menu_item_impl("bad", "Bad Item", "bogus", "approve");
        assert!(json.contains("error"), "Expected error: {json}");
    }

    #[test]
    fn test_create_menu_item_invalid_action() {
        let json = create_menu_item_impl("bad", "Bad Item", "urgent-important", "bogus");
        assert!(json.contains("error"), "Expected error: {json}");
    }

    #[test]
    fn test_gate_decision_allow() {
        let json = gate_decision_to_js_impl("allow");
        assert!(json.contains("allow"), "Expected allow: {json}");
        assert!(
            json.contains("\"exit_code\":0"),
            "Expected exit_code 0: {json}"
        );
    }

    #[test]
    fn test_gate_decision_deny() {
        let json = gate_decision_to_js_impl("deny");
        assert!(json.contains("deny"), "Expected deny: {json}");
        assert!(
            json.contains("\"exit_code\":1"),
            "Expected exit_code 1: {json}"
        );
    }

    #[test]
    fn test_gate_decision_hook() {
        let json = gate_decision_to_js_impl("hook");
        assert!(json.contains("hook"), "Expected hook: {json}");
        assert!(
            json.contains("\"exit_code\":2"),
            "Expected exit_code 2: {json}"
        );
    }

    #[test]
    fn test_gate_decision_case_insensitive() {
        for input in ["Allow", "DENY", "Hook"] {
            let json = gate_decision_to_js_impl(input);
            assert!(
                !json.contains("error"),
                "Case-insensitive parse should work for '{input}': {json}"
            );
        }
    }

    #[test]
    fn test_gate_decision_invalid() {
        let json = gate_decision_to_js_impl("maybe");
        assert!(json.contains("error"), "Expected error: {json}");
    }

    #[test]
    fn test_kv_entry_new() {
        let json = kv_entry_new_impl("my.key", "my.value");
        assert!(json.contains("my.key"), "Missing key: {json}");
        assert!(json.contains("my.value"), "Missing value: {json}");
        assert!(
            json.contains("wasm"),
            "Missing agent_id placeholder: {json}"
        );
        assert!(
            json.contains("browser"),
            "Missing session_id placeholder: {json}"
        );
    }
}
