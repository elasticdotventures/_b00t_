use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

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
    /// Document all guards with IDs, searchable. Pipe output for agent study.
    #[clap(about = "Document all guards — structured output with unique IDs for bug/waste/gap reports")]
    Docs {
        #[clap(long, short, help = "Search filter: finds guards matching pattern/message")]
        search: Option<String>,
        #[clap(long, help = "Filter by action: warn or block")]
        action: Option<String>,
        #[clap(long, help = "Emit JSON for machine processing")]
        json: bool,
    },
    /// Flag a guard issue: bug, token waste, or control gap.
    /// Files a structured report that ledgrrr can pick up for issue tracking.
    /// Agents are STRONGLY ENCOURAGED to flag: token wastage, inadequate controls,
    /// false positives, missing redirects, or any guard that wastes agent context.
    #[clap(about = "Flag a guard issue to ledgrrr — bugs, token waste, control gaps")]
    Flag {
        /// Guard ID from b00t guard docs output (e.g. hive-guards.hive.toml:35)
        #[arg(help = "Guard ID — copy from b00t guard docs")]
        guard_id: String,
        /// This guard has a bug — fires when it shouldn't, or doesn't fire when it should
        #[arg(long, default_value_t = false)]
        bug: bool,
        /// This guard wastes tokens or agent context — message too long, unnecessary check
        #[arg(long, default_value_t = false)]
        waste: bool,
        /// There's a control gap — missing guard that should exist for a common pattern
        #[arg(long, default_value_t = false)]
        gap: bool,
        /// Description of the issue — what's wrong, what should happen instead
        #[arg(long)]
        description: String,
        /// Suggested fix or alternative pattern (optional)
        #[arg(long)]
        suggestion: Option<String>,
    },
}

/// Guard definition with line-number ID for precise bug reporting.
#[derive(Debug, Clone, Serialize)]
pub struct GuardDocEntry {
    pub id: String,
    pub action: String,
    pub pattern: String,
    pub message: String,
    pub redirect: String,
}

pub fn load_guard_docs(path: &Path) -> Result<Vec<GuardDocEntry>> {
    let content = std::fs::read_to_string(path)
        .context("Failed to read hive-guards.hive.toml")?;

    let mut entries = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();

    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim_start().starts_with("[[b00t.hive.guards]]") {
            let start_line = i + 1;
            let block_start = i;
            let mut pattern = String::new();
            let mut action = String::new();
            let mut message = String::new();
            let mut redirect = String::new();

            while i + 1 < lines.len() {
                let next = lines[i + 1].trim();
                if next.starts_with('[') { break; }
                i += 1;
                let l = lines[i].trim();
                if let Some(val) = l.strip_prefix("pattern = ") {
                    pattern = clean_toml_value(val);
                } else if let Some(val) = l.strip_prefix("action = ") {
                    action = clean_toml_value(val);
                } else if let Some(val) = l.strip_prefix("message = ") {
                    message = clean_toml_value(val);
                } else if let Some(val) = l.strip_prefix("redirect = ") {
                    redirect = clean_toml_value(val);
                }
            }

            let id = format!("{}:{}", filename, start_line);
            if !pattern.is_empty() {
                entries.push(GuardDocEntry { id, action, pattern, message, redirect });
            }
        }
        i += 1;
    }

    Ok(entries)
}

fn clean_toml_value(s: &str) -> String {
    s.trim().trim_matches('"').to_string()
}

// ── Handlers ────────────────────────────────────────────────────────────

pub fn handle_guard_command(cmd: &GuardCommands) -> Result<()> {
    match cmd {
        GuardCommands::List { json } => handle_list(*json),
        GuardCommands::Docs {
            search,
            action,
            json,
        } => handle_docs(search.as_deref(), action.as_deref(), *json),
        GuardCommands::Flag {
            guard_id,
            bug,
            waste,
            gap,
            description,
            suggestion,
        } => handle_flag(guard_id, *bug, *waste, *gap, description, suggestion.as_deref()),
    }
}

fn handle_list(json: bool) -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let path = home.join(".b00t").join("guard-violations.jsonl");

    let violations: Vec<GuardViolation> = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        content.lines()
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
        let msg = "No guard violations found. This environment is clean.";
        println!("{}", msg);
        return Ok(());
    }

    println!("{:<40} {:>5}", "Pattern", "Count");
    println!("{}", "-".repeat(50));
    for v in &violations {
        let emoji = emoji_for_count(v.count);
        println!("{:<40} {:>5}  {}", v.pattern, v.count, emoji);
    }
    Ok(())
}

fn handle_docs(search: Option<&str>, action_filter: Option<&str>, json: bool) -> Result<()> {
    // Try multiple paths for the TOML file
    let paths = [
        PathBuf::from("_b00t_/hive-guards.hive.toml"),
        PathBuf::from("../_b00t_/hive-guards.hive.toml"),
        dirs::home_dir().unwrap_or_default().join(".b00t/_b00t_/hive-guards.hive.toml"),
    ];

    let mut guards = Vec::new();
    for p in &paths {
        if p.exists() {
            guards = load_guard_docs(p)?;
            break;
        }
    }

    // Apply filters
    if let Some(s) = search {
        let s_lower = s.to_lowercase();
        guards.retain(|g|
            g.pattern.to_lowercase().contains(&s_lower) ||
            g.message.to_lowercase().contains(&s_lower) ||
            g.id.to_lowercase().contains(&s_lower)
        );
    }
    if let Some(a) = action_filter {
        guards.retain(|g| g.action == a);
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&guards)?);
        return Ok(());
    }

    if guards.is_empty() {
        println!("🔍 No guards match your search.");
        println!("   Try: b00t guard docs --search pip");
        println!("   Or:  b00t guard docs --action block");
        return Ok(());
    }

    // Summary header
    let action_counts = {
        let mut warn = 0u32; let mut block = 0u32;
        for g in &guards { match g.action.as_str() { "warn" => warn += 1, "block" => block += 1, _ => {} } }
        (warn, block)
    };
    println!("🥾 b00t Guard System — {} rules ({} warn, {} block)", guards.len(), action_counts.0, action_counts.1);
    println!("{}", "=".repeat(72));
    if search.is_some() {
        println!("  Filter: --search \"{}\"", search.unwrap());
    }
    println!();

    for g in &guards {
        let emoji = match g.action.as_str() {
            "block" => "🚫",
            "warn" => "🦨",
            _ => "⚠️",
        };
        println!("{} {} [{}] {}", emoji, g.id, g.action, g.pattern);
        if !g.message.is_empty() {
            println!("  └─ {}", g.message);
        }
        println!();
    }

    // Self-diagnosis footer — encourages agents to flag issues (verb, not noun — idiomatic: polyseme-purge)
    println!("{}", "=".repeat(72));
    println!("📋 Agents: Found an issue with a guard? Flag it:");
    println!("   b00t guard flag <guard-id> --bug --description \"...\"        # fires wrong");
    println!("   b00t guard flag <guard-id> --waste --description \"...\"      # token waste");
    println!("   b00t guard flag <guard-id> --gap --description \"...\"        # missing guard");
    println!("   b00t guard flag <guard-id> --bug --waste --description \"...\" # bitwise combine");
    println!();
    println!("💡 Strongly encouraged flags: token wastage, inadequate controls,");
    println!("   false positives, missing redirects, guards that waste agent context.");
    println!("   Flags auto-file to ledgrrr for issue tracking.");
    println!("{}", "=".repeat(72));

    Ok(())
}

fn handle_flag(guard_id: &str, bug: bool, waste: bool, gap: bool, description: &str, suggestion: Option<&str>) -> Result<()> {
    let reports_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?
        .join(".b00t")
        .join("guard-reports");

    std::fs::create_dir_all(&reports_dir)?;

    // Build flags bitwise — all three can be true simultaneously
    let mut flags: Vec<&str> = Vec::new();
    if bug { flags.push("bug"); }
    if waste { flags.push("waste"); }
    if gap { flags.push("gap"); }

    let timestamp = chrono::Utc::now().to_rfc3339();
    let report = serde_json::json!({
        "type": "guard-flag",
        "guard_id": guard_id,
        "flags": flags,
        "description": description,
        "suggestion": suggestion,
        "reported_at": timestamp,
        "reported_by": "b00t agent",
    });

    let report_file = reports_dir.join(format!(
        "{}-{}.json",
        guard_id.replace(':', "-"),
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    ));
    std::fs::write(&report_file, serde_json::to_string_pretty(&report)?)?;

    println!("✅ Guard flagged for review");
    println!("   ID:    {}", guard_id);
    println!("   Flags: {}", flags.join(", "));
    println!("   File:  {}", report_file.display());
    println!();
    println!("📋 What happens next:");
    println!("   - ledgrrr picks up the flag on next audit cycle");
    if bug { println!("   - 🐛 Bug: guard will be patched in the next release"); }
    if waste { println!("   - 🦨 Waste: token optimization will be prioritized"); }
    if gap { println!("   - 🚩 Gap: new guard will be drafted for review"); }
    println!();
    println!("💡 Thank you for improving the guard system. Every flag");
    println!("   makes b00t more efficient for every agent that follows.");
    println!("   🍰 Cake earned.");

    Ok(())
}

fn emoji_for_count(count: u32) -> &'static str {
    match count {
        0 => "\u{2705}",       // ✅
        1 => "\u{1F9A8}",      // 🦨
        _ => "\u{1F4A9}",      // 💩
    }
}
