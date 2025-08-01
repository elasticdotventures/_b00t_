use anyhow::Result;
use clap::Parser;
use crate::{handle_session_init, handle_session_status, handle_session_end};
use crate::session_memory::SessionMemory;

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
    #[clap(about = "Get value by key")]
    Get {
        #[clap(help = "Key to retrieve")]
        key: String,
    },
    #[clap(about = "Set string value by key")]
    Set {
        #[clap(help = "Key to set")]
        key: String,
        #[clap(help = "Value to set")]
        value: String,
    },
    #[clap(about = "Increment numeric value by key")]
    Incr {
        #[clap(help = "Key to increment")]
        key: String,
    },
    #[clap(about = "Decrement numeric value by key")]
    Decr {
        #[clap(help = "Key to decrement")]
        key: String,
    },
    #[clap(about = "List all keys and their types")]
    Keys,
    #[clap(about = "Clear all session data")]
    Clear,
    #[clap(about = "Mark README.md as read for this session")]
    MarkReadmeRead,
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
            SessionCommands::Get { key } => {
                let memory = SessionMemory::load()?;
                if let Some(value) = memory.get(key) {
                    println!("{}", value);
                } else if memory.get_num(key) != 0 || memory.numbers.contains_key(key) {
                    println!("{}", memory.get_num(key));
                } else if memory.flags.contains_key(key) {
                    println!("{}", memory.get_flag(key));
                } else {
                    eprintln!("Key '{}' not found", key);
                    std::process::exit(1);
                }
                Ok(())
            }
            SessionCommands::Set { key, value } => {
                let mut memory = SessionMemory::load()?;
                memory.set(key, value)?;
                println!("Set '{}' = '{}'", key, value);
                Ok(())
            }
            SessionCommands::Incr { key } => {
                let mut memory = SessionMemory::load()?;
                let new_value = memory.incr(key)?;
                println!("{}", new_value);
                Ok(())
            }
            SessionCommands::Decr { key } => {
                let mut memory = SessionMemory::load()?;
                let new_value = memory.decr(key)?;
                println!("{}", new_value);
                Ok(())
            }
            SessionCommands::Keys => {
                let memory = SessionMemory::load()?;
                let keys = memory.list_keys();
                if keys.is_empty() {
                    println!("No keys found");
                } else {
                    for (key, key_type) in keys {
                        println!("{} ({})", key, key_type);
                    }
                }
                Ok(())
            }
            SessionCommands::Clear => {
                let mut memory = SessionMemory::load()?;
                memory.clear()?;
                println!("Session memory cleared");
                Ok(())
            }
            SessionCommands::MarkReadmeRead => {
                let mut memory = SessionMemory::load()?;
                memory.mark_readme_read()?;
                println!("✅ README.md marked as read for this session");
                Ok(())
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