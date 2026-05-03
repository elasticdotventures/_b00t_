output "mcp_endpoint" {
  description = "MCP HTTP endpoint URL for the b00t Azure control plane."
  value       = "https://${azurerm_container_app.cp.latest_revision_fqdn}/mcp"
}

output "aca_fqdn" {
  description = "ACA app FQDN (without path)."
  value       = azurerm_container_app.cp.latest_revision_fqdn
}

output "resource_group_name" {
  description = "Resource group name for the control plane."
  value       = azurerm_resource_group.cp.name
}

output "storage_account_name" {
  description = "Storage account name for lease state."
  value       = azurerm_storage_account.cp.name
}

output "identity_client_id" {
  description = "Client ID of the managed identity used by the control plane."
  value       = azurerm_user_assigned_identity.cp.client_id
}
