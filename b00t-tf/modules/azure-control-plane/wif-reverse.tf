# ---------------------------------------------------------------------------
# Reverse federation: a GCP (or GitHub) identity assumes THIS Azure user-
# assigned identity with no client secret. Symmetric to the Entra->GCP
# provider in modules/gcp-build-plane/wif-k0s.tf.
#
# Gated: with gcp_federation_subject = "" nothing is created.
# See docs/superpowers/specs/2026-09-09-keyless-gcp-identity-k0s.md +
#     docs/superpowers/specs/2026-09-09-spiffe-spire-cross-cloud.md
# ---------------------------------------------------------------------------

variable "gcp_federation_subject" {
  description = "OIDC `sub` allowed to assume azurerm_user_assigned_identity.cp keylessly — a GCP service account's numeric unique_id (issuer accounts.google.com), or a GitHub `repo:org/repo:...` sub. Empty = no reverse federation."
  type        = string
  default     = ""
}

variable "gcp_federation_issuer" {
  description = "OIDC issuer for the reverse credential."
  type        = string
  default     = "https://accounts.google.com"
}

resource "azurerm_federated_identity_credential" "gcp_reverse" {
  count               = var.gcp_federation_subject == "" ? 0 : 1
  name                = "gcp-reverse"
  resource_group_name = azurerm_resource_group.cp.name
  parent_id           = azurerm_user_assigned_identity.cp.id
  audience            = ["api://AzureADTokenExchange"]
  issuer              = var.gcp_federation_issuer
  subject             = var.gcp_federation_subject
}

output "reverse_federation_client_id" {
  description = "client_id of the Azure identity a GCP/GitHub token can assume (empty if disabled)."
  value       = var.gcp_federation_subject == "" ? "" : azurerm_user_assigned_identity.cp.client_id
}
