use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
pub enum McpCommands {
    #[clap(
        about = "Register or remove MCP server configuration",
        long_about = "Register or remove MCP server configuration from JSON or command.\n\nJSON Examples:\n  b00t-cli mcp register '{\"name\":\"filesystem\",\"command\":\"npx\",\"args\":[\"-y\",\"@modelcontextprotocol/server-filesystem\"]}'\n  echo '{...}' | b00t-cli mcp register -\n\nCommand Examples:\n  b00t-cli mcp register brave-search -- npx -y @modelcontextprotocol/server-brave-search\n  b00t-cli mcp register filesystem --hint \"File system access\" -- npx -y @modelcontextprotocol/server-filesystem\n\nRemoval Examples:\n  b00t-cli mcp register --remove filesystem\n  b00t-cli mcp register --remove brave-search\n\nInstallation Examples:\n  b00t-cli mcp install brave-search claudecode\n  b00t-cli app vscode mcp install filesystem"
    )]
    Register {
        #[clap(help = "MCP server name (for command mode) or JSON configuration (for JSON mode)")]
        name_or_json: String,
        #[clap(long, help = "Description/hint for the MCP server")]
        hint: Option<String>,
        #[clap(long, help = "Remove the MCP server configuration")]
        remove: bool,
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
        about = "Install MCP server to a target (claudecode, vscode, geminicli, dotmcpjson)",
        long_about = "Install MCP server to a target application.\n\nExamples:\n  b00t-cli mcp install gh claudecode\n  b00t-cli mcp install filesystem geminicli --repo\n  b00t-cli mcp install browser-use dotmcpjson --stdio-command uvx\n  b00t-cli mcp install aws-knowledge dotmcpjson --httpstream\n  b00t-cli app vscode mcp install filesystem"
    )]
    Install {
        #[clap(help = "MCP server name")]
        name: String,
        #[clap(help = "Installation target: claudecode, vscode, geminicli, dotmcpjson")]
        target: String,
        #[clap(long, help = "Install to repository-specific location (for geminicli)")]
        repo: bool,
        #[clap(long, help = "Install to user-global location (for geminicli)")]
        user: bool,
        #[clap(long, help = "Select stdio method by command (for multi-source MCP configs)")]
        stdio_command: Option<String>,
        #[clap(long, help = "Use httpstream method (for multi-source MCP configs)")]
        httpstream: bool,
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
    pub fn execute(&self, path: &str) -> Result<()> {
        match self {
            McpCommands::Register { name_or_json, hint: _, remove, dwiw, no_dwiw, command_args } => {
                if *remove {
                    // Remove mode: delete the MCP server configuration
                    crate::mcp_remove(name_or_json, path)
                } else {
                    let actual_dwiw = !no_dwiw && *dwiw;
                    
                    // Check if it's JSON mode (starts with { or -)
                    if name_or_json.starts_with('{') || name_or_json == "-" {
                        // JSON mode
                        crate::mcp_add_json(name_or_json, actual_dwiw, path)
                    } else if !command_args.is_empty() {
                        // Command mode: b00t-cli mcp register server-name -- npx -y @package
                        let server_name = name_or_json;
                        let command = &command_args[0];
                        let args = if command_args.len() > 1 {
                            command_args[1..].to_vec()
                        } else {
                            vec![]
                        };
                        
                        let json_str = serde_json::json!({
                            "name": server_name,
                            "command": command,
                            "args": args
                        }).to_string();
                        
                        crate::mcp_add_json(&json_str, actual_dwiw, path)
                    } else {
                        anyhow::bail!("Invalid register command. Use JSON format or command format with --");
                    }
                }
            }
            McpCommands::List { json } => {
                crate::mcp_list(path, *json)
            }
            McpCommands::Install { name, target, repo, user, stdio_command, httpstream } => {
                match target.as_str() {
                    "claudecode" | "claude" => {
                        crate::claude_code_install_mcp(name, path)
                    }
                    "vscode" => {
                        crate::vscode_install_mcp(name, path)
                    }
                    "geminicli" => {
                        // Determine installation location: default to repo if in git repo, otherwise user
                        let use_repo = if *repo && *user {
                            anyhow::bail!("Error: Cannot specify both --repo and --user flags");
                        } else if *repo {
                            true
                        } else if *user {
                            false
                        } else {
                            // Default behavior: repo if in git repo, otherwise user
                            crate::utils::is_git_repo()
                        };
                        crate::gemini_install_mcp(name, path, use_repo)
                    }
                    "dotmcpjson" => {
                        crate::dotmcpjson_install_mcp(name, path, stdio_command.as_deref(), *httpstream)
                    }
                    _ => {
                        anyhow::bail!(
                            "Error: Invalid target '{}'. Valid targets are: claudecode, vscode, geminicli, dotmcpjson",
                            target
                        );
                    }
                }
            }
            McpCommands::Output { json, mcp_servers, servers } => {
                let use_mcp_servers_wrapper = !json && (*mcp_servers || !servers.contains(','));
                crate::mcp_output(path, use_mcp_servers_wrapper, servers)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_commands_exist() {
        // Test with JSON format
        let register_cmd = McpCommands::Register {
            name_or_json: r#"{"name":"test-server","command":"npx","args":["-y","@test/package"]}"#.to_string(),
            hint: None,
            remove: false,
            dwiw: false,
            no_dwiw: false,
            command_args: vec![],
        };
        
        // This should fail because we don't have a valid test directory, but the command should parse correctly
        // The important thing is that it doesn't panic and processes the JSON correctly
        let result = register_cmd.execute("/tmp/nonexistent");
        assert!(result.is_err()); // Expected to fail due to invalid path, but should not panic
        
        // Test install command enum creation
        let install_cmd = McpCommands::Install {
            name: "test-server".to_string(),
            target: "claudecode".to_string(),
            repo: false,
            user: false,
            stdio_command: None,
            httpstream: false,
        };
        
        // This should fail because the server doesn't exist, but should not panic
        let result = install_cmd.execute("/tmp/nonexistent");
        assert!(result.is_err()); // Expected to fail, but should not panic
    }
}