#!/usr/bin/env bash
# build-plane-enroll-repo.sh — enrol GitHub repos onto the GCP dstack build plane.
#
# Personal-account repos (owner `elasticdotventures`) have NO org-level Actions
# secrets, so the two settings `ci-build-plane.yml` needs are written per-repo:
#   variable DSTACK_URL         — the scale-to-zero waker endpoint
#   secret   DSTACK_ADMIN_TOKEN — the dstack server project admin token
# Optionally also creates the `build-plane-ci` PR label the workflow keys off.
#
# Deliberately selective: the build plane only pays off for repos whose cold
# `cargo build` exceeds ~10 min. Do NOT fan this out across every repo.
#
#   scripts/build-plane-enroll-repo.sh [--check] [--label] REPO [REPO ...]
#
#   REPO                owner/repo (e.g. elasticdotventures/_b00t_)
#   --check             print current state for each repo, change nothing
#   --label             also `gh label create build-plane-ci` (idempotent)
#   --url URL           override DSTACK_URL (default: tofu output from b00t-tf)
#
# Token source, in order: $DSTACK_ADMIN_TOKEN, then $DSTACK_ADMIN_TOKEN_FILE,
# then an interactive prompt. Never baked into this file.
set -euo pipefail

REPO_ROOT="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
CHECK=0 LABEL=0 URL_OVERRIDE=""
REPOS=()

while [ $# -gt 0 ]; do
  case "$1" in
    --check) CHECK=1 ;;
    --label) LABEL=1 ;;
    --url) URL_OVERRIDE="${2:?--url needs a value}"; shift ;;
    -h|--help) sed -n '2,26p' "${BASH_SOURCE[0]}"; exit 0 ;;
    -*) echo "unknown flag: $1" >&2; exit 2 ;;
    */*) REPOS+=("$1") ;;
    *) echo "not an owner/repo: $1" >&2; exit 2 ;;
  esac
  shift
done

[ "${#REPOS[@]}" -gt 0 ] || { echo "no repos given" >&2; exit 2; }
command -v gh >/dev/null || { echo "gh CLI not found" >&2; exit 1; }

resolve_url() {
  [ -n "$URL_OVERRIDE" ] && { printf '%s' "$URL_OVERRIDE"; return; }
  ( cd "$REPO_ROOT/b00t-tf" && tofu output -raw gcp_control_node_endpoint )
}

resolve_token() {
  if [ -n "${DSTACK_ADMIN_TOKEN:-}" ]; then printf '%s' "$DSTACK_ADMIN_TOKEN"; return; fi
  if [ -n "${DSTACK_ADMIN_TOKEN_FILE:-}" ]; then tr -d '\n' < "$DSTACK_ADMIN_TOKEN_FILE"; return; fi
  local t; read -rsp "dstack admin token: " t </dev/tty; echo >&2
  printf '%s' "$t"
}

if [ "$CHECK" -eq 1 ]; then
  for r in "${REPOS[@]}"; do
    echo "── $r"
    gh variable list --repo "$r" 2>/dev/null | grep -E '^DSTACK_URL\b' || echo "  DSTACK_URL         (unset)"
    gh secret   list --repo "$r" 2>/dev/null | grep -E '^DSTACK_ADMIN_TOKEN\b' || echo "  DSTACK_ADMIN_TOKEN (unset)"
    gh label list --repo "$r" --search build-plane-ci 2>/dev/null | grep -q build-plane-ci \
      && echo "  label build-plane-ci present" || echo "  label build-plane-ci (absent)"
  done
  exit 0
fi

URL="$(resolve_url)"; [ -n "$URL" ] || { echo "could not resolve DSTACK_URL" >&2; exit 1; }
TOKEN="$(resolve_token)"; [ -n "$TOKEN" ] || { echo "empty token" >&2; exit 1; }
echo "DSTACK_URL = $URL"

for r in "${REPOS[@]}"; do
  echo "── enrolling $r"
  gh variable set DSTACK_URL         --repo "$r" --body "$URL"
  printf '%s' "$TOKEN" | gh secret set DSTACK_ADMIN_TOKEN --repo "$r"
  if [ "$LABEL" -eq 1 ]; then
    gh label create build-plane-ci --repo "$r" --color BFD4F2 \
      --description "run CI on the GCP dstack build plane" 2>/dev/null \
      || echo "  label build-plane-ci already exists"
  fi
  echo "  ✅ DSTACK_URL + DSTACK_ADMIN_TOKEN set"
done
