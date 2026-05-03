
#!/usr/bin/env bash
# ~/.local/bin/mcp-bootstrap.sh

# load user env
#source ~/.profile  # or whichever file sets up ~/.local/bin, ~/.bun/bin, etc.

# optionally remove Windows-mounted paths
export PATH=$(echo "$PATH" | tr ':' '\n' | grep -v '^/mnt/c' | paste -sd ':' -)

# inject secrets
# export OPENAI_API_KEY="${OPENAI_API_KEY}"
# other env vars as needed

env > /tmp/mcpenv


# run the real server command
exec bunx "$@"
