use crate::BootDatum;

#[derive(Debug, Clone, PartialEq)]
pub enum VersionStatus {
    Match,   // 👍🏻
    Newer,   // 🐣
    Older,   // 😭
    Missing, // 😱
    Unknown, // ⏹️
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

pub trait DatumChecker {
    fn is_installed(&self) -> bool;
    fn current_version(&self) -> Option<String>;
    fn desired_version(&self) -> Option<String>;
    fn version_status(&self) -> VersionStatus;
}

pub trait StatusProvider: DatumChecker {
    fn name(&self) -> &str;
    fn subsystem(&self) -> &str;
    fn hint(&self) -> &str;
    fn is_disabled(&self) -> bool;
}

pub trait FilterLogic {
    fn is_available(&self) -> bool;
    fn prerequisites_satisfied(&self) -> bool;
    fn evaluate_constraints(&self, require: &[String]) -> bool;
}

pub trait DatumProvider: DatumChecker + StatusProvider + FilterLogic + Send + Sync {
    /// Used by ConstraintEvaluator trait methods (compiler doesn't detect indirect usage)
    #[allow(dead_code)]
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
            "ubuntu" | "debian" => std::fs::read_to_string("/etc/os-release")
                .map(|content| content.contains("ubuntu") || content.contains("debian"))
                .unwrap_or(false),
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
                }
                constraint if constraint.starts_with("CMD:") => {
                    crate::check_command_available(&constraint[4..])
                }
                _ => true, // Unknown constraints default to true
            }
        })
    }
}
