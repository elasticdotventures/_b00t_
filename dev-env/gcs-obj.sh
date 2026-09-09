#!/usr/bin/env bash
# gcs-obj.sh — dependency-free GCS object get/put/exists for the dstack plane.
#
#   gcs-obj.sh put    <bucket> <object-name> <local-file>
#   gcs-obj.sh get    <bucket> <object-name> <local-file>
#   gcs-obj.sh exists <bucket> <object-name>            # exit 0 if present
#
# Auth, in order:
#   1. GCE metadata server  — the build VM's attached SA (GCP backend; sccache
#      uses the same identity).
#   2. GOOGLE_APPLICATION_CREDENTIALS — a service-account key JSON. Used on the
#      kubernetes backend (k0s on Vultr has no GCE metadata). Token is minted
#      locally via a signed JWT bearer grant (openssl + curl, no gcloud).
set -euo pipefail

_SCOPE="https://www.googleapis.com/auth/devstorage.read_write"

_b64url() { openssl base64 -A | tr '+/' '-_' | tr -d '='; }

_token_metadata() {
  curl -sf --max-time 3 -H 'Metadata-Flavor: Google' \
    'http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token' \
    | python3 -c 'import sys,json; print(json.load(sys.stdin)["access_token"])'
}

_token_sa_key() {
  local key="${GOOGLE_APPLICATION_CREDENTIALS:-}"
  [ -n "$key" ] && [ -f "$key" ] || return 1
  local iss pem now hdr clm si sig jwt
  iss=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["client_email"])' "$key")
  pem=$(mktemp); python3 -c 'import json,sys;open(sys.argv[2],"w").write(json.load(open(sys.argv[1]))["private_key"])' "$key" "$pem"
  now=$(date +%s)
  hdr=$(printf '{"alg":"RS256","typ":"JWT"}' | _b64url)
  clm=$(printf '{"iss":"%s","scope":"%s","aud":"https://oauth2.googleapis.com/token","iat":%d,"exp":%d}' \
        "$iss" "$_SCOPE" "$now" "$((now + 3600))" | _b64url)
  si="${hdr}.${clm}"
  sig=$(printf '%s' "$si" | openssl dgst -sha256 -sign "$pem" | _b64url)
  rm -f "$pem"
  jwt="${si}.${sig}"
  curl -sf --retry 3 --retry-all-errors -X POST https://oauth2.googleapis.com/token \
    -d grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer \
    --data-urlencode "assertion=${jwt}" \
    | python3 -c 'import sys,json; print(json.load(sys.stdin)["access_token"])'
}

_token() {
  _token_metadata 2>/dev/null || _token_sa_key || {
    echo "gcs-obj: no usable credentials (GCE metadata + GOOGLE_APPLICATION_CREDENTIALS both failed)" >&2
    return 1
  }
}

_enc() { python3 -c 'import sys,urllib.parse as u; print(u.quote(sys.argv[1], safe=""))' "$1"; }

cmd="${1:?usage: get|put|exists}"; bucket="${2:?bucket}"; obj="${3:?object-name}"
TOKEN="$(_token)"

case "$cmd" in
  put)
    file="${4:?local-file}"
    curl -sf --retry 5 --retry-all-errors -X POST \
      -H "Authorization: Bearer $TOKEN" \
      -H "Content-Type: application/octet-stream" \
      --data-binary @"$file" \
      "https://storage.googleapis.com/upload/storage/v1/b/${bucket}/o?uploadType=media&name=$(_enc "$obj")" \
      >/dev/null
    echo "put gs://${bucket}/${obj} ($(stat -c%s "$file") bytes)"
    ;;
  get)
    file="${4:?local-file}"
    curl -sf --retry 5 --retry-all-errors \
      -H "Authorization: Bearer $TOKEN" \
      -o "$file" \
      "https://storage.googleapis.com/storage/v1/b/${bucket}/o/$(_enc "$obj")?alt=media"
    echo "get gs://${bucket}/${obj} -> $file ($(stat -c%s "$file") bytes)"
    ;;
  exists)
    code=$(curl -s -o /dev/null -w '%{http_code}' \
      -H "Authorization: Bearer $TOKEN" \
      "https://storage.googleapis.com/storage/v1/b/${bucket}/o/$(_enc "$obj")")
    [ "$code" = "200" ]
    ;;
  *) echo "unknown subcommand: $cmd" >&2; exit 2 ;;
esac
