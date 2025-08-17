pub mod acl;
pub mod mcp_server_rusty;
// pub mod oauth;  // 🤓 Disabled complex OAuth until handler trait fixed
pub mod oauth_minimal;
pub mod github_auth;
pub mod params;
pub mod clap_reflection;
pub mod derive_mcp;
pub mod mcp_tools;
pub mod acp_hive;
pub mod acp_tools;

pub use acl::{AclConfig, AclFilter, Policy};
pub use mcp_server_rusty::B00tMcpServerRusty;
// pub use oauth::{OAuthConfig, OAuthState, oauth_router};  // 🤓 Disabled
pub use oauth_minimal::{MinimalOAuthConfig, MinimalOAuthState, minimal_oauth_router};
pub use github_auth::{GitHubAuthConfig, GitHubAuthState, github_auth_router, GitHubUser};
pub use params::*;
pub use acp_hive::{AcpHiveClient, HiveMission, AgentStatus, HiveStatus};
pub use acp_tools::*;