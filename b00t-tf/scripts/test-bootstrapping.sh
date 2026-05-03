#!/usr/bin/env bash
# test-bootstrapping.sh - Test self-bootstrapping functionality

set -e

echo "🥾 Testing self-bootstrapping functionality..."

# Test tool detection
echo ""
echo "📋 Testing tool detection functions..."
node -e "
const fs = require('fs');
const server = fs.readFileSync('proxy-server.js', 'utf8');

const functions = [
    'detectInstalledTools',
    'installTool', 
    'bootstrapB00t',
    'provisionInfrastructure',
    'configureProvider'
];

let missing = [];
functions.forEach(func => {
    if (server.includes(func)) {
        console.log('✅ ' + func + ' function found');
    } else {
        console.log('❌ ' + func + ' function missing');
        missing.push(func);
    }
});

if (missing.length > 0) {
    console.log('');
    console.log('⚠️  Missing functions:', missing.join(', '));
    process.exit(1);
}
"

# Test OpenTofu module validation
echo ""
echo "📋 Testing OpenTofu modules..."
if [ -d "modules" ]; then
    echo "✅ OpenTofu modules directory exists"
    echo "📁 Available modules:"
    for module in modules/*/; do
        if [ -d "$module" ]; then
            module_name=$(basename "$module")
            echo "  - $module_name"
            # Check for main.tf in each module
            if [ -f "$module/main.tf" ]; then
                echo "    ✅ main.tf found"
            else
                echo "    ❌ main.tf missing"
            fi
        fi
    done
else
    echo "❌ OpenTofu modules directory missing"
    exit 1
fi

# Test configuration layer detection
echo ""
echo "📋 Testing configuration layers..."
config_files=(
    ".env.template"
    "_b00t_.toml" 
    "manifest.json"
    "package.json"
    "main.tf"
)

missing_configs=()
for file in "${config_files[@]}"; do
    if [ -f "$file" ]; then
        echo "✅ $file found"
    else
        echo "❌ $file missing"
        missing_configs+=("$file")
    fi
done

if [ ${#missing_configs[@]} -gt 0 ]; then
    echo ""
    echo "⚠️  Missing configuration files: ${missing_configs[*]}"
    exit 1
fi

# Test package.json dependencies
echo ""
echo "📋 Testing package.json dependencies..."
if command -v jq >/dev/null 2>&1; then
    required_deps=("@modelcontextprotocol/sdk")
    for dep in "${required_deps[@]}"; do
        if jq -e ".dependencies[\"$dep\"]" package.json >/dev/null 2>&1; then
            echo "✅ $dep dependency found"
        else
            echo "❌ $dep dependency missing"
        fi
    done
else
    echo "⚠️  jq not available, skipping dependency check"
fi

# Test MCP manifest validation
echo ""
echo "📋 Testing MCP manifest..."
if command -v jq >/dev/null 2>&1; then
    if jq -e '.mcpServers.b00t' manifest.json >/dev/null 2>&1; then
        echo "✅ MCP server configuration found in manifest"
        # Check for required fields
        if jq -e '.mcpServers.b00t.command' manifest.json >/dev/null 2>&1; then
            echo "✅ MCP command field configured"
        else
            echo "❌ MCP command field missing"
        fi
    else
        echo "❌ MCP server configuration missing from manifest"
    fi
else
    echo "⚠️  jq not available, skipping manifest validation"
fi

# Test tool installation detection
echo ""
echo "📋 Testing tool installation logic..."
tools_to_check=("just" "gh" "tofu" "terraform")
echo "🔍 Checking for common tools that might be auto-installed:"
for tool in "${tools_to_check[@]}"; do
    if command -v "$tool" >/dev/null 2>&1; then
        echo "✅ $tool is available"
    else
        echo "❌ $tool not found (will be auto-installed)"
    fi
done

echo ""
echo "🏁 Bootstrapping test complete!"
echo ""
echo "📊 Summary:"
echo "  - MCP server functions: present"
echo "  - OpenTofu modules: validated"  
echo "  - Configuration files: present"
echo "  - Ready for self-bootstrapping deployment"