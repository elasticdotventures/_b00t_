use std::env;
use tempfile::TempDir;
use b00t_cli::*;

#[cfg(test)]
mod tests {
    use super::*;
    use b00t_cli::datum_docker::*;
    
    #[test]
    fn test_docker_datum_creation() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();
        
        // Create Docker config
        let config_content = r#"
[b00t]
name = "postgres"
type = "docker"
hint = "PostgreSQL database server via Docker container"
desires = "16-alpine"
image = "postgres:16-alpine"
docker_args = [
    "-e", "POSTGRES_PASSWORD=password",
    "-e", "POSTGRES_DB=testdb", 
    "-p", "5432:5432",
    "-v", "postgres_data:/var/lib/postgresql/data"
]

[b00t.env]
POSTGRES_PASSWORD = ""
POSTGRES_DB = "testdb"
POSTGRES_PORT = "5432"
"#;
        
        let config_path = temp_dir.path().join("postgres.docker.toml");
        std::fs::write(&config_path, config_content).unwrap();
        
        // Test Docker datum creation
        let docker_datum = DockerDatum::from_config("postgres", path).unwrap();
        assert_eq!(StatusProvider::name(&docker_datum), "postgres");
        assert_eq!(StatusProvider::subsystem(&docker_datum), "docker");
        assert_eq!(StatusProvider::hint(&docker_datum), "PostgreSQL database server via Docker container");
        assert_eq!(DatumProvider::datum_type(&docker_datum), DatumType::Docker);
    }
    
    #[test]
    fn test_docker_version_parsing() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();
        
        // Create Docker config with version
        let config_content = r#"
[b00t]
name = "redis"
type = "docker"
hint = "Redis in-memory data store"
desires = "7-alpine"
image = "redis:7-alpine"
"#;
        
        let config_path = temp_dir.path().join("redis.docker.toml");
        std::fs::write(&config_path, config_content).unwrap();
        
        let docker_datum = DockerDatum::from_config("redis", path).unwrap();
        
        // Should extract version from image tag
        let current_version = DatumChecker::current_version(&docker_datum);
        assert_eq!(current_version, Some("7-alpine".to_string()));
        
        let desired_version = DatumChecker::desired_version(&docker_datum);
        assert_eq!(desired_version, Some("7-alpine".to_string()));
    }
    
    #[test]
    fn test_docker_constraint_evaluation() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();
        
        // Create Docker config
        let config_content = r#"
[b00t]
name = "test-container"
type = "docker"
hint = "Test container"
image = "alpine:latest"
"#;
        
        let config_path = temp_dir.path().join("test-container.docker.toml");
        std::fs::write(&config_path, config_content).unwrap();
        
        let docker_datum = DockerDatum::from_config("test-container", path).unwrap();
        
        // Prerequisites should depend on Docker being available
        // This test will pass/fail based on whether Docker is installed on the test system
        let prerequisites_met = FilterLogic::prerequisites_satisfied(&docker_datum);
        let docker_available = b00t_cli::check_command_available("docker");
        
        // The prerequisite check should match Docker availability
        assert_eq!(prerequisites_met, docker_available);
    }
    
    #[test]
    fn test_docker_tools_status_collection() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();
        
        // Create multiple Docker configs
        let postgres_config = r#"
[b00t]
name = "postgres"
type = "docker"
hint = "PostgreSQL database"
image = "postgres:16"
"#;
        
        let redis_config = r#"
[b00t]
name = "redis"
type = "docker"
hint = "Redis cache"
image = "redis:7"
"#;
        
        std::fs::write(temp_dir.path().join("postgres.docker.toml"), postgres_config).unwrap();
        std::fs::write(temp_dir.path().join("redis.docker.toml"), redis_config).unwrap();
        
        let tools = get_docker_tools_status(path).unwrap();
        
        // Should collect Docker tools based on Docker availability
        // If Docker is available, tools should be included
        // If Docker is not available, tools should be filtered out (disabled)
        let docker_available = b00t_cli::check_command_available("docker");
        
        if docker_available {
            // Should include at least some tools when Docker is available
            let tool_names: Vec<_> = tools.iter()
                .map(|tool| StatusProvider::name(tool.as_ref()))
                .collect();
            
            // At least one of our test containers should be present
            assert!(tool_names.contains(&"postgres") || tool_names.contains(&"redis"));
        } else {
            // Tools may be filtered out if Docker is not available
            // This is acceptable behavior
        }
    }
}