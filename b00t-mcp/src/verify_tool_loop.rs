//! # Verify tool loop — agentic proxy leg of the hallucination-reduction path (#597)
//!
//! When the upstream model (vLLM/Qwen3 function-calling) answers a chat
//! completion with `finish_reason: "tool_calls"` naming the `verify` tool,
//! `proxy_chat` does not return that to the client. It executes the Z3 verify
//! surface locally, appends the assistant message + `tool`-role results to the
//! conversation, and re-enters the model — up to [`MAX_TOOL_ITERATIONS`] times
//! before passing through whatever the model last said.
//!
//! 🤓 This makes b00t-mcp an *agentic proxy*, not a passthrough: the LLM
//! believes it is doing tool calling; b00t-mcp is the executor. Only the
//! `verify` tool is intercepted — foreign tool_calls pass through untouched so
//! clients running their own tool loops are not hijacked.
//!
//! Streaming requests (`"stream": true`) bypass the loop entirely: SSE bytes
//! are forwarded verbatim (loop-in-stream is follow-up work, see gh#597).

use serde_json::{json, Value};

/// Hard cap on model re-entries per request; beyond this the last upstream
/// response is returned as-is. Prevents a model that keeps emitting verify
/// calls from looping the proxy forever (and burning 🎂).
pub const MAX_TOOL_ITERATIONS: usize = 3;

/// OpenAI-style tool definition for `verify`, injectable into `tools=[]`.
pub fn verify_tool_definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "verify",
            "description": "Formally verify an SMT2 assertion via Z3. Returns sat/unsat/unknown with elapsed_ms.",
            "parameters": {
                "type": "object",
                "properties": {
                    "assertion": { "type": "string", "description": "SMT2 assertion, e.g. '(declare-const x Int)(assert (= x 42))(check-sat)'" },
                    "format": { "type": "string", "enum": ["smt2"], "description": "Input format (smt2 only today)" }
                },
                "required": ["assertion"]
            }
        }
    })
}

/// Opt-in injection: when `B00T_VERIFY_TOOL_INJECT=1`, proxy_chat adds the
/// verify tool definition to requests that don't already carry one. Returns
/// true when the request was modified. Default off — silently rewriting every
/// consumer's tools array is not the proxy's call to make.
pub fn inject_verify_tool(request: &mut Value) -> bool {
    let already_present = request["tools"]
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .any(|t| t["function"]["name"].as_str() == Some("verify"))
        })
        .unwrap_or(false);
    if already_present {
        return false;
    }
    let mut tools = request["tools"].as_array().cloned().unwrap_or_default();
    tools.push(verify_tool_definition());
    request["tools"] = Value::Array(tools);
    true
}

/// A verify invocation requested by the model.
#[derive(Debug, Clone, PartialEq)]
pub struct VerifyCall {
    pub id: String,
    pub assertion: String,
}

/// Parse verify tool calls out of a chat-completion response.
///
/// Empty unless `choices[0].finish_reason == "tool_calls"`. Only
/// `function.name == "verify"` entries are extracted — other tools belong to
/// the client's own loop. Per OpenAI spec `arguments` is a JSON-encoded
/// string, but some servers inline an object; both are accepted (Postel).
/// Malformed arguments skip that call, never fail the batch.
pub fn extract_verify_calls(response: &Value) -> Vec<VerifyCall> {
    let choice = &response["choices"][0];
    if choice["finish_reason"].as_str() != Some("tool_calls") {
        return Vec::new();
    }
    let Some(tool_calls) = choice["message"]["tool_calls"].as_array() else {
        return Vec::new();
    };
    tool_calls
        .iter()
        .filter(|tc| tc["function"]["name"].as_str() == Some("verify"))
        .filter_map(|tc| {
            let id = tc["id"].as_str()?.to_string();
            let args = &tc["function"]["arguments"];
            let parsed: Value = match args {
                Value::String(s) => serde_json::from_str(s).ok()?,
                Value::Object(_) => args.clone(),
                _ => return None,
            };
            let assertion = parsed["assertion"].as_str()?.to_string();
            Some(VerifyCall { id, assertion })
        })
        .collect()
}

/// Append the assistant's tool_calls message and the tool results to the
/// original request body, producing the re-entry request. Non-message fields
/// (model, temperature, tools, …) are preserved verbatim.
pub fn build_reentry_body(
    original_request: &Value,
    assistant_message: &Value,
    results: &[(String, String)],
) -> Value {
    let mut next = original_request.clone();
    let messages = next["messages"].as_array().cloned().unwrap_or_default();
    let mut messages = messages;
    messages.push(assistant_message.clone());
    for (tool_call_id, content) in results {
        messages.push(json!({
            "role": "tool",
            "tool_call_id": tool_call_id,
            "content": content,
        }));
    }
    next["messages"] = Value::Array(messages);
    next
}

/// Drive the tool loop: given the original request and its first upstream
/// response, execute verify calls and re-enter the model until it stops
/// calling verify or [`MAX_TOOL_ITERATIONS`] is hit. Returns the final
/// response to hand to the client.
///
/// `send` posts a request body upstream; `execute` runs one verify call and
/// returns the tool-message content. Both are injected so the loop is
/// unit-testable without network or z3.
pub async fn run_tool_loop<S, Fut, E>(
    original_request: &Value,
    first_response: Value,
    send: S,
    execute: E,
) -> Value
where
    S: Fn(Value) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Value>>,
    E: Fn(&VerifyCall) -> String,
{
    let mut request = original_request.clone();
    let mut response = first_response;
    for _ in 0..MAX_TOOL_ITERATIONS {
        let calls = extract_verify_calls(&response);
        if calls.is_empty() {
            break;
        }
        let assistant_message = response["choices"][0]["message"].clone();
        let results: Vec<(String, String)> =
            calls.iter().map(|c| (c.id.clone(), execute(c))).collect();
        request = build_reentry_body(&request, &assistant_message, &results);
        match send(request.clone()).await {
            Ok(next) => response = next,
            // Upstream died mid-loop: the last good response is still the
            // most useful thing we have — return it rather than erroring.
            Err(e) => {
                eprintln!("⚠️ verify tool loop: upstream re-entry failed: {e}");
                break;
            }
        }
    }
    response
}

/// Execute one verify call against the local z3 surface (BVerifyCommand).
pub fn execute_verify_call(call: &VerifyCall) -> String {
    use crate::clap_reflection::McpExecutor;
    let mut params = std::collections::HashMap::new();
    params.insert("assertion".to_string(), Value::String(call.assertion.clone()));
    crate::mcp_tools::BVerifyCommand::execute_mcp_call(&params)
        .unwrap_or_else(|e| json!({"result": "error", "verified": false, "error": e.to_string()}).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_cases() -> Vec<(String, Value, Vec<VerifyCall>)> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/verify_tool_loop_cases.json");
        let fx: Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("fixture readable"))
                .expect("fixture parses");
        fx["cases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|case| {
                let expected = case["extracted"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| VerifyCall {
                        id: e["id"].as_str().unwrap().to_string(),
                        assertion: e["assertion"].as_str().unwrap().to_string(),
                    })
                    .collect();
                (
                    case["name"].as_str().unwrap().to_string(),
                    case["response"].clone(),
                    expected,
                )
            })
            .collect()
    }

    #[test]
    fn extract_matches_fixture_cases() {
        for (name, response, expected) in fixture_cases() {
            assert_eq!(extract_verify_calls(&response), expected, "case '{name}'");
        }
    }

    #[test]
    fn inject_adds_tool_once_and_preserves_existing() {
        let mut req = json!({"model": "m", "messages": []});
        assert!(inject_verify_tool(&mut req));
        assert!(!inject_verify_tool(&mut req), "second inject must be a no-op");
        assert_eq!(req["tools"].as_array().unwrap().len(), 1);

        let mut with_other = json!({"tools": [{"type": "function", "function": {"name": "get_weather"}}]});
        assert!(inject_verify_tool(&mut with_other));
        let names: Vec<&str> = with_other["tools"].as_array().unwrap().iter()
            .filter_map(|t| t["function"]["name"].as_str()).collect();
        assert_eq!(names, vec!["get_weather", "verify"]);
    }

    #[test]
    fn reentry_body_appends_assistant_and_tool_messages() {
        let original = json!({
            "model": "ch0nky",
            "temperature": 0.2,
            "messages": [{"role": "user", "content": "is x=42 satisfiable?"}]
        });
        let assistant = json!({"role": "assistant", "tool_calls": [{"id": "call_1"}]});
        let next = build_reentry_body(
            &original,
            &assistant,
            &[("call_1".into(), "{\"result\":\"sat\"}".into())],
        );
        assert_eq!(next["model"], "ch0nky");
        assert_eq!(next["temperature"], json!(0.2));
        let msgs = next["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[1], assistant);
        assert_eq!(msgs[2]["role"], "tool");
        assert_eq!(msgs[2]["tool_call_id"], "call_1");
        assert_eq!(msgs[2]["content"], "{\"result\":\"sat\"}");
    }

    fn tool_call_response(id: &str, assertion: &str) -> Value {
        json!({"choices": [{"finish_reason": "tool_calls", "message": {
            "role": "assistant",
            "tool_calls": [{"id": id, "type": "function", "function": {
                "name": "verify",
                "arguments": format!("{{\"assertion\": \"{assertion}\"}}"),
            }}]
        }}]})
    }

    fn stop_response(content: &str) -> Value {
        json!({"choices": [{"finish_reason": "stop", "message": {"role": "assistant", "content": content}}]})
    }

    #[tokio::test]
    async fn loop_executes_then_returns_final_answer() {
        let original = json!({"model": "m", "messages": [{"role": "user", "content": "q"}]});
        let first = tool_call_response("call_1", "(check-sat)");
        let sends = std::sync::atomic::AtomicUsize::new(0);
        let final_resp = run_tool_loop(
            &original,
            first,
            |body| {
                sends.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                // Re-entry body must carry the tool result
                let msgs = body["messages"].as_array().unwrap().clone();
                async move {
                    assert_eq!(msgs.last().unwrap()["role"], "tool");
                    Ok(stop_response("verified: sat"))
                }
            },
            |call| {
                assert_eq!(call.assertion, "(check-sat)");
                "{\"result\":\"sat\",\"verified\":true}".to_string()
            },
        )
        .await;
        assert_eq!(sends.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(final_resp["choices"][0]["message"]["content"], "verified: sat");
    }

    #[tokio::test]
    async fn loop_caps_iterations_and_passes_through() {
        let original = json!({"model": "m", "messages": []});
        let first = tool_call_response("call_0", "(check-sat)");
        let sends = std::sync::atomic::AtomicUsize::new(0);
        // Model never stops calling verify → loop must cap at MAX_TOOL_ITERATIONS
        let final_resp = run_tool_loop(
            &original,
            first,
            |_body| {
                let n = sends.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async move { Ok(tool_call_response(&format!("call_{}", n + 1), "(check-sat)")) }
            },
            |_call| "{\"result\":\"sat\"}".to_string(),
        )
        .await;
        assert_eq!(sends.load(std::sync::atomic::Ordering::SeqCst), MAX_TOOL_ITERATIONS);
        // Passthrough: last response still asks for tools, client sees it verbatim
        assert_eq!(final_resp["choices"][0]["finish_reason"], "tool_calls");
    }

    #[tokio::test]
    async fn upstream_failure_returns_last_good_response() {
        let original = json!({"model": "m", "messages": []});
        let first = tool_call_response("call_1", "(check-sat)");
        let final_resp = run_tool_loop(
            &original,
            first.clone(),
            |_body| async move { anyhow::bail!("connection refused") },
            |_call| "{\"result\":\"sat\"}".to_string(),
        )
        .await;
        assert_eq!(final_resp, first);
    }

    #[tokio::test]
    async fn plain_response_short_circuits_without_send() {
        let original = json!({"model": "m", "messages": []});
        let final_resp = run_tool_loop(
            &original,
            stop_response("no tools needed"),
            |_body| async move { panic!("send must not be called") },
            |_call| panic!("execute must not be called"),
        )
        .await;
        assert_eq!(final_resp["choices"][0]["message"]["content"], "no tools needed");
    }
}
