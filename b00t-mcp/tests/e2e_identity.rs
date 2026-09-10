//! Live worker/MCP test. Requires B00T_IDENTITY_URL (origin),
//! B00T_IDENTITY_ISSUANCE_TOKEN, B00T_MCP_URL, B00T_E2E_TENANT, B00T_E2E_NODE,
//! B00T_E2E_GATED_TOOL and B00T_E2E_GATED_SKILL. The agent/e2e grant must
//! authorize the worker role and project scope. Run with --ignored.
mod support;

use b00t_c0re_identity::{HttpTokenSource, TokenRequest, TokenSource};
use serde_json::json;

#[tokio::test]
#[ignore = "requires live identity and MCP services with a provisioned agent grant"]
async fn worker_jwt_drives_filtering_and_the_unlock_gate() {
    let source =
        HttpTokenSource::new(std::env::var("B00T_IDENTITY_URL").expect("B00T_IDENTITY_URL"))
            .with_issuance_credential(
                std::env::var("B00T_IDENTITY_ISSUANCE_TOKEN")
                    .expect("B00T_IDENTITY_ISSUANCE_TOKEN"),
            );
    let jwt = source
        .obtain(&TokenRequest {
            tenant_id: std::env::var("B00T_E2E_TENANT").expect("B00T_E2E_TENANT"),
            agent_id: "agent/e2e".into(),
            node_id: std::env::var("B00T_E2E_NODE").expect("B00T_E2E_NODE"),
            r0le: "worker".into(),
            requested_shards: vec!["project".into()],
        })
        .await
        .expect("mint worker JWT");
    let mut client =
        support::HttpMcpClient::connect(&std::env::var("B00T_MCP_URL").expect("B00T_MCP_URL"), jwt)
            .await;
    let listed = client.call("tools/list", json!({})).await;
    let tools = listed["result"]["tools"].as_array().expect("tools[]");
    assert!(
        tools
            .iter()
            .any(|t| t["name"] == "b00t_r0le_request_escalation")
    );
    let tool = std::env::var("B00T_E2E_GATED_TOOL").expect("B00T_E2E_GATED_TOOL");
    let skill = std::env::var("B00T_E2E_GATED_SKILL").expect("B00T_E2E_GATED_SKILL");
    let denied = client
        .call("tools/call", json!({"name":tool,"arguments":{}}))
        .await;
    assert_eq!(denied["error"]["code"], -32003, "{denied}");
    let learned = client
        .call(
            "tools/call",
            json!({"name":"b00t_learn","arguments":{"topic":skill}}),
        )
        .await;
    assert!(learned.get("error").is_none(), "{learned}");
    assert_ne!(learned["result"]["isError"], true, "{learned}");
    let retry = client
        .call("tools/call", json!({"name":tool,"arguments":{}}))
        .await;
    assert!(retry.get("error").is_none(), "{retry}");
    assert_ne!(retry["result"]["isError"], true, "{retry}");
}
