//! Toon format report generator for bootstrap results
//!
//! Toon (Token-Oriented Object Notation) achieves 30-60% token reduction
//! vs JSON for LLM context by using TOML-based tabular arrays.
//!
//! See: https://github.com/toon-format/toon

use crate::bootstrap::prereq::PrereqResult;
use crate::bootstrap::skeleton::SkeletonResult;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Bootstrap report encompassing all checks
#[derive(Debug)]
pub struct BootstrapReport {
    pub timestamp: String,
    pub prereq_result: PrereqResult,
    pub skeleton_result: Option<SkeletonResult>,
}

/// Generate Toon format report from bootstrap results
pub fn generate_toon_report(report: &BootstrapReport, output_path: &Path) -> Result<()> {
    let toon_content = serialize_to_toon(report)?;

    let expanded_path = shellexpand::tilde(output_path.to_str().unwrap());
    let final_path = Path::new(expanded_path.as_ref());

    // Create parent directory if needed
    if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create parent directory: {}", parent.display()))?;
    }

    fs::write(final_path, toon_content)
        .with_context(|| format!("Failed to write Toon report to {}", final_path.display()))?;

    Ok(())
}

/// Serialize bootstrap report to Toon format
fn serialize_to_toon(report: &BootstrapReport) -> Result<String> {
    let mut toon = String::new();

    // Meta section
    toon.push_str("# b00t Bootstrap Report\n");
    toon.push_str("# Generated in Toon format (30-60% more token-efficient than JSON)\n\n");
    toon.push_str("[meta]\n");
    toon.push_str(&format!("timestamp = \"{}\"\n", report.timestamp));
    toon.push_str("format = \"toon\"\n");
    toon.push_str(
        "schema = \"https://b00t.promptexecution.com/schemas/bootstrap-report/v0.1.0\"\n\n",
    );

    // Summary section
    toon.push_str("[summary]\n");
    toon.push_str(&format!(
        "all_required_met = {}\n",
        report.prereq_result.all_required_met
    ));
    toon.push_str(&format!(
        "required_binaries_count = {}\n",
        report.prereq_result.required.len()
    ));
    toon.push_str(&format!(
        "optional_binaries_count = {}\n",
        report.prereq_result.optional.len()
    ));

    let missing_required = report.prereq_result.missing_required().len();
    let missing_optional = report.prereq_result.missing_optional().len();

    toon.push_str(&format!("missing_required = {}\n", missing_required));
    toon.push_str(&format!("missing_optional = {}\n", missing_optional));

    if let Some(ref skeleton) = report.skeleton_result {
        toon.push_str(&format!(
            "directories_created = {}\n",
            skeleton.created.len()
        ));
        toon.push_str(&format!(
            "directories_existed = {}\n",
            skeleton.already_existed.len()
        ));
        toon.push_str(&format!(
            "directories_processed = {}\n",
            skeleton.total_count()
        ));
        toon.push_str(&format!("directory_errors = {}\n\n", skeleton.errors.len()));
    } else {
        toon.push_str("directories_created = 0\n");
        toon.push_str("directories_existed = 0\n");
        toon.push_str("directories_processed = 0\n");
        toon.push_str("directory_errors = 0\n\n");
    }

    // Required binaries table (Toon's tabular format)
    if !report.prereq_result.required.is_empty() {
        toon.push_str("# Required binaries (tabular format - very token-efficient)\n");
        toon.push_str("[[required_bins]]\n");

        for bin in &report.prereq_result.required {
            toon.push_str(&format!("name = \"{}\"\n", bin.name));
            toon.push_str(&format!("found = {}\n", bin.found));
            toon.push_str(&format!(
                "required_version = \"{}\"\n",
                bin.required_version
            ));

            if let Some(ref installed) = bin.installed_version {
                toon.push_str(&format!("installed_version = \"{}\"\n", installed));
            } else {
                toon.push_str("installed_version = \"not_installed\"\n");
            }

            toon.push_str(&format!("meets_requirement = {}\n", bin.meets_requirement));

            if let Some(ref path) = bin.path {
                toon.push_str(&format!("path = \"{}\"\n", path.display()));
            }

            toon.push_str("\n"); // Separator between table entries
        }
    }

    // Optional binaries table
    if !report.prereq_result.optional.is_empty() {
        toon.push_str("# Optional binaries\n");
        toon.push_str("[[optional_bins]]\n");

        for bin in &report.prereq_result.optional {
            toon.push_str(&format!("name = \"{}\"\n", bin.name));
            toon.push_str(&format!("found = {}\n", bin.found));

            if bin.found {
                if let Some(ref installed) = bin.installed_version {
                    toon.push_str(&format!("installed_version = \"{}\"\n", installed));
                }
                toon.push_str(&format!("meets_requirement = {}\n", bin.meets_requirement));
            }

            if let Some(ref hint) = bin.install_hint {
                toon.push_str(&format!("install_hint = \"{}\"\n", hint));
            }

            toon.push_str("\n");
        }
    }

    // Directory creation results
    if let Some(ref skeleton) = report.skeleton_result {
        if !skeleton.created.is_empty() {
            toon.push_str("# Directories created during bootstrap\n");
            toon.push_str("[[directories_created]]\n");
            for dir in &skeleton.created {
                toon.push_str(&format!("path = \"{}\"\n\n", dir.display()));
            }
        }

        if !skeleton.errors.is_empty() {
            toon.push_str("# Directory creation errors\n");
            toon.push_str("[[directory_errors]]\n");
            for (path, error) in &skeleton.errors {
                toon.push_str(&format!("path = \"{}\"\n", path.display()));
                toon.push_str(&format!("error = \"{}\"\n\n", error));
            }
        }
    }

    Ok(toon)
}

/// Print Toon report to stdout in human-readable format
pub fn print_toon_report(report: &BootstrapReport) {
    println!("🥾 b00t Bootstrap Report");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Generated: {}", report.timestamp);
    println!();

    // Required binaries
    println!("📦 Required Binaries:");
    for bin in &report.prereq_result.required {
        let status = if bin.found && bin.meets_requirement {
            "✅"
        } else if bin.found {
            "⚠️"
        } else {
            "❌"
        };

        print!("  {} {} ", status, bin.name);

        if let Some(ref installed) = bin.installed_version {
            print!("(installed: {}", installed);
            if bin.meets_requirement {
                println!(", OK)");
            } else {
                println!(", requires: {}) ⚠️", bin.required_version);
            }
        } else {
            println!("(not installed) - requires: {}", bin.required_version);
        }
    }

    // Optional binaries
    if !report.prereq_result.optional.is_empty() {
        println!();
        println!("🔧 Optional Binaries:");
        for bin in &report.prereq_result.optional {
            let status = if bin.found && bin.meets_requirement {
                "✅"
            } else {
                "  "
            };

            print!("  {} {} ", status, bin.name);

            if let Some(ref installed) = bin.installed_version {
                println!("({})", installed);
            } else {
                if let Some(ref hint) = bin.install_hint {
                    println!("- {}", hint);
                } else {
                    println!("(not installed)");
                }
            }
        }
    }

    // Directories
    if let Some(ref skeleton) = report.skeleton_result {
        if !skeleton.created.is_empty() || !skeleton.already_existed.is_empty() {
            println!();
            println!("📁 Directories:");
            for dir in &skeleton.created {
                println!("  ✨ Created: {}", dir.display());
            }
            for dir in &skeleton.already_existed {
                println!("  ✅ Exists:  {}", dir.display());
            }
        }

        if !skeleton.errors.is_empty() {
            println!();
            println!("❌ Errors:");
            for (path, error) in &skeleton.errors {
                println!("  {}: {}", path.display(), error);
            }
        }
    }

    println!();
    if report.prereq_result.all_required_met {
        println!("✅ All required prerequisites met!");
    } else {
        println!("⚠️  Some required prerequisites are missing");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toon_serialization() {
        // 🤓 Toon format should be more compact than equivalent JSON
        let report = BootstrapReport {
            timestamp: "2025-11-09T12:00:00Z".to_string(),
            prereq_result: PrereqResult {
                required: vec![],
                optional: vec![],
                all_required_met: true,
            },
            skeleton_result: None,
        };

        let toon = serialize_to_toon(&report).unwrap();
        assert!(toon.contains("[meta]"));
        assert!(toon.contains("format = \"toon\""));
    }
}
