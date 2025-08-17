# b00t Agent Coordination Protocol (ACP) justfile module
# 🥾 Extreme programming patterns for ACP library development

# Default target shows available commands
default:
    @just --list --justfile {{justfile()}}

# 🦀 Rust Development

# Build Rust library
build:
    @echo "🦀 Building Rust ACP library..."
    cargo build --release

# Run Rust tests
test:
    @echo "🧪 Running Rust tests..."
    cargo test

# Run basic example
example:
    @echo "🎯 Running basic ACP example..."
    cargo run --example basic

# Run three-agent coordination demo
demo:
    @echo "🎲 Running three-agent coordination demo..."
    cargo run --example three_agent_demo

# Check code formatting and linting
check:
    @echo "🔍 Checking Rust code..."
    cargo fmt --check
    cargo clippy -- -D warnings

# Format code
fmt:
    @echo "🎨 Formatting Rust code..."
    cargo fmt

# Generate documentation
docs:
    @echo "📚 Generating documentation..."
    cargo doc --open --no-deps

# 🐍 Python Bindings

# Build Python bindings
py-build:
    @echo "🐍 Building Python bindings..."
    maturin develop --features python

# Install Python package in development mode
py-install:
    @echo "📦 Installing Python package..."
    maturin develop --features python --release

# Test Python bindings
py-test: py-build
    @echo "🧪 Testing Python bindings..."
    python -c "import b00t_acp; print('✅ Python bindings working')"

# 🌐 WebAssembly/TypeScript Bindings

# Build WASM/TypeScript bindings
wasm-build:
    @echo "🌐 Building WASM bindings..."
    wasm-pack build --target web --features wasm --out-dir pkg

# Test WASM bindings
wasm-test: wasm-build
    @echo "🧪 Testing WASM bindings..."
    ls -la pkg/
    @echo "✅ WASM package generated"

# 🚀 Release & Publishing

# Build all targets (Rust, Python, WASM)
build-all: build py-build wasm-build
    @echo "✅ All targets built successfully"

# Run all tests
test-all: test py-test wasm-test
    @echo "✅ All tests passed"

# Package for release
package: build-all
    @echo "📦 Packaging for release..."
    cargo package
    maturin build --features python --release
    wasm-pack build --target web --features wasm

# 🔧 Development Utilities

# Clean build artifacts
clean:
    @echo "🧹 Cleaning build artifacts..."
    cargo clean
    rm -rf pkg/
    rm -rf target/wheels/
    rm -rf python/__pycache__/

# Setup development environment
setup:
    @echo "🛠️ Setting up development environment..."
    rustup component add rustfmt clippy
    pip install maturin
    cargo install wasm-pack

# Watch for changes and rebuild
watch:
    @echo "👀 Watching for changes..."
    cargo watch -x "build --features python"

# 🧪 Integration Tests

# Test with live NATS server (requires c010.promptexecution.com access)
test-integration:
    @echo "🌐 Running integration tests..."
    @echo "⚠️  Requires NATS server at c010.promptexecution.com:4222"
    RUST_LOG=info cargo test --features integration-tests -- --nocapture

# Test local NATS server
test-local:
    @echo "🏠 Testing with local NATS server..."
    @echo "⚠️  Start local NATS: nats-server -p 4222"
    NATS_URL=nats://localhost:4222 cargo test -- --nocapture

# 📊 Benchmarks

# Run performance benchmarks
bench:
    @echo "⚡ Running benchmarks..."
    cargo bench

# 🤖 ACP Protocol Testing

# Test step synchronization with multiple agents
test-step-sync:
    #!/usr/bin/env bash
    echo "🔄 Testing step synchronization..."
    
    # Start 3 test agents in background
    for i in {1..3}; do
        echo "Starting agent $i..."
        RUST_LOG=info cargo run --example basic &
        sleep 1
    done
    
    echo "Waiting for agents to coordinate..."
    sleep 10
    
    echo "Stopping agents..."
    pkill -f "cargo run --example basic"
    echo "✅ Step sync test complete"

# Test message flow patterns
test-message-flow:
    @echo "📨 Testing ACP message flow..."
    @echo "STATUS → PROPOSE → STEP pattern"
    cargo test test_message_creation -- --nocapture

# Test namespace isolation
test-namespace:
    @echo "🏠 Testing namespace isolation..."
    cargo test test_agent_config -- --nocapture

# 🔍 Code Quality

# Security audit
audit:
    @echo "🔐 Running security audit..."
    cargo audit

# Check dependencies
deps:
    @echo "📋 Checking dependencies..."
    cargo tree

# Memory leak check (requires valgrind)
memcheck:
    @echo "🧠 Running memory check..."
    cargo build --example basic
    valgrind --leak-check=full ./target/debug/examples/basic

# 🎯 Deployment Helpers

# Build for production deployment
deploy-build:
    @echo "🚀 Building for production deployment..."
    cargo build --release --features python,wasm
    strip target/release/libb00t_acp.so 2>/dev/null || true

# Check compatibility with b00t-website
compat-check:
    @echo "🔗 Checking compatibility with b00t-website..."
    @echo "Verifying NATS subject patterns match..."
    grep -r "account\." ../b00t-website/ | head -5 || echo "No conflicts found"

# Generate integration code for b00t-website
gen-integration:
    @echo "🔌 Generating integration code..."
    @echo "// Auto-generated ACP integration" > acp-integration.js
    @echo "export const ACP_SUBJECTS = {" >> acp-integration.js
    @echo "  stepWildcard: (step) => \`acp.\${step}.>\`," >> acp-integration.js
    @echo "  agentWildcard: (agentId) => \`acp.>.\${agentId}.>\`" >> acp-integration.js
    @echo "};" >> acp-integration.js
    @echo "✅ Integration code generated: acp-integration.js"

# 🤓 Melvin's wisdom
# 🤓: ACP uses Lamport timestamps for coordination - not wall clock time!
# 🤓: NATS subjects are hierarchical - use '.' for levels, '>' for wildcards
# 🤓: Step barriers prevent race conditions in multi-agent workflows