# ---------------------------------------------------------------------------
# Keyless GCP identity for the kubernetes backend (k0s on b00t-node) and an
# Entra ID (Azure AD) provider — retires the two long-lived keys from the
# k0s/SOCI PR. Plan gleaming-jingling-nygaard.
#
# ⚠️ The `k0s-oidc` provider (raw k0s ServiceAccount-token issuer) is a
# DELIBERATELY TEMPORARY BOOTSTRAP. The multi-cloud (GCP+Azure+AWS) end state
# uses a SPIRE trust domain as the single OIDC issuer that every cloud
# federates against — see
# docs/superpowers/specs/2026-09-09-identity-plane-minimum-requirements.md.
# When SPIRE lands, add a `spire-oidc` provider in this same `external-idp`
# pool and point `k0s_oidc_issuer_uri` at SPIRE's discovery URL; the SA
# bindings, attribute_mapping and gcs-obj.sh do not change. Do NOT add a
# second per-cluster issuer here.
#
# Everything is gated: with k0s_oidc_issuer_uri = "" nothing is created, so
# `tofu validate` / existing applies are unaffected.
#
# See also docs/superpowers/specs/2026-09-09-keyless-gcp-identity-k0s.md and
# .../2026-09-09-spiffe-spire-cross-cloud.md.
# ---------------------------------------------------------------------------

locals {
  k0s_wif_enabled   = var.k0s_oidc_issuer_uri != ""
  spire_wif_enabled = var.spire_oidc_issuer_uri != ""
  entra_wif_enabled = var.entra_tenant_id != ""
  any_wif_enabled   = local.k0s_wif_enabled || local.spire_wif_enabled || local.entra_wif_enabled
}

# A single pool holds both the k0s-issuer and the Entra providers.
resource "google_iam_workload_identity_pool" "external" {
  count                     = local.any_wif_enabled ? 1 : 0
  workload_identity_pool_id = "external-idp"
  display_name              = "External IdPs (k0s, Entra)"
  description               = "Keyless federation for the kubernetes backend and Azure AD."
}

# --- k0s ServiceAccount-token issuer -----------------------------------------
resource "google_iam_workload_identity_pool_provider" "k0s_oidc" {
  count                              = local.k0s_wif_enabled ? 1 : 0
  workload_identity_pool_id          = google_iam_workload_identity_pool.external[0].workload_identity_pool_id
  workload_identity_pool_provider_id = "k0s-oidc"
  display_name                       = "k0s SA issuer"

  attribute_mapping = {
    "google.subject"      = "assertion.sub"
    "attribute.namespace" = "assertion['kubernetes.io']['namespace']"
    "attribute.ksa"       = "assertion['kubernetes.io']['serviceaccount']['name']"
  }
  # Lock to the exact ServiceAccount that runs build-plane tasks.
  attribute_condition = "assertion.sub == \"${var.k0s_wif_subject}\""

  oidc {
    issuer_uri = var.k0s_oidc_issuer_uri
  }
}

# --- SPIRE trust domain (the end-state issuer, identity-plane step 3) ------
# When set, this supersedes k0s-oidc: point spire_oidc_issuer_uri at the SPIRE
# OIDC Discovery Provider URL and the same workload SA is reachable via a
# SPIFFE-ID subject. k0s-oidc can then be removed.
resource "google_iam_workload_identity_pool_provider" "spire_oidc" {
  count                              = local.spire_wif_enabled ? 1 : 0
  workload_identity_pool_id          = google_iam_workload_identity_pool.external[0].workload_identity_pool_id
  workload_identity_pool_provider_id = "spire-oidc"
  display_name                       = "SPIRE (${var.trust_domain})"

  attribute_mapping = {
    "google.subject" = "assertion.sub" # spiffe://<trust-domain>/...
  }
  attribute_condition = "assertion.sub == \"${var.spire_spiffe_id}\""

  oidc {
    issuer_uri = var.spire_oidc_issuer_uri
  }
}

resource "google_service_account_iam_member" "spire_wif_user" {
  count              = local.spire_wif_enabled ? 1 : 0
  service_account_id = google_service_account.k0s_workload[0].name
  role               = "roles/iam.workloadIdentityUser"
  member             = "principalSet://iam.googleapis.com/${google_iam_workload_identity_pool.external[0].name}/subject/${var.spire_spiffe_id}"
}

# --- Entra ID (Azure AD) ---------------------------------------------------
resource "google_iam_workload_identity_pool_provider" "entra" {
  count                              = local.entra_wif_enabled ? 1 : 0
  workload_identity_pool_id          = google_iam_workload_identity_pool.external[0].workload_identity_pool_id
  workload_identity_pool_provider_id = "entra-oidc"
  display_name                       = "Entra ID"

  attribute_mapping = {
    "google.subject"  = "assertion.sub"
    "attribute.appid" = "assertion.appid"
  }
  attribute_condition = var.entra_wif_condition

  oidc {
    issuer_uri        = "https://login.microsoftonline.com/${var.entra_tenant_id}/v2.0"
    allowed_audiences = var.entra_allowed_audiences
  }
}

# --- SA the federated identities impersonate -----------------------------
resource "google_service_account" "k0s_workload" {
  count        = local.any_wif_enabled ? 1 : 0
  account_id   = "b00t-k0s-workload"
  display_name = "b00t k0s workload (federated, keyless)"
  description  = "Impersonated via WIF by k0s Pods / Entra. Read-only on the buildcache + AR."
}

resource "google_storage_bucket_iam_member" "k0s_workload_buildcache" {
  count  = local.any_wif_enabled ? 1 : 0
  bucket = google_storage_bucket.buildcache.name
  role   = "roles/storage.objectViewer"
  member = "serviceAccount:${google_service_account.k0s_workload[0].email}"
}

resource "google_artifact_registry_repository_iam_member" "k0s_workload_ar" {
  count      = local.any_wif_enabled ? 1 : 0
  location   = google_artifact_registry_repository.containers.location
  repository = google_artifact_registry_repository.containers.name
  role       = "roles/artifactregistry.reader"
  member     = "serviceAccount:${google_service_account.k0s_workload[0].email}"
}

# k0s: only the bound ServiceAccount subject may impersonate.
resource "google_service_account_iam_member" "k0s_wif_user" {
  count              = local.k0s_wif_enabled ? 1 : 0
  service_account_id = google_service_account.k0s_workload[0].name
  role               = "roles/iam.workloadIdentityUser"
  member             = "principalSet://iam.googleapis.com/${google_iam_workload_identity_pool.external[0].name}/subject/${var.k0s_wif_subject}"
}

# Entra: the bound app registration.
resource "google_service_account_iam_member" "entra_wif_user" {
  count              = local.entra_wif_enabled ? 1 : 0
  service_account_id = google_service_account.k0s_workload[0].name
  role               = "roles/iam.workloadIdentityUser"
  member             = "principalSet://iam.googleapis.com/${google_iam_workload_identity_pool.external[0].name}/attribute.appid/${var.entra_app_id}"
}
