# ── MCP Service Mesh (podman pod) ──────────────────────────────────────────
# Extracted from root justfile for module isolation.

# Start b00t MCP servers as sidecar containers in a pod
# 🤓 b00t-mcp (HTTP :3000) + codebase-memory-mcp (socat :9101).
#    OpenCode connects via localhost URLs — no binary mounts needed.
opencode-mesh-start:
    @echo "🔧 Starting b00t MCP mesh..."
    @podman pod exists b00t-mesh 2>/dev/null && echo "  Pod already running" || \
        (podman pod create --name b00t-mesh -p 3000:3000 -p 9101:9101 2>&1)
    @podman ps --filter name=b00t-mcp-http | grep -q b00t-mcp-http || \
        (podman run -d --pod b00t-mesh --name b00t-mcp-http \
            -v $$(realpath $$(which b00t-mcp)):/usr/local/bin/b00t-mcp:ro \
            -v $$(pwd):/workspace:ro -w /workspace \
            docker.io/ubuntu:24.04 /usr/local/bin/b00t-mcp --http --host 0.0.0.0 --port 3000 2>&1)
    @podman ps --filter name=cb-mcp-socat | grep -q cb-mcp-socat || \
        (podman run -d --pod b00t-mesh --name cb-mcp-socat \
            -v $$(realpath $$(which codebase-memory-mcp)):/usr/local/bin/codebase-memory-mcp:ro \
            docker.io/ubuntu:24.04 \
            bash -c "apt-get update -qq && apt-get install -y -qq socat && socat TCP-LISTEN:9101,fork,reuseaddr EXEC:/usr/local/bin/codebase-memory-mcp" 2>&1)
    @echo "✅ MCP mesh running: b00t-mcp :3000, cb-mcp :9101"

# Stop the MCP service mesh
opencode-mesh-stop:
    @echo "🛑 Stopping b00t MCP mesh..."
    @podman pod rm -f b00t-mesh 2>/dev/null || true
    @echo "✅ MCP mesh stopped"

# Run OpenCode connected to MCP service mesh (no binary mounts)
opencode-mesh workspace=".":
    @echo "🌐 Launching OpenCode (mesh mode)..."
    @mkdir -p {{workspace}}/.opencode/skills
    podman run -it --rm \
        --security-opt label=disable \
        --network host \
        -v {{workspace}}:/workspace \
        -v {{workspace}}/.opencode/skills:/root/.opencode/skills:ro \
        -v ${HOME}/.gitconfig:/root/.gitconfig:ro \
        -v ${HOME}/.ssh:/root/.ssh:ro \
        -e OPENAI_API_KEY \
        -e ANTHROPIC_API_KEY \
        -w /workspace \
        ghcr.io/anomalyco/opencode
