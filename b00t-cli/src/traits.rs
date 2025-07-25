use crate::{BootDatum, DatumType};

#[derive(Debug, Clone, PartialEq)]
pub enum VersionStatus {
    Match,      // 👍🏻 
    Newer,      // 🐣
    Older,      // 😭
    Missing,    // 😱
    Unknown,    // ⏹️
}

impl VersionStatus {
    pub fn emoji(&self) -> &'static str {
        match self {
            VersionStatus::Match => "👍🏻",
            VersionStatus::Newer => "🐣",
            VersionStatus::Older => "😭",
            VersionStatus::Missing => "😱",
            VersionStatus::Unknown => "⏹️",
        }
    }
}

pub struct DisplayFilters {
    pub subsystem: Option<String>,
    pub only_installed: bool,
    pub only_available: bool,
}

pub trait DatumChecker {
    fn is_installed(&self) -> bool;
    fn current_version(&self) -> Option<String>;
    fn desired_version(&self) -> Option<String>;
    fn version_status(&self) -> VersionStatus;
}

pub trait StatusProvider: DatumChecker {
    fn name(&self) -> &str;
    fn subsystem(&self) -> &str;
    fn display_name(&self) -> &str;
    fn hint(&self) -> &str;
    fn status_icon(&self) -> &str {
        if self.is_disabled() {
            "🔴"
        } else if self.is_installed() {
            "☑️"
        } else {
            "⏹️"
        }
    }
    fn version_emoji(&self) -> &str {
        self.version_status().emoji()
    }
    fn is_disabled(&self) -> bool;
}

pub trait FilterLogic {
    fn is_available(&self) -> bool;
    fn prerequisites_satisfied(&self) -> bool;
    fn evaluate_constraints(&self, require: &[String]) -> bool;
    fn should_display(&self, filters: &DisplayFilters) -> bool {
        // Apply subsystem filter
        if let Some(ref filter_subsystem) = filters.subsystem {
            if self.subsystem() != filter_subsystem {
                return false;
            }
        }
        
        // Apply installation status filters
        if filters.only_installed && !self.is_installed() {
            return false;
        }
        
        if filters.only_available && (self.is_installed() || self.is_disabled()) {
            return false;
        }
        
        true
    }
    fn is_disabled(&self) -> bool;
    fn is_installed(&self) -> bool;
    fn subsystem(&self) -> &str;
}

pub trait DatumProvider: DatumChecker + StatusProvider + FilterLogic + Send + Sync {
    fn datum_type(&self) -> DatumType;
    fn datum(&self) -> &BootDatum;
}

// Base implementation for common constraint evaluation
pub trait ConstraintEvaluator {
    fn datum(&self) -> &BootDatum;
    
    fn has_any_env_vars(&self) -> bool {
        if let Some(env) = &self.datum().env {
            env.keys().any(|key| std::env::var(key).is_ok())
        } else {
            false
        }
    }
    
    fn has_all_env_vars(&self) -> bool {
        if let Some(env) = &self.datum().env {
            env.keys().all(|key| std::env::var(key).is_ok())
        } else {
            true // No env vars = satisfied
        }
    }
    
    fn check_os_requirement(&self, os: &str) -> bool {
        match os {
            "ubuntu" | "debian" => {
                std::fs::read_to_string("/etc/os-release")
                    .map(|content| content.contains("ubuntu") || content.contains("debian"))
                    .unwrap_or(false)
            }
            "macos" => cfg!(target_os = "macos"),
            "windows" => cfg!(target_os = "windows"),
            "linux" => cfg!(target_os = "linux"),
            _ => false,
        }
    }
    
    fn evaluate_constraints_default(&self, require: &[String]) -> bool {
        if require.is_empty() {
            // Default behavior: NEEDS_ALL_ENV for datums with env vars
            return self.has_all_env_vars();
        }
        
        require.iter().all(|constraint| {
            match constraint.as_str() {
                "NEEDS_ANY_ENV" => self.has_any_env_vars(),
                "NEEDS_ALL_ENV" => self.has_all_env_vars(),
                constraint if constraint.starts_with("OS:") => {
                    self.check_os_requirement(&constraint[3..])
                },
                constraint if constraint.starts_with("CMD:") => {
                    crate::check_command_available(&constraint[4..])
                },
                _ => true, // Unknown constraints default to true
            }
        })
    }
}