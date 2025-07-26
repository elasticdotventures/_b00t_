use anyhow::Result;
use duct::cmd;
use crate::{BootDatum, get_config, check_command_available};
use crate::traits::*;

pub struct AptDatum {
    pub datum: BootDatum,
}

impl AptDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(AptDatum {
            datum: config.b00t,
        })
    }

    fn is_package_installed(&self) -> bool {
        if let Some(package_name) = &self.datum.package_name {
            let result = cmd!("dpkg", "-l", package_name).read();
            match result {
                Ok(output) => {
                    output.lines().any(|line| {
                        line.starts_with("ii") && line.contains(package_name)
                    })
                }
                Err(_) => false,
            }
        } else {
            false
        }
    }

    fn get_package_version(&self) -> Option<String> {
        if let Some(package_name) = &self.datum.package_name {
            let result = cmd!("dpkg", "-l", package_name).read();
            match result {
                Ok(output) => {
                    for line in output.lines() {
                        if line.starts_with("ii") && line.contains(package_name) {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if parts.len() >= 3 {
                                return Some(parts[2].to_string());
                            }
                        }
                    }
                }
                Err(_) => {}
            }
        }
        None
    }

    fn is_ubuntu() -> bool {
        if let Ok(output) = cmd!("lsb_release", "-i").read() {
            output.to_lowercase().contains("ubuntu")
        } else if let Ok(output) = std::fs::read_to_string("/etc/os-release") {
            output.to_lowercase().contains("ubuntu")
        } else {
            false
        }
    }
}

impl TryFrom<(&str, &str)> for AptDatum {
    type Error = anyhow::Error;
    
    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
    }
}

impl DatumChecker for AptDatum {
    fn is_installed(&self) -> bool {
        check_command_available("apt") && 
        check_command_available("dpkg") && 
        Self::is_ubuntu() && 
        self.is_package_installed()
    }
    
    fn current_version(&self) -> Option<String> {
        if self.is_package_installed() {
            self.get_package_version()
        } else {
            None
        }
    }
    
    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }
    
    fn version_status(&self) -> VersionStatus {
        if !check_command_available("apt") || !Self::is_ubuntu() {
            return VersionStatus::Missing;
        }
        
        if self.is_package_installed() {
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

impl StatusProvider for AptDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }
    
    fn subsystem(&self) -> &str {
        "apt"
    }
    
    fn hint(&self) -> &str {
        &self.datum.hint
    }
    
    fn is_disabled(&self) -> bool {
        !check_command_available("apt") || !Self::is_ubuntu()
    }
}

impl FilterLogic for AptDatum {
    fn is_available(&self) -> bool {
        !DatumChecker::is_installed(self) && self.prerequisites_satisfied()
    }
    
    fn prerequisites_satisfied(&self) -> bool {
        if let Some(require) = &self.datum.require {
            self.evaluate_constraints(require)
        } else {
            check_command_available("apt") && Self::is_ubuntu()
        }
    }
    
    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
    
}

impl ConstraintEvaluator for AptDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for AptDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}


