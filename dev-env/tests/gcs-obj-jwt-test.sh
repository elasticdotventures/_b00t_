#!/usr/bin/env bash
# Offline tests for gcs-obj.sh credential handling. No network / GCS.
#   1. service_account key -> signed JWT bearer assertion is well-formed
#   2. external_account (WIF) config -> the right branch is selected and the
#      STS token-exchange request is assembled from the config + subject token
#
#   dev-env/tests/gcs-obj-jwt-test.sh   ->  PASS / FAIL
set -euo pipefail

tmpd=$(mktemp -d)
trap 'rm -rf "$tmpd"' EXIT
SRC="$(cd "$(dirname "$0")/.." && pwd)/gcs-obj.sh"

# ─── 1. service_account key: JWT bearer assertion ──────────────────────────
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$tmpd/priv.pem" 2>/dev/null
python3 -c '
import json, sys
d = sys.argv[1]
json.dump(
    {"type": "service_account",
     "client_email": "test@promptexecution.iam.gserviceaccount.com",
     "private_key": open(f"{d}/priv.pem").read()},
    open(f"{d}/sa.json", "w"),
)' "$tmpd"

SCOPE="https://www.googleapis.com/auth/devstorage.read_write"
b64url() { openssl base64 -A | tr '+/' '-_' | tr -d '='; }
key="$tmpd/sa.json"
iss=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["client_email"])' "$key")
pem="$tmpd/extracted.pem"
python3 -c 'import json,sys;open(sys.argv[2],"w").write(json.load(open(sys.argv[1]))["private_key"])' "$key" "$pem"
now=$(date +%s)
hdr=$(printf '{"alg":"RS256","typ":"JWT"}' | b64url)
clm=$(printf '{"iss":"%s","scope":"%s","aud":"https://oauth2.googleapis.com/token","iat":%d,"exp":%d}' \
      "$iss" "$SCOPE" "$now" "$((now + 3600))" | b64url)
si="${hdr}.${clm}"
sig=$(printf '%s' "$si" | openssl dgst -sha256 -sign "$pem" | b64url)
python3 -c '
import base64, json, sys
p = sys.argv[1].split(".")
assert len(p) == 3, f"expected 3 JWT segments, got {len(p)}"
pad = lambda s: s + "=" * (-len(s) % 4)
assert json.loads(base64.urlsafe_b64decode(pad(p[0]))) == {"alg": "RS256", "typ": "JWT"}
c = json.loads(base64.urlsafe_b64decode(pad(p[1])))
assert c["aud"] == "https://oauth2.googleapis.com/token"
assert c["exp"] - c["iat"] == 3600
assert len(base64.urlsafe_b64decode(pad(p[2]))) == 256
print("PASS [1] service_account: JWT bearer assertion well-formed")
' "${si}.${sig}"

# ─── 2. external_account (WIF): branch selection + STS request assembly ────
printf 'header.payload.sig-fake-projected-k8s-sa-jwt' > "$tmpd/token"
python3 -c '
import json, sys
d = sys.argv[1]
json.dump({
    "type": "external_account",
    "audience": "//iam.googleapis.com/projects/308167228204/locations/global/workloadIdentityPools/k0s/providers/b00t-node",
    "subject_token_type": "urn:ietf:params:oauth:token-type:jwt",
    "token_url": "https://sts.googleapis.com/v1/token",
    "credential_source": {"file": f"{d}/token"},
    "service_account_impersonation_url":
        "https://iamcredentials.googleapis.com/v1/projects/-/serviceAccounts/b00t-k0s-gcsread@promptexecution.iam.gserviceaccount.com:generateAccessToken",
}, open(f"{d}/wif.json", "w"))' "$tmpd"

# assert gcs-obj.sh routes external_account -> _token_external_account (not the
# key path): stub curl so no network is hit, capture the STS request.
cat > "$tmpd/curl" <<'STUB'
#!/usr/bin/env bash
# record args + body, emit fake token JSON so the pipeline continues.
echo "$@" >> "$CURL_LOG"
for a in "$@"; do case "$a" in *subject_token=*) echo "${a}" >> "$CURL_LOG";; esac; done
if printf '%s\n' "$@" | grep -q 'metadata.google.internal'; then exit 7;   # no GCE metadata off-GCP
elif printf '%s\n' "$@" | grep -q 'sts.googleapis.com'; then echo '{"access_token":"fed-xyz"}';
elif printf '%s\n' "$@" | grep -q 'generateAccessToken'; then echo '{"accessToken":"imp-xyz"}';
elif printf '%s\n' "$@" | grep -q 'storage.googleapis.com'; then :;   # `exists` HEAD-ish call
else echo '{}'; fi
STUB
chmod +x "$tmpd/curl"

export GOOGLE_APPLICATION_CREDENTIALS="$tmpd/wif.json" CURL_LOG="$tmpd/curl.log" PATH="$tmpd:$PATH"
: > "$CURL_LOG"
# `exists` on a bogus bucket — we only care that token acquisition took the WIF path
OUT=$(bash "$SRC" exists dummy-bucket dummy-obj 2>&1 || true)

python3 -c '
import sys
log = open(sys.argv[1]).read()
assert "sts.googleapis.com/v1/token" in log, "STS endpoint not called -> wrong branch"
assert "grant-type:token-exchange" in log, "not a token-exchange grant"
assert "workloadIdentityPools/k0s/providers/b00t-node" in log, "audience not passed"
assert "sig-fake-projected-k8s-sa-jwt" in log, "projected subject token not read from credential_source.file"
assert "generateAccessToken" in log, "SA impersonation step not reached"
print("PASS [2] external_account: WIF branch selected, STS token-exchange assembled from config + projected token")
' "$CURL_LOG"

echo "PASS: gcs-obj.sh credential handling (service_account + external_account/WIF)"
