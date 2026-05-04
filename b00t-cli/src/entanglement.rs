//! Datum Entanglement Validation
//!
//! Validates cross-datum references (entanglements) to build a capability graph.
//! Ensures referenced datums exist and match their declared stereotypes (types).

use crate::{BootDatum, DatumType};
use anyhow::{Context, Result, bail};
use std::collections::{HashMap, HashSet};

/// Parse an entanglement reference into (name, optional_type)
///
/// Supports formats:
/// - "datum-name" → ("datum-name", None)
/// - "datum-name.mcp" → ("datum-name", Some(DatumType::Mcp))
/// - "datum-name.cli" → ("datum-name", Some(DatumType::Cli))
pub fn parse_entanglement_ref(reference: &str) -> Result<(String, Option<DatumType>)> {
    let parts: Vec<&str> = reference.split('.').collect();

    match parts.len() {
        1 => {
            // Simple reference: "datum-name"
            Ok((parts[0].to_string(), None))
        }
        2 => {
            // Qualified reference: "datum-name.type"
            let name = parts[0].to_string();
            let type_str = parts[1];

            let datum_type = match type_str.to_lowercase().as_str() {
                "agent" => Some(DatumType::Agent),
                "mcp" => Some(DatumType::Mcp),
                "cli" => Some(DatumType::Cli),
                "bash" => Some(DatumType::Bash),
                "vscode" => Some(DatumType::Vscode),
                "docker" => Some(DatumType::Docker),
                "k8s" => Some(DatumType::K8s),
                "ai" => Some(DatumType::Ai),
                "ai_model" => Some(DatumType::AiModel),
                "api" => Some(DatumType::Api),
                "stack" => Some(DatumType::Stack),
                _ => bail!(
                    "Unknown datum type '{}' in reference '{}'",
                    type_str,
                    reference
                ),
            };

            Ok((name, datum_type))
        }
        _ => {
            bail!(
                "Invalid entanglement reference format: '{}'. Expected 'name' or 'name.type'",
                reference
            )
        }
    }
}

/// Validate a single entanglement reference
///
/// Checks:
/// 1. Referenced datum exists
/// 2. If type specified, datum matches that type
///
/// Returns the resolved BootDatum if valid
pub fn validate_entanglement_ref(
    reference: &str,
    all_datums: &HashMap<String, BootDatum>,
    expected_type: Option<DatumType>,
) -> Result<BootDatum> {
    let (datum_name, ref_type) = parse_entanglement_ref(reference)?;

    // Check if datum exists
    let datum = all_datums.get(&datum_name).ok_or_else(|| {
        anyhow::anyhow!(
            "Entanglement reference '{}' not found. Datum '{}' does not exist.",
            reference,
            datum_name
        )
    })?;

    // Determine which type to validate against
    let type_to_check = ref_type.or(expected_type);

    // If a type is specified, validate it matches
    if let Some(expected) = type_to_check {
        let actual = datum.datum_type.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Datum '{}' has no type specified, cannot validate entanglement",
                datum_name
            )
        })?;

        if actual != &expected {
            bail!(
                "Type mismatch: entanglement '{}' expects type '{:?}' but datum has type '{:?}'",
                reference,
                expected,
                actual
            );
        }
    }

    Ok(datum.clone())
}

/// Validate all entanglements for a datum
///
/// Checks each entanglement field and validates references.
/// Returns a map of entanglement type to validated datums.
pub fn validate_datum_entanglements(
    datum: &BootDatum,
    all_datums: &HashMap<String, BootDatum>,
) -> Result<HashMap<String, Vec<BootDatum>>> {
    let mut validated = HashMap::new();

    // Validate agent entanglements
    if let Some(refs) = &datum.entangled_agents {
        let mut agents = Vec::new();
        for reference in refs {
            let resolved =
                validate_entanglement_ref(reference, all_datums, Some(DatumType::Agent)).context(
                    format!("Invalid agent entanglement in datum '{}'", datum.name),
                )?;
            agents.push(resolved);
        }
        validated.insert("agents".to_string(), agents);
    }

    // Validate CLI entanglements
    if let Some(refs) = &datum.entangled_cli {
        let mut cli_tools = Vec::new();
        for reference in refs {
            let resolved =
                validate_entanglement_ref(reference, all_datums, Some(DatumType::Cli)).context(
                    format!("Invalid CLI entanglement in datum '{}'", datum.name),
                )?;
            cli_tools.push(resolved);
        }
        validated.insert("cli".to_string(), cli_tools);
    }

    // Validate MCP entanglements
    if let Some(refs) = &datum.entangled_mcp {
        let mut mcp_servers = Vec::new();
        for reference in refs {
            let resolved =
                validate_entanglement_ref(reference, all_datums, Some(DatumType::Mcp)).context(
                    format!("Invalid MCP entanglement in datum '{}'", datum.name),
                )?;
            mcp_servers.push(resolved);
        }
        validated.insert("mcp".to_string(), mcp_servers);
    }

    // Validate AI model entanglements
    if let Some(refs) = &datum.entangled_ai_models {
        let mut ai_models = Vec::new();
        for reference in refs {
            let resolved =
                validate_entanglement_ref(reference, all_datums, Some(DatumType::AiModel))
                    .context(format!(
                        "Invalid AI model entanglement in datum '{}'",
                        datum.name
                    ))?;
            ai_models.push(resolved);
        }
        validated.insert("ai_models".to_string(), ai_models);
    }

    // Validate API entanglements
    if let Some(refs) = &datum.entangled_apis {
        let mut apis = Vec::new();
        for reference in refs {
            let resolved =
                validate_entanglement_ref(reference, all_datums, Some(DatumType::Api)).context(
                    format!("Invalid API entanglement in datum '{}'", datum.name),
                )?;
            apis.push(resolved);
        }
        validated.insert("apis".to_string(), apis);
    }

    // Validate Docker entanglements
    if let Some(refs) = &datum.entangled_docker {
        let mut docker_containers = Vec::new();
        for reference in refs {
            let resolved =
                validate_entanglement_ref(reference, all_datums, Some(DatumType::Docker)).context(
                    format!("Invalid Docker entanglement in datum '{}'", datum.name),
                )?;
            docker_containers.push(resolved);
        }
        validated.insert("docker".to_string(), docker_containers);
    }

    // Validate K8s entanglements
    if let Some(refs) = &datum.entangled_k8s {
        let mut k8s_resources = Vec::new();
        for reference in refs {
            let resolved =
                validate_entanglement_ref(reference, all_datums, Some(DatumType::K8s)).context(
                    format!("Invalid K8s entanglement in datum '{}'", datum.name),
                )?;
            k8s_resources.push(resolved);
        }
        validated.insert("k8s".to_string(), k8s_resources);
    }

    Ok(validated)
}

/// Detect circular entanglements (for dependency resolution)
///
/// Performs a depth-first search to detect cycles in the entanglement graph.
/// Returns an error if a cycle is detected.
pub fn detect_circular_entanglements(
    datum_name: &str,
    all_datums: &HashMap<String, BootDatum>,
    visited: &mut HashSet<String>,
    path: &mut Vec<String>,
) -> Result<()> {
    if path.contains(&datum_name.to_string()) {
        let cycle = path.join(" → ");
        bail!("Circular entanglement detected: {} → {}", cycle, datum_name);
    }

    if visited.contains(datum_name) {
        return Ok(());
    }

    visited.insert(datum_name.to_string());
    path.push(datum_name.to_string());

    if let Some(datum) = all_datums.get(datum_name) {
        // Check all entanglement types
        let all_refs: Vec<String> = vec![
            datum.entangled_agents.clone().unwrap_or_default(),
            datum.entangled_cli.clone().unwrap_or_default(),
            datum.entangled_mcp.clone().unwrap_or_default(),
            datum.entangled_ai_models.clone().unwrap_or_default(),
            datum.entangled_apis.clone().unwrap_or_default(),
            datum.entangled_docker.clone().unwrap_or_default(),
            datum.entangled_k8s.clone().unwrap_or_default(),
        ]
        .into_iter()
        .flatten()
        .collect();

        for reference in all_refs {
            let (ref_name, _) = parse_entanglement_ref(&reference)?;
            detect_circular_entanglements(&ref_name, all_datums, visited, path)?;
        }
    }

    path.pop();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_reference() {
        let (name, dtype) = parse_entanglement_ref("geminicli").unwrap();
        assert_eq!(name, "geminicli");
        assert_eq!(dtype, None);
    }

    #[test]
    fn test_parse_qualified_reference() {
        let (name, dtype) = parse_entanglement_ref("geminicli.cli").unwrap();
        assert_eq!(name, "geminicli");
        assert_eq!(dtype, Some(DatumType::Cli));
    }

    #[test]
    fn test_parse_mcp_reference() {
        let (name, dtype) = parse_entanglement_ref("gemini-mcp-tool.mcp").unwrap();
        assert_eq!(name, "gemini-mcp-tool");
        assert_eq!(dtype, Some(DatumType::Mcp));
    }

    #[test]
    fn test_parse_invalid_type() {
        let result = parse_entanglement_ref("foo.invalid");
        assert!(result.is_err());
    }
}
