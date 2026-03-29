# 🦄 qwen-code CLI - Justfile Recipes
# Setup and manage Qwen Code MCP integration with b00t

set shell := ["bash", "-cu"]

# 🚀 Quick Setup - One command to install & configure qwen-code MCP
qwen-setup:
    #!/bin/bash
    set -euo pipefail
    echo "🦄 Setting up qwen-code MCP integration..."
    
    # Check if qwen CLI is installed
    if ! command -v qwen >/dev/null 2>&1; then
        echo "⚠️  qwen CLI not found, installing..."
        npm install -g @qwen-code/qwen-code@latest
    fi
    
    # Verify version
    echo "✅ qwen CLI version: $(qwen --version)"
    
    # Add b00t-mcp to qwen MCP servers
    echo "🔧 Registering b00t-mcp with qwen CLI..."
    qwen mcp add b00t-mcp b00t-mcp --stdio 2>/dev/null || {
        echo "  ℹ️  b00t-mcp already registered, updating..."
        qwen mcp remove b00t-mcp
        qwen mcp add b00t-mcp b00t-mcp --stdio
    }
    
    # Verify configuration
    echo "🧪 Verifying MCP configuration..."
    qwen mcp list
    
    echo "✅ qwen-code MCP setup complete!"
    echo "💡 Usage: qwen (interactive) or qwen -p 'your prompt'"

# 🧪 Verify qwen-code installation
qwen-check:
    #!/bin/bash
    set -euo pipefail
    echo "🦄 qwen-code Installation Check"
    echo "=============================="
    
    # Check CLI
    if command -v qwen >/dev/null 2>&1; then
        echo "✅ qwen CLI: $(qwen --version)"
    else
        echo "❌ qwen CLI: not installed"
        echo "   Run: just qwen-setup"
        exit 1
    fi
    
    # Check MCP servers
    echo ""
    echo "📡 MCP Servers:"
    qwen mcp list
    
    # Check b00t-mcp
    echo ""
    echo "🔧 b00t-mcp:"
    if b00t-mcp --version >/dev/null 2>&1; then
        echo "✅ b00t-mcp: $(b00t-mcp --version)"
    else
        echo "❌ b00t-mcp: not installed"
    fi

# 🧪 Test qwen MCP connection
qwen-test:
    #!/bin/bash
    set -euo pipefail
    echo "🧪 Testing qwen MCP connection..."
    
    # List MCP servers
    echo "📡 MCP Servers:"
    qwen mcp list
    
    # Test b00t-mcp stdio
    echo ""
    echo "🔧 Testing b00t-mcp stdio..."
    echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}' | b00t-mcp --stdio 2>&1 | head -5
    
    echo ""
    echo "✅ Test complete!"

# 📚 Load qwen-code learn skill
qwen-learn:
    b00t learn qwen-code

# 🔍 Show qwen-code datum
qwen-datum:
    b00t learn qwen-code --show-datum

# 🆙 Update qwen-code CLI
qwen-update:
    #!/bin/bash
    set -euo pipefail
    echo "🔄 Updating qwen-code CLI..."
    npm install -g @qwen-code/qwen-code@latest
    echo "✅ Updated to: $(qwen --version)"

# 🧹 Remove qwen-code MCP configuration
qwen-clean:
    #!/bin/bash
    set -euo pipefail
    echo "🧹 Removing qwen-code MCP configuration..."
    qwen mcp remove b00t-mcp 2>/dev/null || echo "  ℹ️  b00t-mcp not configured"
    echo "✅ Cleanup complete!"

# b00t:map v1
# summary: qwen-code justfile recipes - setup, check, test, learn, update, clean
# tags: qwen-code, justfile, mcp, b00t-mcp, setup
# tier: ch0nky
# cmds: just qwen-setup, just qwen-check, just qwen-test
# complexity: 5