use anyhow::Result;
use duct::cmd;
use crate::{BootDatum, DatumType, get_config, check_command_available, get_expanded_path};
use crate::traits::*;

pub struct McpDatum {
    pub datum: BootDatum,
    pub config_path: String,
}

impl McpDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(McpDatum {
            datum: config.b00t,
            config_path: path.to_string(),
        })
    }
    
    pub fn from_datum(datum: BootDatum, config_path: String) -> Self {
        McpDatum { datum, config_path }
    }
}

impl DatumChecker for McpDatum {
    fn is_installed(&self) -> bool {
        // For MCP servers, check if the command exists and is executable
        if let Some(command) = &self.datum.command {
            check_command_available(command)
        } else {
            false
        }
    }
    
    fn current_version(&self) -> Option<String> {
        // MCP servers don't typically have semantic versions
        // Return the command if installed
        if DatumChecker::is_installed(self) {
            if let Some(command) = &self.datum.command {
                Some(format!("{} available", command))
            } else {
                Some("available".to_string())
            }
        } else {
            None
        }
    }
    
    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }
    
    fn version_status(&self) -> VersionStatus {
        if DatumChecker::is_installed(self) {
            VersionStatus::Unknown // MCP servers are just available/not available
        } else {
            VersionStatus::Missing
        }
    }
}

impl StatusProvider for McpDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }
    
    fn subsystem(&self) -> &str {
        "mcp"
    }
    
    fn display_name(&self) -> &str {
        &self.datum.name
    }
    
    fn hint(&self) -> &str {
        &self.datum.hint
    }
    
    fn is_disabled(&self) -> bool {
        false // MCP servers are never disabled by default
    }
}

impl FilterLogic for McpDatum {
    fn is_available(&self) -> bool {
        !DatumChecker::is_installed(self) && self.prerequisites_satisfied()
    }
    
    fn prerequisites_satisfied(&self) -> bool {
        // Check if require constraints are satisfied
        if let Some(require) = &self.datum.require {
            self.evaluate_constraints(require)
        } else {
            // Default: check if all ENV vars are satisfied
            self.evaluate_constraints(&[])
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

impl ConstraintEvaluator for McpDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for McpDatum {
    fn datum_type(&self) -> DatumType {
        DatumType::Mcp
    }
    
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

pub fn get_mcp_tools_status(path: &str) -> Result<Vec<Box<dyn DatumProvider>>> {
    let mut tools: Vec<Box<dyn DatumProvider>> = Vec::new();
    let expanded_path = get_expanded_path(path)?;
    
    if let Ok(entries) = std::fs::read_dir(&expanded_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                    if file_name.ends_with(".mcp.toml") {
                        if let Some(tool_name) = file_name.strip_suffix(".mcp.toml") {
                            if let Ok(mcp_datum) = McpDatum::from_config(tool_name, path) {
                                // Apply filtering logic: only include if prerequisites satisfied
                                if !FilterLogic::is_disabled(&mcp_datum) || DatumChecker::is_installed(&mcp_datum) {
                                    tools.push(Box::new(mcp_datum));
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