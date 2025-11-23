pub mod acl;
pub mod mcp_server_rusty;
// pub mod oauth;  // 🤓 Disabled complex OAuth until handler trait fixed
// pub mod acp_hive;  // 🤓 Disabled - uses full NATS Agent from old ACP; chat refactor simplified to stubs
// pub mod acp_tools;  // 🤓 Disabled - depends on acp_hive
pub mod chat;
pub mod clap_reflection;
pub mod derive_mcp;
pub mod github_auth;
pub mod mcp_registry_tools;
pub mod mcp_tools;
pub mod oauth_minimal;
pub mod params;
pub mod proxy_mcp_tools;
pub mod rag_mcp_tools;

pub use acl::{AclConfig, AclFilter, Policy};
pub use mcp_server_rusty::B00tMcpServerRusty;
// pub use oauth::{OAuthConfig, OAuthState, oauth_router};  // 🤓 Disabled
pub use chat::ChatRuntime;
pub use github_auth::{GitHubAuthConfig, GitHubAuthState, GitHubUser, github_auth_router};
pub use mcp_registry_tools::*;
pub use oauth_minimal::{MinimalOAuthConfig, MinimalOAuthState, minimal_oauth_router};
pub use params::*;
pub use proxy_mcp_tools::*;
pub use rag_mcp_tools::*;

// Re-export from b00t-c0re-lib
pub use b00t_c0re_lib::{
    DocumentSource, GenericMcpProxy, LoaderType, McpRegistry, McpServerConfig,
    McpServerRegistration, McpToolDefinition, McpToolRequest, McpToolResponse, RagLightConfig,
    RagLightManager, create_registration_from_datum,
};
