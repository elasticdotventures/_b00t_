// b00t-cli/src/commands/ontology.rs
use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Parser, Debug)]
pub enum OntologyCommands {
    #[clap(about = "Query live capability ontology from datum TOMLs")]
    Query {
        #[clap(long, help = "Filter by agent role (developer|orchestrator|analyst)")]
        role: Option<String>,
        #[clap(long, default_value = "table", value_parser = ["table", "json"])]
        format: String,
    },
}

impl OntologyCommands {
    pub fn execute(&self) -> Result<()> {
        match self {
            OntologyCommands::Query { role, format } => {
                let workspace = crate::utils::get_workspace_root();
                let datum_dir = format!("{}/_b00t_", workspace);
                let ontology = build_ontology(role.as_deref(), &datum_dir)?;
                match format.as_str() {
                    "json" => println!("{}", serde_json::to_string_pretty(&ontology)?),
                    _ => print_ontology_table(&ontology),
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct DatumMeta {
    pub b00t: B00tSection,
    #[serde(default)]
    pub validate: ValidateSection,
    #[serde(default)]
    pub roles: RolesSection,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct B00tSection {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type", default)]
    pub datum_type: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ValidateSection {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub regex: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct RolesSection {
    #[serde(default)]
    pub required_for: Vec<String>,
    #[serde(default)]
    pub optional_for: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ontology {
    pub role: String,
    pub available: Vec<String>,
    pub installable: Vec<String>,
    pub blessings: Vec<String>,
    pub timestamp: String,
}

pub fn build_ontology(role: Option<&str>, datum_dir: &str) -> Result<Ontology> {
    let role_str = role.unwrap_or("developer").to_string();
    let datums = scan_datums(datum_dir)?;

    let role_datums: Vec<_> = datums
        .iter()
        .filter(|d| {
            d.roles.required_for.contains(&role_str) || d.roles.optional_for.contains(&role_str)
        })
        .collect();

    let mut available = Vec::new();
    let mut installable = Vec::new();

    for datum in &role_datums {
        if datum.validate.command.is_empty() {
            continue;
        }
        if is_validated(datum) {
            available.push(datum.b00t.name.clone());
        } else {
            installable.push(datum.b00t.name.clone());
        }
    }

    Ok(Ontology {
        role: role_str,
        available,
        installable,
        blessings: detect_blessings(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

pub fn scan_datums(datum_dir: &str) -> Result<Vec<DatumMeta>> {
    let mut datums = Vec::new();
    if !Path::new(datum_dir).exists() {
        return Ok(datums);
    }
    for entry in fs::read_dir(datum_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "toml") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(datum) = toml::from_str::<DatumMeta>(&content) {
                    if !datum.b00t.name.is_empty() {
                        datums.push(datum);
                    }
                }
            }
        }
    }
    Ok(datums)
}

pub fn is_validated(datum: &DatumMeta) -> bool {
    if datum.validate.command.is_empty() {
        return false;
    }
    let parts: Vec<&str> = datum.validate.command.split_whitespace().collect();
    if parts.is_empty() {
        return false;
    }
    match Command::new(parts[0]).args(&parts[1..]).output() {
        Ok(output) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if datum.validate.regex.is_empty() {
                output.status.success()
            } else {
                regex::Regex::new(&datum.validate.regex)
                    .map(|re| re.is_match(&combined))
                    .unwrap_or(false)
            }
        }
        Err(_) => false,
    }
}

pub fn detect_blessings() -> Vec<String> {
    [
        "ANTHROPIC_API_KEY",
        "GITHUB_TOKEN",
        "OPENAI_API_KEY",
        "HF_TOKEN",
        "CLOUDFLARE_API_TOKEN",
    ]
    .iter()
    .filter(|k| std::env::var(k).map_or(false, |v| !v.is_empty()))
    .map(|k| k.to_string())
    .collect()
}

fn print_ontology_table(o: &Ontology) {
    println!("Ontology for role: {}", o.role);
    println!("\nAvailable ({}):", o.available.len());
    for a in &o.available {
        println!("   {}", a);
    }
    println!("\nInstallable ({}):", o.installable.len());
    for i in &o.installable {
        println!("   b00t cli install {}", i);
    }
    println!("\nBlessings ({}):", o.blessings.len());
    for b in &o.blessings {
        println!("   {}", b);
    }
}

pub fn filter_required_for_role<'a>(datums: &'a [DatumMeta], role: &str) -> Vec<&'a DatumMeta> {
    let role_string = role.to_string();
    datums
        .iter()
        .filter(|d| d.roles.required_for.contains(&role_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_datum(
        name: &str,
        required_for: &[&str],
        optional_for: &[&str],
        validate_cmd: &str,
    ) -> DatumMeta {
        DatumMeta {
            b00t: B00tSection {
                name: name.to_string(),
                datum_type: "cli".to_string(),
            },
            validate: ValidateSection {
                command: validate_cmd.to_string(),
                regex: String::new(),
            },
            roles: RolesSection {
                required_for: required_for.iter().map(|s| s.to_string()).collect(),
                optional_for: optional_for.iter().map(|s| s.to_string()).collect(),
            },
        }
    }

    #[test]
    fn test_datum_roles_parsed() {
        let toml_str = r#"
[b00t]
name = "git"
type = "cli"

[validate]
command = "git --version"
regex = "git version \\d+"

[roles]
required_for = ["developer", "orchestrator"]
optional_for = ["analyst"]
"#;
        let datum: DatumMeta = toml::from_str(toml_str).unwrap();
        assert!(datum.roles.required_for.contains(&"developer".to_string()));
        assert!(datum.roles.optional_for.contains(&"analyst".to_string()));
        assert_eq!(datum.validate.command, "git --version");
    }

    #[test]
    fn test_ontology_filters_by_role() {
        let datums = vec![
            make_datum("git", &["developer"], &[], "git --version"),
            make_datum("k9s", &["orchestrator"], &["developer"], "k9s version"),
        ];
        let required = filter_required_for_role(&datums, "developer");
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].b00t.name, "git");
    }

    #[test]
    fn test_detect_blessings_no_panic() {
        let _ = detect_blessings();
    }

    #[test]
    fn test_scan_datums_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = scan_datums(dir.path().to_str().unwrap());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_scan_datums_nonexistent_dir() {
        let result = scan_datums("/nonexistent/path/that/does/not/exist");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_filter_required_for_role_empty() {
        let datums: Vec<DatumMeta> = vec![];
        let result = filter_required_for_role(&datums, "developer");
        assert_eq!(result.len(), 0);
    }
}
