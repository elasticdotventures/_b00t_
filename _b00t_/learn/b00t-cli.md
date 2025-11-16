---
mcp path configuration: Use --path flag to specify MCP datum location when datums are in subdirectory. Default searches ~/.dotfiles/_b00t_/ but datum organization may use nested _b00t_/_b00t_/ structure. Verify with: b00t-cli --path ~/.dotfiles/_b00t_/_b00t_ mcp list

dotmcpjson target for self-install: Install b00t-mcp to project .mcp.json using 'b00t-cli mcp install b00t-mcp dotmcpjson'. Added in commit f499a97. Enables self-bootstrapping of b00t MCP server into any project. Use just b00t::mcp-self-install for convenience.

rustc version upgrades: When dependencies require newer rustc (e.g., rig-core needs 1.88+ for unstable let expressions), upgrade with 'rustup update stable && rustup override set stable' from project root. Never use manual workarounds or skip builds - fix the toolchain.

just module invocation: Justfile modules (e.g., 'mod b00t') must be invoked from the justfile root where module is declared. Use 'just b00t::recipe' not 'just recipe' when in subdirectory. cd to project root or use -f flag.

