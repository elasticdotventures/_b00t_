use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
pub enum McpCommands {
    #[clap(
        about = "Create MCP server configuration",
        long_about = "Create MCP server configuration from JSON or command.\n\nJSON Examples:\n  b00t-cli mcp create '{\"name\":\"filesystem\",\"command\":\"npx\",\"args\":[\"-y\",\"@modelcontextprotocol/server-filesystem\"]}'\n  echo '{...}' | b00t-cli mcp create -\n\nCommand Examples:\n  b00t-cli mcp create brave-search -- npx -y @modelcontextprotocol/server-brave-search\n  b00t-cli mcp create filesystem --hint \"File system access\" -- npx -y @modelcontextprotocol/server-filesystem\n\nInstallation Examples:\n  b00t-cli mcp install brave-search claudecode\n  b00t-cli app vscode mcp install filesystem"
    )]
    Create {
        #[clap(help = "MCP server name (for command mode) or JSON configuration (for JSON mode)")]
        name_or_json: String,
        #[clap(long, help = "Description/hint for the MCP server")]
        hint: Option<String>,
        #[clap(
            long,
            help = "Do What I Want - auto-cleanup and format JSON (default: enabled)"
        )]
        dwiw: bool,
        #[clap(
            long,
            help = "Disable auto-cleanup and format JSON",
            conflicts_with = "dwiw"
        )]
        no_dwiw: bool,
        #[clap(
            last = true,
            help = "Command and arguments (after --) for command mode"
        )]
        command_args: Vec<String>,
    },
    #[clap(
        about = "List available MCP server configurations",
        long_about = "List available MCP server configurations.\n\nExamples:\n  b00t-cli mcp list\n  b00t-cli mcp list --json"
    )]
    List {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },
    #[clap(
        about = "Install MCP server to a target (claudecode, vscode, geminicli)",
        long_about = "Install MCP server to a target application.\n\nExamples:\n  b00t-cli mcp install gh claudecode\n  b00t-cli mcp install filesystem geminicli --repo\n  b00t-cli app vscode mcp install filesystem"
    )]
    Install {
        #[clap(help = "MCP server name")]
        name: String,
        #[clap(help = "Installation target: claudecode, vscode, geminicli")]
        target: String,
        #[clap(long, help = "Install to repository-specific location (for geminicli)")]
        repo: bool,
        #[clap(long, help = "Install to user-global location (for geminicli)")]
        user: bool,
    },
    #[clap(
        about = "Output MCP servers in various formats",
        long_about = "Output MCP servers in various formats for configuration files.\n\nExamples:\n  b00t-cli mcp output filesystem,brave-search\n  b00t-cli mcp output --json filesystem\n  b00t-cli mcp output --mcpServers filesystem,brave-search"
    )]
    Output {
        #[clap(long = "json", help = "Output raw JSON format without wrapper", action = clap::ArgAction::SetTrue)]
        json: bool,
        #[clap(long = "mcpServers", help = "Output in mcpServers format (default)", action = clap::ArgAction::SetTrue)]
        mcp_servers: bool,
        #[clap(help = "Comma-separated list of MCP server names to output")]
        servers: String,
    },
}

impl McpCommands {
    pub fn execute(&self, _path: &str) -> Result<()> {
        match self {
            McpCommands::Create { .. } => {
                println!("🔧 MCP create functionality coming soon...");
                Ok(())
            }
            McpCommands::List { .. } => {
                println!("📋 MCP list functionality coming soon...");
                Ok(())
            }
            McpCommands::Install { .. } => {
                println!("📦 MCP install functionality coming soon...");
                Ok(())
            }
            McpCommands::Output { json, mcp_servers, servers } => {
                let use_mcp_servers_wrapper = !json && (*mcp_servers || !servers.contains(','));
                crate::mcp_output(_path, use_mcp_servers_wrapper, servers)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_commands_exist() {
        let create_cmd = McpCommands::Create {
            name_or_json: "test-server".to_string(),
            hint: None,
            dwiw: false,
            no_dwiw: false,
            command_args: vec![],
        };
        
        assert!(create_cmd.execute("test").is_ok());
    }
}