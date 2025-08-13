use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, Redirect},
    routing::get,
    Router,
};
use reqwest;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

// GitHub OAuth configuration
#[derive(Clone)]
pub struct GitHubAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub base_url: String,
}

impl Default for GitHubAuthConfig {
    fn default() -> Self {
        Self {
            client_id: std::env::var("GITHUB_CLIENT_ID")
                .unwrap_or_else(|_| "your-github-client-id".to_string()),
            client_secret: std::env::var("GITHUB_CLIENT_SECRET")
                .unwrap_or_else(|_| "your-github-client-secret".to_string()),
            redirect_uri: "http://127.0.0.1:8080/auth/github/callback".to_string(),
            base_url: "http://127.0.0.1:8080".to_string(),
        }
    }
}

// User session after GitHub authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub id: u64,
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: String,
    pub authenticated_at: u64,
}

// GitHub OAuth state
#[derive(Clone)]
pub struct GitHubAuthState {
    pub config: GitHubAuthConfig,
    pub sessions: Arc<RwLock<HashMap<String, GitHubUser>>>, // session_id -> user
    pub oauth_states: Arc<RwLock<HashMap<String, String>>>, // oauth_state -> return_url
}

impl GitHubAuthState {
    pub fn new(config: GitHubAuthConfig) -> Self {
        Self {
            config,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            oauth_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_user_from_session(&self, session_id: &str) -> Option<GitHubUser> {
        self.sessions.read().unwrap().get(session_id).cloned()
    }

    pub fn create_user_session(&self, user: GitHubUser) -> String {
        let session_id = Uuid::new_v4().to_string();
        self.sessions.write().unwrap().insert(session_id.clone(), user);
        session_id
    }
}

// GitHub OAuth response types
#[derive(Debug, Deserialize)]
pub struct GitHubCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubTokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    #[allow(dead_code)]
    scope: String,
}

#[derive(Debug, Deserialize)]
struct GitHubUserResponse {
    id: u64,
    login: String,
    name: Option<String>,
    email: Option<String>,
    avatar_url: String,
}

// GitHub auth router
pub fn github_auth_router(state: GitHubAuthState) -> Router {
    Router::new()
        .route("/auth/github", get(github_login))
        .route("/auth/github/callback", get(github_callback))
        .route("/auth/logout", get(logout))
        .with_state(state)
}

// GitHub OAuth login initiation
async fn github_login(
    State(state): State<GitHubAuthState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Redirect, (StatusCode, String)> {
    // Generate OAuth state parameter
    let oauth_state = Uuid::new_v4().to_string();
    
    // Store return URL for after authentication
    let return_url = params.get("return_to")
        .unwrap_or(&format!("{}/oauth/consent", state.config.base_url))
        .to_string();
    
    state.oauth_states.write().unwrap().insert(oauth_state.clone(), return_url);

    // GitHub authorization URL
    let github_auth_url = format!(
        "https://github.com/login/oauth/authorize?client_id={}&redirect_uri={}&state={}&scope=user:email",
        urlencoding::encode(&state.config.client_id),
        urlencoding::encode(&state.config.redirect_uri),
        urlencoding::encode(&oauth_state)
    );

    Ok(Redirect::to(&github_auth_url))
}

// GitHub OAuth callback handler
async fn github_callback(
    State(state): State<GitHubAuthState>,
    Query(query): Query<GitHubCallbackQuery>,
) -> Result<Redirect, (StatusCode, String)> {
    // Handle OAuth errors
    if let Some(error) = query.error {
        let error_msg = query.error_description
            .unwrap_or_else(|| "GitHub OAuth error".to_string());
        return Err((StatusCode::BAD_REQUEST, format!("GitHub OAuth error: {} - {}", error, error_msg)));
    }

    let code = query.code
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing authorization code".to_string()))?;

    let oauth_state = query.state
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "Missing state parameter".to_string()))?;

    // Retrieve return URL and remove OAuth state
    let return_url = {
        let mut states = state.oauth_states.write().unwrap();
        states.remove(&oauth_state)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "Invalid OAuth state".to_string()))?
    };

    // Exchange code for access token
    let client = reqwest::Client::new();
    let token_response = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .header("User-Agent", "b00t-mcp/1.0")
        .form(&[
            ("client_id", &state.config.client_id),
            ("client_secret", &state.config.client_secret),
            ("code", &code),
        ])
        .send()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to exchange code: {}", e)))?
        .json::<GitHubTokenResponse>()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to parse token response: {}", e)))?;

    // Get user info from GitHub API
    let user_response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("token {}", token_response.access_token))
        .header("User-Agent", "b00t-mcp/1.0")
        .send()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to get user info: {}", e)))?
        .json::<GitHubUserResponse>()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to parse user response: {}", e)))?;

    // Create user session
    let github_user = GitHubUser {
        id: user_response.id,
        login: user_response.login,
        name: user_response.name,
        email: user_response.email,
        avatar_url: user_response.avatar_url,
        authenticated_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
    };

    let session_id = state.create_user_session(github_user);

    // Redirect with session cookie
    let redirect_url = format!("{}?session={}", return_url, session_id);
    Ok(Redirect::to(&redirect_url))
}

// Logout handler
async fn logout(State(_state): State<GitHubAuthState>) -> Html<String> {
    let html = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Logged Out</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; text-align: center; }
        .container { max-width: 400px; margin: 0 auto; }
    </style>
</head>
<body>
    <div class="container">
        <h2>🥾 Logged Out</h2>
        <p>You have been successfully logged out of b00t-mcp.</p>
        <a href="/auth/github">Login Again</a>
    </div>
</body>
</html>
    "#;

    Html(html.to_string())
}

// Helper function to check if user is authenticated
pub async fn require_github_auth(
    state: &GitHubAuthState,
    session_id: Option<&str>,
) -> Result<GitHubUser, Redirect> {
    let session_id = session_id.ok_or_else(|| {
        Redirect::to("/auth/github")
    })?;

    state.get_user_from_session(session_id)
        .ok_or_else(|| Redirect::to("/auth/github"))
}

// Generate login URL with return path
pub fn github_login_url(base_url: &str, return_to: &str) -> String {
    format!("{}/auth/github?return_to={}", base_url, urlencoding::encode(return_to))
}