#!/usr/bin/env bash
# Plain bash test script for dstack-server-config recipe
# Translates the brief's two @test cases to plain if/then assertions

set -euo pipefail

# Setup: create temporary HOME and export test credentials
export HOME="$(mktemp -d)"
export RUNPOD_API_KEY="test-runpod-key"
export GCP_PROJECT_ID="test-gcp-project"

# Move to the _b00t_ directory (parent of tests/)
cd "$(dirname "$0")/.."

# Get the git root directory
git_root="$(git rev-parse --show-toplevel)"

# Create .env file with required credentials in git root
echo "RUNPOD_API_KEY=$RUNPOD_API_KEY" > "$git_root/.env"
echo "GCP_PROJECT_ID=$GCP_PROJECT_ID" >> "$git_root/.env"

echo "=== Test 1: dstack-server-config writes a runpod backend block ==="
just dstack-sdd dstack-server-config
if ! grep -q "type: runpod" ~/.dstack/server/config.yml; then
    echo "FAIL: Could not find 'type: runpod' in config.yml"
    cat ~/.dstack/server/config.yml
    exit 1
fi
echo "PASS: Found 'type: runpod' in config.yml"

echo ""
echo "=== Test 2: dstack-server-config writes a gcp backend block using ambient ADC ==="
# Re-run with both credentials set
just dstack-sdd dstack-server-config
if ! grep -q "type: gcp" ~/.dstack/server/config.yml; then
    echo "FAIL: Could not find 'type: gcp' in config.yml"
    cat ~/.dstack/server/config.yml
    exit 1
fi
echo "PASS: Found 'type: gcp' in config.yml"

if ! grep -q "project_id: \"$GCP_PROJECT_ID\"" ~/.dstack/server/config.yml; then
    echo "FAIL: Could not find 'project_id: \"$GCP_PROJECT_ID\"' in config.yml"
    cat ~/.dstack/server/config.yml
    exit 1
fi
echo "PASS: Found 'project_id: \"$GCP_PROJECT_ID\"' in config.yml"

if ! grep -q "type: default" ~/.dstack/server/config.yml; then
    echo "FAIL: Could not find 'type: default' in config.yml"
    cat ~/.dstack/server/config.yml
    exit 1
fi
echo "PASS: Found 'type: default' in config.yml"

echo ""
echo "=== All tests passed ==="
exit 0
