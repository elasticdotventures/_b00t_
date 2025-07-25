use anyhow::Result;
use duct::cmd;
use crate::{BootDatum, DatumType, get_config, check_command_available, get_expanded_path};
use crate::traits::*;

pub struct DockerDatum {
    pub datum: BootDatum,
    pub config_path: String,
}

impl DockerDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(DockerDatum {
            datum: config.b00t,
            config_path: path.to_string(),
        })
    }
    
    pub fn from_datum(datum: BootDatum, config_path: String) -> Self {
        DockerDatum { datum, config_path }
    }

    fn is_container_running(&self) -> bool {
        if let Some(image) = &self.datum.image {
            // Check if container with this image is running
            let result = cmd!("docker", "ps", "--filter", &format!("ancestor={}", image), "--format", "{{.ID}}")
                .read();
            match result {
                Ok(output) => !output.trim().is_empty(),
                Err(_) => false,
            }
        } else {
            false
        }
    }

    fn is_image_available(&self) -> bool {
        if let Some(image) = &self.datum.image {
            // Check if Docker image exists locally
            let result = cmd!("docker", "images", "-q", image).read();
            match result {
                Ok(output) => !output.trim().is_empty(),
                Err(_) => false,
            }
        } else {
            false
        }
    }
}

impl DatumChecker for DockerDatum {
    fn is_installed(&self) -> bool {
        // Docker containers are "installed" if:
        // 1. Docker is available
        // 2. The image is available locally OR the container is running
        check_command_available("docker") && 
        (self.is_image_available() || self.is_container_running())
    }
    
    fn current_version(&self) -> Option<String> {
        if let Some(image) = &self.datum.image {
            // Try to get the image tag/version
            if let Some((_, tag)) = image.split_once(':') {
                Some(tag.to_string())
            } else {
                Some("latest".to_string())
            }
        } else {
            None
        }
    }
    
    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }
    
    fn version_status(&self) -> VersionStatus {
        if !check_command_available("docker") {
            return VersionStatus::Missing;
        }
        
        if self.is_container_running() {
            VersionStatus::Match // Running containers are considered "matching"
        } else if self.is_image_available() {
            VersionStatus::Unknown // Image available but not running
        } else {
            VersionStatus::Missing
        }
    }
}

impl StatusProvider for DockerDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }
    
    fn subsystem(&self) -> &str {
        "docker"
    }
    
    fn display_name(&self) -> &str {
        &self.datum.name
    }
    
    fn hint(&self) -> &str {
        &self.datum.hint
    }
    
    fn is_disabled(&self) -> bool {
        // Docker containers are disabled if Docker is not available
        !check_command_available("docker")
    }
}

impl FilterLogic for DockerDatum {
    fn is_available(&self) -> bool {
        !DatumChecker::is_installed(self) && self.prerequisites_satisfied()
    }
    
    fn prerequisites_satisfied(&self) -> bool {
        // Check if require constraints are satisfied
        if let Some(require) = &self.datum.require {
            self.evaluate_constraints(require)
        } else {
            // Default: Docker must be available
            check_command_available("docker")
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

impl ConstraintEvaluator for DockerDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for DockerDatum {
    fn datum_type(&self) -> DatumType {
        DatumType::Docker
    }
    
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

pub fn get_docker_tools_status(path: &str) -> Result<Vec<Box<dyn DatumProvider>>> {
    let mut tools: Vec<Box<dyn DatumProvider>> = Vec::new();
    let expanded_path = get_expanded_path(path)?;
    
    if let Ok(entries) = std::fs::read_dir(&expanded_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                    if file_name.ends_with(".docker.toml") {
                        if let Some(tool_name) = file_name.strip_suffix(".docker.toml") {
                            if let Ok(docker_datum) = DockerDatum::from_config(tool_name, path) {
                                // Apply filtering logic: only include if prerequisites satisfied or already installed
                                if !FilterLogic::is_disabled(&docker_datum) || DatumChecker::is_installed(&docker_datum) {
                                    tools.push(Box::new(docker_datum));
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