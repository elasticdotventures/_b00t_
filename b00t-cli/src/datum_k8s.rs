use crate::traits::*;
use crate::{BootDatum, check_command_available, get_config};
use anyhow::Result;
use duct::cmd;
use std::path::Path;

pub struct K8sDatum {
    pub datum: BootDatum,
}

impl K8sDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(K8sDatum { datum: config.b00t })
    }

    fn is_chart_available(&self) -> bool {
        if let Some(chart_path) = &self.datum.chart_path {
            // Check if helm chart exists relative to REPO_ROOT
            if let Ok(repo_root) = std::env::var("REPO_ROOT") {
                let full_path = Path::new(&repo_root).join(chart_path);
                full_path.exists() && full_path.is_dir()
            } else {
                // Fallback to relative path from current dir
                Path::new(chart_path).exists()
            }
        } else {
            false
        }
    }

    fn is_deployed(&self) -> bool {
        if let Some(chart_path) = &self.datum.chart_path {
            let chart_name = Path::new(chart_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&self.datum.name);

            let namespace = self.datum.namespace.as_deref().unwrap_or("default");

            // Check if helm release exists
            let result = cmd!("helm", "list", "-n", namespace, "-q").read();

            match result {
                Ok(output) => output.lines().any(|line| line.trim() == chart_name),
                Err(_) => false,
            }
        } else {
            false
        }
    }

    fn get_chart_version(&self) -> Option<String> {
        if let Some(chart_path) = &self.datum.chart_path {
            let chart_yaml_path = if let Ok(repo_root) = std::env::var("REPO_ROOT") {
                Path::new(&repo_root).join(chart_path).join("Chart.yaml")
            } else {
                Path::new(chart_path).join("Chart.yaml")
            };

            if let Ok(content) = std::fs::read_to_string(chart_yaml_path) {
                // Parse version from Chart.yaml
                for line in content.lines() {
                    if line.starts_with("version:") {
                        return line.split(':').nth(1).map(|v| v.trim().to_string());
                    }
                }
            }
        }
        None
    }
}

impl TryFrom<(&str, &str)> for K8sDatum {
    type Error = anyhow::Error;

    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
    }
}

impl DatumChecker for K8sDatum {
    fn is_installed(&self) -> bool {
        // K8s charts are "installed" if:
        // 1. kubectl and helm are available
        // 2. The chart is deployed OR the chart files are available
        check_command_available("kubectl")
            && check_command_available("helm")
            && (self.is_deployed() || self.is_chart_available())
    }

    fn current_version(&self) -> Option<String> {
        if self.is_deployed() {
            // Try to get the deployed chart version
            if let Some(chart_path) = &self.datum.chart_path {
                let chart_name = Path::new(chart_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&self.datum.name);

                let namespace = self.datum.namespace.as_deref().unwrap_or("default");

                let result = cmd!("helm", "list", "-n", namespace, "-o", "json").read();

                if let Ok(output) = result {
                    // Parse JSON to get chart version (simplified)
                    if output.contains(&format!("\"name\":\"{chart_name}\"")) {
                        // Extract version - this is a simple approach
                        return Some("deployed".to_string());
                    }
                }
            }
        }

        // Fallback to chart file version
        self.get_chart_version()
    }

    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }

    fn version_status(&self) -> VersionStatus {
        if !check_command_available("kubectl") || !check_command_available("helm") {
            return VersionStatus::Missing;
        }

        if self.is_deployed() {
            VersionStatus::Match // Deployed charts are considered "matching"
        } else if self.is_chart_available() {
            VersionStatus::Unknown // Chart available but not deployed
        } else {
            VersionStatus::Missing
        }
    }
}

impl StatusProvider for K8sDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }

    fn subsystem(&self) -> &str {
        "k8s"
    }

    fn hint(&self) -> &str {
        &self.datum.hint
    }

    fn is_disabled(&self) -> bool {
        // K8s charts are disabled if kubectl or helm are not available
        !check_command_available("kubectl") || !check_command_available("helm")
    }
}

impl FilterLogic for K8sDatum {
    fn is_available(&self) -> bool {
        !DatumChecker::is_installed(self) && self.prerequisites_satisfied()
    }

    fn prerequisites_satisfied(&self) -> bool {
        // Check if require constraints are satisfied
        if let Some(require) = &self.datum.require {
            self.evaluate_constraints(require)
        } else {
            // Default: kubectl and helm must be available
            check_command_available("kubectl") && check_command_available("helm")
        }
    }

    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
}

impl ConstraintEvaluator for K8sDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for K8sDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}
