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
    local check_aws="${5:-no}"
    local check_azure="${6:-no}"

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
    # Azure credentials are always required by the recipe's hard-fail checks
    echo "AZURE_TENANT_ID=test-tenant-id" >> "$git_root/.env"
    echo "AZURE_SUBSCRIPTION_ID=test-subscription-id" >> "$git_root/.env"

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

    # If this test should verify AWS block
    if [ "$check_aws" = "yes" ]; then
        if ! grep -q "type: aws" "$HOME/.dstack/server/config.yml"; then
            echo "FAIL: Could not find 'type: aws' in config.yml"
            cat "$HOME/.dstack/server/config.yml"
            return 1
        fi
        echo "PASS: Found 'type: aws' in config.yml"

        # Scoped check: verify AWS block uses 'type: default' credentials
        if ! grep -A2 "type: aws" "$HOME/.dstack/server/config.yml" | grep -q "type: default"; then
            echo "FAIL: AWS block does not use 'type: default' credentials"
            cat "$HOME/.dstack/server/config.yml"
            return 1
        fi
        echo "PASS: AWS block uses 'type: default' credentials"
    fi

    # If this test should verify Azure block
    if [ "$check_azure" = "yes" ]; then
        if ! grep -q "type: azure" "$HOME/.dstack/server/config.yml"; then
            echo "FAIL: Could not find 'type: azure' in config.yml"
            cat "$HOME/.dstack/server/config.yml"
            return 1
        fi
        echo "PASS: Found 'type: azure' in config.yml"

        # Scoped check: verify Azure block contains expected tenant_id value
        if ! grep -A5 "type: azure" "$HOME/.dstack/server/config.yml" | grep -q 'tenant_id: "test-tenant-id"'; then
            echo "FAIL: Azure block missing expected tenant_id value"
            cat "$HOME/.dstack/server/config.yml"
            return 1
        fi
        echo "PASS: Azure tenant_id matches expected value"

        # Scoped check: verify Azure block contains expected subscription_id value
        if ! grep -A5 "type: azure" "$HOME/.dstack/server/config.yml" | grep -q 'subscription_id: "test-subscription-id"'; then
            echo "FAIL: Azure block missing expected subscription_id value"
            cat "$HOME/.dstack/server/config.yml"
            return 1
        fi
        echo "PASS: Azure subscription_id matches expected value"

        # Scoped check: verify Azure block uses 'type: default' credentials
        if ! grep -A5 "type: azure" "$HOME/.dstack/server/config.yml" | grep -q "type: default"; then
            echo "FAIL: Azure block does not use 'type: default' credentials"
            cat "$HOME/.dstack/server/config.yml"
            return 1
        fi
        echo "PASS: Azure block uses 'type: default' credentials"
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

# Test 3: RunPod + GCP + AWS (three backends)
run_test_scenario \
    "Test 3: dstack-server-config writes an aws backend block" \
    "test-runpod-key" \
    "test-gcp-project" \
    "yes" \
    "yes"

# Test 4: RunPod + GCP + AWS + Azure (all four backends)
run_test_scenario \
    "Test 4: dstack-server-config writes an azure backend block" \
    "test-runpod-key" \
    "test-gcp-project" \
    "yes" \
    "yes" \
    "yes"

echo ""
echo "=== All tests passed ==="
exit 0
