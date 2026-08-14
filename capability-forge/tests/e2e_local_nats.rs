//! End-to-end integration test against a REAL, ephemeral local `nats-server` process
//! (v2.14.5; resolved via the `NATS_SERVER_BIN` env var if set, otherwise `nats-server`
//! on `PATH` -- see `nats_server_bin()` below).
//!
//! This is the capstone test for the whole `capability-forge` plan: enroll an agent,
//! sign a capability request, mint a JWT through `CapabilityForge::handle_request` (not a
//! shortcut), connect to a real `nats-server` with that JWT, prove NATS itself enforces the
//! granted scope (not just that our own code decided not to send something), then revoke the
//! grant and prove a reconnect is refused.
//!
//! ## Real crate/binary findings that shaped this file
//!
//! `nats-jwt` 0.3.0 (verified directly against
//! `~/.cargo/registry/src/*/nats-jwt-0.3.0/src/lib.rs` -- no docs.rs page renders for it) has
//! **no operator-token support at all**: `Token<T: IntoNatsClaims>` is only implemented for
//! `User` and `Account`. Its own module doc says so explicitly: "operator JWTs are not
//! typically generated on the fly and so aren't supported". It also has **no revocation
//! support**: `NatsClaims::Account`'s fields are `limits`, `signing_keys`,
//! `default_permissions`, `version` -- there is no `revocations` field anywhere in the crate,
//! and no builder method to add one. Both gaps are real, not a version-lag documentation
//! problem -- the crate genuinely cannot produce either token shape. This file therefore
//! follows the plan's documented fallback for exactly those two cases: hand-build the JWT
//! claims JSON per the NATS JWT v2 wire format and sign it directly via `nkeys::KeyPair::sign`
//! (`sign_claims` below), replicating the exact header/base64url/segment-join shape
//! `nats_jwt::Token::sign` itself uses internally (confirmed by reading its source) so the
//! hand-rolled tokens are wire-compatible with the ones `nats-jwt` produces. `nats-jwt` is
//! still used, correctly, for the non-revoked account JWT in `NatsFixture::start()` and (via
//! `capability_forge::jwt_mint`, unchanged from Task 7) for every user JWT this test mints --
//! it only gets replaced for operator tokens and the post-revocation account update.
//!
//! Two more brief-vs-reality mismatches caught by reading the source instead of trusting the
//! plan's sample code: `Token::name()` is the real builder method (the plan's sample called a
//! nonexistent `set_name()`), and `Token::sign()` returns a bare `String`, not a `Result` (the
//! plan's sample called a nonexistent `.expect()` on it).
//!
//! `async-nats` 0.45's `ConnectOptions::credentials_file` (confirmed in
//! `async-nats-0.45.0/src/options.rs`) *is* genuinely `async fn(self, path) -> io::Result<Self>`,
//! so the plan's `.credentials_file(&creds).await.unwrap()` usage was correct as written.
//!
//! The disallowed-publish assertion is the one place this file diverges most from a naive
//! reading of the plan: `Client::publish()` only enqueues a PUB frame onto a local write
//! buffer and returns `Ok(())` immediately (confirmed in `async-nats-0.45.0/src/lib.rs`) --
//! nats-server's rejection of a permissions-violating publish arrives asynchronously as a
//! `-ERR 'Permissions Violation for Publish to ...'` protocol frame, which async-nats surfaces
//! only via `Event::ServerError` through an `event_callback` (or `client.flush()`'s own
//! ping-pong, but that only proves prior writes reached the wire, not that the server accepted
//! them -- it does not itself fail on an unrelated async `-ERR`). So proving NATS actually
//! rejected the disallowed publish requires registering an event callback *before* connecting
//! and asserting a `ServerError` naming a permissions violation arrives after the publish --
//! not inspecting `.publish()`'s or `.flush()`'s own return value, both of which are expected
//! to return `Ok` regardless.

use async_nats::Event;
use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
use b00t_c0re_gov::scope_store::ScopeId;
use capability_forge::enroll::enroll_agent;
use capability_forge::grant::revoke_grant;
use capability_forge::judge::FakeJudge;
use capability_forge::jwt_mint::skill_subject;
use capability_forge::request::{CapabilityReply, CapabilityRequest, SignedRequest};
use capability_forge::service::{handle_wire_request, CapabilityForge};
use futures::StreamExt;
use nats_jwt::Token;
use nkeys::KeyPair;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Resolves the `nats-server` binary to spawn for these tests: an explicit
/// `NATS_SERVER_BIN` override (for CI or any machine where it is not on `PATH`) if set,
/// otherwise the bare command name resolved via `PATH` -- as opposed to a single
/// hardcoded machine-specific path, which cannot run on CI or any other machine.
fn nats_server_bin() -> String {
    std::env::var("NATS_SERVER_BIN").unwrap_or_else(|_| "nats-server".into())
}

struct NatsFixture {
    child: Child,
    port: u16,
    account_pubkey: String,
    account_signing_key: KeyPair,
    /// Kept so `reload()` can re-sign an updated account JWT under the same operator identity
    /// -- an account JWT's `iss` must match the operator's public key for the trust chain
    /// nats-server validates on load to hold, so reload cannot mint from a fresh operator.
    operator: KeyPair,
    /// The operator JWT never changes across a reload (only the account JWT does), so it's
    /// built once in `start()` and reused verbatim by `render_config` on every reload.
    operator_jwt: String,
    config_path: std::path::PathBuf,
    _tempdir: tempfile::TempDir,
}

impl NatsFixture {
    fn start() -> Self {
        let tempdir = tempfile::tempdir().unwrap();
        let port = pick_free_port();

        let operator = KeyPair::new_operator();
        let account_signing_key = KeyPair::new_account();
        let account_pubkey = account_signing_key.public_key();

        let operator_jwt = build_operator_jwt(&operator);
        // `nats-jwt` genuinely can do this half: a plain (non-revoked) account token signed by
        // the operator is exactly the shape `Token::new_account(..).sign(..)` produces.
        let account_jwt = Token::new_account(&account_pubkey).name("capforge-test").sign(&operator);

        let config_path = tempdir.path().join("nats.conf");
        std::fs::write(&config_path, render_config(port, &operator_jwt, &account_pubkey, &account_jwt)).unwrap();

        let bin = nats_server_bin();
        let mut child = Command::new(&bin)
            .arg("-c")
            .arg(&config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("spawn nats-server (resolved as {bin:?}, override with NATS_SERVER_BIN): {e}"));

        wait_for_port_or_panic(&mut child, port);

        Self {
            child,
            port,
            account_pubkey,
            account_signing_key,
            operator,
            operator_jwt,
            config_path,
            _tempdir: tempdir,
        }
    }

    fn url(&self) -> String {
        format!("nats://127.0.0.1:{}", self.port)
    }

    fn reload(&self, new_account_jwt: &str) {
        let config = render_config(self.port, &self.operator_jwt, &self.account_pubkey, new_account_jwt);
        std::fs::write(&self.config_path, &config).unwrap();

        // `--signal reload=<pid>` is nats-server's own CLI-driven equivalent of `kill -HUP
        // <pid>`: a short-lived second invocation of the binary that resolves <pid> (accepts
        // either a bare PID or a path to a pidfile -- `child.id()` gives a bare PID) and sends
        // the OS signal, then exits; it is not a second long-running server.
        let status = Command::new(nats_server_bin())
            .arg("--signal")
            .arg(format!("reload={}", self.child.id()))
            .status()
            .expect("send reload signal");
        assert!(status.success(), "nats-server --signal reload exited non-zero: {status:?}");

        // The MEMORY resolver re-reads `resolver_preload` on config reload, but that happens
        // on the server's own event loop -- give it a moment to actually apply before a test
        // tries to (re)connect against the new state.
        std::thread::sleep(Duration::from_millis(300));
    }
}

impl Drop for NatsFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn pick_free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

/// Renders the exact same config shape for both the initial start and every reload, so the
/// two never drift apart from each other except in the one field (`account_jwt`) that's
/// actually meant to change.
fn render_config(port: u16, operator_jwt: &str, account_pubkey: &str, account_jwt: &str) -> String {
    format!(
        r#"
port: {port}
operator: {operator_jwt:?}
resolver: MEMORY
resolver_preload: {{
  {account_pubkey}: {account_jwt:?}
}}
"#
    )
}

/// Polls the port instead of a fixed sleep so this fails fast (with the server's own captured
/// stderr, which names exactly which config key is wrong) rather than always waiting out a
/// fixed timeout, and doesn't flake on a slow box either.
fn wait_for_port_or_panic(child: &mut Child, port: u16) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        if let Ok(Some(status)) = child.try_wait() {
            let mut stderr = String::new();
            if let Some(mut s) = child.stderr.take() {
                let _ = s.read_to_string(&mut stderr);
            }
            let mut stdout = String::new();
            if let Some(mut o) = child.stdout.take() {
                let _ = o.read_to_string(&mut stdout);
            }
            panic!(
                "nats-server exited early with {status:?} before opening port {port}\n--- stderr ---\n{stderr}\n--- stdout ---\n{stdout}"
            );
        }
        if std::time::Instant::now() > deadline {
            panic!("nats-server did not open port {port} within 10s");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Signs an arbitrary claims JSON value into a three-segment NATS JWT v2 string, replicating
/// `nats_jwt::Token::sign`'s own header/base64url/signature construction exactly (confirmed
/// against its source) so tokens built here are wire-compatible with ones `nats-jwt` produces.
/// Used only for the two token shapes `nats-jwt` 0.3.0 cannot build at all: operator tokens
/// and account tokens carrying a `revocations` entry.
fn sign_claims(claims: &serde_json::Value, signing_key: &KeyPair) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    const JWT_HEADER: &str = r#"{"typ":"JWT","alg":"ed25519-nkey"}"#;

    let claims_str = serde_json::to_string(claims).expect("claims json serialization cannot fail");
    let b64_header = URL_SAFE_NO_PAD.encode(JWT_HEADER.as_bytes());
    let b64_body = URL_SAFE_NO_PAD.encode(claims_str.as_bytes());
    let jwt_half = format!("{b64_header}.{b64_body}");
    let sig = signing_key.sign(jwt_half.as_bytes()).expect("nkeys sign should not fail for a valid keypair");
    let b64_sig = URL_SAFE_NO_PAD.encode(&sig);
    format!("{jwt_half}.{b64_sig}")
}

/// Hand-built self-signed operator JWT (`iss == sub == operator's own public key`) -- the
/// simplest possible NATS trust root: no additional operator signing keys, so every account
/// JWT below is signed directly with the operator's own identity key. `jti` here is a random
/// id rather than the sha256-of-claims hash `nats-jwt` computes for its own tokens: nats-server
/// verifies the *signature* over the token, not that `jti` matches some recomputed hash, so
/// the hash's specific derivation is cosmetic and not worth a `sha2` dependency to reproduce.
fn build_operator_jwt(operator: &KeyPair) -> String {
    let pubkey = operator.public_key();
    let claims = serde_json::json!({
        "iat": chrono::Utc::now().timestamp(),
        "iss": pubkey,
        "jti": uuid::Uuid::new_v4().to_string(),
        "sub": pubkey,
        "name": pubkey,
        "nats": { "type": "operator", "version": 2 },
    });
    sign_claims(&claims, operator)
}

/// Hand-built account JWT carrying an optional revocation entry (`nats-jwt` cannot represent
/// this field at all -- see the module doc). Mirrors the exact `nats.limits` /
/// `default_permissions` shape `nats_jwt::Token::new_account(..).sign(..)` itself produces
/// (verified from its source) so the revoked token nats-server loads on reload is shaped
/// identically to the one it already trusted, except for the added `revocations` map.
/// `revoked` is `(user_pubkey, unix_timestamp)`: NATS account-level revocation is keyed by the
/// *user's* public key (or `"*"` for all users), not by any jti -- any user JWT with `iat <=`
/// the timestamp is rejected at connect time.
fn build_account_jwt(account_pubkey: &str, operator: &KeyPair, revoked: Option<(&str, i64)>) -> String {
    let mut nats = serde_json::json!({
        "type": "account",
        "version": 2,
        "limits": {
            "subs": -1, "data": -1, "payload": -1,
            "imports": -1, "exports": -1, "wildcards": true,
            "conn": -1, "leaf": -1
        },
        "default_permissions": {}
    });
    if let Some((user_pubkey, ts)) = revoked {
        nats["revocations"] = serde_json::json!({ user_pubkey: ts });
    }
    let claims = serde_json::json!({
        "iat": chrono::Utc::now().timestamp(),
        "iss": operator.public_key(),
        "jti": uuid::Uuid::new_v4().to_string(),
        "sub": account_pubkey,
        "name": "capforge-test",
        "nats": nats,
    });
    sign_claims(&claims, operator)
}

#[test]
fn nats_server_starts_and_stops_cleanly() {
    let fixture = NatsFixture::start();
    assert!(fixture.url().starts_with("nats://127.0.0.1:"));
    drop(fixture);
}

#[tokio::test]
async fn full_flow_enroll_request_connect_scope_enforced_then_revoked() {
    let fixture = NatsFixture::start();

    let db_dir = tempfile::tempdir().unwrap();
    let mut store = RedbScopeStore::open(db_dir.path().join("capforge.redb"), ScopeId::Global, None).unwrap();

    let agent_kp = enroll_agent(&mut store, "agent-e2e", &["skill.read".to_string()]).unwrap();

    let judge = FakeJudge::always_grant();
    let mut forge = CapabilityForge {
        store: &mut store,
        judge: &judge,
        account_signing_key: &fixture.account_signing_key,
        account_pubkey: &fixture.account_pubkey,
        grant_ttl: chrono::Duration::minutes(30),
    };

    let signed = SignedRequest::sign(
        &agent_kp,
        CapabilityRequest {
            agent_id: "agent-e2e".into(),
            requested_skills: vec!["skill.read".into()],
            justification: "".into(),
        },
    )
    .unwrap();
    let reply = forge.handle_request(signed).await;
    assert!(reply.jwt.is_some(), "expected a minted jwt, got denials: {:?}", reply.denied);
    let jwt = reply.jwt.clone().unwrap();
    // The real `Grant.jti` this reply's mint persisted -- distinct from the NATS JWT's own
    // internal `jti` claim (see the `CapabilityReply::jti` doc comment). Needed below to
    // exercise capability-forge's own application-level revocation bookkeeping, separately
    // from the NATS-level account-JWT revocation that's what actually makes the reconnect fail.
    let grant_jti = reply.jti.clone().expect("a minted jwt implies a persisted Grant with a jti");

    let allowed_subject = skill_subject(&agent_kp.public_key(), "skill.read");
    let disallowed_subject = skill_subject(&agent_kp.public_key(), "skill.write");

    let creds = write_creds_file(&jwt, &agent_kp.seed().unwrap());

    // Captures async `Event::ServerError` notifications so the disallowed-publish assertion
    // below can prove NATS itself rejected it, not merely that our own client code chose not
    // to send it (see module doc for why `.publish()`'s and `.flush()`'s own return values
    // can't be used for that).
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<Event>(16);
    let client = async_nats::ConnectOptions::new()
        .event_callback(move |event| {
            let event_tx = event_tx.clone();
            async move {
                let _ = event_tx.send(event).await;
            }
        })
        .credentials_file(&creds)
        .await
        .unwrap()
        .connect(fixture.url())
        .await
        .expect("agent should connect with minted jwt");

    client.publish(allowed_subject.clone(), "hi".into()).await.expect("publish on granted subject should succeed");
    client.flush().await.expect("flush after an allowed publish must not itself error");

    // Drain whatever arrived from the allowed publish before moving on, so a stray event from
    // it can't be mistaken for the disallowed publish's rejection below.
    while tokio::time::timeout(Duration::from_millis(200), event_rx.recv()).await.is_ok() {}

    let publish_result = client.publish(disallowed_subject.clone(), "hi".into()).await;
    assert!(publish_result.is_ok(), "publish() itself only enqueues locally and should not fail synchronously");
    // `flush()`'s round-trip only proves the PUB frame reached the wire, not that the server
    // accepted it -- deliberately not asserted on here; the real proof is the ServerError.
    let _ = client.flush().await;

    let violation = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match event_rx.recv().await {
                Some(Event::ServerError(err)) => return Some(err),
                Some(_) => continue,
                None => return None,
            }
        }
    })
    .await;

    match violation {
        Ok(Some(err)) => {
            let text = err.to_string().to_lowercase();
            assert!(
                text.contains("permission"),
                "expected a permissions-violation server error for the disallowed publish, got: {err}"
            );
        }
        Ok(None) => panic!("event channel closed before a ServerError arrived for the disallowed publish"),
        Err(_) => panic!(
            "timed out waiting for nats-server to reject the disallowed publish on {disallowed_subject} — \
             no ServerError event arrived within 5s, meaning NATS did not enforce the scope"
        ),
    }

    // --- Revocation: application-level bookkeeping first, then the NATS-level enforcement ---

    revoke_grant(&mut store, &grant_jti).unwrap();

    let revoked_account_jwt = build_account_jwt(
        &fixture.account_pubkey,
        &fixture.operator,
        Some((&agent_kp.public_key(), chrono::Utc::now().timestamp())),
    );
    fixture.reload(&revoked_account_jwt);

    // A fresh connection attempt, not a reuse of `client` above: revocation must be proven
    // against a NEW auth handshake, since an already-established connection wouldn't
    // necessarily be torn down by a revocation the same way a fresh CONNECT would be refused.
    let reconnect = async_nats::ConnectOptions::new().credentials_file(&creds).await.unwrap().connect(fixture.url()).await;
    // A bare `is_err()` would accept *any* connect failure -- DNS, IO, timeout, or nats-server
    // simply failing to come back up after `reload()` -- none of which would prove revocation
    // specifically worked. `ConnectError::kind()` (confirmed in async-nats-0.45.0/src/lib.rs)
    // distinguishes `ConnectErrorKind::AuthorizationViolation` (the server rejected the JWT
    // itself) from every other failure mode, so asserting on the kind is what actually proves
    // the rejection came from the revocation, mirroring the same rigor already applied to the
    // disallowed-publish assertion above (checking the real error content, not just "an Err
    // happened").
    let reconnect_err = reconnect.expect_err("revoked agent should be refused on reconnect");
    assert_eq!(
        reconnect_err.kind(),
        async_nats::ConnectErrorKind::AuthorizationViolation,
        "expected reconnect to fail specifically with AuthorizationViolation (proving the revocation was what rejected it), got: {reconnect_err:?}"
    );

    drop(client);
}

/// Finding-6 coverage: every other test in this crate calls `CapabilityForge::handle_request`
/// directly (a library-level shortcut), so nothing previously exercised the actual production
/// path -- a `SignedRequest` serialized to JSON, published to `capability.request.*`, received
/// by `handle_wire_request` (the exact function `bin/main.rs`'s subscribe loop calls per
/// message), deserialized, handled, and replied. This test drives exactly that, against a real
/// NATS connection from `NatsFixture`, proving the wire format round-trips through the real
/// subscribe/reply mechanics -- not a second copy of the full-flow test's scenario coverage
/// above, just the one happy path through the actual wire boundary.
#[tokio::test]
async fn wire_request_round_trips_through_publish_subscribe_reply() {
    let fixture = NatsFixture::start();

    let db_dir = tempfile::tempdir().unwrap();
    let mut store = RedbScopeStore::open(db_dir.path().join("capforge.redb"), ScopeId::Global, None).unwrap();
    let agent_kp = enroll_agent(&mut store, "agent-wire", &["skill.read".to_string()]).unwrap();

    let judge = FakeJudge::always_grant();

    // `NatsFixture`'s server requires every connection to present a valid user JWT (it's the
    // same operator/account trust chain the full-flow test above uses to prove skill-scoped
    // enforcement) -- there is no anonymous access. The service and requester connections
    // here are exercising the wire *mechanics* of the capability.request.* control-plane
    // channel, not the skill-scoped grant it hands out, so each gets an unrestricted user JWT
    // (no `.allow_publish`/`.allow_subscribe` calls -- per `jwt_mint.rs`'s own doc comment, an
    // absent NatsPermissions block means unrestricted access) rather than a `mint_user_jwt`
    // skill-scoped one.
    let service_creds = mint_unrestricted_user_creds(&fixture);
    let requester_creds = mint_unrestricted_user_creds(&fixture);

    // Service-side connection: subscribes on the real production subject, mirroring
    // bin/main.rs's `client.subscribe("capability.request.*")`.
    let service_client =
        async_nats::ConnectOptions::new().credentials_file(&service_creds).await.unwrap().connect(fixture.url()).await.unwrap();
    let mut sub = service_client.subscribe("capability.request.*").await.unwrap();

    let signed = SignedRequest::sign(
        &agent_kp,
        CapabilityRequest {
            agent_id: "agent-wire".into(),
            requested_skills: vec!["skill.read".into()],
            justification: "".into(),
        },
    )
    .unwrap();
    let payload = serde_json::to_vec(&signed).unwrap();

    // Requester-side connection, separate from the service connection above -- a real agent
    // and the service are distinct NATS clients. `Client::request` is a request-reply call
    // that only resolves once a reply is published to the subject it generates, so it's
    // spawned to run concurrently with the service side actually answering it below.
    let requester_client =
        async_nats::ConnectOptions::new().credentials_file(&requester_creds).await.unwrap().connect(fixture.url()).await.unwrap();
    let reply_handle = tokio::spawn(async move {
        requester_client.request("capability.request.agent-wire".to_string(), payload.into()).await
    });

    // Drive exactly one message through the real wire-handling function -- the same one
    // bin/main.rs's subscribe loop calls per message, not a call back into handle_request.
    let msg = sub.next().await.expect("service should receive the published request");
    let reply_subject = msg.reply.clone().expect("Client::request sets a reply subject");
    let mut forge = CapabilityForge {
        store: &mut store,
        judge: &judge,
        account_signing_key: &fixture.account_signing_key,
        account_pubkey: &fixture.account_pubkey,
        grant_ttl: chrono::Duration::minutes(30),
    };
    handle_wire_request(&mut forge, &service_client, reply_subject, &msg.payload).await;

    let reply_msg = reply_handle.await.unwrap().expect("requester should receive a reply");
    let capability_reply: CapabilityReply = serde_json::from_slice(&reply_msg.payload).unwrap();
    assert!(capability_reply.jwt.is_some(), "expected a minted jwt, got denials: {:?}", capability_reply.denied);
    assert_eq!(capability_reply.granted, vec!["skill.read".to_string()]);
}

/// Mints a user JWT under the fixture's account with no `.allow_publish`/`.allow_subscribe`
/// calls at all -- an unrestricted user, distinct from `jwt_mint::mint_user_jwt`'s always
/// skill-scoped output. Used only for this test's internal service/requester plumbing
/// connections, which need to move `capability.request.*` control-plane traffic and aren't
/// themselves what's being proven here (the full-flow test above already proves skill-scoped
/// NATS enforcement).
fn mint_unrestricted_user_creds(fixture: &NatsFixture) -> std::path::PathBuf {
    let user_kp = KeyPair::new_user();
    let jwt = Token::new_user(&fixture.account_pubkey, user_kp.public_key()).sign(&fixture.account_signing_key);
    write_creds_file(&jwt, &user_kp.seed().unwrap())
}

fn write_creds_file(jwt: &str, seed: &str) -> std::path::PathBuf {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("agent.creds");
    let mut f = std::fs::File::create(&path).unwrap();
    write!(
        f,
        "-----BEGIN NATS USER JWT-----\n{jwt}\n------END NATS USER JWT------\n\n\
         -----BEGIN USER NKEY SEED-----\n{seed}\n------END USER NKEY SEED------\n"
    )
    .unwrap();
    // Intentional test-only leak: the creds file must outlive this function call (it's read
    // later by `credentials_file`), and the whole process is short-lived anyway.
    std::mem::forget(dir);
    path
}
