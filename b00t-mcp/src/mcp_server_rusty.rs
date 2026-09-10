use anyhow::Result;
use rmcp::{
    Peer,
    handler::server::ServerHandler,
    model::{
        Annotated,
        CallToolRequestParams,
        CallToolResult,
        Content,
        ErrorData as McpError,
        Extensions,
        Implementation,
        // Add resource support
        ListResourcesResult,
        ListToolsResult,
        PaginatedRequestParams,
        RawResource,
        ReadResourceRequestParams,
        ReadResourceResult,
        ResourceContents,
        ServerCapabilities,
        ServerInfo,
        ServerNotification,
        ToolListChangedNotification,
        ToolListChangedNotificationMethod,
    },
    service::{RequestContext, RoleServer},
};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, error, info};

use crate::clap_reflection::McpCommandRegistry;
use crate::{chat::ChatRuntime, mcp_tools::create_code_mode_registry};
use b00t_c0re_lib::{B00tContext, utils};

/// Rusty b00t MCP server with compile-time generated tools
///
/// This replaces the brittle dynamic approach with proper Rust trait-based
/// compile-time tool generation that dtolnay would approve of.
#[derive(Clone)]
pub struct B00tMcpServerRusty {
    working_dir: std::path::PathBuf,
    registry: McpCommandRegistry,
    chat_runtime: ChatRuntime,
    /// Captured from MCP initialize request — identifies the host client
    /// (e.g., "hermes", "claude-code", "opencode") for response customization.
    client_info: std::sync::Arc<std::sync::Mutex<Option<rmcp::model::Implementation>>>,
    /// Peer handle stored on_initialized; used to send tools/list_changed notifications
    /// when b00t_mcp_stack_load/unload dynamically changes the active tool set.
    notification_peer: std::sync::Arc<tokio::sync::Mutex<Option<Peer<RoleServer>>>>,
    /// SP3-02b — the verified caller for this (single-identity) stdio process.
    /// Lazily resolved from `pending_jwt` on first [`Self::caller`] call.
    caller: std::sync::Arc<std::sync::Mutex<Option<crate::identity::CallerIdentity>>>,
    /// Raw JWT awaiting verification — `$B00T_AGENT_JWT` at startup, or the
    /// `initialize.params.meta.b00t_jwt` override.
    pending_jwt: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    /// JWKS verifier (from `$B00T_IDENTITY_JWKS` / `$B00T_IDENTITY_URL`).
    identity_verifier: std::sync::Arc<crate::identity::JwksVerifier>,
    /// SP3-04 — resolves `caller.r0le` → tool allow-list for `list_tools`
    /// filtering. Default resolves nothing (non-anon callers get the full set
    /// until SP3-09 wires a real `DatumR0leResolver`).
    r0le_resolver: std::sync::Arc<dyn crate::r0le_resolver::R0leResolver>,
    /// SP3-06 — ledgrrr spend gate for `call_tool`. Default is an always-ok
    /// mock (SP3-09 wires `HttpSpendAuthorizer` when `B00T_LEDGRRR_MODE=http`).
    spend_authorizer: std::sync::Arc<dyn b00t_c0re_ledgrrr::SpendAuthorizer>,
    /// SP3-05 — skills learned this session. A gated tool stays locked until
    /// its unlocking skill is in here (populated by a successful `b00t_learn`).
    learned: std::sync::Arc<std::sync::RwLock<std::collections::HashSet<String>>>,
    /// SP3-07 — tools granted this session via `b00t_r0le_request_escalation`,
    /// merged into the `list_tools` allow-list. Session-scoped, never persisted,
    /// never widens the JWT.
    granted_extra: std::sync::Arc<std::sync::RwLock<Vec<String>>>,
    /// SP3-07 — escalation policy (`B00T_MCP_JUDGE`: `deny` default | `grant`).
    judge: std::sync::Arc<dyn capability_forge::judge::EscalationJudge>,
}

/// SP3-07 — pick the escalation policy from `$B00T_MCP_JUDGE`
/// (`grant` → always-grant; anything else → always-deny).
fn default_escalation_judge() -> std::sync::Arc<dyn capability_forge::judge::EscalationJudge> {
    match std::env::var("B00T_MCP_JUDGE").as_deref() {
        Ok("grant") => std::sync::Arc::new(capability_forge::judge::FakeJudge::always_grant()),
        _ => std::sync::Arc::new(capability_forge::judge::FakeJudge::always_deny(
            "escalation disabled (set B00T_MCP_JUDGE=grant)",
        )),
    }
}

/// The synthetic name of the runtime-escalation tool (not a registry command).
pub const ESCALATION_TOOL: &str = "b00t_r0le_request_escalation";

/// The `list_tools` entry for [`ESCALATION_TOOL`].
fn escalation_tool_def() -> rmcp::model::Tool {
    let mut t = rmcp::model::Tool::default();
    t.name = ESCALATION_TOOL.into();
    t.title = Some(ESCALATION_TOOL.to_string());
    t.description =
        Some("Request runtime escalation of tool access. Judged; grants are session-scoped and fire tools/list_changed.".into());
    let mut schema = serde_json::Map::new();
    schema.insert("type".to_string(), serde_json::json!("object"));
    schema.insert(
        "properties".to_string(),
        serde_json::json!({
            "tools": { "type": "array", "items": { "type": "string" } },
            "justification": { "type": "string" }
        }),
    );
    schema.insert(
        "required".to_string(),
        serde_json::json!(["tools", "justification"]),
    );
    t.input_schema = std::sync::Arc::new(schema);
    t
}

impl B00tMcpServerRusty {
    pub fn new<P: AsRef<Path>>(
        working_dir: P,
        _config_path: &str,
        code_mode: bool,
    ) -> Result<Self> {
        let working_dir = working_dir.as_ref().to_path_buf();

        // Build the peer Arc first so the notify closure can capture it before self exists.
        let notification_peer: std::sync::Arc<tokio::sync::Mutex<Option<Peer<RoleServer>>>> =
            std::sync::Arc::new(tokio::sync::Mutex::new(None));

        let notify_fn: std::sync::Arc<dyn Fn() + Send + Sync> = {
            let peer_arc = std::sync::Arc::clone(&notification_peer);
            std::sync::Arc::new(move || {
                let peer_arc = std::sync::Arc::clone(&peer_arc);
                tokio::spawn(async move {
                    let guard = peer_arc.lock().await;
                    if let Some(peer) = guard.as_ref() {
                        let notification = ToolListChangedNotification {
                            method: ToolListChangedNotificationMethod,
                            extensions: Extensions::new(),
                        };
                        let msg = ServerNotification::ToolListChangedNotification(notification);
                        if let Err(e) = peer.send_notification(msg).await {
                            tracing::warn!("tools/list_changed notification failed: {e}");
                        }
                    }
                });
            })
        };

        let registry = if code_mode {
            create_code_mode_registry()
        } else {
            crate::mcp_tools::create_mcp_registry_with_notify(notify_fn)
        };

        Ok(Self {
            working_dir,
            registry,
            chat_runtime: ChatRuntime::global(),
            client_info: std::sync::Arc::new(std::sync::Mutex::new(None)),
            notification_peer,
            caller: std::sync::Arc::new(std::sync::Mutex::new(None)),
            pending_jwt: std::sync::Arc::new(std::sync::Mutex::new(
                std::env::var("B00T_AGENT_JWT").ok().filter(|s| !s.is_empty()),
            )),
            identity_verifier: std::sync::Arc::new(crate::identity::JwksVerifier::from_env()),
            r0le_resolver: std::sync::Arc::new(
                crate::r0le_resolver::FixtureR0leResolver::new(),
            ),
            spend_authorizer: std::sync::Arc::new(
                b00t_c0re_ledgrrr::MockSpendAuthorizer::always_ok(),
            ),
            learned: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::HashSet::new(),
            )),
            granted_extra: std::sync::Arc::new(std::sync::RwLock::new(Vec::new())),
            judge: default_escalation_judge(),
        })
    }

    /// SP3-05 — the unlock gate for `r0le`, built from its blessing chain.
    /// Empty (gates nothing) when the r0le has no discoverable skills.
    pub(crate) fn unlock_gate_for(&self, r0le: &str) -> crate::unlock_gate::UnlockGate {
        let b00t_path = std::env::var("_B00T_Path").unwrap_or_else(|_| {
            self.working_dir.join("_b00t_").to_string_lossy().into_owned()
        });
        match b00t_cli::commands::blessing::collect_role_unlocks(&b00t_path, r0le) {
            Ok(manifest) => crate::unlock_gate::UnlockGate::from_manifest(&manifest),
            Err(_) => crate::unlock_gate::UnlockGate::default(),
        }
    }

    /// SP3-02b — the verified caller for this process. Lazily verifies
    /// `pending_jwt` (`$B00T_AGENT_JWT` or an `initialize` override) on first
    /// call and caches the result; `None` pending → `CallerIdentity::anon()`.
    pub async fn caller(&self) -> crate::identity::CallerIdentity {
        if let Some(id) = self.caller.lock().unwrap().clone() {
            return id;
        }
        let pending = self.pending_jwt.lock().unwrap().clone();
        let resolved = match pending {
            Some(jwt) => self
                .identity_verifier
                .verify(&jwt)
                .await
                .unwrap_or_else(|_| crate::identity::CallerIdentity::anon()),
            None => crate::identity::CallerIdentity::anon(),
        };
        *self.caller.lock().unwrap() = Some(resolved.clone());
        resolved
    }

    /// SP3-02b — override the pending JWT (from `initialize.params.meta.b00t_jwt`)
    /// and drop the cached identity so the next [`Self::caller`] re-resolves.
    pub fn set_pending_jwt(&self, jwt: impl Into<String>) {
        *self.pending_jwt.lock().unwrap() = Some(jwt.into());
        *self.caller.lock().unwrap() = None;
    }

    /// Test hook: swap in a pinned JWKS verifier.
    #[cfg(test)]
    pub fn with_test_verifier(self, verifier: crate::identity::JwksVerifier) -> Self {
        Self {
            identity_verifier: std::sync::Arc::new(verifier),
            ..self
        }
    }

    /// Test / SP3-09 hook: install the r0le → allow-list resolver.
    pub fn with_r0le_resolver(
        self,
        resolver: std::sync::Arc<dyn crate::r0le_resolver::R0leResolver>,
    ) -> Self {
        Self {
            r0le_resolver: resolver,
            ..self
        }
    }

    /// Test / SP3-09 hook: install the ledgrrr spend authorizer.
    pub fn with_spend_authorizer(
        self,
        authorizer: std::sync::Arc<dyn b00t_c0re_ledgrrr::SpendAuthorizer>,
    ) -> Self {
        Self {
            spend_authorizer: authorizer,
            ..self
        }
    }

    /// Test / SP3-09 hook: install the escalation judge.
    pub fn with_judge(
        self,
        judge: std::sync::Arc<dyn capability_forge::judge::EscalationJudge>,
    ) -> Self {
        Self { judge, ..self }
    }

    /// SP3-07 — `b00t_r0le_request_escalation` handler. Judges each requested
    /// tool; granted tools are added to `granted_extra` (session-scoped) and a
    /// `tools/list_changed` is fired. Returns `{granted:[], denied:[{tool,reason}]}`.
    async fn handle_escalation(
        &self,
        caller: &crate::identity::CallerIdentity,
        params: &HashMap<String, serde_json::Value>,
    ) -> CallToolResult {
        let tools: Vec<String> = params
            .get("tools")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let justification = params
            .get("justification")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mut granted: Vec<String> = Vec::new();
        let mut denied: Vec<serde_json::Value> = Vec::new();
        for tool in tools {
            match self
                .judge
                .judge(&caller.agent, &tool, "runtime tool escalation", justification)
                .await
            {
                capability_forge::judge::JudgeOutcome::Granted => granted.push(tool),
                capability_forge::judge::JudgeOutcome::Denied { reason } => {
                    denied.push(serde_json::json!({ "tool": tool, "reason": reason }))
                }
            }
        }

        if !granted.is_empty() {
            if let Ok(mut g) = self.granted_extra.write() {
                for t in &granted {
                    if !g.contains(t) {
                        g.push(t.clone());
                    }
                }
            }
            // reuse the same peer notification stack_load/unload uses
            self.notify_tools_changed();
        }

        self.create_success_result(
            &serde_json::json!({ "granted": granted, "denied": denied }).to_string(),
            "",
        )
    }

    /// SP3-06 — flat per-call spend gate. `anon` callers are not metered.
    /// Returns `Err` with JSON-RPC code -32004 when ledgrrr denies the spend.
    async fn authorize_call(
        &self,
        caller: &crate::identity::CallerIdentity,
        tool_name: &str,
    ) -> Result<(), McpError> {
        if caller.r0le == "anon" {
            return Ok(());
        }
        let idem = if caller.budget_ref.is_empty() {
            format!("{}:{}", caller.agent, tool_name)
        } else {
            caller.budget_ref.clone()
        };
        let resp = self
            .spend_authorizer
            .authorize_spend(&caller.tenant, &caller.agent, 1, &idem)
            .await
            .map_err(|e| McpError::internal_error(format!("ledgrrr: {e}"), None))?;
        if !resp.ok {
            return Err(McpError {
                code: rmcp::model::ErrorCode(-32004),
                message: format!(
                    "budget exceeded for tenant '{}'{}",
                    caller.tenant,
                    resp.reason.map(|r| format!(": {r}")).unwrap_or_default()
                )
                .into(),
                data: None,
            });
        }
        Ok(())
    }

    /// SP3-04 — keep only the tools `r0le` is allowed to see. `anon`, or an
    /// r0le the resolver can't resolve, passes everything through.
    pub(crate) fn filter_tools_for_r0le(
        &self,
        tools: Vec<rmcp::model::Tool>,
        r0le: &str,
    ) -> Vec<rmcp::model::Tool> {
        if r0le == "anon" {
            return tools;
        }
        match self.r0le_resolver.resolve(None, r0le) {
            Ok(resolved) => {
                // SP3-07 — session escalations widen the live allow-list.
                let mut allow = resolved.tool_allowlist;
                if let Ok(extra) = self.granted_extra.read() {
                    allow.extend(extra.iter().cloned());
                }
                let filter = crate::acl::AllowlistFilter::new(&allow);
                tools
                    .into_iter()
                    .filter(|t| filter.allows(t.name.as_ref()))
                    .collect()
            }
            Err(_) => tools,
        }
    }

    /// Convenience constructor for flat mode (backward compatible)
    pub fn new_flat<P: AsRef<Path>>(working_dir: P, config_path: &str) -> Result<Self> {
        Self::new(working_dir, config_path, false)
    }

    /// Convenience constructor for code mode
    pub fn new_code_mode<P: AsRef<Path>>(working_dir: P, config_path: &str) -> Result<Self> {
        Self::new(working_dir, config_path, true)
    }

    /// Get the number of available tools
    pub fn tool_count(&self) -> usize {
        self.registry.get_tools().len()
    }

    /// Send `notifications/tools/list_changed` to the connected client.
    ///
    /// Call after dynamically loading/unloading an MCP stack so the host (Claude,
    /// opencode, hermes) re-fetches the tool list without requiring reconnect.
    pub fn notify_tools_changed(&self) {
        let peer_arc = std::sync::Arc::clone(&self.notification_peer);
        tokio::spawn(async move {
            let guard = peer_arc.lock().await;
            if let Some(peer) = guard.as_ref() {
                let notification = ToolListChangedNotification {
                    method: ToolListChangedNotificationMethod,
                    extensions: Extensions::new(),
                };
                let msg = ServerNotification::ToolListChangedNotification(notification);
                if let Err(e) = peer.send_notification(msg).await {
                    tracing::warn!("tools/list_changed notification failed: {e}");
                }
            }
        });
    }
}

impl ServerHandler for B00tMcpServerRusty {
    async fn ping(&self, _context: RequestContext<RoleServer>) -> Result<(), McpError> {
        debug!("🏓 Ping received - Rusty MCP server is alive and well");

        // Log server health info for debugging
        let tools_count = self.registry.get_tools().len();
        debug!(
            "🦀 Server status: {} compile-time tools available",
            tools_count
        );
        debug!("📁 Working directory: {}", self.working_dir.display());

        // Verify b00t-cli is available
        let b00t_cli_available = std::process::Command::new("b00t-cli")
            .arg("--help")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);

        debug!(
            "🥾 b00t-cli availability: {}",
            if b00t_cli_available { "✅" } else { "❌" }
        );

        if !b00t_cli_available {
            info!("⚠️  b00t-cli not available - MCP tools may fail to execute properly");
        }

        Ok(())
    }

    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.server_info = Implementation::from_build_env();
        info.instructions = Some(
            "🦀 Rusty MCP server for b00t-cli with compile-time generated tools. \
             Features type-safe command dispatch, zero runtime parsing failures, \
             and full CLAP structure synchronization."
                .into(),
        );
        info.capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()
            .build();
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        debug!("🦀 list_tools called - using compile-time generated tools");

        let tools = self.registry.get_tools();

        // SP3-04 — an identified (non-anon) caller sees only the tools its
        // r0le package allows. Anon callers, and callers whose r0le can't be
        // resolved, get the full set unchanged (SP3-09 can tighten this).
        let r0le = self.caller().await.r0le;
        let mut tools = self.filter_tools_for_r0le(tools, &r0le);

        // SP3-07 — identified callers may request runtime escalation.
        if r0le != "anon" {
            tools.push(escalation_tool_def());
        }

        info!(
            "🦀 Serving {} compile-time tools from b00t-cli CLAP structures",
            tools.len()
        );

        for tool in &tools {
            debug!("🔧 Tool: {}", tool.name);
        }

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let tool_name = request.name.as_ref();

        // Extract client identity for response customization
        let client_name = context
            .peer
            .peer_info()
            .map(|p| p.client_info.name.clone())
            .unwrap_or_default();

        // Convert request arguments to HashMap
        let params: HashMap<String, serde_json::Value> =
            request.arguments.unwrap_or_default().into_iter().collect();

        if !client_name.is_empty() {
            debug!("🦀 call_tool: {} via client: {}", tool_name, client_name);
        }

        info!(
            "🦀 Executing compile-time tool: {} with params: {:?}",
            tool_name, params
        );

        let caller = self.caller().await;

        // SP3-07 — the runtime-escalation tool is handled here (not a registry
        // command); the judge decides, grants widen the live allow-list.
        if tool_name == ESCALATION_TOOL {
            return Ok(self.handle_escalation(&caller, &params).await);
        }

        // SP3-05 — unlock gate. A gated tool is refused until its unlocking
        // skill has been learned this session (`b00t_learn`). anon is not gated.
        if caller.r0le != "anon" {
            let gate = self.unlock_gate_for(&caller.r0le);
            if !gate.is_empty() {
                let learned = self
                    .learned
                    .read()
                    .map(|g| g.clone())
                    .unwrap_or_default();
                if !gate.is_satisfied(tool_name, &learned) {
                    let skill = gate.required_skill(tool_name).unwrap_or("<skill>");
                    return Err(McpError {
                        code: rmcp::model::ErrorCode(-32003),
                        message: format!(
                            "tool '{tool_name}' locked: run b00t_learn('{skill}') first"
                        )
                        .into(),
                        data: None,
                    });
                }
            }
        }

        // SP3-06 — ledgrrr spend precheck. On denial the tool is NOT executed.
        self.authorize_call(&caller, tool_name).await?;

        let execution_result = self.registry.execute(tool_name, &params);
        let chat_indicator = self.chat_runtime.drain_indicator().await;

        match execution_result {
            Ok(output) => {
                info!("✅ Successfully executed tool: {}", tool_name);
                // SP3-05 — a successful b00t_learn unlocks its topic for the
                // rest of the session.
                if tool_name == "b00t_learn" {
                    if let Some(topic) = params.get("topic").and_then(|v| v.as_str()) {
                        if let Ok(mut g) = self.learned.write() {
                            g.insert(topic.to_string());
                        }
                    }
                }
                // best-effort usage record (never fails the call)
                if caller.r0le != "anon" {
                    let _ = self
                        .spend_authorizer
                        .record_usage(
                            &caller.tenant,
                            &caller.agent,
                            1,
                            serde_json::json!({ "tool": tool_name }),
                        )
                        .await;
                }
                Ok(self.create_success_result(&output, &chat_indicator))
            }
            Err(e) => {
                error!("❌ Failed to execute tool {}: {}", tool_name, e);
                Ok(self.create_error_result(&e.to_string(), &chat_indicator))
            }
        }
    }

    // 🦀 MCP Resources Support
    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        debug!("🦀 list_resources called - providing b00t ecosystem resources");

        let mut resources: Vec<Annotated<RawResource>> = Vec::new();

        // Add b00t skills directory as a resource
        if let Ok(b00t_dir) = utils::get_b00t_config_dir() {
            if b00t_dir.exists() {
                let skills_uri = format!("file://{}", b00t_dir.display());
                let mut resource = RawResource::new(skills_uri, "b00t_skills_directory");
                resource.description = Some("B00t skills and configuration directory".to_string());
                resource.mime_type = Some("application/x-directory".to_string());
                resources.push(Annotated::new(resource, None));
            }
        }

        // Add b00t learn topics as resources
        if let Ok(entries) = std::fs::read_dir(utils::get_b00t_config_dir().unwrap_or_default()) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "md" {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let topic_name = name.strip_suffix(".md").unwrap_or(&name);
                        let uri = format!("b00t://learn/{}", topic_name);
                        let mut resource = RawResource::new(
                            uri,
                            format!("b00t_skill_{}", topic_name.replace('.', "_")),
                        );
                        resource.description = Some(format!("B00t skill: {}", topic_name));
                        resource.mime_type = Some("text/markdown".to_string());
                        resources.push(Annotated::new(resource, None));
                    }
                }
            }
        }

        // Add current context as a resource
        let mut context_resource =
            RawResource::new("b00t://context/current", "b00t_current_context");
        context_resource.description =
            Some("Current b00t agent context and environment".to_string());
        context_resource.mime_type = Some("application/json".to_string());
        resources.push(Annotated::new(context_resource, None));

        info!("🦀 Providing {} b00t resources", resources.len());

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = &request.uri;
        debug!("🦀 read_resource called for URI: {}", uri);

        match uri.as_str() {
            uri if uri.starts_with("b00t://learn/") => {
                let topic = uri.strip_prefix("b00t://learn/").unwrap_or("");
                info!("📚 Reading b00t skill: {}", topic);

                match self.read_b00t_skill(topic).await {
                    Ok(content) => Ok(ReadResourceResult::new(vec![ResourceContents::text(
                        content, uri,
                    )])),
                    Err(e) => {
                        error!("❌ Failed to read b00t skill {}: {}", topic, e);
                        let error_msg = format!("Failed to read skill: {}", e);
                        Err(McpError::internal_error(error_msg, None))
                    }
                }
            }
            "b00t://context/current" => {
                info!("🎯 Reading current b00t context");

                match self.read_current_context().await {
                    Ok(content) => Ok(ReadResourceResult::new(vec![
                        ResourceContents::TextResourceContents {
                            uri: uri.clone(),
                            mime_type: Some("application/json".to_string()),
                            text: content,
                            meta: None,
                        },
                    ])),
                    Err(e) => {
                        error!("❌ Failed to read current context: {}", e);
                        let error_msg = format!("Failed to read context: {}", e);
                        Err(McpError::internal_error(error_msg, None))
                    }
                }
            }
            uri if uri.starts_with("file://") => {
                let file_path = uri.strip_prefix("file://").unwrap_or(uri);
                info!("📁 Reading file resource: {}", file_path);

                match std::fs::read_to_string(file_path) {
                    Ok(content) => Ok(ReadResourceResult::new(vec![ResourceContents::text(
                        content, uri,
                    )])),
                    Err(e) => {
                        error!("❌ Failed to read file {}: {}", file_path, e);
                        let error_msg = format!("Failed to read file: {}", e);
                        Err(McpError::internal_error(error_msg, None))
                    }
                }
            }
            _ => {
                error!("❌ Unknown resource URI: {}", uri);
                let error_msg = format!("Unknown resource URI: {}", uri);
                Err(McpError::invalid_params(error_msg, None))
            }
        }
    }

    async fn on_initialized(
        &self,
        context: rmcp::service::NotificationContext<rmcp::service::RoleServer>,
    ) {
        // Store peer handle for tools/list_changed notifications (dynamic stack load/unload)
        *self.notification_peer.lock().await = Some(context.peer.clone());

        // Capture client info from peer metadata (hermes, claude-code, opencode, etc.)
        if let Some(peer_info) = context.peer.peer_info() {
            let client_name = peer_info.client_info.name.clone();
            let client_version = peer_info.client_info.version.clone();
            if let Ok(mut info) = self.client_info.lock() {
                *info = Some(peer_info.client_info.clone());
            }
            info!(
                "🦀 b00t-mcp connected to client: {} v{}",
                client_name, client_version
            );
        }

        info!("🦀 Rusty b00t-mcp server initialized successfully");

        let tools = self.registry.get_tools();
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();

        info!("🦀 Available compile-time tools: {}", tool_names.join(", "));

        // Log some statistics
        info!("📊 Total tools: {}", tools.len());

        let tool_categories: HashMap<&str, usize> =
            tools.iter().fold(HashMap::new(), |mut acc, tool| {
                let prefix = tool.name.as_ref().split('_').nth(1).unwrap_or("unknown");
                *acc.entry(prefix).or_insert(0) += 1;
                acc
            });

        for (category, count) in tool_categories {
            info!("📋 {} tools: {}", category, count);
        }
    }
}

impl B00tMcpServerRusty {
    /// Create successful MCP tool result
    fn create_success_result(&self, output: &str, indicator: &str) -> CallToolResult {
        #[derive(serde::Serialize)]
        struct B00tOutput {
            output: String,
            success: bool,
            server_type: String,
            working_dir: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            indicator: Option<String>,
        }

        let decorated_output = match (output.trim().is_empty(), indicator.trim().is_empty()) {
            (true, true) => String::new(),
            (true, false) => indicator.to_string(),
            (false, true) => output.to_string(),
            (false, false) => format!("{}\n{}", output, indicator),
        };

        let result = B00tOutput {
            output: decorated_output,
            success: true,
            server_type: "rusty".to_string(),
            working_dir: self.working_dir.display().to_string(),
            indicator: (!indicator.trim().is_empty()).then(|| indicator.to_string()),
        };

        let content = serde_json::to_string_pretty(&result)
            .unwrap_or_else(|_| "Failed to serialize result".to_string());

        CallToolResult::success(vec![Content::text(content)])
    }

    /// Create error MCP tool result
    fn create_error_result(&self, error: &str, indicator: &str) -> CallToolResult {
        #[derive(serde::Serialize)]
        struct B00tError {
            error: String,
            success: bool,
            server_type: String,
            working_dir: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            indicator: Option<String>,
        }

        let decorated_error = match (error.trim().is_empty(), indicator.trim().is_empty()) {
            (true, true) => String::new(),
            (true, false) => indicator.to_string(),
            (false, true) => error.to_string(),
            (false, false) => format!("{}\n{}", error, indicator),
        };

        let result = B00tError {
            error: decorated_error,
            success: false,
            server_type: "rusty".to_string(),
            working_dir: self.working_dir.display().to_string(),
            indicator: (!indicator.trim().is_empty()).then(|| indicator.to_string()),
        };

        let content = serde_json::to_string_pretty(&result)
            .unwrap_or_else(|_| "Failed to serialize error".to_string());

        CallToolResult::error(vec![Content::text(content)])
    }

    /// Read a b00t skill using the shared library
    async fn read_b00t_skill(&self, topic: &str) -> Result<String> {
        use b00t_c0re_lib::TemplateRenderer;
        use b00t_c0re_lib::learn::get_learn_lesson;
        let path = self.working_dir.to_str().unwrap_or("");
        let lesson = get_learn_lesson(path, topic)?;
        let renderer = TemplateRenderer::with_defaults()?;
        let rendered = renderer.render(&lesson)?;
        Ok(rendered)
    }

    /// Read current b00t context as JSON
    async fn read_current_context(&self) -> Result<String> {
        let context = B00tContext::current()?;
        let json = serde_json::to_string_pretty(&context)?;
        Ok(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn with_tokio_runtime<T>(f: impl FnOnce() -> T) -> T {
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
        let guard = runtime.enter();
        let result = f();
        drop(guard);
        runtime.shutdown_background();
        result
    }

    #[test]
    fn test_server_creation() {
        with_tokio_runtime(|| {
            let temp_dir = TempDir::new().unwrap();
            let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "").unwrap();

            assert_eq!(server.working_dir, temp_dir.path());

            // Test that registry has tools
            let tools = server.registry.get_tools();
            assert!(!tools.is_empty());
        });
    }

    #[test]
    fn test_server_info() {
        with_tokio_runtime(|| {
            let temp_dir = TempDir::new().unwrap();
            let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "").unwrap();

            let info = server.get_info();
            assert!(info.instructions.unwrap().contains("🦀 Rusty MCP server"));
            assert!(info.capabilities.tools.is_some());
        });
    }

    // 🦨 TODO: Fix RequestContext creation for tests
    // #[tokio::test]
    // async fn test_list_tools() {
    //     let temp_dir = TempDir::new().unwrap();
    //     let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "").unwrap();
    //
    //     // Need to create proper RequestContext - RequestContext::default() doesn't exist
    //     // let result = server.list_tools(None, context).await;
    //     // assert!(result.is_ok());
    // }

    // #[tokio::test]
    // async fn test_ping() {
    //     let temp_dir = TempDir::new().unwrap();
    //     let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "").unwrap();
    //
    //     // Need to create proper RequestContext - RequestContext::default() doesn't exist
    //     // let result = server.ping(context).await;
    //     // assert!(result.is_ok());
    // }

    #[test]
    fn test_result_creation() {
        with_tokio_runtime(|| {
            let temp_dir = TempDir::new().unwrap();
            let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "").unwrap();

            let success_result = server.create_success_result("Test output", "");
            assert!(!success_result.content.is_empty());
            let success_value = serde_json::to_value(&success_result).unwrap();
            let success_text = success_value["content"][0]["text"].as_str().unwrap();
            let success_payload: serde_json::Value = serde_json::from_str(success_text).unwrap();
            assert_eq!(success_payload["output"], "Test output");
            assert!(success_payload.get("indicator").is_none());
            assert!(!success_text.contains("<🥾>"));

            let error_result = server.create_error_result("Test error", "");
            assert!(!error_result.content.is_empty());
            let error_value = serde_json::to_value(&error_result).unwrap();
            let error_text = error_value["content"][0]["text"].as_str().unwrap();
            let error_payload: serde_json::Value = serde_json::from_str(error_text).unwrap();
            assert_eq!(error_payload["error"], "Test error");
            assert!(error_payload.get("indicator").is_none());
            assert!(!error_text.contains("<🥾>"));
        });
    }

    #[test]
    fn nonempty_indicator_is_preserved() {
        with_tokio_runtime(|| {
            let temp_dir = TempDir::new().unwrap();
            let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "").unwrap();
            let indicator = "<🥾>{ \"chat\": { \"msgs\": 1 } }</🥾>";

            let result = server.create_success_result("Test output", indicator);
            let value = serde_json::to_value(&result).unwrap();
            let text = value["content"][0]["text"].as_str().unwrap();
            let payload: serde_json::Value = serde_json::from_str(text).unwrap();

            assert_eq!(payload["indicator"], indicator);
            assert!(payload["output"].as_str().unwrap().contains(indicator));
        });
    }

    #[tokio::test]
    async fn sp3_02b_caller_resolves_pending_jwt() {
        use crate::identity::JwksVerifier;
        use b00t_c0re_identity::{MockTokenSource, TokenRequest};

        let temp_dir = TempDir::new().unwrap();
        let mock = MockTokenSource::new();
        let jwt = mock
            .mint(&TokenRequest {
                tenant_id: "promptexecution".into(),
                agent_id: "agent/x".into(),
                node_id: "root".into(),
                r0le: "worker".into(),
                requested_shards: vec![],
            })
            .unwrap();

        let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "")
            .unwrap()
            .with_test_verifier(JwksVerifier::pinned(mock.jwks_json()));

        // no pending jwt yet -> anon
        assert!(server.caller().await.is_anon());

        // initialize.meta override (or $B00T_AGENT_JWT) -> resolves + caches
        server.set_pending_jwt(jwt);
        let id = server.caller().await;
        assert_eq!(id.tenant, "promptexecution");
        assert_eq!(id.r0le, "worker");
        assert!(!id.is_anon());
    }

    #[test]
    fn sp3_04_list_tools_filtered_by_r0le() {
        use crate::r0le_resolver::{FixtureR0leResolver, ResolvedR0le};
        use b00t_cli::datum_agent_profile::ModelTier;

        with_tokio_runtime(|| {
            let temp_dir = TempDir::new().unwrap();
            let all_tools = B00tMcpServerRusty::new_flat(temp_dir.path(), "")
                .unwrap()
                .registry
                .get_tools();
            assert!(all_tools.len() > 2, "need a few tools to prove filtering");
            let allow: Vec<String> =
                all_tools.iter().take(2).map(|t| t.name.to_string()).collect();

            let resolver = std::sync::Arc::new(FixtureR0leResolver::new().with(
                "worker",
                ResolvedR0le {
                    tool_allowlist: allow.clone(),
                    skills: vec![],
                    budget_ceiling: 0,
                    model_tier: ModelTier::Ch0nky,
                },
            ));
            let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "")
                .unwrap()
                .with_r0le_resolver(resolver);

            // anon → unchanged
            assert_eq!(
                server.filter_tools_for_r0le(all_tools.clone(), "anon").len(),
                all_tools.len()
            );
            // worker → exactly the 2 allowed
            let worker = server.filter_tools_for_r0le(all_tools.clone(), "worker");
            assert_eq!(worker.len(), 2);
            assert!(worker.iter().all(|t| allow.contains(&t.name.to_string())));
            // unknown r0le → resolver errs → fail-open
            assert_eq!(
                server.filter_tools_for_r0le(all_tools.clone(), "ghost").len(),
                all_tools.len()
            );
        });
    }

    #[tokio::test]
    async fn sp3_06_authorize_call_gates_on_ledgrrr() {
        use crate::identity::CallerIdentity;
        use b00t_c0re_ledgrrr::MockSpendAuthorizer;

        let temp_dir = TempDir::new().unwrap();
        let worker = CallerIdentity {
            tenant: "promptexecution".into(),
            agent: "agent/x".into(),
            r0le: "worker".into(),
            scopes: vec![],
            budget_ref: "jti-1".into(),
        };

        // default (always-ok mock) → passes
        let ok_srv = B00tMcpServerRusty::new_flat(temp_dir.path(), "").unwrap();
        assert!(ok_srv.authorize_call(&worker, "b00t_status").await.is_ok());

        let deny_srv = B00tMcpServerRusty::new_flat(temp_dir.path(), "")
            .unwrap()
            .with_spend_authorizer(std::sync::Arc::new(MockSpendAuthorizer::always_deny()));

        // anon is never metered, even with a denying authorizer
        assert!(
            deny_srv
                .authorize_call(&CallerIdentity::anon(), "b00t_status")
                .await
                .is_ok()
        );

        // non-anon + deny → Err with JSON-RPC code -32004
        let err = deny_srv
            .authorize_call(&worker, "b00t_status")
            .await
            .unwrap_err();
        assert_eq!(err.code, rmcp::model::ErrorCode(-32004));
    }

    #[test]
    fn sp3_05_unlock_gate_from_a_real_role() {
        with_tokio_runtime(|| {
            let temp_dir = TempDir::new().unwrap();
            let b00t = temp_dir.path().join("_b00t_");
            std::fs::create_dir_all(&b00t).unwrap();
            std::fs::write(
                b00t.join("worker.role.toml"),
                "[b00t]\nname = \"worker\"\ntype = \"role\"\ndepends_on = [\"rust.skill\"]\n",
            )
            .unwrap();
            std::fs::write(
                b00t.join("rust.skill.toml"),
                "[b00t]\nname = \"rust\"\ntype = \"skill\"\nunlocks = [\"cargo_*\"]\n",
            )
            .unwrap();

            let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "").unwrap();
            let gate = server.unlock_gate_for("worker");
            assert!(!gate.is_empty());
            assert_eq!(gate.required_skill("cargo_build"), Some("rust.skill"));
            assert_eq!(gate.required_skill("b00t_status"), None);

            let mut learned = std::collections::HashSet::new();
            assert!(!gate.is_satisfied("cargo_build", &learned));
            learned.insert("rust.skill".to_string());
            assert!(gate.is_satisfied("cargo_build", &learned));

            // unknown role → empty gate, nothing locked
            assert!(server.unlock_gate_for("ghost").is_empty());
        });
    }

    #[tokio::test]
    async fn sp3_07_escalation_grant_widens_allowlist() {
        use crate::identity::CallerIdentity;
        use crate::r0le_resolver::{FixtureR0leResolver, ResolvedR0le};
        use b00t_cli::datum_agent_profile::ModelTier;
        use capability_forge::judge::FakeJudge;

        let temp_dir = TempDir::new().unwrap();
        let all_tools = B00tMcpServerRusty::new_flat(temp_dir.path(), "")
            .unwrap()
            .registry
            .get_tools();
        assert!(all_tools.len() > 3);
        let base_allow: Vec<String> =
            all_tools.iter().take(1).map(|t| t.name.to_string()).collect();
        let extra_tool = all_tools[2].name.to_string();

        let resolver = std::sync::Arc::new(FixtureR0leResolver::new().with(
            "worker",
            ResolvedR0le {
                tool_allowlist: base_allow.clone(),
                skills: vec![],
                budget_ceiling: 0,
                model_tier: ModelTier::Ch0nky,
            },
        ));
        let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "")
            .unwrap()
            .with_r0le_resolver(resolver.clone())
            .with_judge(std::sync::Arc::new(FakeJudge::always_grant()));

        let worker = CallerIdentity {
            tenant: "t".into(),
            agent: "a".into(),
            r0le: "worker".into(),
            scopes: vec![],
            budget_ref: String::new(),
        };

        assert_eq!(
            server.filter_tools_for_r0le(all_tools.clone(), "worker").len(),
            1
        );

        let mut params = HashMap::new();
        params.insert("tools".to_string(), serde_json::json!([extra_tool]));
        params.insert(
            "justification".to_string(),
            serde_json::json!("needed for the task"),
        );
        let _ = server.handle_escalation(&worker, &params).await;

        // grant widened the live allow-list
        assert_eq!(
            server.filter_tools_for_r0le(all_tools.clone(), "worker").len(),
            2
        );

        // a denying judge grants nothing
        let deny = B00tMcpServerRusty::new_flat(temp_dir.path(), "")
            .unwrap()
            .with_r0le_resolver(resolver)
            .with_judge(std::sync::Arc::new(FakeJudge::always_deny("no")));
        let _ = deny.handle_escalation(&worker, &params).await;
        assert_eq!(
            deny.filter_tools_for_r0le(all_tools.clone(), "worker").len(),
            1
        );
    }
}
