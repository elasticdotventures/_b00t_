variable "node_id" {
  description = "Unique identifier for this b00t node (used in resource names and lease partition key)."
  type        = string

  validation {
    condition = can(regex("^[a-z0-9-]+$", var.node_id)) && length(var.node_id) >= 3 && length(var.node_id) <= 32
    error_message = "node_id must be 3-32 characters long and contain only lowercase letters, numbers, and hyphens."
  }
}

variable "location" {
  description = "Azure region. Use a region with ACA + ACI support (e.g. australiaeast)."
  type        = string
  default     = "australiaeast"
}

variable "min_replicas" {
  description = "ACA minimum replicas. 0 = scale-to-zero (free when idle, ~30s cold start). 1 = always-warm (~$3/month)."
  type        = number
  default     = 0

  validation {
    condition     = var.min_replicas >= 0 && var.min_replicas <= 1
    error_message = "min_replicas must be 0 (scale-to-zero) or 1 (always-warm)."
  }
}

variable "lease_ttl_minutes" {
  description = "Default lease TTL in minutes. Resources are torn down if no heartbeat arrives within this window."
  type        = number
  default     = 30
}

variable "budget_aud" {
  description = "Monthly spend alert threshold in AUD for the control plane resource group."
  type        = number
  default     = 10
}

variable "alert_emails" {
  description = "Email addresses for budget alert notifications."
  type        = list(string)
  default     = []
}

variable "control_plane_image" {
  description = "Container image for the b00t Azure control plane server."
  type        = string
  default     = "ghcr.io/elasticdotventures/b00t-azure-cp:latest"
}

variable "tags" {
  description = "Tags applied to all resources in this module."
  type        = map(string)
  default     = {}
}
