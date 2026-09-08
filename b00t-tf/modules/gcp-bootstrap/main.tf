# gcp-bootstrap — one-time GCP bootstrap for the b00t build plane.
#
# Purpose: create the GCS bucket that the ROOT module's `backend "gcs"` block
# will use for remote state, and enable the GCP APIs the main `gcp-build-plane`
# module needs. This resolves the chicken-and-egg: you cannot configure
# `backend "gcs"` against a bucket that does not exist yet.
#
# This module keeps its OWN state LOCAL (no backend block below). Run it once
# with operator Application Default Credentials:
#
#   cd b00t-tf && just gcp-bootstrap
#
# See docs/runbooks/remote-build-server.md and
# docs/superpowers/specs/2026-08-10-cloud-build-server-design.md.

terraform {
  required_version = ">= 1.0"

  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 6.0"
    }
  }
}

provider "google" {
  project = var.project_id
  region  = var.region
  # Auth is ambient Application Default Credentials
  # (~/.config/gcloud/application_default_credentials.json) — no creds block,
  # no key file. Matches PROVIDER-GCP.provider.tomllmd.
}

# --- Remote-state bucket ----------------------------------------------------
# Name is referenced verbatim by the root module's `backend "gcs"` block, which
# cannot take variables — keep var.state_bucket_name and that string in lockstep.
resource "google_storage_bucket" "tf_state" {
  name                        = var.state_bucket_name
  location                    = var.region
  storage_class               = "STANDARD"
  force_destroy               = false
  uniform_bucket_level_access = true
  public_access_prevention    = "enforced"

  versioning {
    enabled = true
  }

  # Keep the 10 most recent non-current versions; expire older ones.
  lifecycle_rule {
    condition {
      num_newer_versions = 10
      with_state         = "ARCHIVED"
    }
    action {
      type = "Delete"
    }
  }

  labels = {
    managed-by = "opentofu"
    module     = "gcp-bootstrap"
    purpose    = "tf-remote-state"
  }

  # A state bucket must not be destroyable by an accidental `tofu destroy`.
  # To decommission (see the runbook): migrate root state back to local with
  # `tofu init -migrate-state`, then remove this block and re-apply before
  # `just gcp-bootstrap-destroy`.
  lifecycle {
    prevent_destroy = true
  }
}

# --- Required API enablement ----------------------------------------------
# disable_on_destroy = false: tearing down bootstrap must never disable APIs
# that other project resources (or the main module) still depend on.
resource "google_project_service" "api" {
  for_each = toset([
    "cloudresourcemanager.googleapis.com",
    "compute.googleapis.com",
    "iam.googleapis.com",
    "iamcredentials.googleapis.com",
    "sts.googleapis.com",
    "storage.googleapis.com",
    "serviceusage.googleapis.com",
  ])

  service                    = each.value
  disable_on_destroy         = false
  disable_dependent_services = false
}
