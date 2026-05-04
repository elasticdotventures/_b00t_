use clap::Parser;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GuardViolation {
    pub pattern: String,
    pub count: u32,
    pub last_violation: String,
}

#[derive(Parser, Debug, Clone)]
pub enum GuardCommands {
    #[clap(about = "List guard violations from guard-violations.jsonl")]
    List {
        #[clap(long, help = "Emit JSON output")]
        json: bool,
    },
}

fn emoji_for_count(count: u32) -> &'static str {
    match count {
        0 => "\u{2705}",       // ✅
        1 => "\u{1F9A8}",      // 🦨
        _ => "\u{1F4A9}",      // 💩
    }
}

pub fn handle_guard_command(cmd: &GuardCommands) -> Result<()> {
    match cmd {
        GuardCommands::List { json } => handle_list(*json),
    }
}

fn handle_list(json: bool) -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let path = home.join(".b00t").join("guard-violations.jsonl");

    let violations: Vec<GuardViolation> = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        content
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|line| serde_json::from_str::<GuardViolation>(line))
            .collect::<std::result::Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&violations)?);
        return Ok(());
    }

    if violations.is_empty() {
        println!("No guard violations found.");
        return Ok(());
    }

    // Table header
    println!("{:<40} {:>5}  {}", "Pattern", "Count", "");
    println!("{}", "-".repeat(60));
    for v in &violations {
        let emoji = emoji_for_count(v.count);
        println!("{:<40} {:>5}  {}", v.pattern, v.count, emoji);
    }

    Ok(())
}
