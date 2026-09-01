#!/usr/bin/env bash
# setup-runner.sh — reviewable installer for a bare-metal GitHub Actions
# self-hosted runner, using the official `actions/runner` release tarball's
# own config.sh/svc.sh flow.
#
# THIS IS A REVIEWABLE ARTIFACT. It is not executed as part of this PR.
# It has not been run against this host. No runner has been installed,
# configured, or registered by producing this file.
#
# What this script does when run by a human, in order:
#   1. Downloads a pinned actions-runner-linux-x64 release tarball and
#      verifies its sha256 checksum.
#   2. Extracts it to $GH_RUNNER_HOME (default /opt/gh-runner-gpu), unless
#      already extracted there.
#   3. Runs the runner's own ./config.sh to register it with GitHub, unless
#      it looks already configured (.runner file present).
#   4. Prints (but does NOT run) the sudo ./svc.sh install/start commands
#      needed to actually create and start the systemd unit — those require
#      root and are left as an explicit final step for a human.
#
# It deliberately does NOT hand-write a systemd unit file: actions/runner's
# own ./svc.sh generates and installs the unit (typically at
# /etc/systemd/system/actions.runner.<org>-<repo>.<name>.service), and that
# is the officially-supported way to do this. Hand-rolling a unit risks
# drifting from what a future runner self-update expects.
#
# Required environment variables (see environment.example):
#   GH_RUNNER_URL      Repo or org URL to register against.
#   GH_RUNNER_TOKEN     Short-lived runner REGISTRATION token (not a PAT).
#   GH_RUNNER_LABELS    Comma-separated labels, e.g. self-hosted,gpu,sm3lly
#   GH_RUNNER_NAME      Name this runner registers as.
# Optional:
#   GH_RUNNER_HOME      Install directory (default /opt/gh-runner-gpu).
#   GH_RUNNER_VERSION   actions/runner release version (default pinned below).

set -euo pipefail

# --- Pinned release (bump deliberately; verify the sha256 changes with it) ---
RUNNER_VERSION="${GH_RUNNER_VERSION:-2.337.0}"
RUNNER_ARCH="linux-x64"
RUNNER_TARBALL="actions-runner-${RUNNER_ARCH}-${RUNNER_VERSION}.tar.gz"
RUNNER_URL="https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/${RUNNER_TARBALL}"
# sha256 of actions-runner-linux-x64-2.337.0.tar.gz, from the GitHub Releases
# API asset digest at the time this script was written. Re-verify (and
# update GH_RUNNER_VERSION + this hash together) before using a newer
# release: `gh api repos/actions/runner/releases/latest --jq '.assets[] |
# select(.name | contains("linux-x64")) | .digest'`
RUNNER_SHA256="70920811a4f8ad4328818682bca5c6469c1c942fab52448868071d0063816613"

GH_RUNNER_HOME="${GH_RUNNER_HOME:-/opt/gh-runner-gpu}"

log() { printf '[setup-runner] %s\n' "$*" >&2; }
die() { log "ERROR: $*"; exit 1; }

require_env() {
  local var="$1"
  if [[ -z "${!var:-}" ]]; then
    die "required environment variable ${var} is not set (see environment.example)"
  fi
}

require_env GH_RUNNER_URL
require_env GH_RUNNER_TOKEN
require_env GH_RUNNER_LABELS
require_env GH_RUNNER_NAME

if [[ "${GH_RUNNER_TOKEN}" == "__REPLACE_WITH_SHORT_LIVED_REGISTRATION_TOKEN__" ]]; then
  die "GH_RUNNER_TOKEN still has the placeholder value from environment.example"
fi

# --- 0. sanity: confirm this host actually has the GPU we're registering for ---
if command -v nvidia-smi >/dev/null 2>&1; then
  log "GPU check:"
  nvidia-smi --query-gpu=name,driver_version --format=csv,noheader || true
else
  log "WARNING: nvidia-smi not found on PATH — is this the right host for a GPU runner?"
fi

# --- 1. download + verify pinned tarball ---
mkdir -p "${GH_RUNNER_HOME}"
cd "${GH_RUNNER_HOME}"

if [[ -f "${RUNNER_TARBALL}" ]]; then
  log "tarball ${RUNNER_TARBALL} already present in ${GH_RUNNER_HOME}, skipping download"
else
  log "downloading ${RUNNER_URL}"
  curl -fsSL -o "${RUNNER_TARBALL}" "${RUNNER_URL}"
fi

log "verifying sha256 checksum"
echo "${RUNNER_SHA256}  ${RUNNER_TARBALL}" | sha256sum -c -

# --- 2. extract (idempotent: skip if config.sh already present) ---
if [[ -x "${GH_RUNNER_HOME}/config.sh" ]]; then
  log "runner already extracted in ${GH_RUNNER_HOME}, skipping tar extract"
else
  log "extracting ${RUNNER_TARBALL} to ${GH_RUNNER_HOME}"
  tar xzf "${RUNNER_TARBALL}" -C "${GH_RUNNER_HOME}"
fi

# --- 3. configure (idempotent: skip if already registered) ---
if [[ -f "${GH_RUNNER_HOME}/.runner" ]]; then
  log "${GH_RUNNER_HOME}/.runner already exists — runner appears already configured, skipping config.sh"
  log "(to re-register, first run ./config.sh remove --token <token> as the runner user)"
else
  log "running ./config.sh to register runner '${GH_RUNNER_NAME}' with labels '${GH_RUNNER_LABELS}'"
  "${GH_RUNNER_HOME}/config.sh" \
    --url "${GH_RUNNER_URL}" \
    --token "${GH_RUNNER_TOKEN}" \
    --name "${GH_RUNNER_NAME}" \
    --labels "${GH_RUNNER_LABELS}" \
    --unattended \
    --replace
fi

# --- 4. svc.sh install/start requires root; print, don't run ---
log ""
log "config.sh step complete. Remaining steps require root and are NOT run by"
log "this script — run them explicitly once you've reviewed them:"
log ""
log "  cd ${GH_RUNNER_HOME}"
log "  sudo ./svc.sh install     # creates the systemd unit, e.g.:"
log "                            #   /etc/systemd/system/actions.runner.<org>-<repo>.${GH_RUNNER_NAME}.service"
log "  sudo ./svc.sh start"
log ""
log "To check status later:  sudo ./svc.sh status"
log "To uninstall later:     sudo ./svc.sh uninstall && ./config.sh remove --token <new-token>"
