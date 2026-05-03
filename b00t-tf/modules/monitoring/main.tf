# Monitoring Module
# Basic monitoring and alerting for b00t infrastructure

terraform {
  required_providers {
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 4.0"
    }
  }
}

# Variables
variable "cloudflare_account_id" {
  description = "Cloudflare account ID"
  type        = string
}

variable "project_name" {
  description = "Project name for resource naming"
  type        = string
  default     = "b00t"
}

variable "notification_email" {
  description = "Email for notifications"
  type        = string
  default     = "alerts@b00t.dev"
}

# Basic monitoring - can be expanded later
# For now, just outputs placeholder values

output "monitoring_enabled" {
  description = "Monitoring status"
  value       = true
}

output "notification_email" {
  description = "Email for notifications"
  value       = var.notification_email
}