use crate::traits::*;
use crate::{BootDatum, check_command_available, get_config};
use anyhow::Result;
use duct::cmd;
use std::path::Path;

pub struct DockerDatum {
    pub datum: BootDatum,
}

impl DockerDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        
        let mut datum = config.b00t;
        // Merge top-level env into datum.env
        if let Some(config_env) = config.env {
            if let Some(ref mut datum_env) = datum.env {
                datum_env.extend(config_env);
            } else {
                datum.env = Some(config_env);
            }
        }
        
        Ok(DockerDatum { datum })
    }

    fn is_container_running(&self) -> bool {
        if let Some(image) = &self.datum.image {
            // Check if container with this image is running
            let result = cmd!(
                "docker",
                "ps",
                "--filter",
                &format!("ancestor={}", image),
                "--format",
                "{{.ID}}"
            )
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

    fn is_resource_available(&self) -> bool {
        if let Some(resource_path) = &self.datum.resource_path {
            // Check if resource exists relative to _b00t_/ directory
            if let Ok(b00t_path) = std::env::var("_B00T_Path") {
                let full_path = Path::new(&b00t_path).join(resource_path);
                full_path.exists()
            } else {
                // Fallback to _b00t_ relative to current working directory
                let full_path = Path::new("_b00t_").join(resource_path);
                full_path.exists()
            }
        } else {
            false
        }
    }

    pub fn get_oci_uri(&self) -> Option<String> {
        // Priority: explicit oci_uri > constructed from image
        if let Some(oci_uri) = &self.datum.oci_uri {
            Some(oci_uri.clone())
        } else if let Some(image) = &self.datum.image {
            // Construct OCI URI from image name for drift detection
            if image.contains('/') {
                // Registry/namespace/image:tag format
                if image.starts_with("docker.io/") {
                    Some(image.clone())
                } else {
                    Some(format!("docker.io/{}", image))
                }
            } else {
                // Official library images
                Some(format!("docker.io/library/{}", image))
            }
        } else {
            None
        }
    }

    pub fn check_image_drift(&self) -> bool {
        // Check if local image differs from registry
        if let Some(oci_uri) = self.get_oci_uri() {
            // Basic drift detection - compare local vs remote digests
            let local_digest = cmd!("docker", "images", "--digests", "--format", "{{.Digest}}", &oci_uri)
                .read()
                .unwrap_or_default();
            
            if local_digest.trim().is_empty() {
                return true; // No local image = drifted
            }
            
            // For now, assume no drift if image exists locally
            // Future: compare with remote registry digest
            false
        } else {
            true // No OCI URI = can't validate = assume drifted
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
        // 2. The image is available locally OR the container is running OR the resource files exist
        check_command_available("docker")
            && (self.is_image_available()
                || self.is_container_running()
                || self.is_resource_available())
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
