use anyhow::{Context, Result};
use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
use b00t_c0re_gov::scope_store::ScopeId;
use capability_forge::judge::{EscalationJudge, FakeJudge, OpenAiJudge};
use capability_forge::service::{handle_wire_request, CapabilityForge};
use futures::StreamExt;
use nkeys::KeyPair;
use std::env;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber_init();

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://127.0.0.1:4222".to_string());
    let db_path = env::var("CAPFORGE_DB_PATH").context("CAPFORGE_DB_PATH not set")?;
    let account_seed = env::var("CAPFORGE_ACCOUNT_SEED").context("CAPFORGE_ACCOUNT_SEED not set")?;
    let account_pubkey = env::var("CAPFORGE_ACCOUNT_PUBKEY").context("CAPFORGE_ACCOUNT_PUBKEY not set")?;
    let judge_model = env::var("CAPFORGE_JUDGE_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let mut store = RedbScopeStore::open(&db_path, ScopeId::Global, None)
        .with_context(|| format!("opening redb at {db_path}"))?;
    let account_signing_key = KeyPair::from_seed(&account_seed).context("invalid account seed")?;
    let judge = build_judge(select_judge_provider(), &judge_model);

    let client = connect_nats(&nats_url).await?;
    let mut sub = client.subscribe("capability.request.*").await.context("subscribing")?;

    tracing::info!("capability-forge listening on capability.request.*");

    while let Some(msg) = sub.next().await {
        let Some(reply_subject) = msg.reply.clone() else {
            tracing::warn!("request with no reply subject, dropping");
            continue;
        };

        // Per-message deserialize -> handle_request -> serialize -> reply logic (including
        // its log-and-continue error handling) lives in `handle_wire_request` so it can be
        // exercised directly in tests against a real NATS connection -- see
        // `capability-forge/tests/e2e_local_nats.rs`'s
        // `wire_request_round_trips_through_publish_subscribe_reply` test.
        let mut forge = CapabilityForge {
            store: &mut store,
            judge: judge.as_ref(),
            account_signing_key: &account_signing_key,
            account_pubkey: &account_pubkey,
            grant_ttl: chrono::Duration::minutes(30),
        };
        handle_wire_request(&mut forge, &client, reply_subject, &msg.payload).await;
    }

    Ok(())
}

/// Two supported NATS auth modes, tried in this order:
///
/// 1. Plain username/password (`CAPFORGE_NATS_USER`/`CAPFORGE_NATS_PASSWORD`)
///    — what every real deployment of this service actually needs today.
///    b00t-node's NATS server (see
///    `nats/pyinfra/templates/nats-pod-configured.yaml.j2`) runs simple
///    username/password auth, not operator/JWT — deliberately, per
///    `_b00t_/datums/PROVIDER-VULTR.provider.tomllmd`: "JWT's
///    resolver_preload never had a real app account, and the NSC signing
///    key that would be needed to add one isn't recoverable." No
///    `CAPFORGE_SERVICE_CREDS_FILE` can ever authenticate against a server
///    with no JWT resolver configured at all, regardless of how correct
///    the creds file itself is.
/// 2. JWT/creds-file (`CAPFORGE_SERVICE_CREDS_FILE`) — for a real
///    operator-mode NATS deployment, if this service is ever pointed at
///    one instead (this is what `tests/e2e_local_nats.rs`'s own private,
///    fully JWT-configured `nats-server` process exercises; the core
///    enroll/grant/judge/jwt_mint logic is unaffected either way — only
///    the transport connection differs).
///
/// Neither set: a clear error rather than the network-level connect
/// failure `.credentials_file()` used to produce when the file genuinely
/// couldn't be read (env var simply unset, not a real path problem).
async fn connect_nats(nats_url: &str) -> Result<async_nats::Client> {
    match (
        env::var("CAPFORGE_NATS_USER").ok(),
        env::var("CAPFORGE_NATS_PASSWORD").ok(),
    ) {
        (Some(user), Some(password)) => async_nats::ConnectOptions::new()
            .user_and_password(user, password)
            .connect(nats_url)
            .await
            .context("connecting to NATS (user/password auth)"),
        _ => {
            let service_creds_path = env::var("CAPFORGE_SERVICE_CREDS_FILE").context(
                "no NATS auth configured — set either CAPFORGE_NATS_USER +                  CAPFORGE_NATS_PASSWORD (plain auth, what b00t-node's NATS                  server actually runs) or CAPFORGE_SERVICE_CREDS_FILE                  (JWT/operator-mode auth)",
            )?;
            async_nats::ConnectOptions::new()
                .credentials_file(&service_creds_path)
                .await
                .with_context(|| format!("loading NATS creds from {service_creds_path}"))?
                .connect(nats_url)
                .await
                .context("connecting to NATS (JWT/creds-file auth)")
        }
    }
}

/// Which LLM backend `build_judge` should use, chosen purely from which
/// credential env vars are set — a pure function so the selection logic is
/// testable without any network access. See `build_judge` for how each
/// variant maps to an actual `EscalationJudge`.
#[derive(Debug, Clone, PartialEq)]
enum JudgeProvider {
    /// Telnyx Inference — an OpenAI-Chat-Completions-compatible endpoint
    /// (https://api.telnyx.com/v2/ai/chat/completions). Tried first: it's
    /// the one provider confirmed to actually work with a real credential
    /// found on this hive (see PROVIDER-VULTR.provider.tomllmd's sibling
    /// notes) — Cloudflare Workers AI was tried too and rejected the only
    /// available token (valid token, missing the Workers AI permission
    /// scope), and no OpenAI/OpenRouter key exists anywhere in the hive
    /// yet.
    Telnyx { api_key: String },
    /// OpenRouter — also OpenAI-Chat-Completions-compatible
    /// (https://openrouter.ai/api/v1), a broad model marketplace. No
    /// credential for this exists in the hive as of this writing; wired up
    /// for whenever one does.
    OpenRouter { api_key: String },
    /// Any self-hosted OpenAI-compatible server — e.g. b00t-candle-serve or
    /// b00t-mcp's own `--llm` gateway (see docs/superpowers on b00t-server
    /// dogfooding), or vLLM/llama.cpp directly. Same wire protocol as the
    /// two above, just pointed at a local URL with no real API key needed.
    LocalOpenAiCompatible { base_url: String },
    /// Real OpenAI — the original, unconditional default before this
    /// provider-selection existed. Lowest priority now only because no
    /// OPENAI_API_KEY has actually been found anywhere in this hive this
    /// session, not because it's a worse provider than the others.
    OpenAi,
    /// Nothing configured — `build_judge` returns a `FakeJudge` that
    /// always denies, matching the existing "the judge fails closed by
    /// design" behavior this file already documented before any of this
    /// provider selection existed.
    None,
}

fn select_judge_provider() -> JudgeProvider {
    if let Ok(api_key) = env::var("TELNYX_API_KEY") {
        return JudgeProvider::Telnyx { api_key };
    }
    if let Ok(api_key) = env::var("OPENROUTER_API_KEY") {
        return JudgeProvider::OpenRouter { api_key };
    }
    if let Ok(base_url) = env::var("CAPFORGE_JUDGE_LOCAL_URL") {
        return JudgeProvider::LocalOpenAiCompatible { base_url };
    }
    if env::var("OPENAI_API_KEY").is_ok() {
        return JudgeProvider::OpenAi;
    }
    JudgeProvider::None
}

fn build_judge(provider: JudgeProvider, judge_model: &str) -> Box<dyn EscalationJudge> {
    match provider {
        JudgeProvider::Telnyx { api_key } => Box::new(OpenAiJudge::with_base_url(
            "https://api.telnyx.com/v2/ai",
            api_key,
            env::var("CAPFORGE_JUDGE_MODEL_TELNYX")
                .unwrap_or_else(|_| "meta-llama/Meta-Llama-3.1-8B-Instruct".to_string()),
        )),
        JudgeProvider::OpenRouter { api_key } => Box::new(OpenAiJudge::with_base_url(
            "https://openrouter.ai/api/v1",
            api_key,
            judge_model.to_string(),
        )),
        JudgeProvider::LocalOpenAiCompatible { base_url } => Box::new(OpenAiJudge::with_base_url(
            base_url,
            // A local server behind CAPFORGE_JUDGE_LOCAL_URL is assumed to
            // need no real bearer token; async-openai still requires
            // *some* string here, so a placeholder is sent rather than
            // making the field optional throughout OpenAiJudge for a
            // one-provider edge case.
            "unused",
            judge_model.to_string(),
        )),
        JudgeProvider::OpenAi => Box::new(OpenAiJudge::new(judge_model.to_string())),
        JudgeProvider::None => Box::new(FakeJudge::always_deny(
            "no LLM judge provider configured (set TELNYX_API_KEY, OPENROUTER_API_KEY,              CAPFORGE_JUDGE_LOCAL_URL, or OPENAI_API_KEY) — escalatable-tier requests fail              closed until one is",
        )),
    }
}

fn tracing_subscriber_init() {
    let _ = tracing_subscriber::fmt::try_init();
}

#[cfg(test)]
mod connect_nats_tests {
    use super::*;

    // 🤓 These exercise connect_nats's MODE SELECTION (which branch runs,
    // and that a missing-both-modes error is clear) without needing a
    // real reachable NATS server — every branch here is expected to fail
    // to connect (no server listening on the bogus port), so what's under
    // test is which failure we get and why, proven via the error message.
    // A real connection round-trip through the plain-auth path is covered
    // live by b00t-historian's own smoke test pattern (see
    // vultr_delegate.rs / PR #1151) — this binary's connection code is now
    // the same async-nats API shape, just against a different port/user.

    #[tokio::test]
    async fn prefers_user_password_when_both_set() {
        // SAFETY: capability-forge's test binaries run single-threaded
        // enough in practice that these three tests don't interleave in a
        // way that matters (each fully unsets what it doesn't need before
        // asserting), but to be rigorous under any test-runner threading
        // model, serialize env mutation via a process-wide mutex.
        let _guard = env_mutex().lock().unwrap();
        unsafe {
            env::set_var("CAPFORGE_NATS_USER", "test-user");
            env::set_var("CAPFORGE_NATS_PASSWORD", "test-pass");
            env::remove_var("CAPFORGE_SERVICE_CREDS_FILE");
        }
        // Port 1 is reserved/unroutable — guaranteed connection failure,
        // fast, without needing a real server.
        let err = connect_nats("nats://127.0.0.1:1").await.unwrap_err();
        assert!(
            err.to_string().contains("user/password"),
            "expected the user/password branch's error context, got: {err}"
        );
    }

    #[tokio::test]
    async fn falls_back_to_creds_file_when_user_password_unset() {
        let _guard = env_mutex().lock().unwrap();
        unsafe {
            env::remove_var("CAPFORGE_NATS_USER");
            env::remove_var("CAPFORGE_NATS_PASSWORD");
            env::set_var("CAPFORGE_SERVICE_CREDS_FILE", "/nonexistent/path.creds");
        }
        let err = connect_nats("nats://127.0.0.1:1").await.unwrap_err();
        assert!(
            err.to_string().contains("loading NATS creds"),
            "expected the creds-file branch's error context, got: {err}"
        );
    }

    #[tokio::test]
    async fn errors_clearly_when_neither_auth_mode_configured() {
        let _guard = env_mutex().lock().unwrap();
        unsafe {
            env::remove_var("CAPFORGE_NATS_USER");
            env::remove_var("CAPFORGE_NATS_PASSWORD");
            env::remove_var("CAPFORGE_SERVICE_CREDS_FILE");
        }
        let err = connect_nats("nats://127.0.0.1:1").await.unwrap_err();
        assert!(
            err.to_string().contains("no NATS auth configured"),
            "expected the neither-mode error, got: {err}"
        );
    }

    fn env_mutex() -> &'static std::sync::Mutex<()> {
        static M: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        M.get_or_init(|| std::sync::Mutex::new(()))
    }
}

#[cfg(test)]
mod judge_provider_tests {
    use super::*;

    fn env_mutex() -> &'static std::sync::Mutex<()> {
        static M: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
        M.get_or_init(|| std::sync::Mutex::new(()))
    }

    fn clear_all() {
        unsafe {
            env::remove_var("TELNYX_API_KEY");
            env::remove_var("OPENROUTER_API_KEY");
            env::remove_var("CAPFORGE_JUDGE_LOCAL_URL");
            env::remove_var("OPENAI_API_KEY");
        }
    }

    #[test]
    fn prefers_telnyx_over_everything_else() {
        let _guard = env_mutex().lock().unwrap();
        clear_all();
        unsafe {
            env::set_var("TELNYX_API_KEY", "telnyx-key");
            env::set_var("OPENROUTER_API_KEY", "openrouter-key");
            env::set_var("CAPFORGE_JUDGE_LOCAL_URL", "http://127.0.0.1:8181");
            env::set_var("OPENAI_API_KEY", "openai-key");
        }
        assert_eq!(
            select_judge_provider(),
            JudgeProvider::Telnyx { api_key: "telnyx-key".to_string() }
        );
        clear_all();
    }

    #[test]
    fn falls_back_to_openrouter_when_telnyx_unset() {
        let _guard = env_mutex().lock().unwrap();
        clear_all();
        unsafe {
            env::set_var("OPENROUTER_API_KEY", "openrouter-key");
            env::set_var("OPENAI_API_KEY", "openai-key");
        }
        assert_eq!(
            select_judge_provider(),
            JudgeProvider::OpenRouter { api_key: "openrouter-key".to_string() }
        );
        clear_all();
    }

    #[test]
    fn falls_back_to_local_when_telnyx_and_openrouter_unset() {
        let _guard = env_mutex().lock().unwrap();
        clear_all();
        unsafe {
            env::set_var("CAPFORGE_JUDGE_LOCAL_URL", "http://127.0.0.1:8181");
            env::set_var("OPENAI_API_KEY", "openai-key");
        }
        assert_eq!(
            select_judge_provider(),
            JudgeProvider::LocalOpenAiCompatible {
                base_url: "http://127.0.0.1:8181".to_string()
            }
        );
        clear_all();
    }

    #[test]
    fn falls_back_to_openai_when_only_openai_key_set() {
        let _guard = env_mutex().lock().unwrap();
        clear_all();
        unsafe {
            env::set_var("OPENAI_API_KEY", "openai-key");
        }
        assert_eq!(select_judge_provider(), JudgeProvider::OpenAi);
        clear_all();
    }

    #[test]
    fn none_when_nothing_configured() {
        let _guard = env_mutex().lock().unwrap();
        clear_all();
        assert_eq!(select_judge_provider(), JudgeProvider::None);
    }

    // build_judge itself just constructs a trait object per provider — no
    // network call happens at construction time for any variant (confirmed
    // by reading OpenAiJudge::new/with_base_url and FakeJudge::always_deny,
    // none of which touch the network), so these just confirm it doesn't
    // panic for each provider shape.
    #[test]
    fn build_judge_constructs_without_panicking_for_every_provider() {
        let _ = build_judge(JudgeProvider::None, "unused-model");
        let _ = build_judge(JudgeProvider::OpenAi, "unused-model");
        let _ = build_judge(
            JudgeProvider::Telnyx { api_key: "k".to_string() },
            "unused-model",
        );
        let _ = build_judge(
            JudgeProvider::OpenRouter { api_key: "k".to_string() },
            "unused-model",
        );
        let _ = build_judge(
            JudgeProvider::LocalOpenAiCompatible {
                base_url: "http://127.0.0.1:8181".to_string(),
            },
            "unused-model",
        );
    }
}
