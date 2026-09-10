//! SP3-10 — end-to-end: a real SP1-worker JWT drives the b00t-mcp proxy's
//! r0le filtering + the -32003 unlock gate.
//!
//! `#[ignore]` — this needs live services. Run it with:
//!
//! ```bash
//! # 1. in workers/b00t-identity:  pnpm exec wrangler dev   (→ http://localhost:8787)
//! # 2. seed + grant an agent the `worker` r0le (scripts/seed.ts + a POST /tokens)
//! # 3. in b00t-mcp, with the worker's JWKS pinned:
//! #    B00T_IDENTITY_URL=http://localhost:8787 B00T_MCP_REQUIRE_AUTH=1 b00t-mcp --http --port 8899
//! B00T_E2E=1 \
//! B00T_IDENTITY_URL=http://localhost:8787 \
//! B00T_MCP_URL=http://localhost:8899 \
//! cargo test -p b00t-mcp --test e2e_identity -- --ignored --nocapture
//! ```

use b00t_c0re_identity::{HttpTokenSource, TokenRequest, TokenSource};
use serde_json::{Value, json};

fn e2e_enabled() -> bool {
    std::env::var("B00T_E2E").as_deref() == Ok("1")
}

async fn mint_worker_jwt() -> String {
    let base = std::env::var("B00T_IDENTITY_URL").expect("B00T_IDENTITY_URL");
    let tenant = std::env::var("B00T_E2E_TENANT").unwrap_or_else(|_| "promptexecution".into());
    let node = std::env::var("B00T_E2E_NODE").expect("B00T_E2E_NODE (root node id)");
    let src = HttpTokenSource::new(base);
    src.obtain(&TokenRequest {
        tenant_id: tenant,
        agent_id: "agent/e2e".into(),
        node_id: node,
        r0le: "worker".into(),
        requested_shards: vec!["project".into()],
    })
    .await
    .expect("mint worker JWT")
}

/// One MCP JSON-RPC call over streamable-http.
async fn mcp_call(base: &str, jwt: &str, method: &str, params: Value) -> Value {
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/mcp", base.trim_end_matches('/')))
        .header("authorization", format!("Bearer {jwt}"))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }))
        .send()
        .await
        .expect("mcp request");
    let text = res.text().await.unwrap_or_default();
    // streamable-http may frame the body as SSE; take the last JSON object line.
    text.lines()
        .rev()
        .find_map(|l| serde_json::from_str::<Value>(l.trim_start_matches("data: ")).ok())
        .unwrap_or_else(|| panic!("no JSON in MCP response: {text}"))
}

#[tokio::test]
#[ignore = "requires B00T_E2E=1 + a live wrangler dev + b00t-mcp --http"]
async fn worker_jwt_drives_filtering_and_the_unlock_gate() {
    if !e2e_enabled() {
        eprintln!("skipping: B00T_E2E != 1");
        return;
    }
    let mcp = std::env::var("B00T_MCP_URL").expect("B00T_MCP_URL");
    let jwt = mint_worker_jwt().await;

    // initialize
    let _ = mcp_call(
        &mcp,
        &jwt,
        "initialize",
        json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "e2e", "version": "0" }
        }),
    )
    .await;

    // tools/list — the worker r0le should see a filtered set (fewer than the
    // full registry) and it must include the escalation tool.
    let listed = mcp_call(&mcp, &jwt, "tools/list", json!({})).await;
    let tools = listed["result"]["tools"].as_array().expect("tools[]");
    assert!(!tools.is_empty(), "filtered tool list should be non-empty");
    assert!(
        tools
            .iter()
            .any(|t| t["name"] == "b00t_r0le_request_escalation"),
        "escalation tool must be offered to an identified caller"
    );

    // a gated tool → -32003, then succeeds after b00t_learn (best-effort: the
    // exact gated tool depends on the seeded r0le package, so only assert the
    // shape when the server reports a lock).
    let gated = std::env::var("B00T_E2E_GATED_TOOL").unwrap_or_default();
    if !gated.is_empty() {
        let denied = mcp_call(
            &mcp,
            &jwt,
            "tools/call",
            json!({ "name": gated, "arguments": {} }),
        )
        .await;
        assert_eq!(denied["error"]["code"], json!(-32003), "{denied}");

        let skill = std::env::var("B00T_E2E_GATED_SKILL").expect("B00T_E2E_GATED_SKILL");
        let _ = mcp_call(
            &mcp,
            &jwt,
            "tools/call",
            json!({ "name": "b00t_learn", "arguments": { "topic": skill } }),
        )
        .await;
        let retry = mcp_call(
            &mcp,
            &jwt,
            "tools/call",
            json!({ "name": gated, "arguments": {} }),
        )
        .await;
        assert!(retry.get("error").is_none(), "still gated after learn: {retry}");
    }
}
