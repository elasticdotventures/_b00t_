use anyhow::{Context, Result, bail};
use b00t_c0re_lib::soul_dataframerr::{SoulDataFramerr, SoulRow, SoulValue};
use clap::Subcommand;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const LIFECYCLE_TABLE: &str = "lifecycle";
const VALID_LABELS: &[&str] = &[
    "active",
    "partial",
    "aspirational",
    "deprecated",
    "archive",
    "unresolved",
];

#[derive(Debug, Subcommand, Clone)]
pub enum LifecycleCommands {
    #[clap(about = "Audit lifecycle flashtable coverage against current _b00t_ inventory")]
    Gate {
        #[arg(long, default_value = "_b00t_", help = "Datum directory to audit")]
        datums: PathBuf,
        #[arg(long, help = "Fail when any current file's latest row is unresolved")]
        require_reviewed: bool,
        #[arg(long, help = "Fail on stale or superseded history rows")]
        strict_history: bool,
        #[arg(long, help = "Emit JSON report")]
        json: bool,
    },
    #[clap(about = "Show lifecycle label distribution for current _b00t_ inventory")]
    Status {
        #[arg(long, default_value = "_b00t_", help = "Datum directory to audit")]
        datums: PathBuf,
        #[arg(long, help = "Emit JSON report")]
        json: bool,
    },
    #[clap(about = "List current inventory files with no lifecycle row")]
    Missing {
        #[arg(long, default_value = "_b00t_", help = "Datum directory to audit")]
        datums: PathBuf,
    },
    #[clap(about = "List current inventory files whose latest lifecycle row is unresolved")]
    Unresolved {
        #[arg(long, default_value = "_b00t_", help = "Datum directory to audit")]
        datums: PathBuf,
    },
}

pub fn handle_lifecycle_command(cmd: &LifecycleCommands) -> Result<()> {
    match cmd {
        LifecycleCommands::Gate {
            datums,
            require_reviewed,
            strict_history,
            json,
        } => {
            let audit = audit_active_lifecycle(datums)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&audit.to_json())?);
            }
            if audit.passes(*require_reviewed, *strict_history) {
                if !json {
                    println!(
                        "PASS: {}/{} labeled, {} unresolved, {} superseded, {} stale",
                        audit.labeled_count(),
                        audit.inventory_count,
                        audit.unresolved_count,
                        audit.superseded_count,
                        audit.stale_files.len()
                    );
                }
                Ok(())
            } else {
                if !json {
                    audit.print_failure(*require_reviewed, *strict_history);
                }
                bail!("lifecycle gate failed")
            }
        }
        LifecycleCommands::Status { datums, json } => {
            let audit = audit_active_lifecycle(datums)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&audit.to_json())?);
            } else if audit.distribution.is_empty() {
                println!("(no lifecycle rows for current inventory)");
            } else {
                for (label, count) in &audit.distribution {
                    println!("{:>7} {}", count, label);
                }
            }
            Ok(())
        }
        LifecycleCommands::Missing { datums } => {
            let audit = audit_active_lifecycle(datums)?;
            for file in &audit.missing_files {
                println!("{file}");
            }
            Ok(())
        }
        LifecycleCommands::Unresolved { datums } => {
            let audit = audit_active_lifecycle(datums)?;
            for file in &audit.unresolved_files {
                println!("{file}");
            }
            Ok(())
        }
    }
}

fn audit_active_lifecycle(datums: &Path) -> Result<LifecycleAudit> {
    let inventory = inventory_files(datums)?;
    let doc = crate::commands::soul::load_soul_doc()?;
    let registry = crate::commands::soul::load_registry(&doc)?;
    let table = registry
        .tables
        .get(LIFECYCLE_TABLE)
        .ok_or_else(|| anyhow::anyhow!("no table '{LIFECYCLE_TABLE}'"))?;
    Ok(audit_lifecycle(&inventory, table))
}

fn inventory_files(datums: &Path) -> Result<BTreeSet<String>> {
    if !datums.is_dir() {
        bail!("datum directory does not exist: {}", datums.display());
    }
    let mut files = BTreeSet::new();
    for entry in std::fs::read_dir(datums).with_context(|| format!("read {}", datums.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let name = entry.file_name().to_string_lossy().to_string();
            files.insert(name);
        }
    }
    Ok(files)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LatestLifecycleRow {
    row_id: u64,
    label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LifecycleAudit {
    inventory_count: usize,
    row_count: usize,
    latest_row_count: usize,
    superseded_count: usize,
    unresolved_count: usize,
    malformed_row_count: usize,
    unresolved_files: Vec<String>,
    missing_files: Vec<String>,
    stale_files: Vec<String>,
    invalid_labels: Vec<String>,
    distribution: BTreeMap<String, usize>,
}

impl LifecycleAudit {
    fn labeled_count(&self) -> usize {
        self.inventory_count
            .saturating_sub(self.missing_files.len())
            .saturating_sub(self.unresolved_count)
            .saturating_sub(self.invalid_labels.len())
    }

    fn passes(&self, require_reviewed: bool, strict_history: bool) -> bool {
        self.missing_files.is_empty()
            && self.invalid_labels.is_empty()
            && self.malformed_row_count == 0
            && (!require_reviewed || self.unresolved_count == 0)
            && (!strict_history || (self.stale_files.is_empty() && self.superseded_count == 0))
    }

    fn print_failure(&self, require_reviewed: bool, strict_history: bool) {
        println!("FAIL: lifecycle inventory mismatch");
        if !self.missing_files.is_empty() {
            println!("missing: {}", self.missing_files.join(" "));
        }
        if !self.invalid_labels.is_empty() {
            println!("invalid_labels: {}", self.invalid_labels.join(" "));
        }
        if self.malformed_row_count > 0 {
            println!("malformed_rows: {}", self.malformed_row_count);
        }
        if require_reviewed && self.unresolved_count > 0 {
            println!("unresolved: {}", self.unresolved_count);
        }
        if strict_history {
            if !self.stale_files.is_empty() {
                println!("stale: {}", self.stale_files.join(" "));
            }
            if self.superseded_count > 0 {
                println!("superseded: {}", self.superseded_count);
            }
        } else {
            println!(
                "history: {} superseded, {} stale ignored",
                self.superseded_count,
                self.stale_files.len()
            );
        }
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "inventory_count": self.inventory_count,
            "row_count": self.row_count,
            "latest_row_count": self.latest_row_count,
            "superseded_count": self.superseded_count,
            "unresolved_count": self.unresolved_count,
            "malformed_row_count": self.malformed_row_count,
            "unresolved_files": self.unresolved_files,
            "missing_files": self.missing_files,
            "stale_files": self.stale_files,
            "invalid_labels": self.invalid_labels,
            "distribution": self.distribution,
        })
    }
}

fn audit_lifecycle(inventory: &BTreeSet<String>, table: &SoulDataFramerr) -> LifecycleAudit {
    let mut latest: BTreeMap<String, LatestLifecycleRow> = BTreeMap::new();
    let mut malformed_row_count = 0usize;

    for row in &table.rows {
        let Some(file) = text_field(row, "file") else {
            malformed_row_count += 1;
            continue;
        };
        latest.insert(
            file.to_string(),
            LatestLifecycleRow {
                row_id: row.id,
                label: text_field(row, "label").map(str::to_string),
            },
        );
    }

    let mut missing_files = Vec::new();
    let mut stale_files = Vec::new();
    let mut invalid_labels = Vec::new();
    let mut distribution = BTreeMap::new();
    let mut unresolved_count = 0usize;
    let mut unresolved_files = Vec::new();

    for file in inventory {
        match latest.get(file) {
            Some(row) => match row.label.as_deref() {
                Some(label) if VALID_LABELS.contains(&label) => {
                    *distribution.entry(label.to_string()).or_insert(0) += 1;
                    if label == "unresolved" {
                        unresolved_count += 1;
                        unresolved_files.push(file.clone());
                    }
                }
                Some(label) => invalid_labels.push(format!("{file}={label}")),
                None => invalid_labels.push(format!("{file}=<missing>")),
            },
            None => missing_files.push(file.clone()),
        }
    }

    for file in latest.keys() {
        if !inventory.contains(file) {
            stale_files.push(file.clone());
        }
    }

    LifecycleAudit {
        inventory_count: inventory.len(),
        row_count: table.rows.len(),
        latest_row_count: latest.len(),
        superseded_count: table.rows.len().saturating_sub(latest.len()),
        unresolved_count,
        malformed_row_count,
        unresolved_files,
        missing_files,
        stale_files,
        invalid_labels,
        distribution,
    }
}

fn text_field<'a>(row: &'a SoulRow, field: &str) -> Option<&'a str> {
    row.fields.get(field).and_then(SoulValue::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use b00t_c0re_lib::soul_dataframerr::SoulColumn;
    use std::collections::BTreeMap;

    fn table() -> SoulDataFramerr {
        SoulDataFramerr::new(
            "lifecycle",
            vec![
                SoulColumn::parse("file:text").unwrap(),
                SoulColumn::parse("label:text?").unwrap(),
            ],
        )
    }

    fn fields(file: &str, label: Option<&str>) -> BTreeMap<String, SoulValue> {
        let mut fields = BTreeMap::new();
        fields.insert("file".to_string(), SoulValue::Text(file.to_string()));
        if let Some(label) = label {
            fields.insert("label".to_string(), SoulValue::Text(label.to_string()));
        }
        fields
    }

    fn inventory(files: &[&str]) -> BTreeSet<String> {
        files.iter().map(|f| f.to_string()).collect()
    }

    #[test]
    fn duplicate_rows_are_history_latest_wins() {
        let mut table = table();
        table.insert(fields("a.toml", Some("active"))).unwrap();
        table.insert(fields("a.toml", Some("deprecated"))).unwrap();

        let audit = audit_lifecycle(&inventory(&["a.toml"]), &table);

        assert!(audit.passes(false, false));
        assert_eq!(audit.superseded_count, 1);
        assert_eq!(audit.distribution.get("deprecated"), Some(&1));
    }

    #[test]
    fn stale_rows_are_ignored_by_default() {
        let mut table = table();
        table.insert(fields("gone.toml", Some("active"))).unwrap();
        table.insert(fields("keep.toml", Some("partial"))).unwrap();

        let audit = audit_lifecycle(&inventory(&["keep.toml"]), &table);

        assert!(audit.passes(false, false));
        assert!(!audit.passes(false, true));
        assert_eq!(audit.stale_files, vec!["gone.toml"]);
    }

    #[test]
    fn missing_current_file_fails() {
        let table = table();

        let audit = audit_lifecycle(&inventory(&["missing.toml"]), &table);

        assert!(!audit.passes(false, false));
        assert_eq!(audit.missing_files, vec!["missing.toml"]);
    }

    #[test]
    fn invalid_latest_label_fails() {
        let mut table = table();
        table.insert(fields("a.toml", Some("wat"))).unwrap();

        let audit = audit_lifecycle(&inventory(&["a.toml"]), &table);

        assert!(!audit.passes(false, false));
        assert_eq!(audit.invalid_labels, vec!["a.toml=wat"]);
    }

    #[test]
    fn missing_label_fails_as_invalid() {
        let mut table = table();
        table.insert(fields("a.toml", None)).unwrap();

        let audit = audit_lifecycle(&inventory(&["a.toml"]), &table);

        assert!(!audit.passes(false, false));
        assert_eq!(audit.invalid_labels, vec!["a.toml=<missing>"]);
    }

    #[test]
    fn unresolved_passes_by_default_but_fails_when_review_required() {
        let mut table = table();
        table.insert(fields("a.toml", Some("unresolved"))).unwrap();

        let audit = audit_lifecycle(&inventory(&["a.toml"]), &table);

        assert!(audit.passes(false, false));
        assert!(!audit.passes(true, false));
        assert_eq!(audit.unresolved_count, 1);
        assert_eq!(audit.unresolved_files, vec!["a.toml"]);
    }
}
