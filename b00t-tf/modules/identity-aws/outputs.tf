output "oidc_provider_arn" {
  description = "IAM OIDC provider ARN for the SPIRE issuer (empty if disabled)."
  value       = try(aws_iam_openid_connect_provider.spire[0].arn, "")
}

output "web_identity_role_arn" {
  description = "Role a SPIRE JWT-SVID assumes via AssumeRoleWithWebIdentity (empty if disabled)."
  value       = try(aws_iam_role.web_identity[0].arn, "")
}

output "rolesanywhere_trust_anchor_arn" {
  description = "IAM Roles Anywhere trust anchor from the SPIRE CA (empty if disabled)."
  value       = try(aws_rolesanywhere_trust_anchor.spire[0].arn, "")
}

output "rolesanywhere_profile_arn" {
  description = "IAM Roles Anywhere profile ARN (empty if disabled)."
  value       = try(aws_rolesanywhere_profile.spire[0].arn, "")
}

output "rolesanywhere_role_arn" {
  description = "Role a SPIRE X.509-SVID assumes via Roles Anywhere (empty if disabled)."
  value       = try(aws_iam_role.rolesanywhere[0].arn, "")
}
