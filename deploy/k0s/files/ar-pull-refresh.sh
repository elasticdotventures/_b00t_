#!/usr/bin/env bash
# Runs in the ar-pull-refresher CronJob. Exchanges this Pod's projected
# ServiceAccount token for a short-lived GCP access token (keyless WIF), then
# writes it into the `ar-pull` dockerconfigjson Secret so the kubelet can pull
# b00t-build from Artifact Registry. No long-lived key.
#
# env (from the CronJob spec):
#   WIF_AUDIENCE   //iam.googleapis.com/projects/NUM/.../providers/k0s-oidc
#   SA_EMAIL       b00t-k0s-workload@PROJECT_ID.iam.gserviceaccount.com
#   AR_HOST        australia-southeast1-docker.pkg.dev
#   TARGET_NS      b00t-ci
#   SECRET_NAME    ar-pull
set -euo pipefail

TOKEN_FILE=/var/run/secrets/gcp/token
subj="$(cat "$TOKEN_FILE")"

fed=$(curl -sf -X POST https://sts.googleapis.com/v1/token \
  -d grant_type=urn:ietf:params:oauth:grant-type:token-exchange \
  -d requested_token_type=urn:ietf:params:oauth:token-type:access_token \
  --data-urlencode "audience=${WIF_AUDIENCE}" \
  --data-urlencode "scope=https://www.googleapis.com/auth/cloud-platform" \
  --data-urlencode "subject_token_type=urn:ietf:params:oauth:token-type:jwt" \
  --data-urlencode "subject_token=${subj}" \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["access_token"])')

access=$(curl -sf -X POST \
  "https://iamcredentials.googleapis.com/v1/projects/-/serviceAccounts/${SA_EMAIL}:generateAccessToken" \
  -H "Authorization: Bearer ${fed}" -H "Content-Type: application/json" \
  -d '{"scope":["https://www.googleapis.com/auth/cloud-platform"]}' \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["accessToken"])')

auth=$(printf 'oauth2accesstoken:%s' "$access" | base64 -w0)

kubectl create secret generic "$SECRET_NAME" -n "$TARGET_NS" \
  --type=kubernetes.io/dockerconfigjson \
  --from-literal=.dockerconfigjson="$(printf '{"auths":{"%s":{"auth":"%s"}}}' "$AR_HOST" "$auth")" \
  --dry-run=client -o yaml | kubectl apply -f -

echo "refreshed ${TARGET_NS}/${SECRET_NAME} for ${AR_HOST} (token ~55m)"
