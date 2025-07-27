use crate::traits::*;
use crate::{BootDatum, check_command_available, get_config};
use anyhow::Result;

pub struct McpDatum {
    pub datum: BootDatum,
}

impl McpDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(McpDatum { datum: config.b00t })
    }
}

impl TryFrom<(&str, &str)> for McpDatum {
    type Error = anyhow::Error;

    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
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
}

impl ConstraintEvaluator for McpDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for McpDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}
