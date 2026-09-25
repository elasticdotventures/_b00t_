#!/usr/bin/env bash
# Creates the `gh-runner-creds` secret (GitHub App auth) on the GKE Autopilot `arc-runners` cluster,
# from this box's already-provisioned Azure Key Vault credentials — same SPIRE-federated fetch chain
# as ../gh-runner-gpu/create-secret-from-vault.sh (which this script is a re-target of), just applied
# against a real kubectl context instead of the local `k0s kubectl`. One secret backs both
# cpu-builds.values.yaml and gpu-builds.values.yaml (renamed from the single-scale-set
# `gh-runner-gpu-creds` since it's no longer GPU-specific).
#
# Prerequisites (unchanged from the sm3lly PoC — see docs/github-app-b00t-arc-runners.md in
# PromptExecution/infrastructure):
#   - spire-agent running locally on this box, registered as spiffe://promptexecution.com/agent/sm3lly
#   - that identity federated to the Entra app `b00t-agent-sm3lly`
#     (994d6c44-8593-4203-b133-7e69f7c86604), which holds `Key Vault Secrets User` on
#     `kv-pe-agent-secrets`
#   - the b00t-arc-runners GitHub App's credentials already written there as
#     `b00t-arc-runners-app-id`, `b00t-arc-runners-installation-id-b00t`,
#     `b00t-arc-runners-private-key`
#
# New prerequisite for THIS variant: a real kubeconfig context for the GKE cluster, i.e.
#   gcloud container clusters get-credentials arc-runners \
#     --zone <regionzone from infrastructure/terraform/google/main-gcloud.tf's local.c0ntext> \
#     --project promptexecution
# which itself needs `gcloud auth login` (interactive) to have been run first if the ADC token has
# expired.
#
# This script does NOT run `helm install` — it only creates the prerequisite secret, same convention
# as the PoC it's adapted from.
set -euo pipefail

VAULT="kv-pe-agent-secrets"
AZ_SP_APP_ID="994d6c44-8593-4203-b133-7e69f7c86604"  # b00t-agent-sm3lly
AZ_TENANT_ID="1fd87b50-f47c-4023-aad1-50c18cad799d"   # promptexecution.com
NAMESPACE="arc-runners"
SECRET_NAME="gh-runner-creds"

# Real kubectl against a real context, not the local k0s box — must be passed explicitly so this
# script can never accidentally apply against whatever context happens to be current.
: "${GKE_KUBECTL_CONTEXT:?Set GKE_KUBECTL_CONTEXT to the gke_<project>_<zone>_<cluster> context name (see: kubectl config get-contexts)}"

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

echo "📦 Creating $NAMESPACE/$SECRET_NAME (GitHub App auth) on context $GKE_KUBECTL_CONTEXT ..."
kubectl --context "$GKE_KUBECTL_CONTEXT" create namespace "$NAMESPACE" --dry-run=client -o yaml \
  | kubectl --context "$GKE_KUBECTL_CONTEXT" apply -f -
kubectl --context "$GKE_KUBECTL_CONTEXT" create secret generic "$SECRET_NAME" \
  --namespace "$NAMESPACE" \
  --from-literal=github_app_id="$APP_ID" \
  --from-literal=github_app_installation_id="$INSTALLATION_ID" \
  --from-literal=github_app_private_key="$PRIVATE_KEY" \
  --dry-run=client -o yaml \
  | kubectl --context "$GKE_KUBECTL_CONTEXT" apply -f -

echo "✅ $NAMESPACE/$SECRET_NAME created from Key Vault on $GKE_KUBECTL_CONTEXT — no token ever touched disk as a file."
