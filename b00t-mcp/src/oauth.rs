use anyhow::{anyhow, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, Json, Redirect},
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{encode, EncodingKey, Header};
// OAuth 2.1 imports - minimal usage for PKCE validation
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

// OAuth 2.1 server configuration
#[derive(Clone)]
pub struct OAuthConfig {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub jwt_secret: Vec<u8>,
    pub token_lifetime: Duration,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            issuer: "https://b00t-mcp.example.com".to_string(),
            client_id: "b00t-mcp-client".to_string(),
            client_secret: "b00t-mcp-secret".to_string(),
            redirect_uri: "https://claude.ai/oauth/callback".to_string(),
            jwt_secret: b"your-256-bit-secret-key-change-me!".to_vec(),
            token_lifetime: Duration::from_secs(3600), // 1 hour
        }
    }
}

// Authorization server state
#[derive(Clone)]
pub struct OAuthState {
    pub config: OAuthConfig,
    pub sessions: Arc<RwLock<HashMap<String, AuthSession>>>,
    pub clients: Arc<RwLock<HashMap<String, RegisteredClient>>>,
}

// Client registration data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredClient {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    pub token_endpoint_auth_method: String,
    pub created_at: u64,
}

// Authorization session
#[derive(Debug, Clone)]
pub struct AuthSession {
    pub state: String,
    pub code_verifier: String, // Store as String instead of PkceCodeVerifier
    pub redirect_uri: String,
    pub client_id: String,
    pub scope: String,
    pub created_at: SystemTime,
}

// JWT claims for access tokens
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,        // Subject (user ID)
    pub aud: String,        // Audience (client ID)
    pub iss: String,        // Issuer
    pub exp: u64,           // Expiration time
    pub iat: u64,           // Issued at
    pub scope: String,      // Authorized scopes
    pub client_id: String,  // OAuth client ID
}

// OAuth 2.1 request/response types
#[derive(Debug, Deserialize)]
pub struct AuthorizeRequest {
    pub response_type: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: String,
    pub code_challenge_method: String,
}

#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub code_verifier: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OAuthTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub scope: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub error_description: String,
}

#[derive(Debug, Serialize)]
pub struct ClientRegistrationRequest {
    pub redirect_uris: Vec<String>,
    pub grant_types: Option<Vec<String>>,
    pub response_types: Option<Vec<String>>,
    pub token_endpoint_auth_method: Option<String>,
    pub client_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ClientRegistrationResponse {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    pub token_endpoint_auth_method: String,
    pub client_id_issued_at: u64,
}

impl OAuthState {
    pub fn new(config: OAuthConfig) -> Self {
        let mut clients = HashMap::new();
        
        // Register default client for Claude
        let default_client = RegisteredClient {
            client_id: config.client_id.clone(),
            client_secret: config.client_secret.clone(),
            redirect_uris: vec![config.redirect_uri.clone()],
            grant_types: vec!["authorization_code".to_string()],
            response_types: vec!["code".to_string()],
            token_endpoint_auth_method: "client_secret_post".to_string(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        
        clients.insert(config.client_id.clone(), default_client);
        
        Self {
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            clients: Arc::new(RwLock::new(clients)),
        }
    }

    pub fn generate_access_token(&self, client_id: &str, scope: &str, user_id: &str) -> Result<String> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let exp = now + self.config.token_lifetime.as_secs();

        let claims = TokenClaims {
            sub: user_id.to_string(),
            aud: client_id.to_string(),
            iss: self.config.issuer.clone(),
            exp,
            iat: now,
            scope: scope.to_string(),
            client_id: client_id.to_string(),
        };

        let header = Header::default();
        let key = EncodingKey::from_secret(&self.config.jwt_secret);
        
        encode(&header, &claims, &key)
            .map_err(|e| anyhow!("Failed to encode JWT: {}", e))
    }
}

// OAuth 2.1 endpoints
pub fn oauth_router(state: OAuthState) -> Router {
    Router::new()
        .route("/.well-known/oauth-authorization-server", get(discovery))
        .route("/oauth/register", post(register_client))
        .route("/oauth/authorize", get(authorize))
        .route("/oauth/token", post(token))
        .route("/oauth/consent", get(consent_form).post(consent_grant))
        .with_state(state)
}

// OAuth 2.1 Authorization Server Metadata (RFC 8414)
async fn discovery(State(state): State<OAuthState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "issuer": state.config.issuer,
        "authorization_endpoint": format!("{}/oauth/authorize", state.config.issuer),
        "token_endpoint": format!("{}/oauth/token", state.config.issuer),
        "registration_endpoint": format!("{}/oauth/register", state.config.issuer),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code"],
        "token_endpoint_auth_methods_supported": ["client_secret_post", "none"],
        "code_challenge_methods_supported": ["S256"],
        "scopes_supported": ["b00t:read", "b00t:write", "b00t:admin"],
        "response_modes_supported": ["query"],
    }))
}

// Dynamic Client Registration (RFC 7591)
#[axum::debug_handler]
async fn register_client(
    State(state): State<OAuthState>,
    Json(req): Json<ClientRegistrationRequest>,
) -> Result<Json<ClientRegistrationResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Generate client credentials
    let client_id = format!("b00t-{}", Uuid::new_v4().simple());
    let client_secret = URL_SAFE_NO_PAD.encode(Uuid::new_v4().as_bytes());
    
    let client = RegisteredClient {
        client_id: client_id.clone(),
        client_secret: client_secret.clone(),
        redirect_uris: req.redirect_uris.clone(),
        grant_types: req.grant_types.unwrap_or_else(|| vec!["authorization_code".to_string()]),
        response_types: req.response_types.unwrap_or_else(|| vec!["code".to_string()]),
        token_endpoint_auth_method: req.token_endpoint_auth_method
            .unwrap_or_else(|| "client_secret_post".to_string()),
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    // Store registered client
    state.clients.write().unwrap().insert(client_id.clone(), client.clone());

    Ok(Json(ClientRegistrationResponse {
        client_id: client.client_id,
        client_secret: client.client_secret,
        redirect_uris: client.redirect_uris,
        grant_types: client.grant_types,
        response_types: client.response_types,
        token_endpoint_auth_method: client.token_endpoint_auth_method,
        client_id_issued_at: client.created_at,
    }))
}

// Authorization endpoint
async fn authorize(
    State(state): State<OAuthState>,
    Query(req): Query<AuthorizeRequest>,
) -> Result<Redirect, (StatusCode, Json<ErrorResponse>)> {
    // Validate client
    let clients = state.clients.read().unwrap();
    let client = clients.get(&req.client_id)
        .ok_or_else(|| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_client".to_string(),
                error_description: "Unknown client_id".to_string(),
            }),
        ))?;

    // Validate redirect URI
    if !client.redirect_uris.contains(&req.redirect_uri) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_request".to_string(),
                error_description: "Invalid redirect_uri".to_string(),
            }),
        ));
    }

    // Validate PKCE
    if req.code_challenge_method != "S256" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_request".to_string(),
                error_description: "Only S256 code_challenge_method supported".to_string(),
            }),
        ));
    }

    // Create authorization session
    let session_id = Uuid::new_v4().to_string();
    let code_verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(&uuid::Uuid::new_v4().as_bytes());
    
    let session = AuthSession {
        state: req.state.unwrap_or_default(),
        code_verifier,
        redirect_uri: req.redirect_uri.clone(),
        client_id: req.client_id.clone(),
        scope: req.scope.unwrap_or_else(|| "b00t:read".to_string()),
        created_at: SystemTime::now(),
    };

    state.sessions.write().unwrap().insert(session_id.clone(), session);

    // Redirect to consent page
    Ok(Redirect::to(&format!("/oauth/consent?session_id={}", session_id)))
}

// Consent form
async fn consent_form(
    State(_state): State<OAuthState>,
    Query(params): Query<HashMap<String, String>>,
) -> Html<String> {
    let default_session = String::new();
    let session_id = params.get("session_id").unwrap_or(&default_session);
    
    let html = format!(r#"
<!DOCTYPE html>
<html>
<head>
    <title>b00t-mcp Authorization</title>
    <style>
        body {{ font-family: Arial, sans-serif; margin: 40px; }}
        .container {{ max-width: 500px; margin: 0 auto; }}
        .scope {{ margin: 10px 0; padding: 10px; background: #f5f5f5; }}
        button {{ padding: 10px 20px; margin: 10px; }}
        .allow {{ background: #4CAF50; color: white; border: none; }}
        .deny {{ background: #f44336; color: white; border: none; }}
    </style>
</head>
<body>
    <div class="container">
        <h2>🥾 b00t-mcp Authorization</h2>
        <p>Claude is requesting access to your b00t tools with the following permissions:</p>
        
        <div class="scope">
            <strong>b00t:read</strong> - Read tool status and configuration
        </div>
        <div class="scope">
            <strong>b00t:write</strong> - Execute b00t commands (safe operations)
        </div>
        
        <form method="post" action="/oauth/consent">
            <input type="hidden" name="session_id" value="{}" />
            <button type="submit" name="action" value="allow" class="allow">
                ✅ Allow Access
            </button>
            <button type="submit" name="action" value="deny" class="deny">
                ❌ Deny Access
            </button>
        </form>
        
        <p><small>🤓 This grants Claude temporary access to execute b00t commands on your behalf.</small></p>
    </div>
</body>
</html>
    "#, session_id);

    Html(html)
}

// Consent grant/deny
#[derive(Debug, Deserialize)]
struct ConsentForm {
    session_id: String,
    action: String,
}

async fn consent_grant(
    State(state): State<OAuthState>,
    axum::extract::Form(form): axum::extract::Form<ConsentForm>,
) -> Result<Redirect, (StatusCode, String)> {
    let mut sessions = state.sessions.write().unwrap();
    let session = sessions.remove(&form.session_id)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Invalid session".to_string()))?;

    if form.action == "deny" {
        let redirect_url = format!("{}?error=access_denied&state={}", 
                                 session.redirect_uri, session.state);
        return Ok(Redirect::to(&redirect_url));
    }

    // Generate authorization code
    let auth_code = URL_SAFE_NO_PAD.encode(Uuid::new_v4().as_bytes());
    
    // Store session for token exchange (reuse session_id as code)
    sessions.insert(auth_code.clone(), session.clone());

    // Redirect back to client with authorization code
    let redirect_url = format!("{}?code={}&state={}", 
                             session.redirect_uri, auth_code, session.state);
    
    Ok(Redirect::to(&redirect_url))
}

// Token endpoint
async fn token(
    State(state): State<OAuthState>,
    axum::extract::Form(req): axum::extract::Form<TokenRequest>,
) -> Result<Json<OAuthTokenResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate grant type
    if req.grant_type != "authorization_code" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "unsupported_grant_type".to_string(),
                error_description: "Only authorization_code grant supported".to_string(),
            }),
        ));
    }

    let code = req.code.as_ref().ok_or_else(|| (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "Missing authorization code".to_string(),
        }),
    ))?;

    // Retrieve and remove authorization session
    let mut sessions = state.sessions.write().unwrap();
    let session = sessions.remove(code)
        .ok_or_else(|| (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_grant".to_string(),
                error_description: "Invalid or expired authorization code".to_string(),
            }),
        ))?;

    // Validate client
    if session.client_id != req.client_id {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_client".to_string(),
                error_description: "Client ID mismatch".to_string(),
            }),
        ));
    }

    // Validate PKCE
    let code_verifier = req.code_verifier.as_ref().ok_or_else(|| (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "invalid_request".to_string(),
            error_description: "Missing code_verifier".to_string(),
        }),
    ))?;

    // Simple string comparison for code verifier validation
    // 🤓 In production, implement proper SHA256 challenge verification
    if session.code_verifier != *code_verifier {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_grant".to_string(),
                error_description: "Invalid code_verifier".to_string(),
            }),
        ));
    }

    // Generate access token
    let access_token = state.generate_access_token(
        &session.client_id,
        &session.scope,
        "default-user"  // 🤓 Single-user system for now
    ).map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "server_error".to_string(),
            error_description: "Failed to generate access token".to_string(),
        }),
    ))?;

    Ok(Json(OAuthTokenResponse {
        access_token,
        token_type: "Bearer".to_string(),
        expires_in: state.config.token_lifetime.as_secs(),
        scope: session.scope,
    }))
}