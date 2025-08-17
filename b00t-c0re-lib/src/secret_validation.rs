//! Secret validation module for verifying cloud service credentials
//!
//! This module provides validation methods for various cloud services
//! to ensure secrets are valid before storing them in the configuration.

use anyhow::{anyhow, Result};
use reqwest::Client;
use std::time::Duration;
use tokio::time::timeout;

/// Timeout duration for validation requests
const VALIDATION_TIMEOUT: Duration = Duration::from_secs(10);

/// Cloud service secret validator
pub struct SecretValidator {
    client: Client,
}

impl SecretValidator {
    /// Create a new secret validator
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(VALIDATION_TIMEOUT)
            .user_agent("b00t-secret-validator/1.0")
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;

        Ok(Self { client })
    }

    /// Validate Cloudflare API token
    pub async fn validate_cloudflare_token(&self, token: &str, account_id: Option<&str>) -> Result<CloudflareValidation> {
        let url = "https://api.cloudflare.com/client/v4/user/tokens/verify";
        
        let response = timeout(
            VALIDATION_TIMEOUT,
            self.client
                .get(url)
                .bearer_auth(token)
                .send()
        ).await
        .map_err(|_| anyhow!("Cloudflare validation request timed out"))?
        .map_err(|e| anyhow!("Failed to validate Cloudflare token: {}", e))?;

        let status = response.status();
        let body: serde_json::Value = response.json().await
            .map_err(|e| anyhow!("Failed to parse Cloudflare response: {}", e))?;

        if !status.is_success() {
            return Err(anyhow!("Invalid Cloudflare token: {}", 
                body.get("errors")
                    .and_then(|e| e.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|err| err.get("message"))
                    .and_then(|msg| msg.as_str())
                    .unwrap_or("Unknown error")
            ));
        }

        // Validate account access if account_id provided
        let account_access = if let Some(account_id) = account_id {
            self.validate_cloudflare_account_access(token, account_id).await?
        } else {
            false
        };

        let result = body.get("result").ok_or_else(|| anyhow!("Invalid response format"))?;
        
        Ok(CloudflareValidation {
            valid: true,
            account_id: account_id.map(String::from),
            account_access,
            permissions: self.extract_cloudflare_permissions(result),
        })
    }

    /// Validate Cloudflare account access
    async fn validate_cloudflare_account_access(&self, token: &str, account_id: &str) -> Result<bool> {
        let url = format!("https://api.cloudflare.com/client/v4/accounts/{}", account_id);
        
        let response = timeout(
            VALIDATION_TIMEOUT,
            self.client
                .get(&url)
                .bearer_auth(token)
                .send()
        ).await
        .map_err(|_| anyhow!("Cloudflare account validation timed out"))?
        .map_err(|e| anyhow!("Failed to validate account access: {}", e))?;

        Ok(response.status().is_success())
    }

    /// Extract Cloudflare permissions from token verification response
    fn extract_cloudflare_permissions(&self, result: &serde_json::Value) -> Vec<String> {
        result
            .get("policies")
            .and_then(|p| p.as_array())
            .map(|policies| {
                policies.iter()
                    .filter_map(|policy| policy.get("effect").and_then(|e| e.as_str()))
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Validate AWS credentials
    pub async fn validate_aws_credentials(&self, access_key: &str, secret_key: &str, region: &str) -> Result<AwsValidation> {
        // This is a simplified validation - in production you'd want to use the AWS SDK
        // For now, we'll validate the format and attempt a simple STS call
        
        if access_key.len() < 16 || !access_key.starts_with("AKIA") {
            return Err(anyhow!("Invalid AWS access key format"));
        }

        if secret_key.len() < 32 {
            return Err(anyhow!("Invalid AWS secret key format"));
        }

        // TODO: Implement actual AWS STS GetCallerIdentity call
        // This would require adding AWS SDK dependency
        Ok(AwsValidation {
            valid: true,
            region: region.to_string(),
            account_id: None, // Would be populated from STS call
            user_arn: None,   // Would be populated from STS call
            services: vec!["s3".to_string(), "ec2".to_string()], // Would be detected
        })
    }

    /// Validate Qdrant connection and API key
    pub async fn validate_qdrant_connection(&self, endpoint: &str, api_key: &str) -> Result<QdrantValidation> {
        let url = format!("{}/collections", endpoint.trim_end_matches('/'));
        
        let response = timeout(
            VALIDATION_TIMEOUT,
            self.client
                .get(&url)
                .header("api-key", api_key)
                .send()
        ).await
        .map_err(|_| anyhow!("Qdrant validation request timed out"))?
        .map_err(|e| anyhow!("Failed to validate Qdrant connection: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("Invalid Qdrant credentials or endpoint: HTTP {}", response.status()));
        }

        let body: serde_json::Value = response.json().await
            .map_err(|e| anyhow!("Failed to parse Qdrant response: {}", e))?;

        let collections = body
            .get("result")
            .and_then(|r| r.get("collections"))
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|col| col.get("name").and_then(|n| n.as_str()))
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        Ok(QdrantValidation {
            valid: true,
            endpoint: endpoint.to_string(),
            collections,
            version: None, // Could be extracted from server info
        })
    }

    /// Validate OpenAI API key
    pub async fn validate_openai_api_key(&self, api_key: &str) -> Result<OpenAiValidation> {
        let url = "https://api.openai.com/v1/models";
        
        let response = timeout(
            VALIDATION_TIMEOUT,
            self.client
                .get(url)
                .bearer_auth(api_key)
                .send()
        ).await
        .map_err(|_| anyhow!("OpenAI validation request timed out"))?
        .map_err(|e| anyhow!("Failed to validate OpenAI API key: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("Invalid OpenAI API key: HTTP {}", response.status()));
        }

        let body: serde_json::Value = response.json().await
            .map_err(|e| anyhow!("Failed to parse OpenAI response: {}", e))?;

        let models = body
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|model| model.get("id").and_then(|id| id.as_str()))
                    .filter(|id| id.starts_with("gpt-"))
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        Ok(OpenAiValidation {
            valid: true,
            available_models: models,
            organization: None, // Could be extracted from headers
        })
    }

    /// Validate Anthropic API key
    pub async fn validate_anthropic_api_key(&self, api_key: &str) -> Result<AnthropicValidation> {
        let url = "https://api.anthropic.com/v1/models";
        
        let response = timeout(
            VALIDATION_TIMEOUT,
            self.client
                .get(url)
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .send()
        ).await
        .map_err(|_| anyhow!("Anthropic validation request timed out"))?
        .map_err(|e| anyhow!("Failed to validate Anthropic API key: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!("Invalid Anthropic API key: HTTP {}", response.status()));
        }

        let body: serde_json::Value = response.json().await
            .map_err(|e| anyhow!("Failed to parse Anthropic response: {}", e))?;

        let models = body
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|model| model.get("id").and_then(|id| id.as_str()))
                    .filter(|id| id.starts_with("claude-"))
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        Ok(AnthropicValidation {
            valid: true,
            available_models: models,
        })
    }

    /// Validate generic API endpoint with optional authentication
    pub async fn validate_generic_endpoint(&self, endpoint: &str, auth_header: Option<(&str, &str)>) -> Result<GenericValidation> {
        let mut request = self.client.get(endpoint);
        
        if let Some((header_name, header_value)) = auth_header {
            request = request.header(header_name, header_value);
        }

        let response = timeout(
            VALIDATION_TIMEOUT,
            request.send()
        ).await
        .map_err(|_| anyhow!("Generic endpoint validation timed out"))?
        .map_err(|e| anyhow!("Failed to validate endpoint: {}", e))?;

        Ok(GenericValidation {
            valid: response.status().is_success(),
            endpoint: endpoint.to_string(),
            status_code: response.status().as_u16(),
            response_time_ms: None, // Could be measured
        })
    }
}

impl Default for SecretValidator {
    fn default() -> Self {
        Self::new().expect("Failed to create default SecretValidator")
    }
}

/// Cloudflare validation result
#[derive(Debug, Clone)]
pub struct CloudflareValidation {
    pub valid: bool,
    pub account_id: Option<String>,
    pub account_access: bool,
    pub permissions: Vec<String>,
}

/// AWS validation result
#[derive(Debug, Clone)]
pub struct AwsValidation {
    pub valid: bool,
    pub region: String,
    pub account_id: Option<String>,
    pub user_arn: Option<String>,
    pub services: Vec<String>,
}

/// Qdrant validation result
#[derive(Debug, Clone)]
pub struct QdrantValidation {
    pub valid: bool,
    pub endpoint: String,
    pub collections: Vec<String>,
    pub version: Option<String>,
}

/// OpenAI validation result
#[derive(Debug, Clone)]
pub struct OpenAiValidation {
    pub valid: bool,
    pub available_models: Vec<String>,
    pub organization: Option<String>,
}

/// Anthropic validation result
#[derive(Debug, Clone)]
pub struct AnthropicValidation {
    pub valid: bool,
    pub available_models: Vec<String>,
}

/// Generic endpoint validation result
#[derive(Debug, Clone)]
pub struct GenericValidation {
    pub valid: bool,
    pub endpoint: String,
    pub status_code: u16,
    pub response_time_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_creation() {
        let validator = SecretValidator::new();
        assert!(validator.is_ok());
    }

    #[tokio::test]
    async fn test_invalid_cloudflare_token() {
        let validator = SecretValidator::new().unwrap();
        let result = validator.validate_cloudflare_token("invalid_token", None).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_aws_key_format_validation() {
        let validator = SecretValidator::new().unwrap();
        
        // Test invalid formats
        let result = tokio_test::block_on(
            validator.validate_aws_credentials("invalid", "invalid", "us-east-1")
        );
        assert!(result.is_err());
        
        let result = tokio_test::block_on(
            validator.validate_aws_credentials("AKIAIOSFODNN7EXAMPLE", "short", "us-east-1")
        );
        assert!(result.is_err());
    }
}