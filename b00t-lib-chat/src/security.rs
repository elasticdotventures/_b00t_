//! JWT Security and Namespace Enforcement for ACP
//! 
//! Integrates with b00t-website JWT provisioning to enforce namespace isolation
//! based on GitHub user identity.

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

/// JWT Claims structure matching b00t-website NATS provisioner
#[derive(Debug, Serialize, Deserialize)]
pub struct AcpJwtClaims {
    /// Subject: acp.{hive}.{role} for hive JWTs
    pub sub: String,
    /// Audience: github.{hive}
    pub aud: String,
    /// Issued at timestamp
    pub iat: u64,
    /// Expiration timestamp
    pub exp: u64,
    /// NATS-specific claims
    pub nats: NatsPermissions,
    /// ACP-specific claims
    pub acp: Option<AcpPermissions>,
}

/// ACP-specific claims for hive operations
#[derive(Debug, Serialize, Deserialize)]
pub struct AcpPermissions {
    /// Token type ("hive")
    #[serde(rename = "type")]
    pub token_type: String,
    /// Hive namespace (account.{hive}.{role})
    pub namespace: String,
    /// Agent role
    pub role: String,
    /// ACP permissions
    pub permissions: Vec<String>,
    /// Issuer
    pub issued_by: String,
}

/// NATS permissions embedded in JWT
#[derive(Debug, Serialize, Deserialize)]
pub struct NatsPermissions {
    /// User type (always "user" for ACP agents)
    #[serde(rename = "type")]
    pub user_type: String,
    /// NATS JWT version
    pub version: u8,
    /// Subscription limits (-1 = unlimited)
    pub subs: i32,
    /// Data limits (-1 = unlimited)
    pub data: i32,
    /// Payload limits (-1 = unlimited) 
    pub payload: i32,
    /// Connection restrictions
    pub connect_only: bool,
    /// Subject permissions
    pub permissions: SubjectPermissions,
}

/// Subject-based permissions for NATS
#[derive(Debug, Serialize, Deserialize)]
pub struct SubjectPermissions {
    /// Subjects this user can publish to
    pub publish: Vec<String>,
    /// Subjects this user can subscribe to
    pub subscribe: Vec<String>,
}

/// Security context for ACP operations
#[derive(Debug, Clone)]
pub struct AcpSecurityContext {
    /// GitHub user ID (hive identifier) extracted from JWT
    pub hive: String,
    /// Hive's allowed namespace (account.{hive}.{role})
    pub namespace: String,
    /// Agent role from JWT subject
    pub role: String,
    /// Process ID from JWT subject (or "hive" for hive tokens)
    pub pid: String,
    /// JWT expiration time
    pub expires_at: DateTime<Utc>,
    /// Allowed publish subjects
    pub publish_subjects: Vec<String>,
    /// Allowed subscribe subjects  
    pub subscribe_subjects: Vec<String>,
}

/// JWT validator for ACP operations
pub struct AcpJwtValidator {
    /// Signing secret derived from operator JWT (matches b00t-website)
    signing_secret: String,
}

impl AcpJwtValidator {
    /// Create new JWT validator with signing secret
    pub fn new(signing_secret: String) -> Self {
        Self { signing_secret }
    }

    /// Create validator by deriving secret from operator JWT (matches b00t-website logic)
    pub fn from_operator_jwt(operator_jwt: &str) -> Result<Self> {
        let signing_secret = Self::derive_signing_secret(operator_jwt)?;
        Ok(Self::new(signing_secret))
    }

    /// Validate JWT and extract security context
    pub fn validate_jwt(&self, jwt_token: &str) -> Result<AcpSecurityContext> {
        // Set up JWT validation parameters
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;
        validation.validate_aud = false; // We'll validate audience manually

        // Decode and validate JWT
        let decoding_key = DecodingKey::from_secret(self.signing_secret.as_bytes());
        let token_data = decode::<AcpJwtClaims>(jwt_token, &decoding_key, &validation)
            .context("Failed to decode JWT")?;

        let claims = token_data.claims;

        // Parse subject: Handle both user.{hive}.{role}.{pid} and acp.{hive}.{role}
        let subject_parts: Vec<&str> = claims.sub.split('.').collect();
        let (hive, role, pid, namespace) = if subject_parts.len() == 4 && subject_parts[0] == "user" {
            // Standard user token: user.{hive}.{role}.{pid}
            let hive = subject_parts[1].to_string();
            let role = subject_parts[2].to_string();
            let pid = subject_parts[3].to_string();
            let namespace = format!("account.{}.{}", hive, role);
            (hive, role, pid, namespace)
        } else if subject_parts.len() == 3 && subject_parts[0] == "acp" {
            // ACP hive token: acp.{hive}.{role}
            let hive = subject_parts[1].to_string();
            let role = subject_parts[2].to_string();
            let pid = "hive".to_string(); // Use "hive" as pseudo-PID for hive tokens
            let namespace = format!("account.{}.{}", hive, role);
            (hive, role, pid, namespace)
        } else {
            return Err(anyhow::anyhow!("Invalid JWT subject format: {}", claims.sub));
        };

        // Validate audience matches GitHub hive pattern
        let expected_audience = format!("github.{}", hive);
        if claims.aud != expected_audience {
            return Err(anyhow::anyhow!("JWT audience mismatch: expected {}, got {}", expected_audience, claims.aud));
        }

        // Convert expiration timestamp
        let expires_at = DateTime::from_timestamp(claims.exp as i64, 0)
            .ok_or_else(|| anyhow::anyhow!("Invalid expiration timestamp"))?;

        // Validate permissions are for the correct namespace
        Self::validate_namespace_permissions(&namespace, &claims.nats.permissions)?;

        Ok(AcpSecurityContext {
            hive,
            namespace,
            role,
            pid,
            expires_at,
            publish_subjects: claims.nats.permissions.publish,
            subscribe_subjects: claims.nats.permissions.subscribe,
        })
    }

    /// Validate that all permissions are within the user's namespace
    fn validate_namespace_permissions(namespace: &str, permissions: &SubjectPermissions) -> Result<()> {
        // Check publish permissions
        for subject in &permissions.publish {
            if !Self::is_subject_in_namespace(subject, namespace) {
                return Err(anyhow::anyhow!("Publish permission '{}' outside namespace '{}'", subject, namespace));
            }
        }

        // Check subscribe permissions  
        for subject in &permissions.subscribe {
            if !Self::is_subject_in_namespace(subject, namespace) {
                return Err(anyhow::anyhow!("Subscribe permission '{}' outside namespace '{}'", subject, namespace));
            }
        }

        Ok(())
    }

    /// Check if a subject pattern is within the allowed namespace
    fn is_subject_in_namespace(subject: &str, namespace: &str) -> bool {
        // Allow exact matches and wildcard patterns within namespace
        subject.starts_with(namespace) || 
        subject.starts_with(&format!("{}.", namespace)) ||
        subject == format!("{}.>", namespace)
    }

    /// Derive signing secret from operator JWT (matches b00t-website logic)
    fn derive_signing_secret(operator_jwt: &str) -> Result<String> {
        use sha2::{Digest, Sha256};
        
        let salt = "b00t-nats-user-jwt-salt";
        let combined = format!("{}{}", operator_jwt, salt);
        
        let mut hasher = Sha256::new();
        hasher.update(combined.as_bytes());
        let result = hasher.finalize();
        
        Ok(hex::encode(result))
    }
}

/// Namespace enforcement for ACP hive operations
pub struct NamespaceEnforcer {
    security_context: AcpSecurityContext,
}

impl NamespaceEnforcer {
    /// Create new namespace enforcer with security context
    pub fn new(security_context: AcpSecurityContext) -> Self {
        Self { security_context }
    }

    /// Validate that a hive mission is within hive's namespace
    pub fn validate_mission_access(&self, mission_id: &str, namespace: &str) -> Result<()> {
        // Ensure namespace matches hive's allowed namespace
        if namespace != self.security_context.namespace {
            return Err(anyhow::anyhow!(
                "Access denied: mission namespace '{}' does not match hive namespace '{}'",
                namespace,
                self.security_context.namespace
            ));
        }

        // Additional validation: mission ID should not try to escape namespace
        if mission_id.contains("..") || mission_id.contains("/") || mission_id.contains("\\") {
            return Err(anyhow::anyhow!("Invalid mission ID: contains path traversal characters"));
        }

        Ok(())
    }

    /// Validate that an ACP subject is allowed for this hive
    pub fn validate_subject_access(&self, subject: &str, operation: SubjectOperation) -> Result<()> {
        let allowed_subjects = match operation {
            SubjectOperation::Publish => &self.security_context.publish_subjects,
            SubjectOperation::Subscribe => &self.security_context.subscribe_subjects,
        };

        // Check if subject matches any allowed pattern
        for pattern in allowed_subjects {
            if Self::subject_matches_pattern(subject, pattern) {
                return Ok(());
            }
        }

        Err(anyhow::anyhow!(
            "Access denied: {} operation on subject '{}' not permitted for hive '{}'",
            match operation {
                SubjectOperation::Publish => "publish",
                SubjectOperation::Subscribe => "subscribe",
            },
            subject,
            self.security_context.hive
        ))
    }

    /// Check if a subject matches a NATS pattern (with > and * wildcards)
    fn subject_matches_pattern(subject: &str, pattern: &str) -> bool {
        if pattern.ends_with(".>") {
            // Multi-level wildcard
            let prefix = &pattern[..pattern.len() - 2];
            subject.starts_with(prefix)
        } else if pattern.contains('*') {
            // Single-level wildcard - simplified matching
            let pattern_parts: Vec<&str> = pattern.split('.').collect();
            let subject_parts: Vec<&str> = subject.split('.').collect();
            
            if pattern_parts.len() != subject_parts.len() {
                return false;
            }
            
            for (p_part, s_part) in pattern_parts.iter().zip(subject_parts.iter()) {
                if *p_part != "*" && *p_part != *s_part {
                    return false;
                }
            }
            true
        } else {
            // Exact match
            subject == pattern
        }
    }

    /// Get hive's security context
    pub fn security_context(&self) -> &AcpSecurityContext {
        &self.security_context
    }
}

/// Type of subject operation for permission checking
#[derive(Debug, Clone, Copy)]
pub enum SubjectOperation {
    Publish,
    Subscribe,
}

/// Helper function to fetch ACP Hive JWT from b00t-website
pub async fn fetch_jwt_from_website(
    website_url: &str,
    session_token: &str,
    role: &str,
) -> Result<String> {
    let client = reqwest::Client::new();
    
    let response = client
        .post(&format!("{}/api/nats/hive-jwt", website_url))
        .header("Cookie", format!("session_token={}", session_token))
        .json(&serde_json::json!({
            "role": role
        }))
        .send()
        .await
        .context("Failed to request ACP Hive JWT from b00t-website")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(anyhow::anyhow!("ACP Hive JWT request failed: {} - {}", status, error_text));
    }

    let response_data: serde_json::Value = response.json().await
        .context("Failed to parse ACP Hive JWT response")?;

    let jwt = response_data
        .get("jwt")
        .and_then(|j| j.as_str())
        .ok_or_else(|| anyhow::anyhow!("ACP Hive JWT not found in response"))?;

    Ok(jwt.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subject_pattern_matching() {
        // Test exact match
        assert!(NamespaceEnforcer::subject_matches_pattern(
            "account.alice.test",
            "account.alice.test"
        ));

        // Test multi-level wildcard
        assert!(NamespaceEnforcer::subject_matches_pattern(
            "account.alice.test.deep",
            "account.alice.>"
        ));

        // Test single-level wildcard
        assert!(NamespaceEnforcer::subject_matches_pattern(
            "account.alice.test",
            "account.alice.*"
        ));

        // Test no match
        assert!(!NamespaceEnforcer::subject_matches_pattern(
            "account.bob.test",
            "account.alice.*"
        ));
    }

    #[test]
    fn test_namespace_validation() {
        assert!(AcpJwtValidator::is_subject_in_namespace(
            "account.alice.test",
            "account.alice"
        ));

        assert!(AcpJwtValidator::is_subject_in_namespace(
            "account.alice.>",
            "account.alice"
        ));

        assert!(!AcpJwtValidator::is_subject_in_namespace(
            "account.bob.test",
            "account.alice"
        ));

        assert!(!AcpJwtValidator::is_subject_in_namespace(
            "global.system.test",
            "account.alice"
        ));
    }
}