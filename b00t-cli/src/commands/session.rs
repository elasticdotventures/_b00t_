use crate::session_memory::SessionMemory;
use crate::{handle_session_end, handle_session_init, handle_session_status};
use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
pub enum SessionCommands {
    #[clap(about = "Initialize new session")]
    // 🤓 ENTANGLED: b00t-mcp/src/mcp_tools.rs SessionInitCommand
    // When this changes, update b00t-mcp SessionInitCommand structure
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
    #[clap(about = "Generate documented _b00t_.toml template")]
    Template,
    #[clap(about = "Check if output should be verbose for current shell")]
    ShouldShowOutput,
    #[clap(about = "Increment shell start count and return agent info")]
    BashrcInit,
    #[clap(about = "Increment build count for current branch")]
    Build,
    #[clap(about = "Increment compile count for current session")]
    Compile,
    #[clap(about = "Increment test count for current session")]
    Test,
}

impl SessionCommands {
    pub fn execute(&self, _path: &str) -> Result<()> {
        match self {
            SessionCommands::Init {
                budget,
                name: _name,
            } => {
                // For now, using budget directly and no time_limit support in CLI args
                // name parameter exists but not used in handler yet
                handle_session_init(budget, &None, None)
            }
            SessionCommands::Status => handle_session_status(),
            SessionCommands::End => handle_session_end(),
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
            SessionCommands::Template => generate_session_template(),
            SessionCommands::ShouldShowOutput => {
                let memory = SessionMemory::load()?;
                if memory.should_show_verbose_output() {
                    std::process::exit(0); // success = show output
                } else {
                    std::process::exit(1); // failure = suppress output
                }
            }
            SessionCommands::BashrcInit => {
                let mut memory = SessionMemory::load()?;
                let count = memory.increment_shell_count()?;

                // Always detect and print agent (for _B00T_Agent export)
                use crate::commands::whatismy::detect_agent;
                let agent = detect_agent(&memory, false);

                // Print different output based on verbosity settings
                if memory.should_show_verbose_output() {
                    println!(
                        "🥾👋 {} (shell #{}) run `b00t whoami` for superpowers",
                        agent, count
                    );
                } else {
                    // Just print the agent for environment variable assignment
                    println!("{}", agent);
                }

                Ok(())
            }
            SessionCommands::Build => {
                let mut memory = SessionMemory::load()?;
                let count = memory.increment_build_count()?;
                println!(
                    "Build #{} on branch {}",
                    count,
                    memory
                        .metadata
                        .initial_branch
                        .as_deref()
                        .unwrap_or("unknown")
                );
                Ok(())
            }
            SessionCommands::Compile => {
                let mut memory = SessionMemory::load()?;
                let count = memory.increment_compile_count()?;
                println!("Compile #{} this session", count);
                Ok(())
            }
            SessionCommands::Test => {
                let mut memory = SessionMemory::load()?;
                let count = memory.increment_test_count()?;
                println!("Test run #{} this session", count);
                Ok(())
            }
        }
    }
}

fn generate_session_template() -> Result<()> {
    let template = r#"# b00t Session Configuration Template
# 🤓 This file (_b00t_.toml) contains session memory and configuration
# 🤓 It's automatically created in your .git/ directory when using b00t-cli
# 🤓 The file is stored in .git/ so it's automatically ignored by git

[metadata]
# 🤓 Session metadata (automatically managed)
session_id = "uuid-will-be-generated"
created_at = "2025-01-01T00:00:00Z"
updated_at = "2025-01-01T00:00:00Z"
readme_read = false
initial_branch = "main"

[config]
# 🤓 Environment variables to track for agent/environment detection
# 🤓 These are checked in .env file first, then system environment
tracked_env_vars = [
    "TERM",
    "TERM_PROGRAM", 
    "SHELL",
    "PWD",
    "USER",
    "HOME",
    "CLAUDECODE",            # 🤓 Set to "1" when running in Claude Code
    "_B00T_Agent",           # 🤓 Override for agent detection
    "VSCODE_GIT_IPC_HANDLE", # 🤓 VS Code detection
    "SSH_CLIENT",
    "SSH_TTY",
    "container",             # 🤓 Docker/container detection
    "ANTHROPIC_API_KEY",
    "CLAUDE_API_KEY",
]

# 🤓 Output verbosity controls
verbose_interactive = true     # 🤓 Show full output in interactive shells
verbose_noninteractive = false # 🤓 Minimal output for agentic/non-interactive shells

# 🤓 Whether to use .env file overrides (recommended: true)
use_env_overrides = true

# 🤓 Count shell starts per PID for session tracking
count_shell_starts = true

# 🤓 Custom agent detection patterns (pattern -> display name)
[config.agent_patterns]
claude = "🤖 Claude"
gemini = "🤖 Gemini" 
gpt = "🤖 GPT"
openai = "🤖 OpenAI"

# 🤓 String storage (key-value pairs)
[strings]
# example_key = "example_value"

# 🤓 Numeric storage (for incr/decr operations)
[numbers]
# shell_count_12345 = 5  # 🤓 Example: shell starts for PID 12345

# 🤓 Boolean flags
[flags]
# example_flag = true

# 🤓 Usage Examples:
# 
# Check agent detection:
#   b00t-cli whatismy agent
#
# Count shell starts:
#   b00t-cli session incr shell_starts
#
# Set custom values:
#   b00t-cli session set project_name "my-awesome-project"
#   b00t-cli session get project_name
#
# Override environment in .env file:
#   echo '_B00T_Agent=🤖 Custom Agent' >> .env
#
# Check session status:
#   b00t-cli session status
"#;

    println!("{}", template);
    Ok(())
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
