//! b00t Azure Control Plane — MCP server for on-demand ACI compute.
//!
//! Exposes five MCP tools:
//!   azure.provision_aci  — spin up an ACI container, return endpoint_url + lease_id
//!   azure.deprovision    — tear down by lease_id
//!   azure.heartbeat      — renew lease TTL
//!   azure.list_leases    — active resources + TTLs
//!   azure.cost_estimate  — current-month spend for the control plane RG
//!
//! Lease state stored in Azure Table Storage (b00tLeases table).
//! Background watchdog tears down leases where expires_at < now().

use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use azure_core::credentials::TokenCredential;
use azure_data_tables::clients::TableServiceClient;
use azure_identity::ManagedIdentityCredential;
use chrono::{DateTime, Utc};
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{Content, ServerCapabilities, ServerInfo};
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_box, Error as McpError, RoleServer, ServerHandler};
use serde::{Deserialize, Serialize};
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
    client_id: String,
    lease_ttl_minutes: i64,
    location: String,
}

impl Config {
    fn from_env() -> Result<Self> {
        Ok(Self {
            node_id: env::var("B00T_NODE_ID").context("B00T_NODE_ID not set")?,
            storage_account: env::var("AZURE_STORAGE_ACCOUNT_NAME")
                .context("AZURE_STORAGE_ACCOUNT_NAME not set")?,
            table_name: env::var("AZURE_TABLE_NAME")
                .unwrap_or_else(|_| "b00tLeases".to_string()),
            resource_group: env::var("AZURE_RESOURCE_GROUP")
                .context("AZURE_RESOURCE_GROUP not set")?,
            // ACA passes the full resource group ID; extract subscription from it.
            subscription_id: env::var("AZURE_SUBSCRIPTION_ID")
                .context("AZURE_SUBSCRIPTION_ID not set")?,
            client_id: env::var("AZURE_CLIENT_ID").context("AZURE_CLIENT_ID not set")?,
            lease_ttl_minutes: env::var("LEASE_TTL_MINUTES")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
            location: env::var("AZURE_LOCATION").context("AZURE_LOCATION not set")?,
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

// ---------------------------------------------------------------------------
// Azure Table Storage helpers
// ---------------------------------------------------------------------------

fn table_service_client(config: &Config, credential: Arc<dyn TokenCredential>) -> TableServiceClient {
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
        .insert_or_replace_entity(entity)
        .await
        .context("failed to upsert lease entity")?;
    Ok(())
}

async fn delete_lease(client: &TableServiceClient, config: &Config, lease_id: &str) -> Result<()> {
    let table_client = client.table_client(&config.table_name);
    table_client
        .entity_client(&config.node_id, lease_id)
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
    let filter = format!("PartitionKey eq '{}'", config.node_id);
    let mut entities: Vec<LeaseEntity> = Vec::new();
    let mut stream = table_client
        .query()
        .filter(&filter)
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
    let subscription_id = if config.subscription_id.starts_with('/') {
        config
            .subscription_id
            .split('/')
            .nth(2)
            .unwrap_or(&config.subscription_id)
            .to_string()
    } else {
        config.subscription_id.clone()
    };

    let aci_client = azure_mgmt_containerinstance::Client::new(
        format!("https://management.azure.com"),
        _credential,
        azure_mgmt_containerinstance::ClientOptions::default(),
    );

    use azure_mgmt_containerinstance::models::{
        Container, ContainerGroup, ContainerGroupIpAddress, ContainerGroupProperties,
        ContainerGroupSubnetId, ContainerPort, ContainerProperties, GpuResource, GpuSku,
        ImageRegistryCredential, IpAddressType, OperatingSystemTypes, Port, ResourceRequests,
        ResourceRequirements,
    };

    let mut container_props = ContainerProperties {
        image: input.image.clone(),
        resources: ResourceRequirements {
            requests: ResourceRequests {
                cpu: input.cpu,
                memory_in_gb: input.memory_gb,
                gpu: if input.gpu_count > 0 {
                    Some(GpuResource {
                        count: input.gpu_count as i32,
                        sku: match input.gpu_sku.as_deref().unwrap_or("V100") {
                            "K80" => GpuSku::K80,
                            "P100" => GpuSku::P100,
                            _ => GpuSku::V100,
                        },
                    })
                } else {
                    None
                },
            },
            limits: None,
        },
        ports: Some(vec![ContainerPort {
            port: input.port as i32,
            protocol: None,
        }]),
        ..Default::default()
    };

    let container_group = ContainerGroup {
        location: Some(config.location.clone()),
        properties: Some(ContainerGroupProperties {
            containers: vec![Container {
                name: group_name.clone(),
                properties: container_props,
            }],
            os_type: OperatingSystemTypes::Linux,
            ip_address: Some(ContainerGroupIpAddress {
                r#type: IpAddressType::Public,
                ports: vec![Port {
                    port: input.port as i32,
                    protocol: None,
                }],
                ip: None,
                dns_name_label: Some(group_name.clone()),
                fqdn: None,
                auto_generated_domain_name_label_scope: None,
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

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

    // Prefer the FQDN Azure assigns to the container group; fall back to the
    // deterministic pattern using the provisioned location.
    let fqdn = created
        .properties
        .as_ref()
        .and_then(|p| p.ip_address.as_ref())
        .and_then(|ip| ip.fqdn.as_deref())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let fallback = format!(
                "{}.{}.azurecontainer.io",
                group_name, config.location
            );
            warn!(group = %group_name, fallback = %fallback, "FQDN not present in create response; using constructed fallback");
            fallback
        });
    let endpoint_url = format!("http://{}:{}", fqdn, input.port);

    info!(group = %group_name, endpoint = %endpoint_url, "ACI container group created");
    Ok(endpoint_url)
}

async fn deprovision_aci_resource(
    config: &Config,
    credential: Arc<dyn TokenCredential>,
    resource_id: &str,
) -> Result<()> {
    let subscription_id = if config.subscription_id.starts_with('/') {
        config
            .subscription_id
            .split('/')
            .nth(2)
            .unwrap_or(&config.subscription_id)
            .to_string()
    } else {
        config.subscription_id.clone()
    };

    let aci_client = azure_mgmt_containerinstance::Client::new(
        "https://management.azure.com",
        credential,
        azure_mgmt_containerinstance::ClientOptions::default(),
    );

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
                        if let Err(e) =
                            delete_lease(&table_client, &config, &lease.row_key).await
                        {
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
}

#[tool_box]
impl AzureCpServer {
    /// Spin up an ACI container on demand. Returns endpoint_url and lease_id.
    /// The lease expires after the configured TTL unless renewed via heartbeat.
    #[tool(name = "azure.provision_aci")]
    async fn provision_aci(&self, input: ProvisionAciInput) -> Result<serde_json::Value, McpError> {
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

        Ok(serde_json::json!({
            "lease_id": lease_id,
            "endpoint_url": endpoint_url,
            "expires_at": expires_at.to_rfc3339(),
            "ttl_minutes": ttl,
        }))
    }

    /// Tear down a provisioned resource immediately by lease_id.
    #[tool(name = "azure.deprovision")]
    async fn deprovision(&self, input: DeprovisionInput) -> Result<serde_json::Value, McpError> {
        // Look up the lease to get the resource_id.
        let table_client = self.table_client.table_client(&self.config.table_name);
        let entity: LeaseEntity = table_client
            .entity_client(&self.config.node_id, &input.lease_id)
            .get()
            .await
            .map_err(|e| McpError::internal_error(format!("lease not found: {e}"), None))?
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

        Ok(serde_json::json!({
            "lease_id": input.lease_id,
            "status": "deprovisioned",
        }))
    }

    /// Renew a lease TTL. Call periodically to keep resources alive past the default TTL.
    #[tool(name = "azure.heartbeat")]
    async fn heartbeat(&self, input: HeartbeatInput) -> Result<serde_json::Value, McpError> {
        let table_client = self.table_client.table_client(&self.config.table_name);
        let mut entity: LeaseEntity = table_client
            .entity_client(&self.config.node_id, &input.lease_id)
            .get()
            .await
            .map_err(|e| McpError::internal_error(format!("lease not found: {e}"), None))?
            .entity;

        let ttl = input
            .ttl_minutes
            .unwrap_or(self.config.lease_ttl_minutes);
        let new_expires = Utc::now() + chrono::Duration::minutes(ttl);
        entity.expires_at = new_expires.to_rfc3339();

        upsert_lease(&self.table_client, &self.config, &entity)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        Ok(serde_json::json!({
            "lease_id": input.lease_id,
            "new_expires_at": new_expires.to_rfc3339(),
            "ttl_minutes": ttl,
        }))
    }

    /// List all active leases with endpoint URLs, TTLs, and resource identifiers.
    #[tool(name = "azure.list_leases")]
    async fn list_leases(&self) -> Result<serde_json::Value, McpError> {
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

        Ok(serde_json::json!({
            "node_id": self.config.node_id,
            "lease_count": items.len(),
            "leases": items,
        }))
    }

    /// Estimated current-month Azure spend for the control plane resource group.
    /// Requires Cost Management Reader on the resource group.
    #[tool(name = "azure.cost_estimate")]
    async fn cost_estimate(&self) -> Result<serde_json::Value, McpError> {
        // Use the Azure Cost Management REST API via azure_core.
        let subscription_id = if self.config.subscription_id.starts_with('/') {
            self.config
                .subscription_id
                .split('/')
                .nth(2)
                .unwrap_or(&self.config.subscription_id)
                .to_string()
        } else {
            self.config.subscription_id.clone()
        };

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

        let client = reqwest::Client::new();
        let resp = client
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
            Ok(serde_json::json!({
                "resource_group": self.config.resource_group,
                "timeframe": "MonthToDate",
                "data": data,
            }))
        } else {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            Ok(serde_json::json!({
                "resource_group": self.config.resource_group,
                "error": format!("HTTP {}: {}", status, text),
                "note": "Cost Management Reader role required on the resource group.",
            }))
        }
    }
}

#[tool_box]
impl ServerHandler for AzureCpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            name: "b00t-azure-cp".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            ..Default::default()
        }
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

    // Use managed identity (UserAssigned) when running in ACA.
    let credential: Arc<dyn TokenCredential> = Arc::new(
        ManagedIdentityCredential::new(Some(config.client_id.clone()))
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
    };

    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .unwrap_or(8080);

    let bind_addr = format!("0.0.0.0:{port}");
    info!(addr = %bind_addr, "listening for MCP connections");

    // Serve via streamable HTTP transport (MCP over HTTP).
    rmcp::transport::streamable_http_server::serve_server(
        server,
        bind_addr.parse()?,
    )
    .await
    .context("MCP server error")?;

    Ok(())
}
