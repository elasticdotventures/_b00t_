use anyhow::Result;
use crate::{BootDatum, DatumType, AiConfig, get_expanded_path};
use crate::traits::*;
use std::collections::HashMap;

pub struct AiDatum {
    pub datum: BootDatum,
    pub models: Option<HashMap<String, serde_json::Value>>,
    pub config_path: String,
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

        Ok(AiDatum {
            datum: config.b00t,
            models: config.models,
            config_path: path.to_string(),
        })
    }
    
    pub fn from_datum(datum: BootDatum, models: Option<HashMap<String, serde_json::Value>>, config_path: String) -> Self {
        AiDatum { datum, models, config_path }
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
    
    fn display_name(&self) -> &str {
        &self.datum.name
    }
    
    fn hint(&self) -> &str {
        &self.datum.hint
    }
    
    fn is_disabled(&self) -> bool {
        false // AI providers are never disabled by default
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
    
    fn is_disabled(&self) -> bool {
        !self.prerequisites_satisfied()
    }
    
    fn is_installed(&self) -> bool {
        DatumChecker::is_installed(self)
    }
    
    fn subsystem(&self) -> &str {
        StatusProvider::subsystem(self)
    }
}

impl ConstraintEvaluator for AiDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for AiDatum {
    fn datum_type(&self) -> DatumType {
        DatumType::Ai
    }
    
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

pub fn get_ai_tools_status(path: &str) -> Result<Vec<Box<dyn DatumProvider>>> {
    let mut tools: Vec<Box<dyn DatumProvider>> = Vec::new();
    let expanded_path = get_expanded_path(path)?;
    
    if let Ok(entries) = std::fs::read_dir(&expanded_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                    if file_name.ends_with(".ai.toml") {
                        if let Some(tool_name) = file_name.strip_suffix(".ai.toml") {
                            if let Ok(ai_datum) = AiDatum::from_config(tool_name, path) {
                                // Apply filtering logic: only include if prerequisites satisfied or already installed
                                if !FilterLogic::is_disabled(&ai_datum) || DatumChecker::is_installed(&ai_datum) {
                                    tools.push(Box::new(ai_datum));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(tools)
}