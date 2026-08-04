// b00t-cli/src/commands/ontology.rs
use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use ufo_types::Stereotyped;

use crate::DatumType;

#[derive(Parser, Debug)]
pub enum OntologyCommands {
    #[clap(about = "Query live capability ontology from datum TOMLs")]
    Query {
        #[clap(long, help = "Filter by agent role (developer|orchestrator|analyst)")]
        role: Option<String>,
        #[clap(long, default_value = "table", value_parser = ["table", "json"])]
        format: String,
    },
    #[clap(about = "SPARQL-like triple-pattern query over datum knowledge graph")]
    Sparql {
        #[clap(long, help = "Subject pattern (datum name substring match)")]
        subject: Option<String>,
        #[clap(
            long,
            help = "Predicate to expand: type|roles|validate|all",
            default_value = "all"
        )]
        predicate: String,
        #[clap(long, default_value = "json", value_parser = ["json", "table"])]
        format: String,
    },
    #[clap(about = "Semantic agent search: find best-fit agent for a task description")]
    FindAgent {
        #[clap(help = "Task description to match against agent capabilities")]
        task: String,
        #[clap(long, help = "Maximum results", default_value = "5")]
        limit: usize,
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
            OntologyCommands::Sparql {
                subject,
                predicate,
                format,
            } => {
                let workspace = crate::utils::get_workspace_root();
                let datum_dir = format!("{}/_b00t_", workspace);
                let triples = sparql_query(subject.as_deref(), predicate.as_str(), &datum_dir)?;
                match format.as_str() {
                    "table" => {
                        println!("{:<30} {:<20} {}", "SUBJECT", "PREDICATE", "OBJECT");
                        for t in &triples {
                            println!("{:<30} {:<20} {}", t[0], t[1], t[2]);
                        }
                    }
                    _ => println!("{}", serde_json::to_string_pretty(&triples)?),
                }
                Ok(())
            }
            OntologyCommands::FindAgent {
                task,
                limit,
                format,
            } => {
                let workspace = crate::utils::get_workspace_root();
                let datum_dir = format!("{}/_b00t_", workspace);
                let results = find_agents_for_task(task, *limit, &datum_dir)?;
                match format.as_str() {
                    "json" => println!("{}", serde_json::to_string_pretty(&results)?),
                    _ => print_agent_results(task, &results),
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
    // Expand ~ to the actual home directory so validate commands like
    // `git -C ~/.b00t cat-file -e <hash>` work without shell expansion.
    let home = dirs::home_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let expanded = datum.validate.command.replace('~', &home);
    let parts: Vec<&str> = expanded.split_whitespace().collect();
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

/// SPARQL-like triple-pattern query: subject=datum name, predicate=field name.
/// Returns Vec<[subject, predicate, object]> triples.
pub fn sparql_query(
    subject: Option<&str>,
    predicate: &str,
    datum_dir: &str,
) -> Result<Vec<[String; 3]>> {
    let datums = scan_datums(datum_dir)?;
    let mut triples = Vec::new();
    for datum in &datums {
        let name = &datum.b00t.name;
        if let Some(subj) = subject {
            if !name.contains(subj) {
                continue;
            }
        }
        let emit = |pred: &str, obj: &str| [name.clone(), pred.to_string(), obj.to_string()];
        match predicate {
            "type" | "b00t:type" => {
                triples.push(emit("b00t:type", &datum.b00t.datum_type));
                let dt = DatumType::from_type_token(&datum.b00t.datum_type)
                    .unwrap_or(DatumType::Unknown);
                triples.push(emit("ufo:stereotype", &dt.ufo_stereotype().to_string()));
            }
            "roles" | "b00t:roles" => {
                for r in &datum.roles.required_for {
                    triples.push(emit("b00t:requiredFor", r));
                }
                for r in &datum.roles.optional_for {
                    triples.push(emit("b00t:optionalFor", r));
                }
            }
            "validate" | "b00t:validate" => {
                if !datum.validate.command.is_empty() {
                    triples.push(emit("b00t:validateCmd", &datum.validate.command));
                }
            }
            _ => {
                // "all" or unknown — emit all predicates
                triples.push(emit("b00t:type", &datum.b00t.datum_type));
                let dt = DatumType::from_type_token(&datum.b00t.datum_type)
                    .unwrap_or(DatumType::Unknown);
                triples.push(emit("ufo:stereotype", &dt.ufo_stereotype().to_string()));
                for r in &datum.roles.required_for {
                    triples.push(emit("b00t:requiredFor", r));
                }
                for r in &datum.roles.optional_for {
                    triples.push(emit("b00t:optionalFor", r));
                }
                if !datum.validate.command.is_empty() {
                    triples.push(emit("b00t:validateCmd", &datum.validate.command));
                }
            }
        }
    }
    Ok(triples)
}

#[derive(Debug, Serialize, Deserialize)]
struct AgentMatch {
    agent_name: String,
    score: f64,
    matched_keywords: Vec<String>,
    reason: String,
}

fn find_agents_for_task(task: &str, limit: usize, datum_dir: &str) -> Result<Vec<AgentMatch>> {
    let task_lower = task.to_lowercase();
    let keywords: Vec<&str> = task_lower
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .collect();

    let mut scores: Vec<(String, f64, Vec<String>)> = Vec::new();
    let agent_dir = Path::new(datum_dir);

    if agent_dir.exists() {
        for entry in fs::read_dir(agent_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("tomllmd") {
                continue;
            }
            let content = fs::read_to_string(&path)?;
            let fname = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            // Strip suffix extensions like .cli, .mcp, .agent for cleaner names
            let clean_name = fname.split('.').next().unwrap_or(&fname).to_string();
            let content_lower = content.to_lowercase();

            // Score: keyword matches in content and name
            let mut matched: Vec<String> = Vec::new();
            let mut score = 0.0;
            for kw in &keywords {
                if content_lower.contains(kw) {
                    matched.push(kw.to_string());
                    score += 10.0;
                }
                if clean_name.to_lowercase().contains(kw) {
                    if !matched.contains(&kw.to_string()) {
                        matched.push(kw.to_string());
                    }
                    score += 20.0;
                }
            }

            // Bonus: role/agent/capability mentions
            if content_lower.contains("role") || content_lower.contains("agent") {
                score += 5.0;
            }

            if score > 0.0 {
                scores.push((clean_name, score, matched));
            }
        }
    }

    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scores.truncate(limit);

    Ok(scores
        .into_iter()
        .map(|(name, score, matched)| AgentMatch {
            reason: format!(
                "matched {} keyword(s): {}",
                matched.len(),
                matched.join(", ")
            ),
            agent_name: name,
            score,
            matched_keywords: matched,
        })
        .collect())
}

fn print_agent_results(task: &str, results: &[AgentMatch]) {
    println!(
        "{}",
        crate::ansi::bold(&format!("\n🔍 Agent search for: \"{}\"", task))
    );
    println!(
        "{}",
        crate::ansi::dim(&format!("   {} result(s) found\n", results.len()))
    );
    if results.is_empty() {
        println!("   No matching agents found.");
        return;
    }
    for (i, r) in results.iter().enumerate() {
        let pct = (r.score * 100.0) as u32;
        println!(
            " {}. {} ({}%)",
            i + 1,
            crate::ansi::cyan(&r.agent_name),
            crate::ansi::green(&pct.to_string())
        );
        println!("    {}", crate::ansi::dim(&r.reason));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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

    #[test]
    fn test_sparql_query_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let triples = sparql_query(None, "all", dir.path().to_str().unwrap()).unwrap();
        assert_eq!(triples.len(), 0);
    }

    #[test]
    fn test_sparql_query_with_datum() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let toml_path = dir.path().join("rust.cli.toml");
        std::fs::write(
            &toml_path,
            r#"[b00t]
name = "rust"
type = "cli"
hint = "Rust toolchain"

[roles]
required_for = ["developer"]
optional_for = []
"#,
        )
        .unwrap();

        let triples = sparql_query(None, "type", dir.path().to_str().unwrap()).unwrap();
        assert!(!triples.is_empty(), "expected at least one triple");
        assert_eq!(triples[0][0], "rust");
        assert_eq!(triples[0][1], "b00t:type");
        assert_eq!(triples[0][2], "cli");

        // ufo:stereotype triple (#926) — Cli is a SubKind of Executable.
        // NOTE: the vendored ufo_types::UfoStereotype Display impl for SubKind
        // has a real upstream bug (missing closing '>'), so the expected
        // string is "SubKind:Cli<Executable" with no trailing '>'. This is
        // NOT fixed here — see PR description.
        assert_eq!(triples[1][0], "rust");
        assert_eq!(triples[1][1], "ufo:stereotype");
        assert_eq!(triples[1][2], "SubKind:Cli<Executable");

        let triples2 = sparql_query(Some("rust"), "roles", dir.path().to_str().unwrap()).unwrap();
        assert!(
            triples2
                .iter()
                .any(|t| t[1] == "b00t:requiredFor" && t[2] == "developer")
        );
    }

    // ── find_agents_for_task tests ──

    #[test]
    fn test_find_agents_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let results = find_agents_for_task("deploy nats", 5, dir.path().to_str().unwrap()).unwrap();
        assert!(results.is_empty(), "empty dir should return no agents");
    }

    #[test]
    fn test_find_agents_nonexistent_dir() {
        let results = find_agents_for_task("deploy nats", 5, "/nonexistent/b00t/dir").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_find_agents_keyword_match() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nats-mcp.mcp.tomllmd");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(
            f,
            "name = \"nats-mcp\"\ndescription = \"NATS messaging and pub/sub\""
        )
        .unwrap();

        let results =
            find_agents_for_task("deploy NATS pub sub", 5, dir.path().to_str().unwrap()).unwrap();
        assert_eq!(results.len(), 1, "should find nats-mcp");
        assert_eq!(results[0].agent_name, "nats-mcp");
        assert!(results[0].score > 0.0);
        assert!(!results[0].matched_keywords.is_empty());
    }

    #[test]
    fn test_find_agents_multiple_matches_ranked() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();

        // Datum A: filename + content match for "nats"
        let path_a = dir.path().join("nats-operator.cli.tomllmd");
        let mut f_a = std::fs::File::create(&path_a).unwrap();
        write!(f_a, "name = \"nats-operator\"\ntype = \"cli\"").unwrap();

        // Datum B: content-only match for "nats"
        let path_b = dir.path().join("generic-worker.cli.tomllmd");
        let mut f_b = std::fs::File::create(&path_b).unwrap();
        write!(
            f_b,
            "name = \"generic-worker\"\ndescription = \"handles NATS transport\"\ntype = \"cli\""
        )
        .unwrap();

        // Datum C: no match
        let path_c = dir.path().join("git-cli.cli.tomllmd");
        let mut f_c = std::fs::File::create(&path_c).unwrap();
        write!(f_c, "name = \"git-cli\"\ntype = \"cli\"").unwrap();

        let results =
            find_agents_for_task("deploy NATS cluster", 5, dir.path().to_str().unwrap()).unwrap();
        assert_eq!(results.len(), 2, "should find 2 matching agents");
        // nats-operator should rank first (filename match = higher score)
        assert_eq!(results[0].agent_name, "nats-operator");
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn test_find_agents_respects_limit() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            let path = dir.path().join(format!("agent-{}.cli.tomllmd", i));
            let mut f = std::fs::File::create(&path).unwrap();
            write!(f, "name = \"agent-{}\"\ndescription = \"common agent\"", i).unwrap();
        }
        let results =
            find_agents_for_task("common agent", 2, dir.path().to_str().unwrap()).unwrap();
        assert_eq!(results.len(), 2, "should be limited to 2");
    }

    #[test]
    fn test_find_agents_score_json() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("k8s-operator.cli.tomllmd");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "name = \"k8s-operator\"\ndescription = \"Kubernetes cluster management\"\ntype = \"cli\"").unwrap();

        let results =
            find_agents_for_task("kubernetes cluster", 5, dir.path().to_str().unwrap()).unwrap();
        // Should serialize to JSON without error
        let json = serde_json::to_string(&results).unwrap();
        assert!(json.contains("k8s-operator"));
        assert!(json.contains("score"));
    }

    #[test]
    fn test_find_agents_no_match_returns_empty() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("git-cli.cli.tomllmd");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "name = \"git\"\ntype = \"cli\"").unwrap();

        let results =
            find_agents_for_task("kubernetes deployment", 5, dir.path().to_str().unwrap()).unwrap();
        assert!(results.is_empty(), "no matching keywords");
    }

    #[test]
    fn test_find_agents_skips_short_keywords() {
        // "a b c d" — all ≤3 chars, should produce no matches
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anything.cli.tomllmd");
        let mut f = std::fs::File::create(&path).unwrap();
        write!(f, "name = \"anything\"\ntype = \"cli\"").unwrap();

        let results = find_agents_for_task("a b c d", 5, dir.path().to_str().unwrap()).unwrap();
        assert!(results.is_empty());
    }
}
