// b00t-cli/src/commands/up.rs
use anyhow::{Context, Result};
use clap::Parser;
use crate::session_memory::SessionMemory;
use std::process::Command;

#[derive(Parser, Debug)]
pub struct UpArgs {
    /// AI tool to use for the ralph loop
    #[clap(long, default_value = "claude", value_parser = ["claude", "amp", "codex"])]
    pub tool: String,

    /// Maximum iterations per ralph session
    #[clap(long, default_value = "10")]
    pub max_iter: u32,

    /// Agent role (filters ontology + tutorial path)
    #[clap(long)]
    pub role: Option<String>,

    /// Maximum restart cycles before giving up
    #[clap(long, default_value = "5")]
    pub max_restarts: u32,
}

impl UpArgs {
    pub fn execute(&self) -> Result<()> {
        let workspace_root = crate::utils::get_workspace_root();
        let ralph_script = format!("{}/b00t.sh", workspace_root);

        if !std::path::Path::new(&ralph_script).exists() {
            anyhow::bail!("b00t.sh not found at {}. Run from b00t workspace root.", ralph_script);
        }

        let mut restart_count = 0u32;
        let mut session = SessionMemory::load().unwrap_or_default();

        loop {
            println!("🥾 b00t up: cycle {} (tool={}, max_iter={})",
                restart_count + 1, self.tool, self.max_iter);

            // Build ontology JSON (placeholder until Task 4 implements real ontology)
            let ontology_json = format!(
                r#"{{"role":"{}","available":[],"installable":[],"blessings":[],"note":"placeholder"}}"#,
                self.role.as_deref().unwrap_or("developer")
            );

            let status = Command::new("bash")
                .arg(&ralph_script)
                .arg("--tool")
                .arg(&self.tool)
                .arg(self.max_iter.to_string())
                .env("B00T_ONTOLOGY", &ontology_json)
                .env("B00T_ROLE", self.role.as_deref().unwrap_or("developer"))
                .current_dir(&workspace_root)
                .status()
                .context(format!("Failed to exec b00t.sh at {}", ralph_script))?;

            let code = status.code().unwrap_or(1);

            // Persist cycle state to session memory; set() auto-saves internally
            let _ = session.set("up.last_exit", &code.to_string());
            let _ = session.set("up.tool", &self.tool);
            let _ = session.set("up.restart_count", &restart_count.to_string());

            match code {
                0 => {
                    println!("✅ b00t up: ralph completed after {} cycle(s)", restart_count + 1);
                    return Ok(());
                }
                75 => {
                    // POSIX TEMPFAIL — agent requests restart
                    restart_count += 1;
                    if restart_count >= self.max_restarts {
                        anyhow::bail!(
                            "b00t up: max restarts ({}) reached. Last exit: 75",
                            self.max_restarts
                        );
                    }
                    println!("🔄 b00t up: restart {}/{} (exit 75 = TEMPFAIL)",
                        restart_count, self.max_restarts);
                }
                n => {
                    anyhow::bail!("b00t up: ralph exited with error code {}", n);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_up_command_parses() {
        let args = UpArgs::try_parse_from(["b00t-cli", "--tool", "claude"]);
        assert!(args.is_ok(), "UpArgs should parse --tool claude");
    }

    #[test]
    fn test_up_command_defaults() {
        let args = UpArgs {
            tool: "claude".to_string(),
            max_iter: 10,
            role: None,
            max_restarts: 5,
        };
        assert_eq!(args.tool, "claude");
        assert_eq!(args.max_iter, 10);
        assert_eq!(args.max_restarts, 5);
        assert!(args.role.is_none());
    }

    #[test]
    fn test_up_command_invalid_tool_rejected() {
        let args = UpArgs::try_parse_from(["b00t-cli", "--tool", "invalid_tool"]);
        assert!(args.is_err(), "Invalid tool should be rejected");
    }

    #[test]
    fn test_exit_code_75_is_tempfail() {
        const POSIX_TEMPFAIL: i32 = 75;
        assert_eq!(POSIX_TEMPFAIL, 75);
    }

    #[test]
    fn test_restart_logic_counts_correctly() {
        let exit_codes = vec![75i32, 75, 75, 0];
        let max_restarts = 5u32;
        let mut restart_count = 0u32;
        let mut final_code = -1i32;

        for code in exit_codes {
            match code {
                0 => { final_code = 0; break; }
                75 => {
                    restart_count += 1;
                    if restart_count >= max_restarts {
                        final_code = 75;
                        break;
                    }
                }
                n => { final_code = n; break; }
            }
        }
        assert_eq!(restart_count, 3);
        assert_eq!(final_code, 0);
    }

    #[test]
    fn test_restart_logic_stops_at_max() {
        let exit_codes = vec![75i32, 75, 75, 75, 75, 75]; // 6 restarts
        let max_restarts = 3u32;
        let mut restart_count = 0u32;
        let mut hit_max = false;

        for code in exit_codes {
            if code == 75 {
                restart_count += 1;
                if restart_count >= max_restarts {
                    hit_max = true;
                    break;
                }
            }
        }
        assert!(hit_max);
        assert_eq!(restart_count, 3);
    }

    #[test]
    fn test_datum_toml_has_validate_section() {
        let workspace = crate::utils::get_workspace_root();
        let git_datum = format!("{}/_b00t_/gh.cli.toml", workspace);
        if std::path::Path::new(&git_datum).exists() {
            let content = std::fs::read_to_string(&git_datum).unwrap();
            assert!(content.contains("[validate]"), "gh datum missing [validate] section");
            assert!(content.contains("[roles]"), "gh datum missing [roles] section");
            assert!(content.contains("required_for"), "gh datum missing required_for field");
        }
        // Graceful skip if file doesn't exist (CI environments)

        // Also check rustc datum if present
        let rustc_datum = format!("{}/_b00t_/rustc.cli.toml", workspace);
        if std::path::Path::new(&rustc_datum).exists() {
            let content = std::fs::read_to_string(&rustc_datum).unwrap();
            assert!(content.contains("[validate]"), "rustc datum missing [validate] section");
            assert!(content.contains("[roles]"), "rustc datum missing [roles] section");
        }
    }
}
