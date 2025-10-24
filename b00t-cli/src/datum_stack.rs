use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::{BootDatum, DatumType, UnifiedConfig};
use crate::dependency_resolver::DependencyResolver;

/// Stack operations for managing multi-component software stacks
pub struct StackDatum {
    pub datum: BootDatum,
    pub stack_path: PathBuf,
}

impl StackDatum {
    /// Load a stack from a TOML file
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .context(format!("Failed to read stack file: {}", path.display()))?;

        let config: crate::UnifiedConfig = toml::from_str(&content)
            .context(format!("Failed to parse stack TOML: {}", path.display()))?;

        let datum = config.b00t;

        // Validate this is actually a stack datum
        if datum.datum_type != Some(DatumType::Stack) {
            bail!("File {} is not a stack datum (type: {:?})",
                  path.display(), datum.datum_type);
        }

        // Validate members field exists
        if datum.members.is_none() || datum.members.as_ref().unwrap().is_empty() {
            bail!("Stack {} has no members defined", datum.name);
        }

        Ok(Self {
            datum,
            stack_path: path.to_path_buf(),
        })
    }

    /// Get all member datum IDs in this stack
    pub fn get_members(&self) -> Vec<String> {
        self.datum.members.clone().unwrap_or_default()
    }

    /// Validate that all stack members exist and are accessible
    pub fn validate_members(&self, available_datums: &HashMap<String, BootDatum>) -> Result<Vec<String>> {
        let mut errors = Vec::new();

        for member_id in self.get_members() {
            if !available_datums.contains_key(&member_id) {
                errors.push(format!(
                    "Stack '{}' references missing datum: '{}'",
                    self.datum.name, member_id
                ));
            }
        }

        Ok(errors)
    }

    /// Resolve all dependencies for stack members
    /// Returns installation order including transitive dependencies
    pub fn resolve_dependencies<'a>(
        &self,
        available_datums: &'a HashMap<String, &'a BootDatum>
    ) -> Result<Vec<String>> {
        let resolver = DependencyResolver::new(available_datums.values().copied().collect());

        // Get all members
        let members = self.get_members();

        // Resolve dependencies for all members
        resolver.resolve_many(&members)
    }

    /// Generate docker-compose.yml for Docker-based stacks
    pub fn generate_docker_compose(&self, available_datums: &HashMap<String, BootDatum>) -> Result<String> {
        let mut services = Vec::new();

        for member_id in self.get_members() {
            let datum = available_datums.get(&member_id)
                .context(format!("Member {} not found", member_id))?;

            // Only process Docker datums
            if datum.datum_type != Some(DatumType::Docker) {
                continue;
            }

            let service_name = datum.name.clone();
            let image = datum.image.as_ref()
                .context(format!("Docker datum {} has no image", member_id))?;

            // Build service definition
            let mut service = format!("  {}:\n", service_name);
            service.push_str(&format!("    image: {}\n", image));

            // Add container name
            service.push_str(&format!("    container_name: {}\n", service_name));

            // Add environment variables
            if let Some(env) = &datum.env {
                service.push_str("    environment:\n");
                for (key, value) in env {
                    service.push_str(&format!("      {}: {}\n", key, value));
                }
            }

            // Merge stack-level env vars
            if let Some(stack_env) = &self.datum.env {
                if datum.env.is_none() {
                    service.push_str("    environment:\n");
                }
                for (key, value) in stack_env {
                    service.push_str(&format!("      {}: {}\n", key, value));
                }
            }

            // Add docker args as command/ports/volumes
            if let Some(docker_args) = &datum.docker_args {
                self.parse_docker_args_to_compose(&mut service, docker_args);
            }

            services.push(service);
        }

        // Build final docker-compose.yml
        let mut compose = String::from("version: '3.8'\n\nservices:\n");
        compose.push_str(&services.join("\n"));
        compose.push_str("\n\nnetworks:\n  default:\n    name: ");
        compose.push_str(&self.datum.name);
        compose.push_str("_network\n");

        Ok(compose)
    }

    /// Parse docker args into docker-compose format
    fn parse_docker_args_to_compose(&self, service: &mut String, args: &[String]) {
        let mut i = 0;
        let mut ports = Vec::new();
        let mut volumes = Vec::new();

        while i < args.len() {
            match args[i].as_str() {
                "-p" | "--publish" => {
                    if i + 1 < args.len() {
                        ports.push(args[i + 1].clone());
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                "-v" | "--volume" => {
                    if i + 1 < args.len() {
                        volumes.push(args[i + 1].clone());
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                _ => {
                    i += 1;
                }
            }
        }

        if !ports.is_empty() {
            service.push_str("    ports:\n");
            for port in ports {
                service.push_str(&format!("      - \"{}\"\n", port));
            }
        }

        if !volumes.is_empty() {
            service.push_str("    volumes:\n");
            for volume in volumes {
                service.push_str(&format!("      - {}\n", volume));
            }
        }
    }

    /// List all stacks in a directory
    pub fn list_stacks(b00t_dir: &Path) -> Result<Vec<PathBuf>> {
        let mut stacks = Vec::new();

        if !b00t_dir.exists() {
            return Ok(stacks);
        }

        for entry in std::fs::read_dir(b00t_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.to_string_lossy().ends_with(".stack.toml") {
                stacks.push(path);
            }
        }

        Ok(stacks)
    }

    /// Get stack summary for display
    pub fn get_summary(&self) -> String {
        let members_count = self.get_members().len();
        format!(
            "{} ({} members): {}",
            self.datum.name,
            members_count,
            self.datum.hint
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_docker_datum(name: &str, image: &str) -> BootDatum {
        BootDatum {
            name: name.to_string(),
            datum_type: Some(DatumType::Docker),
            desires: None,
            hint: format!("Test docker datum {}", name),
            install: None,
            update: None,
            version: None,
            version_regex: None,
            command: None,
            args: None,
            vsix_id: None,
            script: None,
            image: Some(image.to_string()),
            docker_args: Some(vec![
                "-p".to_string(),
                "5432:5432".to_string(),
            ]),
            oci_uri: None,
            resource_path: None,
            chart_path: None,
            namespace: None,
            values_file: None,
            keywords: None,
            package_name: None,
            env: Some({
                let mut env = HashMap::new();
                env.insert("TEST_VAR".to_string(), "test_value".to_string());
                env
            }),
            require: None,
            aliases: None,
            depends_on: None,
            members: None,
            mcp: None,
        }
    }

    fn create_test_stack(name: &str, members: Vec<String>) -> BootDatum {
        BootDatum {
            name: name.to_string(),
            datum_type: Some(DatumType::Stack),
            desires: None,
            hint: format!("Test stack {}", name),
            install: None,
            update: None,
            version: None,
            version_regex: None,
            command: None,
            args: None,
            vsix_id: None,
            script: None,
            image: None,
            docker_args: None,
            oci_uri: None,
            resource_path: None,
            chart_path: None,
            namespace: None,
            values_file: None,
            keywords: None,
            package_name: None,
            env: Some({
                let mut env = HashMap::new();
                env.insert("STACK_VAR".to_string(), "stack_value".to_string());
                env
            }),
            require: None,
            aliases: None,
            depends_on: None,
            members: Some(members),
            mcp: None,
        }
    }

    #[test]
    fn test_get_members() {
        let stack = create_test_stack("test-stack", vec![
            "postgres.docker".to_string(),
            "redis.docker".to_string(),
        ]);

        let stack_datum = StackDatum {
            datum: stack,
            stack_path: PathBuf::from("/tmp/test.stack.toml"),
        };

        let members = stack_datum.get_members();
        assert_eq!(members.len(), 2);
        assert!(members.contains(&"postgres.docker".to_string()));
        assert!(members.contains(&"redis.docker".to_string()));
    }

    #[test]
    fn test_validate_members() {
        let stack = create_test_stack("test-stack", vec![
            "postgres.docker".to_string(),
            "missing.docker".to_string(),
        ]);

        let stack_datum = StackDatum {
            datum: stack,
            stack_path: PathBuf::from("/tmp/test.stack.toml"),
        };

        let mut available = HashMap::new();
        available.insert(
            "postgres.docker".to_string(),
            create_test_docker_datum("postgres", "postgres:16"),
        );

        let errors = stack_datum.validate_members(&available).unwrap();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("missing.docker"));
    }

    #[test]
    fn test_generate_docker_compose() {
        let stack = create_test_stack("test-stack", vec![
            "postgres.docker".to_string(),
        ]);

        let stack_datum = StackDatum {
            datum: stack,
            stack_path: PathBuf::from("/tmp/test.stack.toml"),
        };

        let mut available = HashMap::new();
        available.insert(
            "postgres.docker".to_string(),
            create_test_docker_datum("postgres", "postgres:16"),
        );

        let compose = stack_datum.generate_docker_compose(&available).unwrap();

        assert!(compose.contains("version: '3.8'"));
        assert!(compose.contains("postgres:"));
        assert!(compose.contains("image: postgres:16"));
        assert!(compose.contains("5432:5432"));
        assert!(compose.contains("TEST_VAR: test_value"));
        assert!(compose.contains("STACK_VAR: stack_value"));
    }
}
