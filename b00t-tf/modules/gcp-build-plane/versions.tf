# Provider requirements for standalone `tofu validate` of this module.
# The ROOT module supplies the configured `provider "google"` — this module
# declares no `provider` block and inherits it.
terraform {
  required_version = ">= 1.0"

  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 6.0"
    }
  }
}
