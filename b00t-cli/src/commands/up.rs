// b00t-cli/src/commands/up.rs
use anyhow::Result;
use clap::Parser;

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
        println!("🥾 b00t up: launching ralph loop (tool={}, max_iter={}, max_restarts={})",
            self.tool, self.max_iter, self.max_restarts);
        // Phase 1: skeleton — real spawn implemented in Task 2
        println!("⚠️  b00t up spawn not yet implemented");
        Ok(())
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
}
