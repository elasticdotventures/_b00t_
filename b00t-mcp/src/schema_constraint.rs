//! # Schema constraint — one JSON Schema, every inference backend
//!
//! JSON Schema is the portable grammar leg of the grammar-verify pattern
//! (`b00t learn grammar-verify`): any schema compiles mechanically to a
//! decode-time constraint, but each server takes it through a different
//! request field. This module owns that dialect mapping so callers deal in
//! exactly one artifact — the schema — and never in backend quirks.
//!
//! Dialects:
//! - OpenAI structured outputs: `response_format: {type: "json_schema", ...}`
//!   — the standard; recent vLLM and llama-server both accept it.
//! - llama-server native: top-level `"json_schema"` (older builds).
//! - vLLM native: top-level `"guided_json"` (extension field, pre-OpenAI-compat).
//!
//! `Backend::Auto` stamps the OpenAI form plus BOTH native fallbacks; servers
//! ignore fields they don't know, so the union is harmless and survives
//! upstream version drift. Pick a specific backend to keep requests minimal.
//!
//! 🤓 Schema constrains FORM, never truth — string free slots still need
//! their solver/gate audit (see grammar-verify::audit-free-slots).

use serde_json::{Value, json};

/// Which inference server the request is bound for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// OpenAI-compatible structured outputs (`response_format`) only.
    OpenAiCompat,
    /// llama-server: native `json_schema` field (+ OpenAI form).
    LlamaCpp,
    /// vLLM: native `guided_json` field (+ OpenAI form).
    Vllm,
    /// Unknown upstream: stamp all dialects; unknown fields are ignored.
    Auto,
}

impl Backend {
    /// Best-effort detection from an upstream base URL / soul backend name.
    pub fn detect(upstream: &str) -> Self {
        let u = upstream.to_ascii_lowercase();
        if u.contains("llama") {
            Backend::LlamaCpp
        } else if u.contains("vllm") {
            Backend::Vllm
        } else {
            Backend::Auto
        }
    }
}

/// Stamp `schema` onto a chat-completion request body for `backend`.
///
/// Existing constraint fields are left untouched (the client's own constraint
/// wins — same non-hijack posture as the verify tool loop). Returns true when
/// the request was modified.
pub fn constrain_request(
    request: &mut Value,
    name: &str,
    schema: &Value,
    backend: Backend,
) -> bool {
    let has_own_constraint = !request["response_format"].is_null()
        || !request["json_schema"].is_null()
        || !request["guided_json"].is_null()
        || !request["grammar"].is_null();
    if has_own_constraint {
        return false;
    }

    let openai_form = json!({
        "type": "json_schema",
        "json_schema": { "name": name, "schema": schema, "strict": true }
    });

    match backend {
        Backend::OpenAiCompat => {
            request["response_format"] = openai_form;
        }
        Backend::LlamaCpp => {
            request["response_format"] = openai_form;
            request["json_schema"] = schema.clone();
        }
        Backend::Vllm => {
            request["response_format"] = openai_form;
            request["guided_json"] = schema.clone();
        }
        Backend::Auto => {
            request["response_format"] = openai_form;
            request["json_schema"] = schema.clone();
            request["guided_json"] = schema.clone();
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disposition_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "disposition": { "enum": ["grant", "deny", "escalate"] },
                "reason": { "type": "string" }
            },
            "required": ["disposition", "reason"],
            "additionalProperties": false
        })
    }

    #[test]
    fn dialect_fields_per_backend() {
        for (backend, native_field, absent_field) in [
            (Backend::LlamaCpp, "json_schema", "guided_json"),
            (Backend::Vllm, "guided_json", "json_schema"),
        ] {
            let mut req = json!({"model": "ch0nky", "messages": []});
            assert!(constrain_request(
                &mut req,
                "sudo",
                &disposition_schema(),
                backend
            ));
            assert_eq!(req["response_format"]["type"], "json_schema");
            assert_eq!(req["response_format"]["json_schema"]["name"], "sudo");
            assert_eq!(req["response_format"]["json_schema"]["strict"], true);
            assert_eq!(req[native_field], disposition_schema(), "{backend:?}");
            assert!(req[absent_field].is_null(), "{backend:?}");
        }
    }

    #[test]
    fn auto_stamps_all_dialects() {
        let mut req = json!({"model": "m", "messages": []});
        assert!(constrain_request(
            &mut req,
            "s",
            &disposition_schema(),
            Backend::Auto
        ));
        assert!(!req["response_format"].is_null());
        assert!(!req["json_schema"].is_null());
        assert!(!req["guided_json"].is_null());
    }

    #[test]
    fn client_constraint_wins() {
        // Non-hijack: a request already carrying any constraint field is untouched.
        for own in ["response_format", "json_schema", "guided_json", "grammar"] {
            let mut req = json!({"model": "m", "messages": [], own: {"theirs": true}});
            assert!(!constrain_request(
                &mut req,
                "s",
                &disposition_schema(),
                Backend::Auto
            ));
            assert_eq!(req[own], json!({"theirs": true}), "{own} clobbered");
            assert!(req["messages"].as_array().unwrap().is_empty());
        }
    }

    #[test]
    fn detect_backend_from_upstream() {
        assert_eq!(
            Backend::detect("http://127.0.0.1:5273/v1 llama-server"),
            Backend::LlamaCpp
        );
        assert_eq!(
            Backend::detect("http://vllm.b00t-inference:8001/v1"),
            Backend::Vllm
        );
        assert_eq!(Backend::detect("https://api.openai.com/v1"), Backend::Auto);
    }
}
