use crate::traits::*;
use crate::{BootDatum, check_command_available, get_config};
use anyhow::Result;
use duct::cmd;

pub struct VscodeDatum {
    pub datum: BootDatum,
}

impl VscodeDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(VscodeDatum { datum: config.b00t })
    }

    fn is_extension_installed(&self) -> bool {
        if let Some(vsix_id) = &self.datum.vsix_id {
            // Check if VSCode extension is installed
            let result = cmd!("code", "--list-extensions").read();
            match result {
                Ok(output) => output.lines().any(|line| line.trim() == vsix_id),
                Err(_) => false,
            }
        } else {
            false
        }
    }
}

impl TryFrom<(&str, &str)> for VscodeDatum {
    type Error = anyhow::Error;

    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
    }
}

impl DatumChecker for VscodeDatum {
    fn is_installed(&self) -> bool {
        // VSCode extensions are "installed" if:
        // 1. VSCode is available (code command)
        // 2. The extension is installed in VSCode
        check_command_available("code") && self.is_extension_installed()
    }

    fn current_version(&self) -> Option<String> {
        if self.is_extension_installed() {
            // Try to get extension version (VSCode doesn't easily expose this via CLI)
            Some("installed".to_string())
        } else {
            None
        }
    }

    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }

    fn version_status(&self) -> VersionStatus {
        if !check_command_available("code") {
            return VersionStatus::Missing;
        }

        if self.is_extension_installed() {
            VersionStatus::Unknown // Extension installed but version comparison not available
        } else {
            VersionStatus::Missing
        }
    }
}

impl StatusProvider for VscodeDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }

    fn subsystem(&self) -> &str {
        "vscode"
    }

    fn hint(&self) -> &str {
        &self.datum.hint
    }

    fn is_disabled(&self) -> bool {
        // VSCode extensions are disabled if VSCode is not available
        !check_command_available("code")
    }
}

impl FilterLogic for VscodeDatum {
    fn is_available(&self) -> bool {
        !DatumChecker::is_installed(self) && self.prerequisites_satisfied()
    }

    fn prerequisites_satisfied(&self) -> bool {
        // Check if require constraints are satisfied
        if let Some(require) = &self.datum.require {
            self.evaluate_constraints(require)
        } else {
            // Default: VSCode must be available
            check_command_available("code")
        }
    }

    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
}

impl ConstraintEvaluator for VscodeDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for VscodeDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}
