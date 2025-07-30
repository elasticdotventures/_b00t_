use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// ACL configuration for b00t-mcp command filtering
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AclConfig {
    /// Default policy when no rules match (allow/deny)
    pub default_policy: Policy,
    /// Command-specific rules
    pub commands: HashMap<String, CommandRule>,
    /// Global regex patterns for allow/deny
    pub patterns: Option<Patterns>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CommandRule {
    /// Policy for this specific command
    pub policy: Policy,
    /// Optional regex patterns for arguments
    pub arg_patterns: Option<Vec<String>>,
    /// Optional description for documentation
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Patterns {
    /// Regex patterns that always allow (override deny)
    pub allow: Option<Vec<String>>,
    /// Regex patterns that always deny (override allow)
    pub deny: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Policy {
    Allow,
    Deny,
}

#[derive(Clone)]
pub struct AclFilter {
    config: AclConfig,
    allow_patterns: Vec<Regex>,
    deny_patterns: Vec<Regex>,
}

impl AclFilter {
    /// Load ACL configuration from file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let expanded_path = shellexpand::tilde(path.as_ref().to_str().unwrap()).to_string();
        let config_path = Path::new(&expanded_path);
        
        if !config_path.exists() {
            // Create default config if it doesn't exist
            let default_config = AclConfig::default();
            let config_content = toml::to_string_pretty(&default_config)
                .context("Failed to serialize default ACL config")?;
            
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)
                    .context("Failed to create config directory")?;
            }
            
            fs::write(config_path, config_content)
                .context("Failed to write default ACL config")?;
            
            eprintln!("Created default ACL config at: {}", config_path.display());
            return Ok(Self::new(default_config)?);
        }

        let config_content = fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read ACL config from {}", config_path.display()))?;

        let config: AclConfig = toml::from_str(&config_content)
            .with_context(|| format!("Failed to parse ACL config from {}", config_path.display()))?;

        Self::new(config)
    }

    /// Create new ACL filter with given configuration
    pub fn new(config: AclConfig) -> Result<Self> {
        let mut allow_patterns = Vec::new();
        let mut deny_patterns = Vec::new();

        if let Some(patterns) = &config.patterns {
            if let Some(allow) = &patterns.allow {
                for pattern in allow {
                    allow_patterns.push(
                        Regex::new(pattern)
                            .with_context(|| format!("Invalid allow regex pattern: {}", pattern))?,
                    );
                }
            }
            if let Some(deny) = &patterns.deny {
                for pattern in deny {
                    deny_patterns.push(
                        Regex::new(pattern)
                            .with_context(|| format!("Invalid deny regex pattern: {}", pattern))?,
                    );
                }
            }
        }

        Ok(Self {
            config,
            allow_patterns,
            deny_patterns,
        })
    }

    /// Check if a command with arguments is allowed
    pub fn is_allowed(&self, command: &str, args: &[String]) -> bool {
        let full_command = format!("{} {}", command, args.join(" "));

        // Check deny patterns first (they override everything)
        for pattern in &self.deny_patterns {
            if pattern.is_match(&full_command) {
                return false;
            }
        }

        // Check allow patterns (they override command-specific rules)
        for pattern in &self.allow_patterns {
            if pattern.is_match(&full_command) {
                return true;
            }
        }

        // Check command-specific rules
        if let Some(rule) = self.config.commands.get(command) {
            match rule.policy {
                Policy::Deny => return false,
                Policy::Allow => {
                    // Check argument patterns if specified
                    if let Some(arg_patterns) = &rule.arg_patterns {
                        let args_string = args.join(" ");
                        for pattern_str in arg_patterns {
                            if let Ok(pattern) = Regex::new(pattern_str) {
                                if !pattern.is_match(&args_string) {
                                    return false;
                                }
                            }
                        }
                    }
                    return true;
                }
            }
        }

        // Fall back to default policy
        self.config.default_policy == Policy::Allow
    }

    /// Get allowed commands for documentation
    pub fn get_allowed_commands(&self) -> Vec<String> {
        self.config
            .commands
            .iter()
            .filter_map(|(cmd, rule)| {
                if rule.policy == Policy::Allow {
                    Some(cmd.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for AclConfig {
    fn default() -> Self {
        let mut commands = HashMap::new();
        
        // Allow most b00t commands by default
        commands.insert("detect".to_string(), CommandRule {
            policy: Policy::Allow,
            arg_patterns: None,
            description: Some("Detect installed versions of tools".to_string()),
        });
        
        commands.insert("desires".to_string(), CommandRule {
            policy: Policy::Allow,
            arg_patterns: None,
            description: Some("Show desired versions from config".to_string()),
        });
        
        commands.insert("learn".to_string(), CommandRule {
            policy: Policy::Allow,
            arg_patterns: None,
            description: Some("Show learning resources for topics".to_string()),
        });
        
        commands.insert("mcp".to_string(), CommandRule {
            policy: Policy::Allow,
            arg_patterns: Some(vec!["^(list|add)".to_string()]), // Only allow list and add
            description: Some("MCP server management (list/add only)".to_string()),
        });
        
        // Restrict potentially dangerous commands
        commands.insert("install".to_string(), CommandRule {
            policy: Policy::Deny,
            arg_patterns: None,
            description: Some("Install commands denied by default for security".to_string()),
        });
        
        commands.insert("update".to_string(), CommandRule {
            policy: Policy::Deny,
            arg_patterns: None,
            description: Some("Update commands denied by default for security".to_string()),
        });
        
        commands.insert("up".to_string(), CommandRule {
            policy: Policy::Deny,
            arg_patterns: None,
            description: Some("Bulk update commands denied by default for security".to_string()),
        });

        Self {
            default_policy: Policy::Allow,
            commands,
            patterns: Some(Patterns {
                allow: None,
                deny: Some(vec![
                    r".*\b(rm|delete|destroy|kill)\b.*".to_string(),
                    r".*--force.*".to_string(),
                ]),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_acl() {
        let config = AclConfig::default();
        let filter = AclFilter::new(config).unwrap();
        
        assert!(filter.is_allowed("detect", &["git".to_string()]));
        assert!(filter.is_allowed("learn", &["rust".to_string()]));
        assert!(!filter.is_allowed("install", &["git".to_string()]));
        assert!(!filter.is_allowed("unknown", &["--force".to_string()]));
    }
}