# Main b00t OpenTofu configuration
terraform {
  required_version = ">= 1.0"

  # Remote state lives in the GCS bucket stood up by modules/gcp-bootstrap
  # (run once, local state). Backend blocks cannot interpolate — the bucket
  # name is the literal from gcp-bootstrap/variables.tf `state_bucket_name`.
  # First switch-over: `cd b00t-tf && just tf-init-migrate`.
  backend "gcs" {
    bucket = "b00t-tf-state-promptexecution"
    prefix = "gcp-build-plane"
  }

  required_providers {
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 4.0"
    }
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    google = {
      source  = "hashicorp/google"
      version = "~> 6.0"
    }
  }
}

# b00t TOML config reaches OpenTofu as JSON via `just tf-render`
# (scripts/render-tfvars.py -> generated.auto.tfvars.json). See that script's
# header + b00t task #187 for why there is no `toml` provider here.
variable "b00t_config_json" {
  description = "Parsed _b00t_.toml. Populated by generated.auto.tfvars.json (git-ignored, run `just tf-render`). Empty map => hardcoded defaults win."
  type        = any
  default     = {}
}

variable "keyring_keys" {
  description = "The `[[key]]` array from _b00t_/keyring.tomllm. Populated by generated.auto.tfvars.json."
  type        = any
  default     = []
}

variable "dotenv_entries" {
  description = "Flat map from b00t-tf/.env (optional). Populated by generated.auto.tfvars.json — replaces the `germanbrew/dotenv` data source. Precedence: these win over _b00t_.toml."
  type        = map(string)
  default     = {}
}

locals {
  b00t_config = var.b00t_config_json
}

# .env overrides arrive via var.dotenv_entries (generated.auto.tfvars.json,
# `just tf-render`). See scripts/render-tfvars.py.

# Provider configurations with fallback hierarchy: .env -> _b00t_.toml -> hardcoded defaults
provider "cloudflare" {
  api_token = coalesce(
    try(var.dotenv_entries.CLOUDFLARE_API_TOKEN, null),
    try(local.b00t_config.cloudflare.api_token, null)
  )
}

provider "aws" {
  region = coalesce(
    try(var.dotenv_entries.AWS_REGION, null),
    try(local.b00t_config.session.aws_region, null),
    "us-east-1"
  )
}

# GCP build plane. Auth = ambient Application Default Credentials
# (~/.config/gcloud/application_default_credentials.json) — no creds block, no
# key file. Matches modules/gcp-bootstrap and PROVIDER-GCP.provider.tomllmd.
provider "google" {
  project = local.gcp_project_id
  region  = local.gcp_region
}

# Local values with layered configuration: .env overrides _b00t_.toml defaults
locals {
  cloudflare_api_token = coalesce(
    try(var.dotenv_entries.CLOUDFLARE_API_TOKEN, null),
    try(local.b00t_config.cloudflare.api_token, null)
  )

  cloudflare_account_id = coalesce(
    try(var.dotenv_entries.CLOUDFLARE_ACCOUNT_ID, null),
    try(local.b00t_config.cloudflare.account_id, null)
  )

  anthropic_api_key = coalesce(
    try(var.dotenv_entries.ANTHROPIC_API_KEY, null),
    try(local.b00t_config.anthropic.api_key, null)
  )

  aws_region = coalesce(
    try(var.dotenv_entries.AWS_REGION, null),
    try(local.b00t_config.session.aws_region, null),
    "us-east-1"
  )

  project_name = coalesce(
    try(var.dotenv_entries.PROJECT_NAME, null),
    try(local.b00t_config.session.project_name, null),
    "b00t"
  )

  # --- GCP build plane: .env -> _b00t_.toml -> hardcoded default ---
  gcp_project_id = coalesce(
    try(var.dotenv_entries.GCP_PROJECT_ID, null),
    try(local.b00t_config.gcp.project_id, null),
    "promptexecution"
  )

  gcp_region = coalesce(
    try(var.dotenv_entries.GCP_REGION, null),
    try(local.b00t_config.gcp.region, null),
    "australia-southeast1"
  )

  gcp_control_machine_type = coalesce(
    try(var.dotenv_entries.GCP_CONTROL_MACHINE_TYPE, null),
    try(local.b00t_config.gcp.control_machine_type, null),
    "e2-small"
  )

  # Comma-separated in .env (e.g. "1.2.3.4/32,5.6.7.0/24"); empty/unset => no
  # SSH rule (IAP is the address-independent path anyway). `coalesce` rejects
  # an all-empty arg list, so use `try` with an empty-string default directly.
  gcp_allowed_cidrs = compact(split(",", try(var.dotenv_entries.GCP_ALLOWED_CIDRS, "")))

  # "public" (build phase) or "tailnet" (end state — Phase 2.75). Default public.
  gcp_network_mode = coalesce(
    try(var.dotenv_entries.GCP_NETWORK_MODE, null),
    try(local.b00t_config.gcp.network_mode, null),
    "public"
  )

  # Build-plane access list — bare @elastic.ventures emails. _b00t_.toml
  # [gcp].access_accounts (a list), or .env GCP_ACCESS_ACCOUNTS (comma-sep).
  # Each gets roles/iap.tunnelResourceAccessor on the control instance.
  gcp_access_accounts = distinct(concat(
    try(local.b00t_config.gcp.access_accounts, []),
    compact(split(",", try(var.dotenv_entries.GCP_ACCESS_ACCOUNTS, ""))),
  ))

  # Control-node authorized SSH keys, from the keyring datum (var.keyring_keys):
  # active entries whose `scope` includes "gcp-build-plane-control", formatted
  # "<user>:<public_key>" and newline-joined for GCE `ssh-keys` metadata.
  control_authorized_ssh_keys = join("\n", [
    for k in var.keyring_keys : "${k.user}:${k.public_key}"
    if try(k.status, "") == "active" && contains(try(k.scope, []), "gcp-build-plane-control")
  ])
}

# Base module
module "base" {
  source = "./modules/base"

  cloudflare_api_token  = local.cloudflare_api_token
  cloudflare_account_id = local.cloudflare_account_id
  aws_region            = local.aws_region
  project_name          = local.project_name
}

# Cloudflare module
module "cloudflare" {
  source = "./modules/cloudflare"

  account_id        = local.cloudflare_account_id
  project_name      = local.project_name
  anthropic_api_key = local.anthropic_api_key
}

# The Artifact Registry repo for the waker image was created out of band (it
# must exist before the image can be pushed, which must happen before the Cloud
# Run waker can apply cleanly). Adopt it on the next apply. Safe to leave in
# place — a no-op once imported.
import {
  to = module.gcp_build_plane.google_artifact_registry_repository.containers
  id = "projects/promptexecution/locations/australia-southeast1/repositories/b00t"
}

# GCP build plane — control node + build-VM identity + buildcache + GitHub WIF
# + scale-to-zero waker. Apply scoped: `just gcp-apply`.
module "gcp_build_plane" {
  source = "./modules/gcp-build-plane"

  project_id                  = local.gcp_project_id
  region                      = local.gcp_region
  control_machine_type        = local.gcp_control_machine_type
  control_authorized_ssh_keys = local.control_authorized_ssh_keys
  allowed_cidrs               = local.gcp_allowed_cidrs
  network_mode                = local.gcp_network_mode
  access_accounts             = local.gcp_access_accounts

  # Optional spend-alert budget. Set [gcp].budget_billing_account in _b00t_.toml
  # (or GCP_BUDGET_BILLING_ACCOUNT in .env) to enable. Empty = no budget resource.
  # 🤓 `try`, not `coalesce`: OpenTofu's coalesce rejects an all-empty arg list,
  # so the "" fallback (meaning "no budget") crashed the apply once
  # var.dotenv_entries went empty. try() returns the first non-erroring value.
  budget_billing_account = try(
    var.dotenv_entries.GCP_BUDGET_BILLING_ACCOUNT,
    local.b00t_config.gcp.budget_billing_account,
    "",
  )
}

# Outputs
output "worker_url" {
  description = "Cloudflare Worker URL"
  value       = "https://${module.cloudflare.worker_script_name}.${local.cloudflare_account_id}.workers.dev"
}

output "mcp_endpoints" {
  description = "MCP API endpoints"
  value = {
    providers = "https://${module.cloudflare.worker_script_name}.${local.cloudflare_account_id}.workers.dev/mcp/providers"
    tools     = "https://${module.cloudflare.worker_script_name}.${local.cloudflare_account_id}.workers.dev/mcp/tools"
    generate  = "https://${module.cloudflare.worker_script_name}.${local.cloudflare_account_id}.workers.dev/mcp/generate"
  }
}

# --- GCP build plane ---
output "gcp_control_node_endpoint" {
  description = "Public pingap waker URL — `dstack project add --url` target."
  value       = module.gcp_build_plane.control_node_endpoint
}

output "gcp_control_node_external_ip" {
  description = "Control-node external IP (pyinfra SSH / break-glass tunnel)."
  value       = module.gcp_build_plane.control_node_external_ip
}

output "gcp_control_zone" {
  description = "Control-node zone (e.g. for `gcloud compute instances stop`)."
  value       = module.gcp_build_plane.control_zone
}

output "gcp_build_plane_network" {
  description = "Build-plane VPC self-link (dstack backend `vpc_name`)."
  value       = module.gcp_build_plane.network_self_link
}

output "gcp_buildcache_bucket" {
  description = "Shared build-cache GCS bucket."
  value       = module.gcp_build_plane.buildcache_bucket
}

output "gcp_build_vm_sa_email" {
  description = "Set as `vm_service_account` in the dstack GCP backend config."
  value       = module.gcp_build_plane.build_vm_sa_email
}

output "gcp_wif_provider_name" {
  description = "`workload_identity_provider` input for google-github-actions/auth."
  value       = module.gcp_build_plane.wif_provider_name
}

output "gcp_ci_sa_email" {
  description = "SA that GitHub Actions impersonates via WIF."
  value       = module.gcp_build_plane.ci_sa_email
}