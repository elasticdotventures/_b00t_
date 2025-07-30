use b00t_mcp::{AclConfig, AclFilter, Policy};
use std::collections::HashMap;

#[test]
fn test_default_acl_config() {
    let config = AclConfig::default();
    let filter = AclFilter::new(config).unwrap();
    
    // Should allow detect command
    assert!(filter.is_allowed("detect", &["git".to_string()]));
    
    // Should allow learn command
    assert!(filter.is_allowed("learn", &["rust".to_string()]));
    
    // Should deny install command by default
    assert!(!filter.is_allowed("install", &["git".to_string()]));
    
    // Should deny dangerous patterns
    assert!(!filter.is_allowed("test", &["--force".to_string()]));
}

#[test]
fn test_custom_acl_config() {
    let mut commands = HashMap::new();
    commands.insert("test_cmd".to_string(), b00t_mcp::acl::CommandRule {
        policy: Policy::Allow,
        arg_patterns: Some(vec!["^safe.*".to_string()]),
        description: Some("Test command".to_string()),
    });
    
    let config = AclConfig {
        default_policy: Policy::Deny,
        commands,
        patterns: None,
    };
    
    let filter = AclFilter::new(config).unwrap();
    
    // Should allow test_cmd with safe arguments
    assert!(filter.is_allowed("test_cmd", &["safe-operation".to_string()]));
    
    // Should deny test_cmd with unsafe arguments
    assert!(!filter.is_allowed("test_cmd", &["unsafe-operation".to_string()]));
    
    // Should deny unknown commands due to default deny policy
    assert!(!filter.is_allowed("unknown", &["arg".to_string()]));
}

#[test]
fn test_pattern_overrides() {
    let config = AclConfig {
        default_policy: Policy::Allow,
        commands: HashMap::new(),
        patterns: Some(b00t_mcp::acl::Patterns {
            allow: Some(vec![".*special.*".to_string()]),
            deny: Some(vec![".*dangerous.*".to_string()]),
        }),
    };
    
    let filter = AclFilter::new(config).unwrap();
    
    // Should allow special commands even if unknown
    assert!(filter.is_allowed("unknown", &["special-operation".to_string()]));
    
    // Should deny dangerous commands regardless of default allow
    assert!(!filter.is_allowed("safe_cmd", &["dangerous-operation".to_string()]));
}