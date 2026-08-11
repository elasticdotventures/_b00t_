//! b00t Azure Control Plane — MCP server for on-demand ACI compute.
//!
//! Exposes six MCP tools:
//!   azure.provision_aci        — spin up an ACI container, return endpoint_url + lease_id
//!   azure.deprovision          — tear down by lease_id
//!   azure.heartbeat            — renew lease TTL
//!   azure.list_leases          — active resources + TTLs
//!   azure.cost_estimate        — current-month spend for the control plane RG
//!   azure.keyvault_get_secret  — fetch a secret by (vault_name, secret_name)
//!
//! Lease state stored in Azure Table Storage (b00tLeases table).
//! Background watchdog tears down leases where expires_at < now().

use std::env;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::Request,
    http::{StatusCode, header},
    middleware,
    response::IntoResponse,
};
use azure_core::auth::TokenCredential;
use azure_data_tables::clients::TableServiceClient;
use azure_identity::AppServiceManagedIdentityCredential;
use chrono::{DateTime, Utc};
use rmcp::handler::server::{tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo};
use rmcp::schemars::JsonSchema;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::time;
use tracing::{error, info, warn};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Config {
    node_id: String,
    storage_account: String,
    table_name: String,
    resource_group: String,
    subscription_id: String,
    client_id: Option<String>,
    lease_ttl_minutes: i64,
    /// Azure region for ACI deployments (e.g. "australiaeast").
    location: String,
    /// Bearer token required in `Authorization: Bearer <token>` header.
    /// If None, the endpoint is unauthenticated (only appropriate when
    /// external_ingress = false in Terraform keeps the endpoint internal).
    auth_token: Option<String>,
}

impl Config {
    fn from_env() -> Result<Self> {
        Ok(Self {
            node_id: env::var("B00T_NODE_ID").context("B00T_NODE_ID not set")?,
            storage_account: env::var("AZURE_STORAGE_ACCOUNT_NAME")
                .context("AZURE_STORAGE_ACCOUNT_NAME not set")?,
            table_name: env::var("AZURE_TABLE_NAME").unwrap_or_else(|_| "b00tLeases".to_string()),
            resource_group: env::var("AZURE_RESOURCE_GROUP")
                .context("AZURE_RESOURCE_GROUP not set")?,
            subscription_id: env::var("AZURE_SUBSCRIPTION_ID")
                .context("AZURE_SUBSCRIPTION_ID not set")?,
            client_id: env::var("AZURE_CLIENT_ID").ok().filter(|v| !v.is_empty()),
            lease_ttl_minutes: env::var("LEASE_TTL_MINUTES")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
            location: env::var("AZURE_LOCATION").context("AZURE_LOCATION not set")?,
            auth_token: env::var("B00T_CP_AUTH_TOKEN").ok(),
        })
    }
}

// ---------------------------------------------------------------------------
// Lease entity (Azure Table Storage row)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LeaseEntity {
    #[serde(rename = "PartitionKey")]
    partition_key: String,
    #[serde(rename = "RowKey")]
    row_key: String,
    resource_id: String,
    resource_type: String,
    endpoint_url: String,
    expires_at: String, // ISO 8601
    created_at: String,
    client_hint: String,
}

// ---------------------------------------------------------------------------
// MCP tool input/output types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct ProvisionAciInput {
    /// Container image to run (e.g. "ghcr.io/myorg/my-inference:latest").
    image: String,
    /// Number of vCPUs for the container group.
    #[serde(default = "default_cpu")]
    cpu: f64,
    /// Memory in GB.
    #[serde(default = "default_memory_gb")]
    memory_gb: f64,
    /// Number of GPUs (0 for CPU-only).
    #[serde(default)]
    gpu_count: u32,
    /// GPU SKU (e.g. "V100", "K80"). Required if gpu_count > 0.
    #[serde(default)]
    gpu_sku: Option<String>,
    /// Lease TTL in minutes. Default: server-configured default.
    #[serde(default)]
    lease_ttl_minutes: Option<i64>,
    /// Optional label for identifying this resource in list_leases output.
    #[serde(default)]
    client_hint: Option<String>,
    /// Inbound port to expose. Default: 8080.
    #[serde(default = "default_port")]
    port: u16,
}

fn default_cpu() -> f64 {
    2.0
}
fn default_memory_gb() -> f64 {
    4.0
}
fn default_port() -> u16 {
    8080
}

#[derive(Debug, Deserialize, JsonSchema)]
struct DeprovisionInput {
    /// Lease ID returned by provision_aci.
    lease_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct HeartbeatInput {
    /// Lease ID to renew.
    lease_id: String,
    /// New TTL from now in minutes. Defaults to server-configured default.
    #[serde(default)]
    ttl_minutes: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct KeyvaultGetSecretInput {
    /// Key Vault name (the `xyz` in `https://xyz.vault.azure.net`), not the full URL.
    vault_name: String,
    /// Secret name within the vault.
    secret_name: String,
    /// Specific version to fetch. Defaults to the latest enabled version.
    #[serde(default)]
    version: Option<String>,
}

// ---------------------------------------------------------------------------
// Azure Table Storage helpers
// ---------------------------------------------------------------------------

fn table_service_client(
    config: &Config,
    credential: Arc<dyn TokenCredential>,
) -> TableServiceClient {
    TableServiceClient::new(
        format!("https://{}.table.core.windows.net", config.storage_account),
        credential,
    )
}

async fn upsert_lease(
    client: &TableServiceClient,
    config: &Config,
    entity: &LeaseEntity,
) -> Result<()> {
    let table_client = client.table_client(&config.table_name);
    table_client
        .partition_key_client(&entity.partition_key)
        .entity_client(&entity.row_key)
        .insert_or_replace(entity)
        .context("failed to serialize lease entity")?
        .await
        .context("failed to upsert lease entity")?;
    Ok(())
}

async fn delete_lease(client: &TableServiceClient, config: &Config, lease_id: &str) -> Result<()> {
    let table_client = client.table_client(&config.table_name);
    table_client
        .partition_key_client(&config.node_id)
        .entity_client(lease_id)
        .delete()
        .await
        .context("failed to delete lease entity")?;
    Ok(())
}

async fn list_leases_from_table(
    client: &TableServiceClient,
    config: &Config,
) -> Result<Vec<LeaseEntity>> {
    let table_client = client.table_client(&config.table_name);
    let filter = azure_data_tables::Filter::new(format!("PartitionKey eq '{}'", config.node_id));
    let mut entities: Vec<LeaseEntity> = Vec::new();
    let mut stream = table_client
        .query()
        .filter(filter)
        .into_stream::<LeaseEntity>();
    use futures::StreamExt;
    while let Some(page) = stream.next().await {
        let page = page.context("error reading lease page")?;
        entities.extend(page.entities);
    }
    Ok(entities)
}

// ---------------------------------------------------------------------------
// ACI provisioning helpers
// ---------------------------------------------------------------------------

async fn provision_aci_resource(
    config: &Config,
    _credential: Arc<dyn TokenCredential>,
    input: &ProvisionAciInput,
    lease_id: &str,
) -> Result<String> {
    // Build ACI resource via azure_mgmt_containerinstance.
    // The container group name is derived from the lease_id for uniqueness.
    let group_name = format!("b00t-aci-{}", &lease_id[..8]);

    // Extract subscription ID from the resource group ID env var.
    // Format: /subscriptions/{sub}/resourceGroups/{rg}
    let subscription_id = config.subscription_id.clone();

    let aci_client = azure_mgmt_containerinstance::ClientBuilder::new(_credential)
        .endpoint(azure_core::Url::parse("https://management.azure.com").unwrap())
        .build()
        .context("failed to build ACI client")?;

    use azure_mgmt_containerinstance::models::{
        Container, ContainerGroup, ContainerGroupProperties, ContainerPort, ContainerProperties,
        GpuResource, IpAddress, Port, ResourceRequests, ResourceRequirements,
        container_group_properties, gpu_resource, ip_address,
    };

    let container_props = ContainerProperties {
        image: Some(input.image.clone()),
        resources: Some(ResourceRequirements {
            requests: ResourceRequests {
                cpu: input.cpu,
                memory_in_gb: input.memory_gb,
                gpu: if input.gpu_count > 0 {
                    Some(GpuResource {
                        count: input.gpu_count as i32,
                        sku: match input.gpu_sku.as_deref().unwrap_or("V100") {
                            "K80" => gpu_resource::Sku::K80,
                            "P100" => gpu_resource::Sku::P100,
                            _ => gpu_resource::Sku::V100,
                        },
                    })
                } else {
                    None
                },
            },
            limits: None,
        }),
        ports: vec![ContainerPort {
            port: input.port as i32,
            protocol: None,
        }],
        ..Default::default()
    };

    let mut cg_props = container_group_properties::Properties::new(vec![Container {
        name: group_name.clone(),
        properties: container_props,
    }]);
    cg_props.os_type = Some(container_group_properties::properties::OsType::Linux);
    cg_props.ip_address = Some(IpAddress {
        type_: ip_address::Type::Public,
        ports: vec![Port {
            port: input.port as i32,
            protocol: None,
        }],
        ip: None,
        dns_name_label: Some(group_name.clone()),
        auto_generated_domain_name_label_scope: None,
        fqdn: None,
    });

    let mut container_group = ContainerGroup::new(ContainerGroupProperties::new(cg_props));
    container_group.resource.location = Some(config.location.clone());

    let created = aci_client
        .container_groups_client()
        .create_or_update(
            &subscription_id,
            &config.resource_group,
            &group_name,
            container_group,
        )
        .await
        .context("failed to create ACI container group")?;

    // Prefer the FQDN returned by ACI (authoritative); fall back to the
    // deterministic pattern if the API response does not include it yet.
    let endpoint_url = created
        .container_group_properties
        .properties
        .ip_address
        .as_ref()
        .and_then(|ip| ip.fqdn.as_deref())
        .map(|fqdn| format!("http://{}:{}", fqdn, input.port))
        .unwrap_or_else(|| {
            format!(
                "http://{}.{}.azurecontainer.io:{}",
                group_name, config.location, input.port
            )
        });

    info!(group = %group_name, endpoint = %endpoint_url, "ACI container group created");
    Ok(endpoint_url)
}

async fn deprovision_aci_resource(
    config: &Config,
    credential: Arc<dyn TokenCredential>,
    resource_id: &str,
) -> Result<()> {
    let subscription_id = config.subscription_id.clone();

    let aci_client = azure_mgmt_containerinstance::ClientBuilder::new(credential)
        .endpoint(azure_core::Url::parse("https://management.azure.com").unwrap())
        .build()
        .context("failed to build ACI client")?;

    // resource_id is the container group name (stored in lease entity).
    aci_client
        .container_groups_client()
        .delete(&subscription_id, &config.resource_group, resource_id)
        .await
        .context("failed to delete ACI container group")?;

    info!(resource_id = %resource_id, "ACI container group deleted");
    Ok(())
}

// ---------------------------------------------------------------------------
// Background watchdog
// ---------------------------------------------------------------------------

async fn run_watchdog(
    config: Config,
    table_client: TableServiceClient,
    credential: Arc<dyn TokenCredential>,
) {
    let mut interval = time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        match list_leases_from_table(&table_client, &config).await {
            Err(e) => warn!(error = %e, "watchdog: failed to list leases"),
            Ok(leases) => {
                let now = Utc::now();
                for lease in leases {
                    let expires: DateTime<Utc> = match lease.expires_at.parse() {
                        Ok(t) => t,
                        Err(e) => {
                            warn!(lease_id = %lease.row_key, error = %e, "watchdog: bad expires_at");
                            continue;
                        }
                    };
                    if expires < now {
                        info!(lease_id = %lease.row_key, resource_id = %lease.resource_id, "watchdog: lease expired, deprovisioning");
                        if let Err(e) = deprovision_aci_resource(
                            &config,
                            Arc::clone(&credential),
                            &lease.resource_id,
                        )
                        .await
                        {
                            error!(lease_id = %lease.row_key, error = %e, "watchdog: deprovision failed");
                        }
                        if let Err(e) = delete_lease(&table_client, &config, &lease.row_key).await {
                            error!(lease_id = %lease.row_key, error = %e, "watchdog: lease delete failed");
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MCP server handler
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AzureCpServer {
    config: Config,
    table_client: Arc<TableServiceClient>,
    credential: Arc<dyn TokenCredential>,
    /// Shared HTTP client with connect/request timeouts for Cost Management queries.
    http_client: reqwest::Client,
    #[allow(dead_code)]
    tool_router: ToolRouter<AzureCpServer>,
}

#[tool_router]
impl AzureCpServer {
    /// Spin up an ACI container on demand. Returns endpoint_url and lease_id.
    /// The lease expires after the configured TTL unless renewed via heartbeat.
    #[tool(name = "azure.provision_aci")]
    async fn provision_aci(
        &self,
        input: Parameters<ProvisionAciInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = input.0;
        let lease_id = Uuid::new_v4().to_string();
        let ttl = input
            .lease_ttl_minutes
            .unwrap_or(self.config.lease_ttl_minutes);
        let expires_at = Utc::now() + chrono::Duration::minutes(ttl);
        let client_hint = input.client_hint.clone().unwrap_or_default();

        let endpoint_url = provision_aci_resource(
            &self.config,
            Arc::clone(&self.credential),
            &input,
            &lease_id,
        )
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        // Container group name follows the same pattern used in provisioning.
        let group_name = format!("b00t-aci-{}", &lease_id[..8]);

        let lease = LeaseEntity {
            partition_key: self.config.node_id.clone(),
            row_key: lease_id.clone(),
            resource_id: group_name,
            resource_type: "aci_container_group".to_string(),
            endpoint_url: endpoint_url.clone(),
            expires_at: expires_at.to_rfc3339(),
            created_at: Utc::now().to_rfc3339(),
            client_hint,
        };

        upsert_lease(&self.table_client, &self.config, &lease)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        {
            let _mjson = serde_json::json!({
                "lease_id": lease_id,
                "endpoint_url": endpoint_url,
                "expires_at": expires_at.to_rfc3339(),
                "ttl_minutes": ttl,
            });
            Ok(CallToolResult::success(vec![
                Content::json(_mjson).map_err(|e| McpError::internal_error(e.to_string(), None))?,
            ]))
        }
    }

    /// Tear down a provisioned resource immediately by lease_id.
    #[tool(name = "azure.deprovision")]
    async fn deprovision(
        &self,
        input: Parameters<DeprovisionInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = input.0;
        // Look up the lease to get the resource_id.
        let table_client = self.table_client.table_client(&self.config.table_name);
        let entity: LeaseEntity = table_client
            .partition_key_client(&self.config.node_id)
            .entity_client(&input.lease_id)
            .get()
            .await
            .map_err(|e| {
                McpError::invalid_request(format!("lease not found or inaccessible: {e}"), None)
            })?
            .entity;

        deprovision_aci_resource(
            &self.config,
            Arc::clone(&self.credential),
            &entity.resource_id,
        )
        .await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        delete_lease(&self.table_client, &self.config, &input.lease_id)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        {
            let _mjson = serde_json::json!({
                "lease_id": input.lease_id,
                "status": "deprovisioned",
            });
            Ok(CallToolResult::success(vec![
                Content::json(_mjson).map_err(|e| McpError::internal_error(e.to_string(), None))?,
            ]))
        }
    }

    /// Renew a lease TTL. Call periodically to keep resources alive past the default TTL.
    #[tool(name = "azure.heartbeat")]
    async fn heartbeat(
        &self,
        input: Parameters<HeartbeatInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = input.0;
        let table_client = self.table_client.table_client(&self.config.table_name);
        let mut entity: LeaseEntity = table_client
            .partition_key_client(&self.config.node_id)
            .entity_client(&input.lease_id)
            .get()
            .await
            .map_err(|e| McpError::internal_error(format!("lease not found: {e}"), None))?
            .entity;

        let ttl = input.ttl_minutes.unwrap_or(self.config.lease_ttl_minutes);
        let new_expires = Utc::now() + chrono::Duration::minutes(ttl);
        entity.expires_at = new_expires.to_rfc3339();

        upsert_lease(&self.table_client, &self.config, &entity)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        {
            let _mjson = serde_json::json!({
                "lease_id": input.lease_id,
                "new_expires_at": new_expires.to_rfc3339(),
                "ttl_minutes": ttl,
            });
            Ok(CallToolResult::success(vec![
                Content::json(_mjson).map_err(|e| McpError::internal_error(e.to_string(), None))?,
            ]))
        }
    }

    /// List all active leases with endpoint URLs, TTLs, and resource identifiers.
    #[tool(name = "azure.list_leases")]
    async fn list_leases(&self) -> Result<CallToolResult, McpError> {
        let leases = list_leases_from_table(&self.table_client, &self.config)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let now = Utc::now();
        let items: Vec<serde_json::Value> = leases
            .iter()
            .map(|l| {
                let expires: DateTime<Utc> = l.expires_at.parse().unwrap_or(now);
                let remaining_seconds = (expires - now).num_seconds().max(0);
                serde_json::json!({
                    "lease_id": l.row_key,
                    "resource_id": l.resource_id,
                    "resource_type": l.resource_type,
                    "endpoint_url": l.endpoint_url,
                    "expires_at": l.expires_at,
                    "remaining_seconds": remaining_seconds,
                    "client_hint": l.client_hint,
                    "created_at": l.created_at,
                })
            })
            .collect();

        {
            let _mjson = serde_json::json!({
                "node_id": self.config.node_id,
                "lease_count": items.len(),
                "leases": items,
            });
            Ok(CallToolResult::success(vec![
                Content::json(_mjson).map_err(|e| McpError::internal_error(e.to_string(), None))?,
            ]))
        }
    }

    /// Estimated current-month Azure spend for the control plane resource group.
    /// Requires Cost Management Reader on the resource group.
    #[tool(name = "azure.cost_estimate")]
    async fn cost_estimate(&self) -> Result<CallToolResult, McpError> {
        // Use the Azure Cost Management REST API via azure_core.
        let subscription_id = &self.config.subscription_id;

        let scope = format!(
            "/subscriptions/{}/resourceGroups/{}",
            subscription_id, self.config.resource_group
        );

        // Build query body for month-to-date actual cost.
        let query_body = serde_json::json!({
            "type": "ActualCost",
            "timeframe": "MonthToDate",
            "dataset": {
                "granularity": "None",
                "aggregation": {
                    "totalCost": { "name": "Cost", "function": "Sum" }
                }
            }
        });

        let url = format!(
            "https://management.azure.com{}/providers/Microsoft.CostManagement/query?api-version=2023-11-01",
            scope
        );

        let token = self
            .credential
            .get_token(&["https://management.azure.com/.default"])
            .await
            .map_err(|e| McpError::internal_error(format!("credential error: {e}"), None))?;

        let resp = self
            .http_client
            .post(&url)
            .bearer_auth(token.token.secret())
            .json(&query_body)
            .send()
            .await
            .map_err(|e| McpError::internal_error(format!("cost query failed: {e}"), None))?;

        if resp.status().is_success() {
            let data: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            {
                let _mjson = serde_json::json!({
                    "resource_group": self.config.resource_group,
                    "timeframe": "MonthToDate",
                    "data": data,
                });
                Ok(CallToolResult::success(vec![
                    Content::json(_mjson)
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?,
                ]))
            }
        } else {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            {
                let _mjson = serde_json::json!({
                    "resource_group": self.config.resource_group,
                    "error": format!("HTTP {}: {}", status, text),
                    "note": "Cost Management Reader role required on the resource group.",
                });
                Ok(CallToolResult::success(vec![
                    Content::json(_mjson)
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?,
                ]))
            }
        }
    }

    /// Fetch a secret's value from an Azure Key Vault by (vault_name, secret_name).
    ///
    /// Uses the same managed-identity credential as every other tool on this
    /// server — the identity needs the "Key Vault Secrets User" role (or
    /// equivalent access policy) on the target vault. This is the
    /// server-side counterpart to `b00t secret resolve --azure-vault/--azure-secret`
    /// in b00t-cli, which instead shells out to `az keyvault secret show`
    /// using the developer's own `az login` session — that path is for local,
    /// one-shot CLI/script use; this tool is for agents talking to the
    /// deployed control plane, where no local `az` session exists.
    #[tool(name = "azure.keyvault_get_secret")]
    async fn keyvault_get_secret(
        &self,
        input: Parameters<KeyvaultGetSecretInput>,
    ) -> Result<CallToolResult, McpError> {
        let input = input.0;

        let path = match &input.version {
            Some(v) => format!("secrets/{}/{}", input.secret_name, v),
            None => format!("secrets/{}", input.secret_name),
        };
        let url = format!(
            "https://{}.vault.azure.net/{}?api-version=7.4",
            input.vault_name, path
        );

        let token = self
            .credential
            .get_token(&["https://vault.azure.net/.default"])
            .await
            .map_err(|e| McpError::internal_error(format!("credential error: {e}"), None))?;

        let resp = self
            .http_client
            .get(&url)
            .bearer_auth(token.token.secret())
            .send()
            .await
            .map_err(|e| McpError::internal_error(format!("keyvault request failed: {e}"), None))?;

        if resp.status().is_success() {
            let body: serde_json::Value = resp
                .json()
                .await
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            let value = body.get("value").and_then(|v| v.as_str()).ok_or_else(|| {
                McpError::internal_error("keyvault response had no 'value' field", None)
            })?;
            {
                let _mjson = serde_json::json!({
                    "vault_name": input.vault_name,
                    "secret_name": input.secret_name,
                    "value": value,
                });
                Ok(CallToolResult::success(vec![
                    Content::json(_mjson)
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?,
                ]))
            }
        } else {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            Err(McpError::internal_error(
                format!(
                    "keyvault get_secret failed: HTTP {} for '{}/{}': {}",
                    status, input.vault_name, input.secret_name, text
                ),
                None,
            ))
        }
    }
}

#[tool_handler]
impl ServerHandler for AzureCpServer {
    fn get_info(&self) -> ServerInfo {
        let mut server_info = Implementation::from_build_env();
        server_info.name = "b00t-azure-cp".into();
        server_info.version = env!("CARGO_PKG_VERSION").into();
        let mut info = ServerInfo::default();
        info.server_info = server_info;
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "b00t_azure_cp=info,rmcp=warn".parse().unwrap()),
        )
        .init();

    let config = Config::from_env().context("failed to load config from environment")?;
    info!(node_id = %config.node_id, "b00t-azure-cp starting");

    // Use managed identity when running in ACA.
    // AZURE_CLIENT_ID is optional: when present, the Azure SDK selects a
    // user-assigned identity; when absent, it falls back to system-assigned.
    let managed_identity_client_id = config.client_id.clone();
    if let Some(client_id) = managed_identity_client_id.as_deref() {
        info!(client_id = %client_id, "using user-assigned managed identity");
    } else {
        unsafe {
            env::remove_var("AZURE_CLIENT_ID");
        }
        info!("using system-assigned managed identity");
    }
    let credential: Arc<dyn TokenCredential> = Arc::new(
        AppServiceManagedIdentityCredential::create(
            azure_identity::TokenCredentialOptions::default(),
        )
        .context("failed to build managed identity credential")?,
    );

    let table_credential = Arc::clone(&credential);
    let table_client = Arc::new(table_service_client(&config, table_credential));

    // Start watchdog in background.
    {
        let watchdog_config = config.clone();
        let watchdog_table = TableServiceClient::new(
            format!("https://{}.table.core.windows.net", config.storage_account),
            Arc::clone(&credential),
        );
        let watchdog_cred = Arc::clone(&credential);
        tokio::spawn(async move {
            run_watchdog(watchdog_config, watchdog_table, watchdog_cred).await;
        });
    }

    let server = AzureCpServer {
        config,
        table_client,
        credential,
        http_client: reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .context("failed to build HTTP client")?,
        tool_router: AzureCpServer::tool_router(),
    };

    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .unwrap_or(8080);

    let bind_addr = format!("0.0.0.0:{port}");
    info!(addr = %bind_addr, "listening for MCP connections");

    // Warn loudly if no auth token is configured — the endpoint should always
    // be authenticated when external_ingress = true in Terraform.
    let auth_token: Option<String> = server.config.auth_token.clone();
    if auth_token.is_none() {
        warn!(
            "B00T_CP_AUTH_TOKEN is not set — MCP endpoint is unauthenticated. \
               Only acceptable when external_ingress = false restricts public access."
        );
    }

    // Build StreamableHttpService from the MCP server handler.
    let mcp_service: StreamableHttpService<AzureCpServer, LocalSessionManager> = {
        let s = server.clone();
        StreamableHttpService::new(
            move || Ok(s.clone()),
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default(),
        )
    };

    // Wrap with bearer-token auth middleware (defense-in-depth on top of
    // Terraform's external_ingress = false / ip_security_restriction controls).
    let app = Router::new()
        .nest_service("/mcp", mcp_service)
        .layer(middleware::from_fn(
            move |req: Request, next: middleware::Next| {
                let expected = auth_token.clone();
                async move {
                    if let Some(ref token) = expected {
                        let provided = req
                            .headers()
                            .get(header::AUTHORIZATION)
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.strip_prefix("Bearer "));
                        // Constant-time comparison to avoid timing side-channels.
                        // Seeds acc with the length-mismatch bit so a prefix match never passes.
                        let ta = provided.unwrap_or("").as_bytes();
                        let tb = token.as_bytes();
                        let max_len = ta.len().max(tb.len());
                        let mut acc: u8 = (ta.len() != tb.len()) as u8;
                        for i in 0..max_len {
                            acc |=
                                ta.get(i).copied().unwrap_or(0) ^ tb.get(i).copied().unwrap_or(0);
                        }
                        if acc != 0 {
                            return (
                                StatusCode::UNAUTHORIZED,
                                Json(serde_json::json!({"error": "unauthorized"})),
                            )
                                .into_response();
                        }
                    }
                    next.run(req).await
                }
            },
        ));

    let listener = TcpListener::bind(&bind_addr)
        .await
        .context("failed to bind TCP listener")?;
    axum::serve(listener, app)
        .await
        .context("MCP server error")?;

    Ok(())
}
