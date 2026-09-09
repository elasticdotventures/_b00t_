#!/usr/bin/env bash
# gcs-obj.sh — dependency-free GCS object get/put/exists for the dstack build
# plane. Auth = the build VM's attached service account via the GCE metadata
# server (b00t-build-vm has storage.objectAdmin on the buildcache bucket —
# same identity sccache already uses). No gcloud, no gcsfuse, no key.
#
#   gcs-obj.sh put    <bucket> <object-name> <local-file>
#   gcs-obj.sh get    <bucket> <object-name> <local-file>
#   gcs-obj.sh exists <bucket> <object-name>            # exit 0 if present
#
# `object-name` is the full key, e.g. ci-archives/12345-1/nextest.tar.zst.
set -euo pipefail

_token() {
  curl -sf -H 'Metadata-Flavor: Google' \
    'http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token' \
    | python3 -c 'import sys,json; print(json.load(sys.stdin)["access_token"])'
}

# percent-encode every reserved char in the object path (slashes included —
# the JSON API wants the object id as one encoded path segment).
_enc() { python3 -c 'import sys,urllib.parse as u; print(u.quote(sys.argv[1], safe=""))' "$1"; }

cmd="${1:?usage: get|put|exists}"; bucket="${2:?bucket}"; obj="${3:?object-name}"

case "$cmd" in
  put)
    file="${4:?local-file}"
    curl -sf --retry 5 --retry-all-errors -X POST \
      -H "Authorization: Bearer $(_token)" \
      -H "Content-Type: application/octet-stream" \
      --data-binary @"$file" \
      "https://storage.googleapis.com/upload/storage/v1/b/${bucket}/o?uploadType=media&name=$(_enc "$obj")" \
      >/dev/null
    echo "put gs://${bucket}/${obj} ($(stat -c%s "$file") bytes)"
    ;;
  get)
    file="${4:?local-file}"
    curl -sf --retry 5 --retry-all-errors \
      -H "Authorization: Bearer $(_token)" \
      -o "$file" \
      "https://storage.googleapis.com/storage/v1/b/${bucket}/o/$(_enc "$obj")?alt=media"
    echo "get gs://${bucket}/${obj} -> $file ($(stat -c%s "$file") bytes)"
    ;;
  exists)
    code=$(curl -s -o /dev/null -w '%{http_code}' \
      -H "Authorization: Bearer $(_token)" \
      "https://storage.googleapis.com/storage/v1/b/${bucket}/o/$(_enc "$obj")")
    [ "$code" = "200" ]
    ;;
  *) echo "unknown subcommand: $cmd" >&2; exit 2 ;;
esac
