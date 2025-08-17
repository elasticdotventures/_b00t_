# Cloudflare infrastructure for b00t
terraform {
  required_providers {
    cloudflare = {
      source = "cloudflare/cloudflare"
      version = "~> 4.0"
    }
  }
}

variable "account_id" {
  description = "Cloudflare account ID"
  type        = string
}

variable "project_name" {
  description = "Project name for resource naming"
  type        = string
}

variable "zone_name" {
  description = "Domain zone name (optional)"
  type        = string
  default     = ""
}

# Cloudflare Workers for MCP proxy
resource "cloudflare_worker_script" "mcp_proxy" {
  account_id = var.account_id
  name       = "${var.project_name}-mcp-proxy"
  content    = file("${path.module}/worker.js")
  
  plain_text_binding {
    name = "ENVIRONMENT"
    text = "production"
  }

  secret_text_binding {
    name = "ANTHROPIC_API_KEY"
    text = var.anthropic_api_key
  }
}

# KV namespace for configuration storage
resource "cloudflare_workers_kv_namespace" "config" {
  account_id = var.account_id
  title      = "${var.project_name}-config"
}

variable "anthropic_api_key" {
  description = "Anthropic API key"
  type        = string
  sensitive   = true
}

# Outputs
output "worker_script_name" {
  value = cloudflare_worker_script.mcp_proxy.name
}

output "kv_namespace_id" {
  value = cloudflare_workers_kv_namespace.config.id
}