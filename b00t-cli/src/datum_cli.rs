use anyhow::Result;
use duct::cmd;
use crate::{BootDatum, DatumType, get_config, check_command_available};
use crate::traits::*;

pub struct CliDatum {
    pub datum: BootDatum,
    pub config_path: String,
}

impl CliDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(CliDatum {
            datum: config.b00t,
            config_path: path.to_string(),
        })
    }
    
    pub fn from_datum(datum: BootDatum, config_path: String) -> Self {
        CliDatum { datum, config_path }
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
                if let (Ok(curr_ver), Ok(des_ver)) = (Version::parse(&current), Version::parse(&desired)) {
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
    
    fn display_name(&self) -> &str {
        &self.datum.name
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
    
    fn is_disabled(&self) -> bool {
        StatusProvider::is_disabled(self)
    }
    
    fn is_installed(&self) -> bool {
        DatumChecker::is_installed(self)
    }
    
    fn subsystem(&self) -> &str {
        StatusProvider::subsystem(self)
    }
}

impl ConstraintEvaluator for CliDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for CliDatum {
    fn datum_type(&self) -> DatumType {
        DatumType::Unknown
    }
    
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

pub fn get_cli_tools_status(path: &str) -> Result<Vec<Box<dyn DatumProvider>>> {
    let mut tools: Vec<Box<dyn DatumProvider>> = Vec::new();
    let expanded_path = crate::get_expanded_path(path)?;
    
    if let Ok(entries) = std::fs::read_dir(&expanded_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                    if file_name.ends_with(".cli.toml") {
                        if let Some(tool_name) = file_name.strip_suffix(".cli.toml") {
                            if let Ok(cli_datum) = CliDatum::from_config(tool_name, path) {
                                tools.push(Box::new(cli_datum));
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(tools)
}