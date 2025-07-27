use crate::traits::*;
use crate::{BootDatum, check_command_available, get_config};
use anyhow::Result;
use duct::cmd;

pub struct CliDatum {
    pub datum: BootDatum,
}

impl CliDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(CliDatum { datum: config.b00t })
    }
}

impl TryFrom<(&str, &str)> for CliDatum {
    type Error = anyhow::Error;

    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
    }
}

impl DatumChecker for CliDatum {
    fn is_installed(&self) -> bool {
        if let Some(version_cmd) = &self.datum.version {
            cmd!("bash", "-c", version_cmd).read().is_ok()
        } else {
            check_command_available(&self.datum.name)
        }
    }

    fn current_version(&self) -> Option<String> {
        if let Some(version_cmd) = &self.datum.version {
            if let Ok(output) = cmd!("bash", "-c", version_cmd).read() {
                if let Some(regex) = &self.datum.version_regex {
                    if let Ok(re) = regex::Regex::new(regex) {
                        if let Some(caps) = re.captures(&output) {
                            return caps.get(1).map(|m| m.as_str().to_string());
                        }
                    }
                }
                return Some(output.trim().to_string());
            }
        }
        None
    }

    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }

    fn version_status(&self) -> VersionStatus {
        match (self.current_version(), self.desired_version()) {
            (Some(current), Some(desired)) => {
                use semver::Version;
                if let (Ok(curr_ver), Ok(des_ver)) =
                    (Version::parse(&current), Version::parse(&desired))
                {
                    match curr_ver.cmp(&des_ver) {
                        std::cmp::Ordering::Equal => VersionStatus::Match,
                        std::cmp::Ordering::Greater => VersionStatus::Newer,
                        std::cmp::Ordering::Less => VersionStatus::Older,
                    }
                } else {
                    // Fallback to string comparison if not semver
                    match current.cmp(&desired) {
                        std::cmp::Ordering::Equal => VersionStatus::Match,
                        _ => VersionStatus::Unknown,
                    }
                }
            }
            (Some(_), None) => VersionStatus::Unknown,
            (None, Some(_)) => VersionStatus::Missing,
            (None, None) => {
                if DatumChecker::is_installed(self) {
                    VersionStatus::Unknown
                } else {
                    VersionStatus::Missing
                }
            }
        }
    }
}

impl StatusProvider for CliDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }

    fn subsystem(&self) -> &str {
        "cli"
    }

    fn hint(&self) -> &str {
        &self.datum.hint
    }

    fn is_disabled(&self) -> bool {
        false // CLI tools are never disabled by default
    }
}

impl FilterLogic for CliDatum {
    fn is_available(&self) -> bool {
        !DatumChecker::is_installed(self)
    }

    fn prerequisites_satisfied(&self) -> bool {
        true // CLI tools have no special prerequisites
    }

    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
}

impl ConstraintEvaluator for CliDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for CliDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}
