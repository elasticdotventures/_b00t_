# ---------------------------------------------------------------------------
# b00t Azure Control Plane — ACA MCP server with lease-managed ACI resources
# ---------------------------------------------------------------------------
# This module provisions a lightweight Rust MCP server running on Azure
# Container Apps. It exposes MCP tools for spinning up / tearing down
# on-demand ACI containers (GPU inference, etc.) with automatic TTL expiry.
#
# Architecture:
#   claude-cli/opencode → MCP (HTTP) → b00t-azure-cp (ACA)
#                                           ├── provision ACI containers
#                                           ├── manage leases in Table Storage
#                                           └── watchdog task (auto-teardown)
#
# RECURRING COSTS:
#   min_replicas=0: free when idle (~30s cold start on first MCP call)
#   min_replicas=1: ~$3 AUD/month always-warm, no cold start
#   Table Storage: negligible (<$0.01/month at low write volume)
#   ACI containers: billed only while provisioned via azure.provision_aci tool
# ---------------------------------------------------------------------------

terraform {
  required_providers {
    azurerm = {
      source  = "hashicorp/azurerm"
      version = "~> 4.0"
    }
  }
}

locals {
  rg_name    = "rg-b00t-control-${var.node_id}"
  app_name   = "b00t-cp-${var.node_id}"
  sa_name    = "b00tsa${replace(var.node_id, "-", "")}"
  table_name = "b00tLeases"
  budget_start_date = formatdate("YYYY-MM-01T00:00:00Z", timestamp())
}

# ---------------------------------------------------------------------------
# Resource group — scoped to this node, blast radius limited
# ---------------------------------------------------------------------------
resource "azurerm_resource_group" "cp" {
  name     = local.rg_name
  location = var.location
  tags     = var.tags
}

# ---------------------------------------------------------------------------
# User-assigned managed identity — used by the ACA app to manage ACI/ARM
# ---------------------------------------------------------------------------
resource "azurerm_user_assigned_identity" "cp" {
  name                = "id-b00t-cp-${var.node_id}"
  location            = var.location
  resource_group_name = azurerm_resource_group.cp.name
  tags                = var.tags
}

# Contributor on this RG only — sufficient to provision/deprovision ACI containers.
# Blast radius: limited to rg-b00t-control-{node_id}.
resource "azurerm_role_assignment" "cp_contributor" {
  scope                = azurerm_resource_group.cp.id
  role_definition_name = "Contributor"
  principal_id         = azurerm_user_assigned_identity.cp.principal_id
}

# ---------------------------------------------------------------------------
# Table Storage — lease state store for active ACI resources
# ---------------------------------------------------------------------------
# Table: b00tLeases
#   PartitionKey: node_id
#   RowKey:       lease_id (uuid)
#   Fields:       resource_id, resource_type, endpoint_url, expires_at,
#                 created_at, client_hint
#
# RECURRING COST: Standard LRS Storage ~$0.02/GB/month + $0.0004/10k ops.
# At low write volume this is < $0.01/month.
resource "azurerm_storage_account" "cp" {
  name                            = local.sa_name
  location                        = var.location
  resource_group_name             = azurerm_resource_group.cp.name
  account_tier                    = "Standard"
  account_replication_type        = "LRS"
  min_tls_version                 = "TLS1_2"
  allow_nested_items_to_be_public = false
  tags                            = var.tags
}

resource "azurerm_storage_table" "leases" {
  name                 = local.table_name
  storage_account_name = azurerm_storage_account.cp.name
}

# Storage Table Data Contributor — lets the ACA app read/write lease records.
resource "azurerm_role_assignment" "cp_storage_table" {
  scope                = azurerm_storage_account.cp.id
  role_definition_name = "Storage Table Data Contributor"
  principal_id         = azurerm_user_assigned_identity.cp.principal_id
}

# ---------------------------------------------------------------------------
# ACA environment + control plane app
# ---------------------------------------------------------------------------
resource "azurerm_container_app_environment" "cp" {
  name                = "cae-b00t-cp-${var.node_id}"
  location            = var.location
  resource_group_name = azurerm_resource_group.cp.name
  tags                = var.tags
}

resource "azurerm_container_app" "cp" {
  name                         = local.app_name
  container_app_environment_id = azurerm_container_app_environment.cp.id
  resource_group_name          = azurerm_resource_group.cp.name
  revision_mode                = "Single"
  tags                         = var.tags

  identity {
    type         = "UserAssigned"
    identity_ids = [azurerm_user_assigned_identity.cp.id]
  }

  ingress {
    external_enabled = true
    target_port      = 8080
    traffic_weight {
      percentage      = 100
      latest_revision = true
    }
  }

  template {
    min_replicas = var.min_replicas
    max_replicas = 1

    container {
      name   = "b00t-azure-cp"
      image  = var.control_plane_image
      cpu    = 0.25
      memory = "0.5Gi"

      env {
        name  = "B00T_NODE_ID"
        value = var.node_id
      }
      env {
        name  = "AZURE_STORAGE_ACCOUNT_NAME"
        value = azurerm_storage_account.cp.name
      }
      env {
        name  = "AZURE_TABLE_NAME"
        value = local.table_name
      }
      env {
        name  = "AZURE_RESOURCE_GROUP"
        value = azurerm_resource_group.cp.name
      }
      env {
        name  = "AZURE_SUBSCRIPTION_ID"
        value = azurerm_resource_group.cp.id  # parsed at runtime
      }
      env {
        name  = "AZURE_CLIENT_ID"
        value = azurerm_user_assigned_identity.cp.client_id
      }
      env {
        name  = "LEASE_TTL_MINUTES"
        value = tostring(var.lease_ttl_minutes)
      }
      env {
        name  = "PORT"
        value = "8080"
      }
    }
  }

  depends_on = [
    azurerm_role_assignment.cp_contributor,
    azurerm_role_assignment.cp_storage_table,
  ]
}

# ---------------------------------------------------------------------------
# Budget guard — alerts at 80% forecasted and 100% actual spend.
# Does NOT auto-stop resources — alerts only.
# RECURRING COST: free (consumption budgets have no charge).
# ---------------------------------------------------------------------------
resource "azurerm_consumption_budget_resource_group" "cp_budget" {
  count             = length(var.alert_emails) > 0 ? 1 : 0
  name              = "b00t-cp-budget-${var.node_id}"
  resource_group_id = azurerm_resource_group.cp.id

  amount     = var.budget_aud
  time_grain = "Monthly"

  time_period {
    start_date = local.budget_start_date
  }

  notification {
    enabled        = true
    threshold      = 80
    operator       = "GreaterThan"
    threshold_type = "Forecasted"
    contact_emails = var.alert_emails
  }

  notification {
    enabled        = true
    threshold      = 100
    operator       = "GreaterThan"
    threshold_type = "Actual"
    contact_emails = var.alert_emails
  }
}
