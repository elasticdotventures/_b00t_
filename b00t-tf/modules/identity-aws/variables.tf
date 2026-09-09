variable "spire_oidc_issuer_uri" {
  description = "SPIRE OIDC Discovery Provider URL (https://oidc.<trust-domain>). Empty = no AWS IAM OIDC provider / web-identity role."
  type        = string
  default     = ""
}

variable "spire_ca_bundle_pem" {
  description = "SPIRE trust bundle (X.509 CA chain, PEM). Empty = no IAM Roles Anywhere trust anchor."
  type        = string
  default     = ""
}

variable "spiffe_id" {
  description = "The SPIFFE ID allowed to assume the AWS roles, e.g. spiffe://b00t.promptexecution.com/ns/b00t-ci/sa/b00t-ci."
  type        = string
  default     = "spiffe://b00t.promptexecution.com/ns/b00t-ci/sa/b00t-ci"
}

variable "oidc_audiences" {
  description = "Accepted `aud` values in the JWT-SVID presented to AWS STS."
  type        = list(string)
  default     = ["sts.amazonaws.com"]
}

variable "name_prefix" {
  description = "Prefix for the IAM role / RA profile names."
  type        = string
  default     = "b00t-spire"
}

variable "role_policy_arns" {
  description = "Managed policy ARNs attached to both the web-identity and Roles-Anywhere roles (keep minimal)."
  type        = list(string)
  default     = ["arn:aws:iam::aws:policy/AmazonS3ReadOnlyAccess"]
}
