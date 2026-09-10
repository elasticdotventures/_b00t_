use super::*;
use crate::identity::{CallerIdentity, JwksVerifier};
use crate::r0le_resolver::DatumR0leResolver;
use b00t_c0re_identity::{MockTokenSource, TokenRequest};
use serde_json::{Value, json};
use std::sync::Arc;

#[path = "../tests/support/mod.rs"]
mod support;

fn fixture() -> Value {
    serde_json::from_str(include_str!("../tests/fixtures/identity-policy.json")).unwrap()
}

fn token_request() -> TokenRequest {
    serde_json::from_value(fixture()["request"].clone()).unwrap()
}

fn write_datum(path: &Path, value: &Value) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, toml::to_string(value).unwrap()).unwrap();
}

fn server_with_profile(tmp: &Path) -> B00tMcpServerRusty {
    let b00t = tmp.join("_b00t_");
    write_datum(
        &b00t.join("worker.agentprofile.toml"),
        &fixture()["profile"],
    );
    write_datum(&b00t.join("inspection.skill.toml"), &fixture()["skill"]);
    B00tMcpServerRusty::new_flat(tmp, "")
        .unwrap()
        .with_r0le_resolver(Arc::new(DatumR0leResolver::new(b00t.to_str().unwrap())))
}

#[tokio::test]
async fn http_caller_drives_allowlist_learning_and_metering() {
    use rmcp::transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    };
    let tmp = tempfile::tempdir().unwrap();
    let server = server_with_profile(tmp.path()).with_spend_authorizer(Arc::new(
        b00t_c0re_ledgrrr::MockSpendAuthorizer::always_deny(),
    ));
    let state_handle = server.clone();
    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default().with_json_response(true),
    );
    let mock = MockTokenSource::new();
    let jwt = mock.mint(&token_request()).unwrap();
    let app = axum::Router::new().nest_service("/mcp", service).layer(
        axum::middleware::from_fn_with_state(
            crate::http_auth::IdentityLayerState {
                verifier: Arc::new(JwksVerifier::pinned(mock.jwks_json())),
                require_auth: true,
            },
            crate::http_auth::identity_middleware,
        ),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let mut client = support::HttpMcpClient::connect(&format!("http://{address}"), jwt).await;
    let listed = client.call("tools/list", json!({})).await;
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&ESCALATION_TOOL));
    assert!(!names.contains(&"b00t_whoami"));
    assert!(
        state_handle
            .registry
            .get_tools()
            .iter()
            .any(|t| t.name == "b00t_whoami")
    );
    let denied = client
        .call("tools/call", json!({"name":"b00t_whoami","arguments":{}}))
        .await;
    assert_eq!(denied["error"]["code"], -32002, "{denied}");
    let locked = client
        .call("tools/call", json!({"name":"b00t_status","arguments":{}}))
        .await;
    assert_eq!(locked["error"]["code"], -32003, "{locked}");
    state_handle
        .learned
        .write()
        .unwrap()
        .insert("inspection.skill".into());
    let budget = client
        .call("tools/call", json!({"name":"b00t_status","arguments":{}}))
        .await;
    assert_eq!(budget["error"]["code"], -32004, "{budget}");
    let mut other = token_request();
    other.tenant_id = "tenant-b".into();
    client.jwt = mock.mint(&other).unwrap();
    let foreign = client.call("tools/list", json!({})).await;
    assert_eq!(foreign["error"]["code"], -32001, "{foreign}");
    task.abort();
    let _ = task.await;
}

#[tokio::test]
async fn cached_identity_expires_and_invalid_jwt_never_becomes_anon() {
    let tmp = tempfile::tempdir().unwrap();
    let mock = MockTokenSource::new();
    let server =
        server_with_profile(tmp.path()).with_test_verifier(JwksVerifier::pinned(mock.jwks_json()));
    server.set_pending_jwt(mock.mint(&token_request()).unwrap());
    assert!(!server.caller().await.unwrap().is_anon());
    server.caller.lock().unwrap().as_mut().unwrap().expires_at = chrono::Utc::now().timestamp();
    assert!(server.caller().await.is_err());
    server.set_pending_jwt("invalid");
    assert!(server.caller().await.is_err());
}

#[tokio::test]
async fn tenant_profile_overlays_and_signature_failures_are_enforced() {
    let tmp = tempfile::tempdir().unwrap();
    let mut server = server_with_profile(tmp.path());
    let mock = MockTokenSource::new();
    let caller = JwksVerifier::pinned(mock.jwks_json())
        .verify(&mock.mint(&token_request()).unwrap())
        .await
        .unwrap();
    let tools = server.registry.get_tools();
    assert_eq!(
        server
            .filter_tools_for_r0le(tools.clone(), &caller)
            .unwrap()
            .len(),
        2
    );
    let mut overlay = fixture()["profile"].clone();
    overlay["b00t"]["agent_profile"]["tool_allowlist"] = json!(["b00t_learn"]);
    write_datum(
        &tmp.path()
            .join("tenants/tenant-a/_b00t_/worker.agentprofile.toml"),
        &overlay,
    );
    assert_eq!(
        server
            .filter_tools_for_r0le(tools.clone(), &caller)
            .unwrap()
            .len(),
        1
    );
    let mut other = caller.clone();
    other.tenant = "tenant-b".into();
    assert_eq!(
        server
            .filter_tools_for_r0le(tools.clone(), &other)
            .unwrap()
            .len(),
        2
    );
    server.r0le_resolver = Arc::new(
        DatumR0leResolver::new(tmp.path().join("_b00t_").to_str().unwrap())
            .require_signature(mock.jwks_json()),
    );
    assert!(
        server
            .filter_tools_for_r0le(tools.clone(), &caller)
            .is_err()
    );
    overlay["b00t"]["agent_profile"]["signature"] = fixture()["badSignature"].clone();
    write_datum(
        &tmp.path()
            .join("tenants/tenant-a/_b00t_/worker.agentprofile.toml"),
        &overlay,
    );
    assert!(server.filter_tools_for_r0le(tools, &caller).is_err());
}

#[tokio::test]
async fn missing_profile_skill_is_denied() {
    let tmp = tempfile::tempdir().unwrap();
    let server = server_with_profile(tmp.path());
    std::fs::remove_file(tmp.path().join("_b00t_/inspection.skill.toml")).unwrap();
    let mut caller = CallerIdentity::anon();
    caller.r0le = "worker".into();
    caller.tenant = "tenant-a".into();
    assert!(server.resolve_caller_role(&caller).is_err());
}

#[tokio::test]
async fn each_billable_call_has_a_distinct_ledger_key() {
    use axum::{Json, routing::post};
    let keys = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let captured = keys.clone();
    let app = axum::Router::new().route(
        "/v1/authorize-spend",
        post(move |Json(body): Json<Value>| {
            let captured = captured.clone();
            async move {
                let mut keys = captured.lock().unwrap();
                let key = body["ref"].as_str().unwrap().to_string();
                // A ledger replays successful authorizations for repeated keys.
                let ok = keys.contains(&key) || keys.is_empty();
                keys.push(key);
                Json(b00t_c0re_ledgrrr::AuthorizeResp {
                    ok,
                    budget_remaining: 0,
                    reason: None,
                })
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let tmp = tempfile::tempdir().unwrap();
    let server = server_with_profile(tmp.path()).with_spend_authorizer(Arc::new(
        b00t_c0re_ledgrrr::HttpSpendAuthorizer::new(format!("http://{address}")),
    ));
    let mut caller = CallerIdentity::anon();
    caller.r0le = "worker".into();
    caller.budget_ref = "issuance-key".into();
    assert!(server.authorize_call(&caller, "b00t_status").await.is_ok());
    assert_eq!(
        server
            .authorize_call(&caller, "b00t_status")
            .await
            .unwrap_err()
            .code,
        rmcp::model::ErrorCode(-32004)
    );
    let keys = keys.lock().unwrap();
    assert_eq!(keys.len(), 2);
    assert_ne!(keys[0], keys[1]);
    assert!(keys.iter().all(|k| k != &caller.budget_ref));
    task.abort();
    let _ = task.await;
}
