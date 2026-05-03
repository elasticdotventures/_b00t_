# Base infrastructure module for b00t
terraform {
  required_providers {
    cloudflare = {
      source = "cloudflare/cloudflare"
      version = "~> 4.0"
    }
    aws = {
      source = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}

variable "cloudflare_api_token" {
  description = "Cloudflare API token"
  type        = string
  sensitive   = true
}

variable "cloudflare_account_id" {
  description = "Cloudflare account ID"
  type        = string
}

variable "aws_region" {
  description = "AWS region"
  type        = string
  default     = "us-east-1"
}

variable "project_name" {
  description = "Project name for resource naming"
  type        = string
  default     = "b00t"
}

# Outputs for other modules
output "project_name" {
  value = var.project_name
}

output "cloudflare_account_id" {
  value = var.cloudflare_account_id
}