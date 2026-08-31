# =============================================================================
# gh-runner-gpu — GPU self-hosted CI runner (ARC) for elasticdotventures/_b00t_
# =============================================================================
# Memoized for posterity: the exact commands used 2026-08-31 to stand up
# this box's first GPU CI runner. See README.md in this directory for full
# context/security tradeoffs, and PromptExecution/infrastructure's
# docs/github-app-b00t-arc-runners.md for where the credentials come from.
#
# Run these via `b00t sh` (audited exec), not raw shell — both mutate a
# live cluster / mint tokens against a live GitHub App installation.
# =============================================================================

# Step 1: create arc-runners/gh-runner-gpu-creds from Azure Key Vault via
# this box's SPIRE workload identity. Safe to re-run (idempotent — it's a
# `kubectl ... --dry-run=client -o yaml | kubectl apply -f -` under the hood).
create-secret:
    #!/usr/bin/env bash
    set -euo pipefail
    bash "{{source_directory()}}/create-secret-from-vault.sh"

# Step 2: install the AutoscalingRunnerSet. Requires create-secret to have
# run first (values.yaml references the secret it creates). NOT idempotent
# in the naive sense — re-running against an existing release does a Helm
# upgrade, which is fine, but this isn't a from-scratch-safe recipe the way
# create-secret is.
install:
    #!/usr/bin/env bash
    set -euo pipefail
    helm install gh-runner-gpu \
        oci://ghcr.io/actions/actions-runner-controller-charts/gha-runner-scale-set \
        --version 0.14.2 \
        --namespace arc-runners \
        -f "{{source_directory()}}/values.yaml"

# Both steps together, in order.
up: create-secret install

# Quick health check: the AutoscalingRunnerSet resource, its pods, and the
# Helm release status.
status:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== helm release ==="
    helm list -n arc-runners
    echo "=== autoscalingrunnersets ==="
    k0s kubectl get autoscalingrunnersets -n arc-runners -o wide
    echo "=== pods ==="
    k0s kubectl get pods -n arc-runners

# Tear down the runner set (does NOT delete the gh-runner-gpu-creds secret
# or the arc-runners namespace — rerun create-secret to recreate it, or
# delete those manually if you want a full clean slate).
down:
    #!/usr/bin/env bash
    set -euo pipefail
    helm uninstall gh-runner-gpu --namespace arc-runners
