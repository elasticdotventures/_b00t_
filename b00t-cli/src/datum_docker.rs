use anyhow::Result;
use duct::cmd;
use crate::{BootDatum, get_config, check_command_available};
use crate::traits::*;

pub struct DockerDatum {
    pub datum: BootDatum,
}

impl DockerDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(DockerDatum {
            datum: config.b00t,
        })
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

impl TryFrom<(&str, &str)> for DockerDatum {
    type Error = anyhow::Error;
    
    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
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
    
}

impl ConstraintEvaluator for DockerDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for DockerDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

