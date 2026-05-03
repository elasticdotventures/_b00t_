# Main b00t OpenTofu configuration
terraform {
  required_version = ">= 1.0"
  
  required_providers {
    cloudflare = {
      source = "cloudflare/cloudflare"
      version = "~> 4.0"
    }
    aws = {
      source = "hashicorp/aws"
      version = "~> 5.0"
    }
    dotenv = {
      source = "germanbrew/dotenv"
      version = "~> 1.0"
    }
    toml = {
      source = "tobotimus/toml"
      version = "~> 1.0"
    }
  }
}

# Read b00t session defaults from TOML
locals {
  b00t_config = provider::toml::decode(file("_b00t_.toml"))
}

# Read environment variables from .env file (optional overrides)
data "dotenv" "config" {
  filename = ".env"
}

# Provider configurations with fallback hierarchy: .env -> _b00t_.toml -> hardcoded defaults
provider "cloudflare" {
  api_token = coalesce(
    try(data.dotenv.config.env.CLOUDFLARE_API_TOKEN, null),
    try(local.b00t_config.cloudflare.api_token, null)
  )
}

provider "aws" {
  region = coalesce(
    try(data.dotenv.config.env.AWS_REGION, null),
    try(local.b00t_config.session.aws_region, null),
    "us-east-1"
  )
}

# Local values with layered configuration: .env overrides _b00t_.toml defaults
locals {
  cloudflare_api_token = coalesce(
    try(data.dotenv.config.env.CLOUDFLARE_API_TOKEN, null),
    try(local.b00t_config.cloudflare.api_token, null)
  )
  
  cloudflare_account_id = coalesce(
    try(data.dotenv.config.env.CLOUDFLARE_ACCOUNT_ID, null),
    try(local.b00t_config.cloudflare.account_id, null)
  )
  
  anthropic_api_key = coalesce(
    try(data.dotenv.config.env.ANTHROPIC_API_KEY, null),
    try(local.b00t_config.anthropic.api_key, null)
  )
  
  aws_region = coalesce(
    try(data.dotenv.config.env.AWS_REGION, null),
    try(local.b00t_config.session.aws_region, null),
    "us-east-1"
  )
  
  project_name = coalesce(
    try(data.dotenv.config.env.PROJECT_NAME, null),
    try(local.b00t_config.session.project_name, null),
    "b00t"
  )
}

# Base module
module "base" {
  source = "./modules/base"
  
  cloudflare_api_token  = local.cloudflare_api_token
  cloudflare_account_id = local.cloudflare_account_id
  aws_region           = local.aws_region
  project_name         = local.project_name
}

# Cloudflare module
module "cloudflare" {
  source = "./modules/cloudflare"
  
  account_id        = local.cloudflare_account_id
  project_name      = local.project_name
  anthropic_api_key = local.anthropic_api_key
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