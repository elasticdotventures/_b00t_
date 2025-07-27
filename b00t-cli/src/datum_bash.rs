use crate::traits::*;
use crate::{BootDatum, check_command_available, get_config};
use anyhow::Result;
use duct::cmd;

pub struct BashDatum {
    pub datum: BootDatum,
}

impl BashDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(BashDatum { datum: config.b00t })
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
                let result =
                    cmd!("bash", "-c", &format!("{} {}", expanded_path, version_cmd)).read();

                match result {
                    Ok(output) => {
                        if let Some(version_regex) = &self.datum.version_regex {
                            if let Ok(re) = regex::Regex::new(version_regex) {
                                if let Some(captures) = re.captures(&output) {
                                    return captures
                                        .get(1)
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

impl TryFrom<(&str, &str)> for BashDatum {
    type Error = anyhow::Error;

    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
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
            if let (Some(current), Some(desired)) = (self.current_version(), self.desired_version())
            {
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
}

impl ConstraintEvaluator for BashDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for BashDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}
