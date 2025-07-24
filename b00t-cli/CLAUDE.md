# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Development
- **Build**: `cargo build` or `cargo run`
- **Run with arguments**: `just cli <args>` (equivalent to `cargo run --bin b00t-cli -- <args>`)
- **List configuration files**: `just list-commands` (finds all .toml config files in ~/.dotfiles/_b00t_)

### Testing
- **Unit tests**: `cargo test`
- **MCP functionality**: `just test-mcp` (test MCP server addition)
- **VSCode integration**: `just test-vscode` (test VSCode MCP installation)
- **Claude Code integration**: `just test-claude-code` (test Claude Code MCP installation)

## Architecture

### Overview
b00t-cli is a Rust-based command-line tool for managing software versions and installations. It acts as a universal package manager that works with TOML configuration files to define how to detect, install, and update various command-line tools.

### Core Components

**Main Application** (`src/main.rs`):
- CLI parser using `clap` with subcommands: `detect`, `desires`, `install`, `update`, `.` (dot), `up`, `mcp`, `vscode`, `claude-code`
- Configuration loading from TOML files in `~/.dotfiles/_b00t_/` directory
- Version comparison using semver for upgrade decisions

**Configuration System**:
- Each tool has its own `.toml` file in `~/.dotfiles/_b00t_/`
- Config structure includes: name, desires (target version), install/update commands, version detection command, version regex, and hint
- Example path: `~/.dotfiles/_b00t_/git.toml`, `~/.dotfiles/_b00t_/node.toml`
- MCP servers stored as: `~/.dotfiles/_b00t_/<name>.mcp-json.toml`

**Command Operations**:
- `detect <command>`: Shows currently installed version
- `desires <command>`: Shows target version from config
- `install <command>`: Runs installation command from config
- `update <command>`: Runs update command (falls back to install if not specified)
- `. <command>`: Compares installed vs desired versions with emoji status
- `up`: Updates all outdated tools found in config directory

**MCP (Model Context Protocol) Management**:
- `mcp add <json>`: Parse JSON and create `.mcp-json.toml` configuration
- `mcp add --dwiw <json>`: "Do What I Want" mode - strips comments from JSON
- `mcp list`: Show all available MCP server configurations
- `mcp list --json`: Show MCP configurations in JSON format
- `vscode install mcp <name>`: Install MCP server to VSCode via `code --add-mcp`
- `claude-code install mcp <name>`: Install MCP server to Claude Code via `claude-code config add-mcp`
- One source of truth (TOML files), multiple export targets

**Version Detection**:
- Executes shell commands defined in config files
- Uses regex patterns to extract version numbers from command output
- Supports custom version detection commands per tool

### Dependencies
- `clap`: Command-line argument parsing
- `serde` + `serde_json` + `toml`: Configuration file parsing and JSON handling
- `regex`: Version string extraction and comment cleaning
- `duct`: Shell command execution
- `shellexpand`: Tilde expansion for paths
- `semver`: Semantic version comparison
- `anyhow`: Error handling
- `tempfile` (dev): Testing utilities

### Integration with _b00t_ Ecosystem
This CLI is part of a larger dotfiles management system located in `~/.dotfiles/_b00t_/` which includes installation scripts for various programming languages, cloud tools, and development environments organized by categories (levels).