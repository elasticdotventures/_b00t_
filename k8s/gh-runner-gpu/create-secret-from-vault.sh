#!/usr/bin/env bash
# Creates the `gh-runner-gpu-creds` secret (GitHub App auth variant) from
# this box's already-provisioned Azure Key Vault credentials, instead of a
# human pasting a PAT.
#
# Prerequisites, all already live on this box (sm3llsl1k3s0ld3r) as of
# 2026-08-31 — see PromptExecution/infrastructure#151 (SPIRE workload
# identity) and docs/github-app-b00t-arc-runners.md in that same repo:
#   - spire-agent running locally, registered as
#     spiffe://promptexecution.com/agent/sm3lly
#   - that identity federated to the Entra app `b00t-agent-sm3lly`
#     (994d6c44-8593-4203-b133-7e69f7c86604), which holds
#     `Key Vault Secrets User` on `kv-pe-agent-secrets`
#   - the b00t-arc-runners GitHub App's credentials written there as
#     `b00t-arc-runners-app-id`, `b00t-arc-runners-installation-id-b00t`,
#     `b00t-arc-runners-private-key` (see `just agent_secrets::write-b00t-arc-runners-secrets`
#     in PromptExecution/infrastructure)
#
# This script does NOT run `helm install` — it only creates the
# prerequisite secret. See README.md for the install step, still a
# separate, deliberate human action.
set -euo pipefail

VAULT="kv-pe-agent-secrets"
AZ_SP_APP_ID="994d6c44-8593-4203-b133-7e69f7c86604"  # b00t-agent-sm3lly
AZ_TENANT_ID="1fd87b50-f47c-4023-aad1-50c18cad799d"   # promptexecution.com
NAMESPACE="arc-runners"
SECRET_NAME="gh-runner-gpu-creds"

echo "🔑 Fetching JWT-SVID for spiffe://promptexecution.com/agent/sm3lly ..."
JWT_SVID=$(spire-agent api fetch jwt \
  -audience api://AzureADTokenExchange \
  -socketPath "$XDG_RUNTIME_DIR/spire-agent/public/api.sock" \
  -output json | python3 -c "import json,sys; print(json.load(sys.stdin)[0]['svids'][0]['svid'])")

echo "🔑 Exchanging for an Azure AD token (az login --service-principal --federated-token) ..."
az login --service-principal \
  -u "$AZ_SP_APP_ID" \
  --federated-token "$JWT_SVID" \
  --tenant "$AZ_TENANT_ID" \
  -o none

echo "🔑 Reading b00t-arc-runners credentials from $VAULT ..."
APP_ID=$(az keyvault secret show --vault-name "$VAULT" --name b00t-arc-runners-app-id --query value -o tsv)
INSTALLATION_ID=$(az keyvault secret show --vault-name "$VAULT" --name b00t-arc-runners-installation-id-b00t --query value -o tsv)
PRIVATE_KEY=$(az keyvault secret show --vault-name "$VAULT" --name b00t-arc-runners-private-key --query value -o tsv)

echo "📦 Creating $NAMESPACE/$SECRET_NAME (GitHub App auth) ..."
k0s kubectl create namespace "$NAMESPACE" --dry-run=client -o yaml | k0s kubectl apply -f -
k0s kubectl create secret generic "$SECRET_NAME" \
  --namespace "$NAMESPACE" \
  --from-literal=github_app_id="$APP_ID" \
  --from-literal=github_app_installation_id="$INSTALLATION_ID" \
  --from-literal=github_app_private_key="$PRIVATE_KEY" \
  --dry-run=client -o yaml | k0s kubectl apply -f -

echo "✅ $NAMESPACE/$SECRET_NAME created from Key Vault — no token ever touched disk as a file."
