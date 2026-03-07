// b00t-cli/src/commands/tutorial.rs
use anyhow::Result;
use clap::Parser;
use crate::session_memory::SessionMemory;

#[derive(Parser, Debug)]
pub enum TutorialCommands {
    #[clap(about = "Show tutorial progression for current role")]
    Status,
    #[clap(about = "Show next recommended datum to install/validate")]
    Next,
    #[clap(about = "Mark a datum as skipped")]
    Skip {
        #[clap(help = "Datum name to skip")]
        datum: String,
        #[clap(long, default_value = "manually skipped")]
        reason: String,
    },
    #[clap(about = "Run datum validate command and record result in session")]
    Validate {
        #[clap(help = "Datum name to validate")]
        datum: String,
    },
}

impl TutorialCommands {
    pub fn execute(&self) -> Result<()> {
        let mut session = SessionMemory::load()?;
        match self {
            TutorialCommands::Status => show_status(&session),
            TutorialCommands::Next => show_next(&session),
            TutorialCommands::Skip { datum, reason } => skip_datum(&mut session, datum, reason),
            TutorialCommands::Validate { datum } => validate_datum(&mut session, datum),
        }
    }
}

pub fn default_role_path(role: &str) -> Vec<String> {
    let path: &[&str] = match role {
        "orchestrator" => &["gh", "just", "context7", "taskmaster-ai", "argo-cli"],
        "analyst"      => &["gh", "uv", "context7"],
        _              => &["gh", "just", "uv", "rustc", "context7"], // developer
    };
    path.iter().map(|s| s.to_string()).collect()
}

pub fn next_uncompleted(path: &[String], completed: &[String], skipped: &[String]) -> Option<String> {
    path.iter()
        .find(|d| !completed.contains(d) && !skipped.contains(d))
        .cloned()
}

pub fn progress_percent(total: usize, completed: usize) -> u32 {
    if total == 0 { return 0; }
    ((completed as f64 / total as f64) * 100.0) as u32
}

fn get_role(session: &SessionMemory) -> String {
    session.get("tutorial.role")
        .cloned()
        .or_else(|| std::env::var("B00T_ROLE").ok())
        .unwrap_or_else(|| "developer".to_string())
}

fn parse_csv(session: &SessionMemory, key: &str) -> Vec<String> {
    session.get(key)
        .map(|s| s.split(',').filter(|x| !x.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

fn show_status(session: &SessionMemory) -> Result<()> {
    let role = get_role(session);
    let path = default_role_path(&role);
    let completed = parse_csv(session, "tutorial.completed");
    let skipped = parse_csv(session, "tutorial.skipped");
    let pct = progress_percent(path.len(), completed.len());

    println!("Tutorial -- role: {} ({}%)", role, pct);
    println!("{}", "-".repeat(40));
    for datum in &path {
        let icon = if completed.contains(datum) { "[x]" }
                   else if skipped.contains(datum) { "[s]" }
                   else { "[ ]" };
        println!(" {} {}", icon, datum);
    }
    println!("\n{}/{} validated.", completed.len(), path.len());
    match next_uncompleted(&path, &completed, &skipped) {
        Some(next) => println!("Next: b00t tutorial validate {}", next),
        None => println!("Role path complete!"),
    }
    Ok(())
}

fn show_next(session: &SessionMemory) -> Result<()> {
    let role = get_role(session);
    let path = default_role_path(&role);
    let completed = parse_csv(session, "tutorial.completed");
    let skipped = parse_csv(session, "tutorial.skipped");
    match next_uncompleted(&path, &completed, &skipped) {
        Some(next) => println!("{}", next),
        None => println!("all done"),
    }
    Ok(())
}

fn skip_datum(session: &mut SessionMemory, datum: &str, reason: &str) -> Result<()> {
    let mut skipped = parse_csv(session, "tutorial.skipped");
    if !skipped.contains(&datum.to_string()) {
        skipped.push(datum.to_string());
        session.set("tutorial.skipped", &skipped.join(","))?;
    }
    println!("Skipped {} ({})", datum, reason);
    Ok(())
}

fn validate_datum(session: &mut SessionMemory, datum: &str) -> Result<()> {
    use std::process::Command;
    let workspace = crate::utils::get_workspace_root();
    let datum_dir = format!("{}/_b00t_", workspace);
    let datums = crate::commands::ontology::scan_datums(&datum_dir)?;

    match datums.iter().find(|d| d.b00t.name == datum) {
        None => {
            println!("Datum '{}' not found in {}", datum, datum_dir);
        }
        Some(d) if d.validate.command.is_empty() => {
            println!("Datum '{}' has no [validate] command -- marking as validated", datum);
            mark_completed(session, datum)?;
        }
        Some(d) => {
            let cmd = d.validate.command.clone();
            print!("Validating {} ({})... ", datum, cmd);
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            match Command::new(parts[0]).args(&parts[1..]).output() {
                Ok(out) if out.status.success() => {
                    println!("OK");
                    mark_completed(session, datum)?;
                }
                Ok(out) => {
                    println!("FAIL (exit {})", out.status.code().unwrap_or(-1));
                    eprintln!("{}", String::from_utf8_lossy(&out.stderr).trim());
                }
                Err(e) => println!("ERROR: {}", e),
            }
        }
    }
    Ok(())
}

fn mark_completed(session: &mut SessionMemory, datum: &str) -> Result<()> {
    let mut completed = parse_csv(session, "tutorial.completed");
    if !completed.contains(&datum.to_string()) {
        completed.push(datum.to_string());
        session.set("tutorial.completed", &completed.join(","))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_path_developer_has_core_datums() {
        let path = default_role_path("developer");
        assert!(path.contains(&"gh".to_string()));
        assert!(path.contains(&"just".to_string()));
        assert!(path.contains(&"uv".to_string()));
    }

    #[test]
    fn test_role_path_orchestrator() {
        let path = default_role_path("orchestrator");
        assert!(path.contains(&"taskmaster-ai".to_string()));
    }

    #[test]
    fn test_next_uncompleted_skips_done() {
        let path = vec!["gh".to_string(), "just".to_string(), "uv".to_string()];
        let completed = vec!["gh".to_string()];
        let next = next_uncompleted(&path, &completed, &[]);
        assert_eq!(next, Some("just".to_string()));
    }

    #[test]
    fn test_next_uncompleted_skips_skipped() {
        let path = vec!["gh".to_string(), "just".to_string(), "uv".to_string()];
        let completed = vec!["gh".to_string()];
        let skipped = vec!["just".to_string()];
        let next = next_uncompleted(&path, &completed, &skipped);
        assert_eq!(next, Some("uv".to_string()));
    }

    #[test]
    fn test_next_uncompleted_all_done() {
        let path = vec!["gh".to_string(), "just".to_string()];
        let completed = vec!["gh".to_string(), "just".to_string()];
        let next = next_uncompleted(&path, &completed, &[]);
        assert_eq!(next, None);
    }

    #[test]
    fn test_progress_percent_partial() {
        assert_eq!(progress_percent(5, 2), 40);
    }

    #[test]
    fn test_progress_percent_zero_total() {
        assert_eq!(progress_percent(0, 0), 0);
    }

    #[test]
    fn test_progress_percent_complete() {
        assert_eq!(progress_percent(4, 4), 100);
    }

    #[test]
    fn test_csv_roundtrip() {
        let list = vec!["gh".to_string(), "just".to_string(), "uv".to_string()];
        let csv = list.join(",");
        let parsed: Vec<String> = csv.split(',').filter(|x| !x.is_empty()).map(String::from).collect();
        assert_eq!(list, parsed);
    }
}
