# b00t justfile - MCP chat integration and utilities

# Interactive MCP chat selection with fzf
mcp-chat:
    #!/usr/bin/env bash
    set -euo pipefail
    
    # Check if required tools are available
    if ! command -v jq &> /dev/null; then
        echo "Error: jq is required but not installed" >&2
        exit 1
    fi
    
    if ! command -v fzf &> /dev/null; then
        echo "Error: fzf is required but not installed" >&2
        exit 1
    fi
    
    if ! command -v bunx &> /dev/null; then
        echo "Error: bunx is required but not installed" >&2
        exit 1
    fi
    
    # Get available MCP servers and format for fzf
    selected=$(b00t-cli mcp list --json | jq -r '.servers[] | "\(.name) - \(.hint // "No description")"' | fzf --prompt="Select MCP server: " --height=40% --border)
    
    if [ -z "$selected" ]; then
        echo "No MCP server selected"
        exit 1
    fi
    
    # Extract server name from selection
    server_name=$(echo "$selected" | cut -d' ' -f1)
    
    # Get the command for the selected server
    server_command=$(b00t-cli mcp list --json | jq -r ".servers[] | select(.name == \"$server_name\") | \"\(.command) \(.args | join(\" \"))\"")
    
    if [ -z "$server_command" ]; then
        echo "Error: Could not find command for server '$server_name'" >&2
        exit 1
    fi
    
    echo "Starting mcp-chat with server: $server_name"
    echo "Command: $server_command"
    echo "---"
    
    # Run mcp-chat with the selected server
    bunx mcp-chat --server "$server_command"

# List available MCP servers in a readable format
mcp-list:
    b00t-cli mcp list

# List available MCP servers in JSON format
mcp-list-json:
    b00t-cli mcp list --json

# Pretty-print MCP servers with jq
mcp-pretty:
    b00t-cli mcp list --json | jq '.'

# Show MCP server commands only
mcp-commands:
    b00t-cli mcp list --json | jq -r '.servers[] | "\(.name): \(.command) \(.args | join(" "))"'

# Quick test of MCP functionality
mcp-test:
    echo "Testing MCP functionality..."
    b00t-cli mcp list
    echo "---"
    b00t-cli mcp list --json | jq '.servers[] | {name, command, hint}'