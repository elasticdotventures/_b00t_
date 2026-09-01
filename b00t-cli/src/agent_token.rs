//! Agent-scoped token issuance — #1104.
//!
//! Orchestrates: check cake balance (fail before privilege) → ensure a k8s
//! ServiceAccount + RoleBinding exist for the requested #1102 shard scope →
//! mint a scoped token via k8s TokenRequest → record the issuance as a
//! double-entry accounting transaction → return the token.
//!
//! The ledger recording ([`AgentTokenJournalEntry`] below) deliberately does
//! NOT depend on vendor/ledgrrr's `ledger-core` crate: pulling it in
//! requires Cargo to fully resolve its optional `arc-kit-au` ->
//! `msft-agent-gov-ledgrrr` -> `agentmesh` dependency chain regardless of
//! feature flags, and `agentmesh v3.5.0` hard-pins `serde =1.0.228`, which
//! conflicts with the rest of this workspace's `serde 1.0.229` — a real,
//! pre-existing version conflict inside that submodule, out of scope to fix
//! here (other sessions are actively working in `vendor/ledgrrr`). The
//! journal entry shape below mirrors `ledger_core::journal`'s
//! `JournalTransaction`/`from_agent_token_issuance`/`to_beancount_entry`
//! field-for-field and format-string-for-format-string, so the on-disk
//! `agent-tokens.beancount` file stays format-compatible if/when a real
//! `ledger-core` dependency becomes usable again.
//!
//! Enforcement lives in `crate::commands::datum`'s `--as-agent-token` flag
//! (and, mechanically, any other command gated on shard access), which
//! calls [`authorize_shard_token`].
//!
//! One parameterized [`ROLE_SHARD_ACCESS`] ClusterRole is shared across all
//! six #1102 shard kinds — it grants no real k8s API access itself (its only
//! job is being a RoleBinding target); the specific kind+id a RoleBinding
//! authorizes is carried as labels on the RoleBinding, not as six separate
//! marker roles.

use anyhow::{Context, Result, bail};
use k8s_openapi::api::authentication::v1::{TokenRequest, TokenRequestSpec, TokenReview, TokenReviewSpec};
use k8s_openapi::api::core::v1::{Namespace, ServiceAccount};
use k8s_openapi::api::rbac::v1::{ClusterRole, RoleBinding, RoleRef, Subject};
use kube::Client;
use kube::api::{Api, ObjectMeta, PostParams};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::cake_ledger::CakeLedger;
use crate::soul_scope::SoulScope;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Namespace all agent ServiceAccounts/RoleBindings live in.
pub const AGENTS_NAMESPACE: &str = "b00t-agents";

/// Marker ClusterRole name — shared across all #1102 shard kinds.
/// [`authorize_shard_token`]'s TokenReview check looks for a RoleBinding
/// naming this Role, filtered by shard labels; the Role itself grants no
/// real k8s API access.
pub const ROLE_SHARD_ACCESS: &str = "role-shard-access";

/// Token lifetime for minted agent tokens (15 minutes).
pub const TOKEN_TTL_SECONDS: i64 = 15 * 60;

const LABEL_SHARD_KIND: &str = "b00t.elastic.ventures/shard-kind";
const LABEL_SHARD_ID: &str = "b00t.elastic.ventures/shard-id";

/// Embedded YAML for the `role-shard-access` marker ClusterRole. Shard data
/// itself is not migrated into k8s-native resources — this Role's `rules`
/// grant no meaningful k8s API access; its real job is being *bound to* via
/// a per-(agent,scope) RoleBinding, not granting access itself. Applied
/// (idempotent create-if-missing) by [`request_agent_token`] the first time
/// it's needed — no manual `kubectl apply` step required.
pub const ROLE_SHARD_ACCESS_YAML: &str = r#"
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: role-shard-access
  labels:
    app.kubernetes.io/managed-by: b00t
rules:
  # Harmless, self-referential permission — this Role's real job is being
  # *bound to*, not granting k8s API access.
  - apiGroups: ["rbac.authorization.k8s.io"]
    resources: ["clusterroles"]
    resourceNames: ["role-shard-access"]
    verbs: ["get"]
"#;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Request parameters for [`request_agent_token`].
#[derive(Debug, Clone)]
pub struct AgentTokenRequest {
    pub agent_id: String,
    pub scope: SoulScope,
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

/// Identity + authorization result from [`authorize_shard_token`].
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
/// from any real tax-ledger data (never mix agent-token/test data with real
/// financial records).
pub fn default_ledger_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("resolve home directory")?;
    Ok(home.join(".b00t").join("ledger").join("agent-tokens.beancount"))
}

/// A double-entry beancount transaction for one agent-token issuance.
/// Mirrors `ledger_core::journal::JournalTransaction` field-for-field — see
/// this module's doc comment for why it's a local copy rather than a
/// dependency on that crate.
struct AgentTokenJournalEntry {
    date: String,
    narration: String,
    asset_account: String,
    counterparty_account: String,
    amount: String,
    currency: String,
    tx_id: String,
    source_ref: String,
}

impl AgentTokenJournalEntry {
    /// Produces a balanced entry: `Assets:Cake:<agent_id>` debited by
    /// `cost`, `Expenses:AgentTokens:<shard-type>` credited by `cost`,
    /// where `<shard-type>` is the portion of `shard_ref` before the first
    /// `:` (e.g. `datum` from `datum:some-datum-id`).
    fn new(agent_id: &str, shard_ref: &str, cost: &str, tx_id: String, date: String) -> Self {
        Self {
            date,
            narration: format!("agent token issued: {shard_ref}"),
            asset_account: format!("Assets:Cake:{agent_id}"),
            counterparty_account: format!(
                "Expenses:AgentTokens:{}",
                shard_ref.split(':').next().unwrap_or(shard_ref)
            ),
            amount: cost.to_string(),
            currency: "CAKE".to_string(),
            tx_id,
            source_ref: format!("agent-token:{agent_id}"),
        }
    }

    fn to_beancount_entry(&self) -> String {
        let inverse = invert_amount(&self.amount);
        format!(
            "{} * \"{}\" \"{}\"\n  txid: \"{}\"\n  source_ref: \"{}\"\n  {} {} {}\n  {} {} {}\n",
            self.date,
            "AgentTokenIssuance",
            self.narration.replace('"', "'"),
            self.tx_id,
            self.source_ref.replace('"', "'"),
            self.asset_account,
            self.amount,
            self.currency,
            self.counterparty_account,
            inverse,
            self.currency
        )
    }
}

fn invert_amount(amount: &str) -> String {
    let trimmed = amount.trim();
    if let Some(rest) = trimmed.strip_prefix('-') {
        rest.to_string()
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        format!("-{rest}")
    } else {
        format!("-{trimmed}")
    }
}

fn append_journal_entry(path: &std::path::Path, entry: &AgentTokenJournalEntry) -> Result<()> {
    use std::io::Write as _;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create ledger directory")?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open ledger file {}", path.display()))?;
    file.write_all(entry.to_beancount_entry().as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .with_context(|| format!("append to ledger file {}", path.display()))
}

// ---------------------------------------------------------------------------
// Component 1: issuance flow
// ---------------------------------------------------------------------------

/// Full issuance flow.
///
/// **Fail-before-privilege ordering is load-bearing**: the cake balance
/// check happens strictly before any k8s client is constructed or any k8s
/// API call is made.
pub async fn request_agent_token(req: AgentTokenRequest) -> Result<AgentTokenIssuance> {
    anyhow::ensure!(req.cost >= 0, "cost must be non-negative, got {}", req.cost);

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

    // --- 4. Ensure the marker ClusterRole exists (shared across all kinds). ---
    ensure_role_shard_access(&client).await?;

    // --- 5. Ensure the scope-labeled RoleBinding. ---
    ensure_role_binding(&client, AGENTS_NAMESPACE, &sa_name, &req.scope).await?;

    // --- 6. Mint a short-lived, scoped token. ---
    let token = mint_token(&client, AGENTS_NAMESPACE, &sa_name).await?;

    // --- 7. Record the issuance as a ledger transaction; debit cake. ---
    let shard_ref = req.scope.to_string();
    let tx_id = format!("agent-token-{}", uuid::Uuid::new_v4());
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let entry = AgentTokenJournalEntry::new(
        &req.agent_id,
        &shard_ref,
        &req.cost.to_string(),
        tx_id.clone(),
        date,
    );
    let ledger_path = default_ledger_path()?;
    append_journal_entry(&ledger_path, &entry).context("append agent-token journal entry")?;

    let remaining_balance = ledger
        .spend(&req.agent_id, req.cost, &format!("agent-token:{shard_ref}"))
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

fn role_binding_name(sa_name: &str, scope: &SoulScope) -> String {
    format!(
        "{}-shard-{}-{}",
        sa_name,
        scope.kind.as_str(),
        k8s_label_safe(&scope.id)
    )
}

/// Renders an arbitrary shard identifier safe for use as a k8s label value:
/// alphanumeric/`-`/`_`/`.` only, <=63 chars, starting and ending
/// alphanumeric (k8s label-value constraints).
fn k8s_label_safe(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect();
    out.truncate(63);
    while out.starts_with(|c: char| !c.is_ascii_alphanumeric()) {
        out.remove(0);
    }
    while out.ends_with(|c: char| !c.is_ascii_alphanumeric()) {
        out.pop();
    }
    if out.is_empty() { "x".to_string() } else { out }
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

async fn ensure_role_shard_access(client: &Client) -> Result<()> {
    let api: Api<ClusterRole> = Api::all(client.clone());
    if api.get(ROLE_SHARD_ACCESS).await.is_ok() {
        return Ok(());
    }
    let role: ClusterRole =
        serde_yaml::from_str(ROLE_SHARD_ACCESS_YAML).context("parse embedded role-shard-access YAML")?;
    match api.create(&PostParams::default(), &role).await {
        Ok(_) => Ok(()),
        Err(e) if is_conflict(&e) => Ok(()),
        Err(e) => Err(e).context("create role-shard-access ClusterRole"),
    }
}

async fn ensure_role_binding(
    client: &Client,
    namespace: &str,
    sa_name: &str,
    scope: &SoulScope,
) -> Result<()> {
    let api: Api<RoleBinding> = Api::namespaced(client.clone(), namespace);
    let rb_name = role_binding_name(sa_name, scope);
    if api.get(&rb_name).await.is_ok() {
        return Ok(());
    }
    let mut labels = BTreeMap::new();
    labels.insert(LABEL_SHARD_KIND.to_string(), scope.kind.as_str().to_string());
    labels.insert(LABEL_SHARD_ID.to_string(), k8s_label_safe(&scope.id));
    let rb = RoleBinding {
        metadata: ObjectMeta {
            name: Some(rb_name),
            namespace: Some(namespace.to_string()),
            labels: Some(labels),
            ..Default::default()
        },
        role_ref: RoleRef {
            api_group: "rbac.authorization.k8s.io".to_string(),
            kind: "ClusterRole".to_string(),
            name: ROLE_SHARD_ACCESS.to_string(),
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
        Err(e) => Err(e).context("create shard-access RoleBinding"),
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
// Enforcement: TokenReview-based
// ---------------------------------------------------------------------------

/// Validate `token` via k8s's TokenReview API, then confirm the resulting
/// ServiceAccount identity has a RoleBinding to [`ROLE_SHARD_ACCESS`] labeled
/// for `expected`'s exact kind + id. Returns a clear "not authorized for
/// this shard" error (distinct from a generic/connection error) on failure.
pub async fn authorize_shard_token(token: &str, expected: &SoulScope) -> Result<AuthorizedIdentity> {
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

    let bound = role_binding_exists_for(&client, &namespace, &sa_name, expected).await?;
    if !bound {
        bail!(
            "not authorized for this shard: ServiceAccount '{}/{}' has no role-shard-access RoleBinding for '{}'",
            namespace,
            sa_name,
            expected
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

async fn role_binding_exists_for(
    client: &Client,
    namespace: &str,
    sa_name: &str,
    scope: &SoulScope,
) -> Result<bool> {
    let api: Api<RoleBinding> = Api::namespaced(client.clone(), namespace);
    let list = api.list(&Default::default()).await.context("list RoleBindings")?;
    let want_id = k8s_label_safe(&scope.id);
    Ok(list.items.iter().any(|rb| {
        rb.role_ref.name == ROLE_SHARD_ACCESS
            && rb
                .subjects
                .as_ref()
                .is_some_and(|subs| subs.iter().any(|s| s.kind == "ServiceAccount" && s.name == sa_name))
            && rb.metadata.labels.as_ref().is_some_and(|labels| {
                labels.get(LABEL_SHARD_KIND).map(String::as_str) == Some(scope.kind.as_str())
                    && labels.get(LABEL_SHARD_ID) == Some(&want_id)
            })
    }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soul_scope::ShardKind;

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

    /// Pinned against ledger_core::journal's own
    /// `from_agent_token_issuance_sets_expected_fields` test — this local
    /// copy must produce identical field values.
    #[test]
    fn journal_entry_sets_expected_fields() {
        let entry = AgentTokenJournalEntry::new(
            "claude-worker-7",
            "datum:some-datum-id",
            "3",
            "tx-abc123".to_string(),
            "2026-08-22".to_string(),
        );
        assert_eq!(entry.date, "2026-08-22");
        assert_eq!(entry.narration, "agent token issued: datum:some-datum-id");
        assert_eq!(entry.asset_account, "Assets:Cake:claude-worker-7");
        assert_eq!(entry.counterparty_account, "Expenses:AgentTokens:datum");
        assert_eq!(entry.amount, "3");
        assert_eq!(entry.currency, "CAKE");
        assert_eq!(entry.tx_id, "tx-abc123");
        assert_eq!(entry.source_ref, "agent-token:claude-worker-7");
    }

    /// Pinned against ledger_core::journal's own
    /// `from_agent_token_issuance_shard_ref_without_colon_falls_back_whole`.
    #[test]
    fn journal_entry_shard_ref_without_colon_falls_back_whole() {
        let entry = AgentTokenJournalEntry::new(
            "agent-x",
            "datumonly",
            "1",
            "tx-1".to_string(),
            "2026-08-22".to_string(),
        );
        assert_eq!(entry.counterparty_account, "Expenses:AgentTokens:datumonly");
    }

    /// Pinned against ledger_core::journal's own
    /// `from_agent_token_issuance_produces_balanced_beancount_entry` — this
    /// local copy's on-disk format must be byte-identical, so the ledger
    /// stays format-compatible if a real `ledger-core` dependency becomes
    /// usable again later.
    #[test]
    fn journal_entry_produces_balanced_beancount_entry() {
        let entry = AgentTokenJournalEntry::new(
            "claude-worker-7",
            "datum:some-datum-id",
            "3",
            "tx-abc123".to_string(),
            "2026-08-22".to_string(),
        );
        let rendered = entry.to_beancount_entry();

        assert!(rendered.contains("Assets:Cake:claude-worker-7 3 CAKE"));
        assert!(rendered.contains("Expenses:AgentTokens:datum -3 CAKE"));
        assert_eq!(invert_amount(&entry.amount), "-3");
        assert_eq!(invert_amount(&invert_amount(&entry.amount)), entry.amount);
        assert!(rendered.contains("2026-08-22 * \"AgentTokenIssuance\""));
        assert!(rendered.contains("txid: \"tx-abc123\""));
        assert!(rendered.contains("source_ref: \"agent-token:claude-worker-7\""));
    }

    #[test]
    fn append_journal_entry_creates_parent_dir_and_appends() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested").join("agent-tokens.beancount");
        let entry = AgentTokenJournalEntry::new(
            "agent-y",
            "skill:b00t-learn",
            "2",
            "tx-2".to_string(),
            "2026-08-22".to_string(),
        );
        append_journal_entry(&path, &entry).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("tx-2"));
        assert!(contents.contains("Expenses:AgentTokens:skill"));
    }

    #[test]
    fn role_binding_name_is_per_agent_and_scope() {
        let scope_a = SoulScope::new(ShardKind::Datum, "foo");
        let scope_b = SoulScope::new(ShardKind::Agent, "foo");
        assert_ne!(
            role_binding_name("agent-x", &scope_a),
            role_binding_name("agent-x", &scope_b),
            "different shard kinds for the same id must not collide"
        );
    }

    #[test]
    fn k8s_label_safe_neutralizes_invalid_chars_and_trims_edges() {
        assert_eq!(k8s_label_safe("weird id/with:colons"), "weird-id-with-colons");
        assert_eq!(k8s_label_safe("-leading-and-trailing-"), "leading-and-trailing");
        assert_eq!(k8s_label_safe(""), "x");
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
    fn test_role_shard_access_yaml_parses() {
        let role: ClusterRole =
            serde_yaml::from_str(ROLE_SHARD_ACCESS_YAML).expect("embedded YAML must parse");
        assert_eq!(role.metadata.name.as_deref(), Some(ROLE_SHARD_ACCESS));
    }

    /// Fail-before-privilege: with an agent that has no seeded cake balance
    /// (0), the request must be denied with the budget error — without ever
    /// attempting a k8s API call. `CakeLedger::open_at` isolates this test
    /// to a private scratch DB, so no env-var mutation or cross-test
    /// locking is needed for this half of the flow.
    #[tokio::test]
    async fn test_insufficient_budget_denied_before_any_k8s_call() {
        // request_agent_token() itself calls CakeLedger::open() (the
        // fixed, real path), so we can't isolate via open_at() here — but
        // a freshly-generated, never-seen agent id is guaranteed a 0
        // balance in that real ledger regardless of other tests' state.
        let req = AgentTokenRequest {
            agent_id: format!("never-funded-{}", uuid::Uuid::new_v4()),
            scope: SoulScope::new(ShardKind::Datum, "some-datum-id"),
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

    /// Full positive flow against a real k8s cluster, exercised across two
    /// different shard kinds for the same agent to confirm per-kind
    /// RoleBinding isolation. `#[ignore]`d: constructing a k8s `Client` at
    /// all (even one that will fail to find a cluster) touches rustls'
    /// process-level `CryptoProvider`, which panics rather than erroring
    /// when it can't auto-select one — this must not run in a normal
    /// `cargo test`. Run explicitly with `cargo test -- --ignored` against
    /// a real cluster (set KUBECONFIG).
    #[tokio::test]
    #[ignore = "requires a live k8s cluster (set KUBECONFIG); run with cargo test -- --ignored"]
    async fn test_full_issuance_flow_against_live_cluster_if_reachable() {
        if live_cluster().await.is_none() {
            eprintln!(
                "SKIP test_full_issuance_flow_against_live_cluster_if_reachable: \
                 no k8s cluster reachable in this environment (set KUBECONFIG to \
                 test against a live cluster)"
            );
            return;
        }

        let agent_id = format!("test-agent-{}", uuid::Uuid::new_v4());
        let ledger = CakeLedger::open().expect("open cake ledger");
        ledger.mint(&agent_id, 10, "test seed").expect("seed balance");

        let issuance = request_agent_token(AgentTokenRequest {
            agent_id: agent_id.clone(),
            scope: SoulScope::new(ShardKind::Datum, "integration-test-datum"),
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

        // Positive half of enforcement: the freshly-minted token IS
        // authorized for the exact scope it was issued for...
        let scope = SoulScope::new(ShardKind::Datum, "integration-test-datum");
        let identity = authorize_shard_token(&issuance.token, &scope)
            .await
            .expect("freshly-minted token should be authorized for its own shard");
        assert_eq!(identity.namespace, AGENTS_NAMESPACE);
        assert_eq!(identity.service_account_name, service_account_name(&agent_id));

        // ...but NOT for a different shard kind/id the same agent hasn't
        // separately been issued a token for.
        let other_scope = SoulScope::new(ShardKind::Agent, "integration-test-datum");
        let denied = authorize_shard_token(&issuance.token, &other_scope).await;
        assert!(
            denied.is_err(),
            "a datum-scoped token must not authorize an agent-scoped request"
        );
    }

    /// A token for a ServiceAccount that has no `role-shard-access`
    /// RoleBinding at all must be denied with the specific "not authorized
    /// for this shard" error — not a crash, not a generic error.
    /// `#[ignore]`d for the same rustls `CryptoProvider` reason as above.
    #[tokio::test]
    #[ignore = "requires a live k8s cluster (set KUBECONFIG); run with cargo test -- --ignored"]
    async fn test_shard_authorization_denied_for_unbound_service_account() {
        let Some(client) = live_cluster().await else {
            eprintln!(
                "SKIP test_shard_authorization_denied_for_unbound_service_account: \
                 no k8s cluster reachable in this environment"
            );
            return;
        };

        // A bare ServiceAccount with NO RoleBinding to role-shard-access, in
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

        let scope = SoulScope::new(ShardKind::Datum, "anything");
        let err = authorize_shard_token(&token, &scope)
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
