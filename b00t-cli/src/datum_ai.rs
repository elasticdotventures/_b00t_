use crate::traits::*;
use crate::{AiConfig, BootDatum, get_expanded_path};
use anyhow::Result;
use std::collections::HashMap;

pub struct AiDatum {
    pub datum: BootDatum,
    pub models: Option<HashMap<String, serde_json::Value>>,
}

impl AiDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let mut path_buf = get_expanded_path(path)?;
        path_buf.push(format!("{}.ai.toml", name));

        if !path_buf.exists() {
            anyhow::bail!("AI provider '{}' not found at {}", name, path_buf.display());
        }

        let content = std::fs::read_to_string(&path_buf)?;
        let config: AiConfig = toml::from_str(&content)?;

        let mut datum = config.b00t;
        // Merge top-level env into datum.env
        if let Some(config_env) = config.env {
            if let Some(ref mut datum_env) = datum.env {
                datum_env.extend(config_env);
            } else {
                datum.env = Some(config_env);
            }
        }

        Ok(AiDatum {
            datum,
            models: config.models,
        })
    }

    fn has_any_env_vars(&self) -> bool {
        // Check if any required environment variables are actually set
        if let Some(env) = &self.datum.env {
            for (key, _) in env {
                if std::env::var(key).is_ok() {
                    return true;
                }
            }
        }
        false
    }
}

impl TryFrom<(&str, &str)> for AiDatum {
    type Error = anyhow::Error;

    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
    }
}

impl DatumChecker for AiDatum {
    fn is_installed(&self) -> bool {
        // AI providers are "installed" if their required environment variables are set
        self.has_any_env_vars()
    }

    fn current_version(&self) -> Option<String> {
        // AI providers don't have traditional versions, show model count instead
        if let Some(models) = &self.models {
            Some(format!("{} models available", models.len()))
        } else {
            Some("API available".to_string())
        }
    }

    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }

    fn version_status(&self) -> VersionStatus {
        if DatumChecker::is_installed(self) {
            VersionStatus::Unknown // AI providers are just available/not available
        } else {
            VersionStatus::Missing
        }
    }
}

impl StatusProvider for AiDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }

    fn subsystem(&self) -> &str {
        "ai"
    }

    fn hint(&self) -> &str {
        &self.datum.hint
    }

    fn is_disabled(&self) -> bool {
        // AI providers are disabled when required environment variables are missing
        !self.has_any_env_vars()
    }
}

impl FilterLogic for AiDatum {
    fn is_available(&self) -> bool {
        !DatumChecker::is_installed(self) && self.prerequisites_satisfied()
    }

    fn prerequisites_satisfied(&self) -> bool {
        // Check if require constraints are satisfied
        if let Some(require) = &self.datum.require {
            self.evaluate_constraints(require)
        } else {
            // Default for AI: check if any ENV vars are satisfied (most AI providers need at least one API key)
            self.evaluate_constraints(&["NEEDS_ANY_ENV".to_string()])
        }
    }

    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
}

impl ConstraintEvaluator for AiDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for AiDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}
