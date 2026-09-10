//! `b00t identity` — obtain / inspect b00t.promptexecution.com agent JWTs.
//!
//! Env contract (also read by the b00t-mcp stdio path, SP3-02b):
//!   B00T_IDENTITY_URL   default `https://b00t.promptexecution.com`
//!   B00T_AGENT_JWT      a JWT for `identity whoami` to inspect
//!   B00T_IDENTITY_JWKS  inline JWKS JSON — if set, `whoami` verifies

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use b00t_c0re_identity::{
    AgentClaims, HttpTokenSource, TokenRequest, TokenSource, decode_claims_unverified, verify_jwt,
};

const DEFAULT_URL: &str = "https://b00t.promptexecution.com";

#[derive(clap::Parser, Clone)]
pub struct IdentityArgs {
    #[command(subcommand)]
    pub cmd: IdentityCmd,
}

#[derive(clap::Subcommand, Clone)]
pub enum IdentityCmd {
    /// Request a signed agent JWT from the identity worker.
    Token {
        #[clap(long)]
        tenant: String,
        #[clap(long)]
        node: String,
        #[clap(long, default_value = "member")]
        r0le: String,
        /// Comma-separated shard names.
        #[clap(long, value_delimiter = ',')]
        shards: Vec<String>,
        /// Print only the token (no file write, no notes).
        #[clap(long)]
        quiet: bool,
    },
    /// Decode `$B00T_AGENT_JWT` (verify if `$B00T_IDENTITY_JWKS` is set).
    Whoami {
        /// A JWT to inspect instead of `$B00T_AGENT_JWT`.
        #[clap(long)]
        jwt: Option<String>,
    },
    /// Print the identity worker's JWKS (`$B00T_IDENTITY_JWKS` or fetched).
    Jwks,
}

fn identity_url() -> String {
    std::env::var("B00T_IDENTITY_URL").unwrap_or_else(|_| DEFAULT_URL.to_string())
}

fn jwt_store_path(tenant: &str) -> Result<PathBuf> {
    anyhow::ensure!(
        !tenant.is_empty() && tenant != "." && tenant != ".." && !tenant.contains(['/', '\\']),
        "invalid tenant id for credential storage"
    );
    let home = dirs::home_dir().ok_or_else(|| anyhow!("no home dir"))?;
    Ok(home
        .join(".b00t")
        .join("identity")
        .join(format!("{tenant}.jwt")))
}

fn store_token(path: &std::path::Path, jwt: &str) -> Result<()> {
    use std::io::Write;
    let dir = path.parent().context("credential path has no parent")?;
    std::fs::create_dir_all(dir).context("create identity directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    // NamedTempFile is owner-only; atomic replacement also secures old files
    // without following a pre-existing symlink or modifying a shared hardlink.
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.write_all(jwt.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(path)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

/// Build the `POST /tokens` request body.
pub fn build_token_request(
    tenant: &str,
    node: &str,
    r0le: &str,
    shards: &[String],
) -> TokenRequest {
    TokenRequest {
        tenant_id: tenant.to_string(),
        agent_id: whoami_agent_id(),
        node_id: node.to_string(),
        r0le: r0le.to_string(),
        requested_shards: shards.to_vec(),
    }
}

/// Best-effort local agent id (`$B00T_AGENT_ID` / `$_B00T_Agent` / `$USER`).
fn whoami_agent_id() -> String {
    std::env::var("B00T_AGENT_ID")
        .or_else(|_| std::env::var("_B00T_Agent"))
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "agent/local".to_string())
}

/// Obtain a token from any `TokenSource` (HTTP in prod, mock in tests).
pub async fn fetch_token(src: &dyn TokenSource, req: &TokenRequest) -> Result<String> {
    src.obtain(req).await
}

/// Human-readable claims dump.
pub fn render_claims_table(claims: &AgentClaims, verified: bool) -> String {
    format!(
        "identity ({})\n  iss        : {}\n  sub        : {}\n  tenant     : {}\n  r0le       : {}\n  scopes     : {}\n  budget_ref : {}\n  iat/exp    : {} / {}  ({}s)\n  jti        : {}",
        if verified {
            "signature VERIFIED"
        } else {
            "unverified decode"
        },
        claims.iss,
        claims.sub,
        claims.tenant,
        claims.r0le,
        claims.scopes.join(", "),
        claims.budget_ref,
        claims.iat,
        claims.exp,
        claims.exp - claims.iat,
        claims.jti,
    )
}

async fn fetch_jwks(url_base: &str) -> Result<String> {
    let url = format!("{}/.well-known/jwks.json", url_base.trim_end_matches('/'));
    reqwest::get(&url)
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .context("JWKS endpoint error")?
        .text()
        .await
        .context("read JWKS body")
}

pub async fn handle_identity(args: &IdentityArgs) -> Result<()> {
    match &args.cmd {
        IdentityCmd::Token {
            tenant,
            node,
            r0le,
            shards,
            quiet,
        } => {
            let credential = std::env::var("B00T_IDENTITY_ISSUANCE_TOKEN")
                .context("set B00T_IDENTITY_ISSUANCE_TOKEN to the registry issuance credential")?;
            let src = HttpTokenSource::new(identity_url()).with_issuance_credential(credential);
            let req = build_token_request(tenant, node, r0le, shards);
            let jwt = fetch_token(&src, &req).await?;
            if *quiet {
                println!("{jwt}");
                return Ok(());
            }
            let path = jwt_store_path(tenant)?;
            store_token(&path, &jwt)?;
            println!("{jwt}");
            eprintln!("saved to {}", path.display());
            Ok(())
        }
        IdentityCmd::Whoami { jwt } => {
            let token = match jwt {
                Some(j) => j.clone(),
                None => std::env::var("B00T_AGENT_JWT")
                    .map_err(|_| anyhow!("no --jwt and $B00T_AGENT_JWT is unset"))?,
            };
            let (claims, verified) = match std::env::var("B00T_IDENTITY_JWKS") {
                Ok(jwks) => (verify_jwt(&token, &jwks).context("verify JWT")?, true),
                Err(_) => (
                    decode_claims_unverified(&token).context("decode JWT")?,
                    false,
                ),
            };
            println!("{}", render_claims_table(&claims, verified));
            Ok(())
        }
        IdentityCmd::Jwks => {
            let json = match std::env::var("B00T_IDENTITY_JWKS") {
                Ok(j) => j,
                Err(_) => fetch_jwks(&identity_url()).await?,
            };
            println!("{json}");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use b00t_c0re_identity::MockTokenSource;

    #[test]
    #[cfg(unix)]
    fn credential_storage_restricts_new_and_existing_files() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("identity/tenant.jwt");
        store_token(&path, "first").unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        store_token(&path, "second").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }

    #[test]
    fn tenant_cannot_escape_credential_directory() {
        assert!(jwt_store_path("../escape").is_err());
        assert!(jwt_store_path("/absolute").is_err());
    }

    #[test]
    fn build_token_request_shapes_the_body() {
        let r = build_token_request(
            "promptexecution",
            "root",
            "worker",
            &["project".to_string()],
        );
        assert_eq!(r.tenant_id, "promptexecution");
        assert_eq!(r.node_id, "root");
        assert_eq!(r.r0le, "worker");
        assert_eq!(r.requested_shards, ["project"]);
        assert!(!r.agent_id.is_empty());
    }

    #[tokio::test]
    async fn fetch_token_via_mock_then_whoami_decodes_it() {
        let src = MockTokenSource::new();
        let req = build_token_request(
            "promptexecution",
            "root",
            "worker",
            &["project".to_string()],
        );
        let jwt = fetch_token(&src, &req).await.unwrap();
        assert_eq!(jwt.split('.').count(), 3);

        let claims = decode_claims_unverified(&jwt).unwrap();
        assert_eq!(claims.tenant, "promptexecution");
        let table = render_claims_table(&claims, false);
        assert!(table.contains("promptexecution"));
        assert!(table.contains("unverified decode"));
    }

    #[tokio::test]
    async fn mock_token_verifies_against_its_own_jwks() {
        let src = MockTokenSource::new();
        let req = build_token_request("promptexecution", "root", "worker", &[]);
        let jwt = fetch_token(&src, &req).await.unwrap();
        let claims = verify_jwt(&jwt, &src.jwks_json()).unwrap();
        assert_eq!(
            render_claims_table(&claims, true).contains("VERIFIED"),
            true
        );
    }
}
