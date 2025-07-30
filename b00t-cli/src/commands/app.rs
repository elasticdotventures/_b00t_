use anyhow::Result;
use clap::Parser;
use crate::datum_gemini::gemini_install_mcp;

#[derive(Parser)]
pub enum AppCommands {
    #[clap(
        about = "VSCode integration commands",
        long_about = "VSCode integration commands.\n\nMCP Examples:\n  b00t-cli app vscode mcp install gh\n  b00t-cli mcp install gh vscode\n\nExtension Examples:\n  b00t-cli app vscode extension list\n  b00t-cli app vscode extension install rust-lang.rust-analyzer"
    )]
    Vscode {
        #[clap(subcommand)]
        vscode_command: AppVscodeCommands,
    },
    #[clap(
        about = "Claude Code integration commands",
        long_about = "Claude Code integration commands.\n\nMCP Examples:\n  b00t-cli app claudecode mcp install gh\n  b00t-cli mcp install gh claudecode"
    )]
    Claudecode {
        #[clap(subcommand)]
        claudecode_command: AppClaudecodeCommands,
    },
    #[clap(
        about = "Gemini CLI integration commands", 
        long_about = "Gemini CLI integration commands.\n\nMCP Examples:\n  b00t-cli app geminicli mcp install gh --repo\n  b00t-cli mcp install gh geminicli --user"
    )]
    Geminicli {
        #[clap(subcommand)]
        geminicli_command: AppGeminicliCommands,
    },
}

#[derive(Parser)]
pub enum AppVscodeCommands {
    #[clap(about = "MCP server management for VSCode")]
    Mcp {
        #[clap(subcommand)]
        mcp_command: AppMcpCommands,
    },
    #[clap(about = "Extension management for VSCode")]
    Extension {
        #[clap(subcommand)]
        extension_command: AppVscodeExtensionCommands,
    },
}

#[derive(Parser)]
pub enum AppClaudecodeCommands {
    #[clap(about = "MCP server management for Claude Code")]
    Mcp {
        #[clap(subcommand)]
        mcp_command: AppMcpCommands,
    },
}

#[derive(Parser)]
pub enum AppGeminicliCommands {
    #[clap(about = "MCP server management for Gemini CLI")]
    Mcp {
        #[clap(subcommand)]
        mcp_command: AppGeminicliMcpCommands,
    },
}

#[derive(Parser)]
pub enum AppMcpCommands {
    #[clap(
        about = "Install MCP server",
        long_about = "Install MCP server to the target application.\n\nExamples:\n  b00t-cli app vscode mcp install gh\n  b00t-cli app claudecode mcp install filesystem"
    )]
    Install {
        #[clap(help = "MCP server name")]
        name: String,
    },
}

#[derive(Parser)]
pub enum AppVscodeExtensionCommands {
    #[clap(
        about = "List installed VS Code extensions",
        long_about = "List all installed VS Code extensions.\n\nExamples:\n  b00t-cli app vscode extension list\n  b00t-cli app vscode extension list --json"
    )]
    List {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(
        about = "Install VS Code extension",
        long_about = "Install VS Code extension by ID.\n\nExamples:\n  b00t-cli app vscode extension install rust-lang.rust-analyzer\n  b00t-cli app vscode extension install ms-python.python"
    )]
    Install {
        #[clap(help = "Extension ID (e.g., rust-lang.rust-analyzer)")]
        extension_id: String,
    },
}

#[derive(Parser)]
pub enum AppGeminicliMcpCommands {
    #[clap(
        about = "Install MCP server to Gemini CLI",
        long_about = "Install MCP server to Gemini CLI extension.\n\nExamples:\n  b00t-cli app geminicli mcp install gh --repo\n  b00t-cli app geminicli mcp install filesystem --user"
    )]
    Install {
        #[clap(help = "MCP server name")]
        name: String,
        #[clap(long, help = "Install to repository-specific location")]
        repo: bool,
        #[clap(long, help = "Install to user-global location")]
        user: bool,
    },
}

impl AppCommands {
    pub fn execute(&self, path: &str) -> Result<()> {
        match self {
            AppCommands::Vscode { .. } => {
                println!("🆚 VSCode app functionality coming soon...");
                Ok(())
            }
            AppCommands::Claudecode { .. } => {
                println!("🤖 Claude Code app functionality coming soon...");
                Ok(())
            }
            AppCommands::Geminicli { geminicli_command } => {
                geminicli_command.execute(path)
            }
        }
    }
}

impl AppGeminicliCommands {
    pub fn execute(&self, path: &str) -> Result<()> {
        match self {
            AppGeminicliCommands::Mcp { mcp_command } => {
                mcp_command.execute(path)
            }
        }
    }
}

impl AppGeminicliMcpCommands {
    pub fn execute(&self, path: &str) -> Result<()> {
        match self {
            AppGeminicliMcpCommands::Install { name, repo, user } => {
                let is_repo = *repo || !*user; // Default to repo if neither specified
                gemini_install_mcp(name, path, is_repo)?;
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_commands_exist() {
        let vscode_cmd = AppCommands::Vscode {
            vscode_command: AppVscodeCommands::Extension {
                extension_command: AppVscodeExtensionCommands::List { json: false }
            }
        };
        
        assert!(vscode_cmd.execute("test").is_ok());
    }
}