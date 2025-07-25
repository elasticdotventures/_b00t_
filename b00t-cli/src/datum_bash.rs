use anyhow::Result;
use duct::cmd;
use crate::{BootDatum, DatumType, get_config, check_command_available, get_expanded_path};
use crate::traits::*;

pub struct BashDatum {
    pub datum: BootDatum,
    pub config_path: String,
}

impl BashDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(BashDatum {
            datum: config.b00t,
            config_path: path.to_string(),
        })
    }
    
    pub fn from_datum(datum: BootDatum, config_path: String) -> Self {
        BashDatum { datum, config_path }
    }

    fn is_script_executable(&self) -> bool {
        if let Some(script_path) = &self.datum.script {
            let expanded_path = shellexpand::tilde(script_path);
            let path = std::path::Path::new(expanded_path.as_ref());
            
            path.exists() && path.is_file() && {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(metadata) = std::fs::metadata(path) {
                        let permissions = metadata.permissions();
                        permissions.mode() & 0o111 != 0
                    } else {
                        false
                    }
                }
                #[cfg(not(unix))]
                {
                    true
                }
            }
        } else {
            false
        }
    }

    fn get_script_version(&self) -> Option<String> {
        if let Some(script_path) = &self.datum.script {
            if let Some(version_cmd) = &self.datum.version {
                let expanded_path = shellexpand::tilde(script_path);
                let result = cmd!("bash", "-c", &format!("{} {}", expanded_path, version_cmd)).read();
                
                match result {
                    Ok(output) => {
                        if let Some(version_regex) = &self.datum.version_regex {
                            if let Ok(re) = regex::Regex::new(version_regex) {
                                if let Some(captures) = re.captures(&output) {
                                    return captures.get(1)
                                        .or_else(|| captures.get(0))
                                        .map(|m| m.as_str().to_string());
                                }
                            }
                        }
                        Some(output.lines().next().unwrap_or("").trim().to_string())
                    }
                    Err(_) => None,
                }
            } else {
                Some("available".to_string())
            }
        } else {
            None
        }
    }
}

impl DatumChecker for BashDatum {
    fn is_installed(&self) -> bool {
        check_command_available("bash") && self.is_script_executable()
    }
    
    fn current_version(&self) -> Option<String> {
        if self.is_script_executable() {
            self.get_script_version()
        } else {
            None
        }
    }
    
    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }
    
    fn version_status(&self) -> VersionStatus {
        if !check_command_available("bash") {
            return VersionStatus::Missing;
        }
        
        if self.is_script_executable() {
            if let (Some(current), Some(desired)) = (self.current_version(), self.desired_version()) {
                if current == desired {
                    VersionStatus::Match
                } else {
                    VersionStatus::Unknown
                }
            } else {
                VersionStatus::Unknown
            }
        } else {
            VersionStatus::Missing
        }
    }
}

impl StatusProvider for BashDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }
    
    fn subsystem(&self) -> &str {
        "bash"
    }
    
    fn display_name(&self) -> &str {
        &self.datum.name
    }
    
    fn hint(&self) -> &str {
        &self.datum.hint
    }
    
    fn is_disabled(&self) -> bool {
        !check_command_available("bash")
    }
}

impl FilterLogic for BashDatum {
    fn is_available(&self) -> bool {
        !DatumChecker::is_installed(self) && self.prerequisites_satisfied()
    }
    
    fn prerequisites_satisfied(&self) -> bool {
        if let Some(require) = &self.datum.require {
            self.evaluate_constraints(require)
        } else {
            check_command_available("bash")
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

impl ConstraintEvaluator for BashDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for BashDatum {
    fn datum_type(&self) -> DatumType {
        DatumType::Bash
    }
    
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

pub fn get_bash_tools_status(path: &str) -> Result<Vec<Box<dyn DatumProvider>>> {
    let mut tools: Vec<Box<dyn DatumProvider>> = Vec::new();
    let expanded_path = get_expanded_path(path)?;
    
    if let Ok(entries) = std::fs::read_dir(&expanded_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                    if file_name.ends_with(".bash.toml") {
                        if let Some(tool_name) = file_name.strip_suffix(".bash.toml") {
                            if let Ok(bash_datum) = BashDatum::from_config(tool_name, path) {
                                if !FilterLogic::is_disabled(&bash_datum) || DatumChecker::is_installed(&bash_datum) {
                                    tools.push(Box::new(bash_datum));
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

pub fn get_bash_datum_providers(path: &str) -> Result<Vec<Box<dyn DatumProvider>>> {
    get_bash_tools_status(path)
}