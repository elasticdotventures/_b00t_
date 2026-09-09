#!/usr/bin/env bash
# Run ON b00t-node. Publishes the k0s cluster's OIDC discovery doc + JWKS to a
# public GCS bucket so GCP STS can verify projected ServiceAccount tokens
# (keyless WIF). Plan gleaming-jingling-nygaard.
#
#   BUCKET=b00t-k0s-oidc ./publish-oidc-discovery.sh
#
# One-time GCP setup (or in Terraform):
#   gcloud storage buckets create gs://$BUCKET --location=US --uniform-bucket-level-access
#   gcloud storage buckets add-iam-policy-binding gs://$BUCKET \
#     --member=allUsers --role=roles/storage.objectViewer
#
# Then set the k0s API server to advertise this bucket as its issuer
# (k0s.yaml -> spec.api.extraArgs):
#   service-account-issuer:  "https://storage.googleapis.com/$BUCKET"
#   service-account-jwks-uri:"https://storage.googleapis.com/$BUCKET/openid/v1/jwks"
# and set modules/gcp-build-plane var k0s_oidc_issuer_uri to that issuer URL.
#
# 🚩 This JWKS is a trust anchor. Lock bucket writes to one principal, enable
# object versioning, rotate the k0s SA signing key on a schedule.
set -euo pipefail
: "${BUCKET:?set BUCKET=<public gcs bucket name>}"

work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
kubectl get --raw /.well-known/openid-configuration > "$work/openid-configuration"
kubectl get --raw /openid/v1/jwks                   > "$work/jwks"

# discovery doc must advertise the PUBLIC issuer + jwks_uri, not the in-cluster
# API server address — rewrite them.
ISSUER="https://storage.googleapis.com/${BUCKET}"
python3 - "$work/openid-configuration" "$ISSUER" <<'PY'
import json, sys
p, issuer = sys.argv[1], sys.argv[2]
d = json.load(open(p))
d["issuer"] = issuer
d["jwks_uri"] = f"{issuer}/openid/v1/jwks"
json.dump(d, open(p, "w"))
PY

gcloud storage cp "$work/openid-configuration" "gs://${BUCKET}/.well-known/openid-configuration" \
  --content-type=application/json --cache-control="public, max-age=300"
gcloud storage cp "$work/jwks" "gs://${BUCKET}/openid/v1/jwks" \
  --content-type=application/json --cache-control="public, max-age=300"

echo "published:"
echo "  https://storage.googleapis.com/${BUCKET}/.well-known/openid-configuration"
echo "  https://storage.googleapis.com/${BUCKET}/openid/v1/jwks"
