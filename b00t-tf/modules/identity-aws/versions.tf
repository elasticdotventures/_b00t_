# Standalone `tofu validate` of this module. The ROOT supplies the configured
# `provider "aws"`; this module declares none and inherits it.
terraform {
  required_version = ">= 1.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }
}
