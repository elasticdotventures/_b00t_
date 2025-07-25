use anyhow::Result;
use duct::cmd;
use crate::{BootDatum, DatumType, get_config, check_command_available, get_expanded_path};
use crate::traits::*;

pub struct VscodeDatum {
    pub datum: BootDatum,
    pub config_path: String,
}

impl VscodeDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(VscodeDatum {
            datum: config.b00t,
            config_path: path.to_string(),
        })
    }
    
    pub fn from_datum(datum: BootDatum, config_path: String) -> Self {
        VscodeDatum { datum, config_path }
    }

    fn is_extension_installed(&self) -> bool {
        if let Some(vsix_id) = &self.datum.vsix_id {
            // Check if VSCode extension is installed
            let result = cmd!("code", "--list-extensions").read();
            match result {
                Ok(output) => {
                    output.lines().any(|line| line.trim() == vsix_id)
                }
                Err(_) => false,
            }
        } else {
            false
        }
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
    
    fn display_name(&self) -> &str {
        &self.datum.name
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

impl ConstraintEvaluator for VscodeDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for VscodeDatum {
    fn datum_type(&self) -> DatumType {
        DatumType::Vscode
    }
    
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

pub fn get_vscode_tools_status(path: &str) -> Result<Vec<Box<dyn DatumProvider>>> {
    let mut tools: Vec<Box<dyn DatumProvider>> = Vec::new();
    let expanded_path = get_expanded_path(path)?;
    
    if let Ok(entries) = std::fs::read_dir(&expanded_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                    if file_name.ends_with(".vscode.toml") {
                        if let Some(tool_name) = file_name.strip_suffix(".vscode.toml") {
                            if let Ok(vscode_datum) = VscodeDatum::from_config(tool_name, path) {
                                // Apply filtering logic: only include if prerequisites satisfied or already installed
                                if !FilterLogic::is_disabled(&vscode_datum) || DatumChecker::is_installed(&vscode_datum) {
                                    tools.push(Box::new(vscode_datum));
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