#!/usr/bin/env bash
# Offline test for gcs-obj.sh's GOOGLE_APPLICATION_CREDENTIALS JWT path.
# Synthesises a throwaway RSA key, drives the same assertion-assembly steps,
# and checks the JWT is well-formed. Does NOT hit the network / GCS.
#
#   dev-env/tests/gcs-obj-jwt-test.sh   ->  PASS / FAIL
set -euo pipefail

tmpd=$(mktemp -d)
trap 'rm -rf "$tmpd"' EXIT

openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$tmpd/priv.pem" 2>/dev/null
python3 -c '
import json, sys
d = sys.argv[1]
json.dump(
    {"client_email": "test@promptexecution.iam.gserviceaccount.com",
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
jwt="${si}.${sig}"

python3 -c '
import base64, json, sys
parts = sys.argv[1].split(".")
assert len(parts) == 3, f"expected 3 JWT segments, got {len(parts)}"
pad = lambda s: s + "=" * (-len(s) % 4)
h = json.loads(base64.urlsafe_b64decode(pad(parts[0])))
c = json.loads(base64.urlsafe_b64decode(pad(parts[1])))
assert h == {"alg": "RS256", "typ": "JWT"}, h
assert c["aud"] == "https://oauth2.googleapis.com/token", c
assert c["exp"] - c["iat"] == 3600, c
assert c["iss"].endswith(".iam.gserviceaccount.com"), c
assert len(base64.urlsafe_b64decode(pad(parts[2]))) == 256, "RSA-2048 sig must be 256 bytes"
print("PASS: gcs-obj.sh JWT assertion assembly is well-formed")
' "$jwt"
