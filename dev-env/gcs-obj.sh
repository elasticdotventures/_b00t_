#!/usr/bin/env bash
# gcs-obj.sh — dependency-free GCS object get/put/exists for the dstack plane.
#
#   gcs-obj.sh put    <bucket> <object-name> <local-file>
#   gcs-obj.sh get    <bucket> <object-name> <local-file>
#   gcs-obj.sh exists <bucket> <object-name>            # exit 0 if present
#
# Auth, in order of preference (first that works wins):
#   1. GCE metadata server — the VM's attached SA (GCP backend; sccache uses
#      the same identity). No config, no key.
#   2. GOOGLE_APPLICATION_CREDENTIALS = an *external_account* config
#      (Workload Identity Federation). KEYLESS: exchanges a projected token
#      (e.g. a k8s ServiceAccount JWT) at STS for a federated/impersonated
#      access token. This is the k0s path — no long-lived key.
#   3. GOOGLE_APPLICATION_CREDENTIALS = a *service_account* key JSON.
#      Long-lived key; bootstrap only. Token minted via a signed JWT bearer
#      grant (openssl + curl).
set -euo pipefail

_SCOPE="https://www.googleapis.com/auth/devstorage.read_write"
_b64url() { openssl base64 -A | tr '+/' '-_' | tr -d '='; }
_jget() { python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(d.get(sys.argv[2],""))' "$1" "$2"; }
_pluck() { python3 -c 'import sys,json; print(json.load(sys.stdin).get(sys.argv[1],""))' "$1"; }

_token_metadata() {
  curl -sf --max-time 3 -H 'Metadata-Flavor: Google' \
    'http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token' \
    | _pluck access_token
}

# WIF: projected subject token -> STS federated token -> (optional) impersonated SA token.
_token_external_account() {
  local cfg="${GOOGLE_APPLICATION_CREDENTIALS:-}"
  [ -n "$cfg" ] && [ -f "$cfg" ] || return 1
  [ "$(_jget "$cfg" type)" = "external_account" ] || return 1

  local aud token_url subj_file subj_type subj imp fed
  aud=$(_jget "$cfg" audience)
  token_url=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1])).get("token_url","https://sts.googleapis.com/v1/token"))' "$cfg")
  subj_type=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1])).get("subject_token_type","urn:ietf:params:oauth:token-type:jwt"))' "$cfg")
  subj_file=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["credential_source"]["file"])' "$cfg")
  imp=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1])).get("service_account_impersonation_url",""))' "$cfg")
  subj=$(cat "$subj_file")

  fed=$(curl -sf --retry 3 --retry-all-errors -X POST "$token_url" \
    -d grant_type=urn:ietf:params:oauth:grant-type:token-exchange \
    -d requested_token_type=urn:ietf:params:oauth:token-type:access_token \
    --data-urlencode "audience=${aud}" \
    --data-urlencode "scope=https://www.googleapis.com/auth/cloud-platform" \
    --data-urlencode "subject_token_type=${subj_type}" \
    --data-urlencode "subject_token=${subj}" \
    | _pluck access_token)
  [ -n "$fed" ] || return 1

  if [ -n "$imp" ]; then
    curl -sf --retry 3 --retry-all-errors -X POST "$imp" \
      -H "Authorization: Bearer ${fed}" -H "Content-Type: application/json" \
      -d "{\"scope\":[\"${_SCOPE}\"]}" \
      | _pluck accessToken
  else
    printf '%s' "$fed"   # direct principalSet:// grant on the bucket
  fi
}

# Long-lived SA key: signed JWT bearer grant.
_token_sa_key() {
  local key="${GOOGLE_APPLICATION_CREDENTIALS:-}"
  [ -n "$key" ] && [ -f "$key" ] || return 1
  [ "$(_jget "$key" type)" = "service_account" ] || return 1
  local iss pem now hdr clm si sig jwt
  iss=$(_jget "$key" client_email)
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
    | _pluck access_token
}

_token() {
  local t
  t=$(_token_metadata 2>/dev/null)        && [ -n "$t" ] && { printf '%s' "$t"; return 0; }
  t=$(_token_external_account 2>/dev/null) && [ -n "$t" ] && { printf '%s' "$t"; return 0; }
  t=$(_token_sa_key 2>/dev/null)           && [ -n "$t" ] && { printf '%s' "$t"; return 0; }
  echo "gcs-obj: no usable credentials (metadata / external_account / service_account key all failed)" >&2
  return 1
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
    # a name ending in "/" is a prefix, not an object — check the listing is non-empty
    case "$obj" in
      */)
        n=$(curl -sf -H "Authorization: Bearer $TOKEN" \
          "https://storage.googleapis.com/storage/v1/b/${bucket}/o?maxResults=1&prefix=$(_enc "$obj")" \
          | python3 -c 'import sys,json; print(len(json.load(sys.stdin).get("items",[])))')
        [ "${n:-0}" -gt 0 ]
        ;;
      *)
        code=$(curl -s -o /dev/null -w '%{http_code}' \
          -H "Authorization: Bearer $TOKEN" \
          "https://storage.googleapis.com/storage/v1/b/${bucket}/o/$(_enc "$obj")")
        [ "$code" = "200" ]
        ;;
    esac
    ;;
  *) echo "unknown subcommand: $cmd" >&2; exit 2 ;;
esac
