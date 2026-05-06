# 🤓 ZAI Coding Helper — auto-activated when ZAI_API_KEY is set.
# Loaded by b00t shell integration.
# Run: zai "<your coding question or task>"

if [ -n "${ZAI_API_KEY:-}" ]; then
    alias zai='npx @z_ai/coding-helper'
    # Also create as function for piping
    zai_pipe() {
        npx @z_ai/coding-helper "$@"
    }
fi
