# ---------------------------------------------------------------------------
# AWS onboarded against the SPIRE trust domain — keyless, no per-cluster AWS
# issuer. Plan gleaming-jingling-nygaard, identity-plane step 4.
#
# Two entry paths from ONE SPIRE identity (requirement #2):
#   - JWT-SVID  -> IAM OIDC provider + AssumeRoleWithWebIdentity   (k8s pods)
#   - X.509-SVID -> IAM Roles Anywhere (SPIRE CA as trust anchor)   (EC2 / on-prem)
#
# Everything gated: empty spire_* vars -> zero resources, `tofu validate`
# unaffected. See docs/superpowers/specs/2026-09-09-identity-plane-minimum-requirements.md
# ---------------------------------------------------------------------------

locals {
  oidc_enabled = var.spire_oidc_issuer_uri != ""
  ra_enabled   = var.spire_ca_bundle_pem != ""
  # the issuer host, as AWS stores it in the OIDC provider condition keys
  oidc_host = local.oidc_enabled ? replace(var.spire_oidc_issuer_uri, "https://", "") : ""
}

# --- JWT-SVID path -------------------------------------------------------------
resource "aws_iam_openid_connect_provider" "spire" {
  count          = local.oidc_enabled ? 1 : 0
  url            = var.spire_oidc_issuer_uri
  client_id_list = var.oidc_audiences
  # AWS validates the OIDC server cert against its own CA store and ignores
  # this list for standard TLS issuers; kept non-empty as the API requires it.
  thumbprint_list = ["ffffffffffffffffffffffffffffffffffffffff"]
}

data "aws_iam_policy_document" "web_identity_trust" {
  count = local.oidc_enabled ? 1 : 0
  statement {
    effect  = "Allow"
    actions = ["sts:AssumeRoleWithWebIdentity"]
    principals {
      type        = "Federated"
      identifiers = [aws_iam_openid_connect_provider.spire[0].arn]
    }
    condition {
      test     = "StringEquals"
      variable = "${local.oidc_host}:sub"
      values   = [var.spiffe_id]
    }
    condition {
      test     = "StringEquals"
      variable = "${local.oidc_host}:aud"
      values   = var.oidc_audiences
    }
  }
}

resource "aws_iam_role" "web_identity" {
  count                = local.oidc_enabled ? 1 : 0
  name                 = "${var.name_prefix}-webidentity"
  assume_role_policy   = data.aws_iam_policy_document.web_identity_trust[0].json
  max_session_duration = 3600
}

resource "aws_iam_role_policy_attachment" "web_identity" {
  for_each   = local.oidc_enabled ? toset(var.role_policy_arns) : toset([])
  role       = aws_iam_role.web_identity[0].name
  policy_arn = each.value
}

# --- X.509-SVID path (IAM Roles Anywhere) -----------------------------------
resource "aws_rolesanywhere_trust_anchor" "spire" {
  count   = local.ra_enabled ? 1 : 0
  name    = "${var.name_prefix}-spire-ca"
  enabled = true
  source {
    source_type = "CERTIFICATE_BUNDLE"
    source_data {
      x509_certificate_data = var.spire_ca_bundle_pem
    }
  }
}

data "aws_iam_policy_document" "rolesanywhere_trust" {
  count = local.ra_enabled ? 1 : 0
  statement {
    effect  = "Allow"
    actions = ["sts:AssumeRole", "sts:SetSourceIdentity", "sts:TagSession"]
    principals {
      type        = "Service"
      identifiers = ["rolesanywhere.amazonaws.com"]
    }
    # Pin the workload by the SPIFFE ID carried in the SVID's SAN URI.
    condition {
      test     = "StringEquals"
      variable = "aws:PrincipalTag/x509SAN/URI"
      values   = [var.spiffe_id]
    }
  }
}

resource "aws_iam_role" "rolesanywhere" {
  count                = local.ra_enabled ? 1 : 0
  name                 = "${var.name_prefix}-rolesanywhere"
  assume_role_policy   = data.aws_iam_policy_document.rolesanywhere_trust[0].json
  max_session_duration = 3600
}

resource "aws_iam_role_policy_attachment" "rolesanywhere" {
  for_each   = local.ra_enabled ? toset(var.role_policy_arns) : toset([])
  role       = aws_iam_role.rolesanywhere[0].name
  policy_arn = each.value
}

resource "aws_rolesanywhere_profile" "spire" {
  count      = local.ra_enabled ? 1 : 0
  name       = "${var.name_prefix}-profile"
  enabled    = true
  role_arns  = [aws_iam_role.rolesanywhere[0].arn]
  depends_on = [aws_iam_role_policy_attachment.rolesanywhere]
}
