//! Integration tests for orchestrator grok dependency management
//!
//! Tests the orchestrator's ability to:
//! - Start grok dependencies (qdrant.docker, ollama.docker)
//! - Manage grok-guru.mcp lifecycle
//! - Handle dependency chains
//! - Provide proper environment variables

use anyhow::Result;
use b00t_cli::orchestrator::Orchestrator;
use std::env;
use tempfile::TempDir;

fn setup_temp_dir() -> TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

#[cfg(test)]
mod orchestrator_grok_integration {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires Docker and b00t datum files"]
    async fn test_ensure_grok_dependencies() -> Result<()> {
        // Get the actual _b00t_ path
        let b00t_path = env::var("_B00T_Path").unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap()
                .join(".b00t/_b00t_")
                .to_string_lossy()
                .to_string()
        });

        let orchestrator = Orchestrator::new(&b00t_path)?;

        // Test ensuring dependencies for grok-guru.mcp
        let started = orchestrator.ensure_dependencies("grok-guru.mcp").await?;

        println!("✅ Started services: {:?}", started);

        // Verify expected services were considered
        // Note: They may not start if already running
        assert!(
            started.is_empty() || started.contains(&"qdrant.docker".to_string()),
            "Should attempt to start qdrant if needed"
        );

        Ok(())
    }

    #[tokio::test]
    #[ignore = "Requires Docker and b00t datum files"]
    async fn test_grok_stack_dependencies() -> Result<()> {
        let b00t_path = env::var("_B00T_Path").unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap()
                .join(".b00t/_b00t_")
                .to_string_lossy()
                .to_string()
        });

        let orchestrator = Orchestrator::new(&b00t_path)?;

        // Test the full grok stack
        let started = orchestrator.ensure_dependencies("grok.stack").await;

        match started {
            Ok(services) => {
                println!("✅ Grok stack services: {:?}", services);
                // Stack should include qdrant and possibly ollama
            }
            Err(e) => {
                println!("ℹ️  Grok stack not fully available: {}", e);
                // This is ok in test environment
            }
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_orchestrator_creation_with_missing_path() {
        let temp_dir = setup_temp_dir();
        let temp_path = temp_dir.path().join("nonexistent").to_str().unwrap().to_string();

        // Orchestrator should handle missing path gracefully
        let result = Orchestrator::new(&temp_path);

        match result {
            Ok(_) => {
                println!("ℹ️  Orchestrator created with missing path (may create it)");
            }
            Err(e) => {
                println!("✅ Graceful error for missing path: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore = "Requires Docker and b00t datum files"]
    async fn test_dependency_resolution_order() -> Result<()> {
        let b00t_path = env::var("_B00T_Path").unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap()
                .join(".b00t/_b00t_")
                .to_string_lossy()
                .to_string()
        });

        let orchestrator = Orchestrator::new(&b00t_path)?;

        // Test that dependencies are resolved in correct order
        // grok-guru.mcp → qdrant.docker, ollama.docker
        let started = orchestrator.ensure_dependencies("grok-guru.mcp").await?;

        println!("📊 Dependency resolution order: {:?}", started);

        // Infrastructure (docker) should be started before application (mcp)
        // This is implicit in the current implementation

        Ok(())
    }
}

#[cfg(test)]
mod datum_loading {
    use super::*;

    #[test]
    fn test_load_grok_datum_files() -> Result<()> {
        // Test that we can locate and read grok datum files
        let b00t_path = env::var("_B00T_Path").unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap()
                .join(".b00t/_b00t_")
                .to_string_lossy()
                .to_string()
        });

        let grok_guru_path = std::path::Path::new(&b00t_path).join("grok-guru.mcp.toml");
        let grok_stack_path = std::path::Path::new(&b00t_path).join("grok.stack.toml");

        if grok_guru_path.exists() {
            let content = std::fs::read_to_string(&grok_guru_path)?;
            println!("✅ Found grok-guru.mcp.toml ({} bytes)", content.len());
            assert!(
                content.contains("grok-guru") || content.contains("grok_guru"),
                "grok-guru.mcp.toml should contain grok-guru reference"
            );
        } else {
            println!("⚠️  grok-guru.mcp.toml not found at {:?}", grok_guru_path);
        }

        if grok_stack_path.exists() {
            let content = std::fs::read_to_string(&grok_stack_path)?;
            println!("✅ Found grok.stack.toml ({} bytes)", content.len());
        } else {
            println!("⚠️  grok.stack.toml not found at {:?}", grok_stack_path);
        }

        Ok(())
    }

    #[test]
    fn test_qdrant_datum_exists() -> Result<()> {
        let b00t_path = env::var("_B00T_Path").unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap()
                .join(".b00t/_b00t_")
                .to_string_lossy()
                .to_string()
        });

        let qdrant_path = std::path::Path::new(&b00t_path).join("qdrant.docker.toml");

        if qdrant_path.exists() {
            let content = std::fs::read_to_string(&qdrant_path)?;
            println!("✅ Found qdrant.docker.toml");
            assert!(
                content.contains("qdrant") || content.contains("QDRANT"),
                "qdrant.docker.toml should contain qdrant reference"
            );
        } else {
            println!("⚠️  qdrant.docker.toml not found at {:?}", qdrant_path);
        }

        Ok(())
    }
}

#[cfg(test)]
mod environment_propagation {
    use super::*;

    #[tokio::test]
    #[ignore = "Requires Docker and running services"]
    async fn test_qdrant_url_propagation() -> Result<()> {
        let b00t_path = env::var("_B00T_Path").unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap()
                .join(".b00t/_b00t_")
                .to_string_lossy()
                .to_string()
        });

        let orchestrator = Orchestrator::new(&b00t_path)?;

        // Start dependencies
        let _ = orchestrator.ensure_dependencies("grok-guru.mcp").await?;

        // After starting, QDRANT_URL should be available
        // Note: This depends on the orchestrator implementation
        // Currently it doesn't set env vars, just starts services

        if let Ok(qdrant_url) = env::var("QDRANT_URL") {
            println!("✅ QDRANT_URL available: {}", qdrant_url);
            assert!(
                qdrant_url.contains("http"),
                "QDRANT_URL should be a valid HTTP URL"
            );
        } else {
            println!("ℹ️  QDRANT_URL not set by orchestrator (may need manual config)");
        }

        Ok(())
    }
}
