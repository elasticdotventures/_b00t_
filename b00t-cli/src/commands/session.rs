use anyhow::Result;
use clap::Parser;
use crate::{handle_session_init, handle_session_status, handle_session_end};

#[derive(Parser)]
pub enum SessionCommands {
    #[clap(about = "Initialize new session")]
    Init {
        #[clap(long, help = "Budget limit in USD")]
        budget: Option<f64>,
        #[clap(long, help = "Session name")]
        name: Option<String>,
    },
    #[clap(about = "Show current session status")]
    Status,
    #[clap(about = "End current session")]
    End,
}

impl SessionCommands {
    pub fn execute(&self, _path: &str) -> Result<()> {
        match self {
            SessionCommands::Init { budget, name: _name } => {
                // For now, using budget directly and no time_limit support in CLI args
                // name parameter exists but not used in handler yet
                handle_session_init(budget, &None, None)
            }
            SessionCommands::Status => {
                handle_session_status()
            }
            SessionCommands::End => {
                handle_session_end()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_commands_exist() {
        let init_cmd = SessionCommands::Init {
            budget: Some(10.0),
            name: Some("test-session".to_string()),
        };
        
        assert!(init_cmd.execute("test").is_ok());
    }
}