//! Agent-scoped token issuance — pilot, `datum` shard-type only.
//!
//! See `docs/superpowers/specs/2026-08-22-agent-scoped-token-issuance.md`
//! for the full design. Orchestrates: check cake/budget → ensure a k8s
//! ServiceAccount+RoleBinding exist → mint a scoped token via k8s
//! TokenRequest → record the issuance as a ledger-core double-entry
//! accounting transaction → return the token.
//!
//! Enforcement (Component 4) lives in `crate::commands::datum` via the
//! `--as-agent-token` flag, which calls [`authorize_datum_shard_token`].

use anyhow::{Context, Result, bail};
use k8s_openapi::api::authentication::v1::{TokenRequest, TokenRequestSpec, TokenReview, TokenReviewSpec};
use k8s_openapi::api::core::v1::{Namespace, ServiceAccount};
use k8s_openapi::api::rbac::v1::{ClusterRole, RoleBinding, RoleRef, Subject};
use kube::Client;
use kube::api::{Api, ObjectMeta, PostParams};
use std::path::PathBuf;

use crate::cake_ledger::CakeLedger;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Namespace all agent ServiceAccounts/RoleBindings live in.
pub const AGENTS_NAMESPACE: &str = "b00t-agents";

/// Marker ClusterRole name — Component 4's TokenReview check looks for a
/// RoleBinding naming this Role, it grants no real k8s API access itself.
pub const ROLE_SHARD_DATUM: &str = "role-shard-datum";

/// Token lifetime for minted agent tokens (15 minutes).
pub const TOKEN_TTL_SECONDS: i64 = 15 * 60;

/// Embedded YAML for the `role-shard-datum` marker ClusterRole. Since
/// datum data is not migrated into k8s-native resources (deliberate scope
/// decision — the datum store stays wherever it already lives), this
/// Role's `rules` grant no meaningful k8s API access; its real job is
/// being *bound to* via RoleBinding, not granting access itself. Applied
/// (idempotent create-if-missing) by [`request_agent_token`] the first
/// time it's needed — no manual `kubectl apply` step required.
pub const ROLE_SHARD_DATUM_YAML: &str = r#"
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: role-shard-datum
  labels:
    app.kubernetes.io/managed-by: b00t
    b00t.elastic.ventures/shard-type: datum
rules:
  # Harmless, self-referential permission — this Role's real job is being
  # *bound to*, not granting k8s API access.
  - apiGroups: ["rbac.authorization.k8s.io"]
    resources: ["clusterroles"]
    resourceNames: ["role-shard-datum"]
    verbs: ["get"]
"#;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Request parameters for [`request_agent_token`].
#[derive(Debug, Clone)]
pub struct AgentTokenRequest {
    pub agent_id: String,
    /// e.g. `"datum:some-datum-id"` — only the `datum` shard-type is
    /// supported by this pilot; other shard-type prefixes are rejected.
    pub shard_ref: String,
    pub cost: i64,
}

/// Successful issuance result.
#[derive(Debug, Clone)]
pub struct AgentTokenIssuance {
    pub token: String,
    pub tx_id: String,
    pub expires_in_seconds: i64,
    pub remaining_balance: i64,
}

/// Identity + authorization result from [`authorize_datum_shard_token`].
#[derive(Debug, Clone)]
pub struct AuthorizedIdentity {
    pub username: String,
    pub namespace: String,
    pub service_account_name: String,
}

// ---------------------------------------------------------------------------
// Ledger path
// ---------------------------------------------------------------------------

/// Dedicated journal file for agent-token issuance — deliberately separate
/// from any real tax-ledger data (never mix pilot/test data with real
/// financial records).
pub fn default_ledger_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("resolve home directory")?;
    Ok(home.join(".b00t").join("ledger").join("agent-tokens.beancount"))
}

// ---------------------------------------------------------------------------
// Component 1: issuance flow
// ---------------------------------------------------------------------------

/// Full issuance flow (Components 1 + 2 + 3).
///
/// **Fail-before-privilege ordering is load-bearing**: the cake balance
/// check happens strictly before any k8s client is constructed or any k8s
/// API call is made. Only the `datum` shard-type is supported in this
/// pilot.
pub async fn request_agent_token(req: AgentTokenRequest) -> Result<AgentTokenIssuance> {
    anyhow::ensure!(req.cost >= 0, "cost must be non-negative, got {}", req.cost);
    anyhow::ensure!(
        req.shard_ref.starts_with("datum:"),
        "only the 'datum' shard-type is supported by this pilot; got shard-ref '{}'",
        req.shard_ref
    );

    // --- 1. Budget check — MUST happen before any k8s API interaction. ---
    let ledger = CakeLedger::open().context("open cake ledger")?;
    let balance = ledger.balance(&req.agent_id).context("check cake balance")?;
    if balance < req.cost {
        bail!(
            "insufficient budget for agent '{}': have {} cake, need {}",
            req.agent_id,
            balance,
            req.cost
        );
    }

    // --- 2. k8s client (only reached once budget is confirmed). ---
    let client = Client::try_default().await.context("connect to k8s cluster")?;

    // --- 3. Ensure namespace + ServiceAccount. ---
    ensure_namespace(&client, AGENTS_NAMESPACE).await?;
    let sa_name = service_account_name(&req.agent_id);
    ensure_service_account(&client, AGENTS_NAMESPACE, &sa_name).await?;

    // --- 4. Ensure the marker ClusterRole exists. ---
    ensure_role_shard_datum(&client).await?;

    // --- 5. Ensure the RoleBinding. ---
    ensure_role_binding(&client, AGENTS_NAMESPACE, &sa_name).await?;

    // --- 6. Mint a short-lived, scoped token. ---
    let token = mint_token(&client, AGENTS_NAMESPACE, &sa_name).await?;

    // --- 7. Record the issuance in ledger-core; debit cake. ---
    let tx_id = format!("agent-token-{}", uuid::Uuid::new_v4());
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let entry = ledger_core::journal::JournalTransaction::from_agent_token_issuance(
        &req.agent_id,
        &req.shard_ref,
        &req.cost.to_string(),
        tx_id.clone(),
        date,
    );
    let ledger_path = default_ledger_path()?;
    if let Some(parent) = ledger_path.parent() {
        std::fs::create_dir_all(parent).context("create ledger directory")?;
    }
    ledger_core::journal::append_entries(&ledger_path, std::slice::from_ref(&entry))
        .context("append agent-token journal entry")?;

    let remaining_balance = ledger
        .debit(&req.agent_id, req.cost)
        .context("debit cake balance")?;

    Ok(AgentTokenIssuance {
        token,
        tx_id,
        expires_in_seconds: TOKEN_TTL_SECONDS,
        remaining_balance,
    })
}

pub fn service_account_name(agent_id: &str) -> String {
    format!("agent-{}", agent_id)
}

fn role_binding_name(sa_name: &str) -> String {
    format!("{}-role-shard-datum", sa_name)
}

// ---------------------------------------------------------------------------
// k8s helpers — idempotent create-if-missing
// ---------------------------------------------------------------------------

fn is_conflict(err: &kube::Error) -> bool {
    matches!(err, kube::Error::Api(e) if e.code == 409)
}

async fn ensure_namespace(client: &Client, namespace: &str) -> Result<()> {
    let api: Api<Namespace> = Api::all(client.clone());
    if api.get(namespace).await.is_ok() {
        return Ok(());
    }
    let ns = Namespace {
        metadata: ObjectMeta {
            name: Some(namespace.to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    match api.create(&PostParams::default(), &ns).await {
        Ok(_) => Ok(()),
        Err(e) if is_conflict(&e) => Ok(()),
        Err(e) => Err(e).context("create b00t-agents namespace"),
    }
}

async fn ensure_service_account(client: &Client, namespace: &str, name: &str) -> Result<()> {
    let api: Api<ServiceAccount> = Api::namespaced(client.clone(), namespace);
    if api.get(name).await.is_ok() {
        return Ok(());
    }
    let sa = ServiceAccount {
        metadata: ObjectMeta {
            name: Some(name.to_string()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    match api.create(&PostParams::default(), &sa).await {
        Ok(_) => Ok(()),
        Err(e) if is_conflict(&e) => Ok(()),
        Err(e) => Err(e).context("create agent ServiceAccount"),
    }
}

async fn ensure_role_shard_datum(client: &Client) -> Result<()> {
    let api: Api<ClusterRole> = Api::all(client.clone());
    if api.get(ROLE_SHARD_DATUM).await.is_ok() {
        return Ok(());
    }
    let role: ClusterRole =
        serde_yaml::from_str(ROLE_SHARD_DATUM_YAML).context("parse embedded role-shard-datum YAML")?;
    match api.create(&PostParams::default(), &role).await {
        Ok(_) => Ok(()),
        Err(e) if is_conflict(&e) => Ok(()),
        Err(e) => Err(e).context("create role-shard-datum ClusterRole"),
    }
}

async fn ensure_role_binding(client: &Client, namespace: &str, sa_name: &str) -> Result<()> {
    let api: Api<RoleBinding> = Api::namespaced(client.clone(), namespace);
    let rb_name = role_binding_name(sa_name);
    if api.get(&rb_name).await.is_ok() {
        return Ok(());
    }
    let rb = RoleBinding {
        metadata: ObjectMeta {
            name: Some(rb_name),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        role_ref: RoleRef {
            api_group: "rbac.authorization.k8s.io".to_string(),
            kind: "ClusterRole".to_string(),
            name: ROLE_SHARD_DATUM.to_string(),
        },
        subjects: Some(vec![Subject {
            kind: "ServiceAccount".to_string(),
            name: sa_name.to_string(),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        }]),
    };
    match api.create(&PostParams::default(), &rb).await {
        Ok(_) => Ok(()),
        Err(e) if is_conflict(&e) => Ok(()),
        Err(e) => Err(e).context("create role-shard-datum RoleBinding"),
    }
}

async fn mint_token(client: &Client, namespace: &str, sa_name: &str) -> Result<String> {
    let api: Api<ServiceAccount> = Api::namespaced(client.clone(), namespace);
    let tr = TokenRequest {
        spec: TokenRequestSpec {
            expiration_seconds: Some(TOKEN_TTL_SECONDS),
            ..Default::default()
        },
        ..Default::default()
    };
    let data = serde_json::to_vec(&tr).context("serialize TokenRequest")?;
    let result: TokenRequest = api
        .create_subresource("token", sa_name, &PostParams::default(), &data)
        .await
        .context("k8s TokenRequest")?;
    result
        .status
        .map(|s| s.token)
        .context("TokenRequest response missing status.token")
}

// ---------------------------------------------------------------------------
// Component 4: TokenReview-based enforcement
// ---------------------------------------------------------------------------

/// Validate `token` via k8s's TokenReview API, then confirm the resulting
/// ServiceAccount identity has a RoleBinding to [`ROLE_SHARD_DATUM`].
/// Returns a clear "not authorized for this shard" error (distinct from a
/// generic/connection error) on either failure.
pub async fn authorize_datum_shard_token(token: &str) -> Result<AuthorizedIdentity> {
    let client = Client::try_default().await.context("connect to k8s cluster")?;

    let api: Api<TokenReview> = Api::all(client.clone());
    let review = TokenReview {
        spec: TokenReviewSpec {
            token: Some(token.to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let result = api
        .create(&PostParams::default(), &review)
        .await
        .context("k8s TokenReview")?;

    let status = result
        .status
        .ok_or_else(|| anyhow::anyhow!("not authorized for this shard: empty TokenReview status"))?;

    if !status.authenticated.unwrap_or(false) {
        bail!(
            "not authorized for this shard: token not authenticated ({})",
            status.error.unwrap_or_else(|| "unknown reason".to_string())
        );
    }

    let username = status
        .user
        .and_then(|u| u.username)
        .ok_or_else(|| anyhow::anyhow!("not authorized for this shard: TokenReview response missing user info"))?;

    let (namespace, sa_name) = parse_service_account_username(&username).ok_or_else(|| {
        anyhow::anyhow!(
            "not authorized for this shard: '{}' is not a ServiceAccount identity",
            username
        )
    })?;

    let bound = role_binding_exists_for(&client, &namespace, &sa_name).await?;
    if !bound {
        bail!(
            "not authorized for this shard: ServiceAccount '{}/{}' has no role-shard-datum RoleBinding",
            namespace,
            sa_name
        );
    }

    Ok(AuthorizedIdentity {
        username,
        namespace,
        service_account_name: sa_name,
    })
}

/// Parse Kubernetes' standard ServiceAccount username format:
/// `system:serviceaccount:<namespace>:<name>`.
fn parse_service_account_username(username: &str) -> Option<(String, String)> {
    let mut parts = username.splitn(4, ':');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("system"), Some("serviceaccount"), Some(ns), Some(name)) if !ns.is_empty() && !name.is_empty() => {
            Some((ns.to_string(), name.to_string()))
        }
        _ => None,
    }
}

async fn role_binding_exists_for(client: &Client, namespace: &str, sa_name: &str) -> Result<bool> {
    let api: Api<RoleBinding> = Api::namespaced(client.clone(), namespace);
    let list = api.list(&Default::default()).await.context("list RoleBindings")?;
    Ok(list.items.iter().any(|rb| {
        rb.role_ref.name == ROLE_SHARD_DATUM
            && rb
                .subjects
                .as_ref()
                .is_some_and(|subs| subs.iter().any(|s| s.kind == "ServiceAccount" && s.name == sa_name))
    }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes tests in this module that mutate process-wide env vars
    /// (HOME/XDG_DATA_HOME), since `cargo test` runs tests in parallel
    /// threads within one process by default.
    static ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Soft-skip helper: probes whether a k8s cluster is actually
    /// reachable (not just that *some* kubeconfig/in-cluster config
    /// exists) via a real API call. Returns the connected client if so.
    async fn live_cluster() -> Option<Client> {
        let client = Client::try_default().await.ok()?;
        let ns_api: Api<Namespace> = Api::all(client.clone());
        ns_api.list(&Default::default()).await.ok()?;
        Some(client)
    }

    #[test]
    fn test_service_account_name() {
        assert_eq!(service_account_name("claude-worker-7"), "agent-claude-worker-7");
    }

    #[test]
    fn test_role_binding_name() {
        assert_eq!(
            role_binding_name("agent-claude-worker-7"),
            "agent-claude-worker-7-role-shard-datum"
        );
    }

    #[test]
    fn test_parse_service_account_username_valid() {
        let parsed = parse_service_account_username("system:serviceaccount:b00t-agents:agent-foo");
        assert_eq!(
            parsed,
            Some(("b00t-agents".to_string(), "agent-foo".to_string()))
        );
    }

    #[test]
    fn test_parse_service_account_username_invalid() {
        assert_eq!(parse_service_account_username("operator"), None);
        assert_eq!(parse_service_account_username("system:anonymous"), None);
        assert_eq!(parse_service_account_username(""), None);
    }

    #[test]
    fn test_role_shard_datum_yaml_parses() {
        let role: ClusterRole =
            serde_yaml::from_str(ROLE_SHARD_DATUM_YAML).expect("embedded YAML must parse");
        assert_eq!(role.metadata.name.as_deref(), Some(ROLE_SHARD_DATUM));
    }

    /// Fail-before-privilege: with an agent that has no seeded cake
    /// balance (0), the request must be denied with the budget error —
    /// without ever attempting a k8s API call. This test runs with no
    /// reachable k8s cluster (no kubeconfig / in-cluster config assumed in
    /// this environment); if the code attempted a k8s call before the
    /// budget check, this would instead surface as a connection/config
    /// error, not this specific message — that distinction is exactly
    /// what this test verifies.
    #[tokio::test]
    async fn test_insufficient_budget_denied_before_any_k8s_call() {
        let _guard = ENV_GUARD.lock().unwrap();

        // Isolate HOME so CakeLedger::open() (which uses dirs::data_dir())
        // resolves to a fresh, empty scheduler DB with a guaranteed-zero
        // balance for this never-before-seen agent id.
        let tmp = tempfile::tempdir().expect("tempdir");
        // SAFETY: guarded by ENV_GUARD above — no other test in this
        // module mutates HOME/XDG_DATA_HOME concurrently.
        unsafe {
            std::env::set_var("XDG_DATA_HOME", tmp.path());
            std::env::set_var("HOME", tmp.path());
        }

        let req = AgentTokenRequest {
            agent_id: format!("never-funded-{}", uuid::Uuid::new_v4()),
            shard_ref: "datum:some-datum-id".to_string(),
            cost: 5,
        };

        let result = request_agent_token(req).await;
        let err = result.expect_err("expected insufficient-budget denial");
        let msg = err.to_string();
        assert!(
            msg.contains("insufficient budget"),
            "expected insufficient-budget error, got: {msg}"
        );
    }

    /// Full positive flow (Components 1-3 + the positive half of
    /// Component 4) against a real k8s cluster, when one is reachable.
    /// Soft-skips (prints a notice, doesn't fail) when no cluster is
    /// configured — e.g. in CI, or a sandbox with no KUBECONFIG pointing
    /// at a live cluster.
    #[tokio::test]
    async fn test_full_issuance_flow_against_live_cluster_if_reachable() {
        let _guard = ENV_GUARD.lock().unwrap();

        if live_cluster().await.is_none() {
            eprintln!(
                "SKIP test_full_issuance_flow_against_live_cluster_if_reachable: \
                 no k8s cluster reachable in this environment (set KUBECONFIG to \
                 test against a live cluster)"
            );
            return;
        }

        let tmp = tempfile::tempdir().expect("tempdir");
        // SAFETY: guarded by ENV_GUARD above.
        unsafe {
            std::env::set_var("XDG_DATA_HOME", tmp.path());
            std::env::set_var("HOME", tmp.path());
        }

        let agent_id = format!("test-agent-{}", uuid::Uuid::new_v4());
        let ledger = CakeLedger::open().expect("open cake ledger");
        crate::cake_ledger::seed_balance_for_test(&ledger, &agent_id, 10);

        let issuance = request_agent_token(AgentTokenRequest {
            agent_id: agent_id.clone(),
            shard_ref: "datum:integration-test-datum".to_string(),
            cost: 3,
        })
        .await
        .expect("request_agent_token should succeed against a live cluster");

        assert!(!issuance.token.is_empty(), "minted token must be non-empty");
        assert_eq!(issuance.remaining_balance, 7);
        assert_eq!(ledger.balance(&agent_id).expect("balance"), 7);

        // Journal entry actually appended to the dedicated ledger file.
        let ledger_path = default_ledger_path().expect("ledger path");
        let contents = std::fs::read_to_string(&ledger_path).expect("read journal file");
        assert!(contents.contains(&issuance.tx_id), "journal must contain this issuance's tx_id");
        assert!(
            contents.contains("datum:integration-test-datum"),
            "journal must reference the shard"
        );
        assert!(
            contents.contains(&format!("Assets:Cake:{agent_id}")),
            "journal must debit the agent's cake account"
        );

        // Positive half of Component 4: the freshly-minted token IS
        // authorized for the datum shard (it has the RoleBinding
        // request_agent_token just ensured).
        let identity = authorize_datum_shard_token(&issuance.token)
            .await
            .expect("freshly-minted token should be authorized for the datum shard");
        assert_eq!(identity.namespace, AGENTS_NAMESPACE);
        assert_eq!(identity.service_account_name, service_account_name(&agent_id));
    }

    /// Negative half of Component 4: a token for a ServiceAccount that has
    /// no `role-shard-datum` RoleBinding must be denied with the specific
    /// "not authorized for this shard" error — not a crash, not a generic
    /// error. Soft-skips when no cluster is reachable.
    #[tokio::test]
    async fn test_datum_shard_authorization_denied_for_unbound_service_account() {
        let _guard = ENV_GUARD.lock().unwrap();

        let Some(client) = live_cluster().await else {
            eprintln!(
                "SKIP test_datum_shard_authorization_denied_for_unbound_service_account: \
                 no k8s cluster reachable in this environment"
            );
            return;
        };

        // A bare ServiceAccount with NO RoleBinding to role-shard-datum, in
        // a throwaway namespace kept separate from b00t-agents on purpose.
        let ns = "default";
        let sa_name = format!("unbound-test-sa-{}", uuid::Uuid::new_v4());
        let sa_api: Api<ServiceAccount> = Api::namespaced(client.clone(), ns);
        let sa = ServiceAccount {
            metadata: ObjectMeta {
                name: Some(sa_name.clone()),
                namespace: Some(ns.to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        sa_api
            .create(&PostParams::default(), &sa)
            .await
            .expect("create throwaway ServiceAccount");

        // Mint a token for it directly — bypassing request_agent_token,
        // which would create the RoleBinding; this test specifically needs
        // a token WITHOUT one.
        let tr = TokenRequest {
            spec: TokenRequestSpec {
                expiration_seconds: Some(300),
                ..Default::default()
            },
            ..Default::default()
        };
        let data = serde_json::to_vec(&tr).expect("serialize TokenRequest");
        let result: TokenRequest = sa_api
            .create_subresource("token", &sa_name, &PostParams::default(), &data)
            .await
            .expect("mint token for unbound ServiceAccount");
        let token = result.status.expect("TokenRequest status").token;

        let err = authorize_datum_shard_token(&token)
            .await
            .expect_err("unbound ServiceAccount token must be denied");
        assert!(
            err.to_string().contains("not authorized for this shard"),
            "expected 'not authorized for this shard' error, got: {err}"
        );

        // Best-effort cleanup — not asserted, this is a throwaway resource.
        let _ = sa_api
            .delete(&sa_name, &kube::api::DeleteParams::default())
            .await;
    }
}
