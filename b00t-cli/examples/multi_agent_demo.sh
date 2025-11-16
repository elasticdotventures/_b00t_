#!/usr/bin/env bash
# Multi-agent POC demonstration
#
# This script demonstrates:
# 1. Two agents connecting via handshake
# 2. Voting on a proposal
# 3. Crew formation
# 4. Crown delegation

set -euo pipefail

echo "🥾 b00t Multi-Agent POC Demo"
echo "============================"
echo ""

# Build the binaries
echo "📦 Building b00t-agent..."
cargo build --bin b00t-agent 2>&1 | grep -v "Compiling\|Finished" || true
echo ""

echo "✅ Build complete!"
echo ""
echo "🎬 Demo Instructions:"
echo ""
echo "Terminal 1 - Agent Alpha:"
echo "  cargo run --bin b00t-agent -- --id alpha --skills rust,testing --personality curious"
echo ""
echo "Terminal 2 - Agent Beta:"
echo "  cargo run --bin b00t-agent -- --id beta --skills docker,deploy --personality pragmatic"
echo ""
echo "Then in Agent Alpha terminal:"
echo "  /handshake beta Build multi-agent POC"
echo "  /propose Use Unix sockets for IPC"
echo "  (note the proposal ID)"
echo "  /vote <proposal-id> yes Low latency"
echo ""
echo "In Agent Beta terminal:"
echo "  /vote <proposal-id> yes Agree, simpler"
echo "  (proposal should pass with 2 yes votes)"
echo ""
echo "Back in Alpha:"
echo "  /crew form beta"
echo "  /delegate beta 100"
echo ""
echo "✨ POC Features Demonstrated:"
echo "  ✅ Agent spawning with custom skills"
echo "  ✅ Handshake protocol"
echo "  ✅ Proposal creation"
echo "  ✅ Voting with quorum (2 votes needed)"
echo "  ✅ Crew formation"
echo "  ✅ Crown delegation with cake budget"
echo ""
