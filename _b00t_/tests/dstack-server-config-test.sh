#!/usr/bin/env bash
# Plain bash test script for dstack-server-config recipe
# Translates the brief's two @test cases to plain if/then assertions
# Each test runs with its own fresh HOME and .env setup

set -euo pipefail

# Get the git root directory (used by the recipe)
git_root="$(cd "$(dirname "$0")/.." && git rev-parse --show-toplevel)"

# Helper function to run a test with its own HOME
run_test_scenario() {
    local test_name="$1"
    local runpod_api_key="$2"
    local gcp_project_id="$3"
    local check_gcp="$4"

    # Create fresh temporary HOME for this test
    local test_home
    test_home="$(mktemp -d)"
    export HOME="$test_home"

    echo ""
    echo "=== $test_name ==="
    echo "  HOME=$HOME (fresh)"

    # Create .env with credentials for this test scenario
    echo "RUNPOD_API_KEY=$runpod_api_key" > "$git_root/.env"
    if [ -n "$gcp_project_id" ]; then
        echo "GCP_PROJECT_ID=$gcp_project_id" >> "$git_root/.env"
    fi

    # Run the recipe (from root directory where dstack-sdd module is available)
    cd "$git_root"
    just dstack-sdd dstack-server-config

    # Verify RunPod block exists in all tests
    if ! grep -q "type: runpod" "$HOME/.dstack/server/config.yml"; then
        echo "FAIL: Could not find 'type: runpod' in config.yml"
        cat "$HOME/.dstack/server/config.yml"
        return 1
    fi
    echo "PASS: Found 'type: runpod' in config.yml"

    # If this test should verify GCP block
    if [ "$check_gcp" = "yes" ]; then
        if ! grep -q "type: gcp" "$HOME/.dstack/server/config.yml"; then
            echo "FAIL: Could not find 'type: gcp' in config.yml"
            cat "$HOME/.dstack/server/config.yml"
            return 1
        fi
        echo "PASS: Found 'type: gcp' in config.yml"

        if ! grep -q "project_id: \"$gcp_project_id\"" "$HOME/.dstack/server/config.yml"; then
            echo "FAIL: Could not find 'project_id: \"$gcp_project_id\"' in config.yml"
            cat "$HOME/.dstack/server/config.yml"
            return 1
        fi
        echo "PASS: Found 'project_id: \"$gcp_project_id\"' in config.yml"

        if ! grep -q "type: default" "$HOME/.dstack/server/config.yml"; then
            echo "FAIL: Could not find 'type: default' in config.yml"
            cat "$HOME/.dstack/server/config.yml"
            return 1
        fi
        echo "PASS: Found 'type: default' (GCP creds type) in config.yml"
    fi

    # Cleanup
    rm -rf "$test_home"
}

# Test 1: RunPod only (GCP_PROJECT_ID unset)
run_test_scenario \
    "Test 1: dstack-server-config writes a runpod backend block (runpod-only path)" \
    "test-runpod-key" \
    "" \
    "no"

# Test 2: RunPod + GCP (explicit GCP_PROJECT_ID)
run_test_scenario \
    "Test 2: dstack-server-config writes a gcp backend block using ambient ADC" \
    "test-runpod-key" \
    "test-gcp-project" \
    "yes"

echo ""
echo "=== All tests passed ==="
exit 0
