# syntax=docker/dockerfile:1.7
ARG NODE_VERSION=20-bookworm

# --- Builder: installs node deps (gh-cli-mcp) ---
FROM node:${NODE_VERSION} AS builder
SHELL ["/bin/bash", "-lc"]
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl gnupg \
  && rm -rf /var/lib/apt/lists/*
# GitHub CLI repo
RUN install -d -m 0755 /etc/apt/keyrings \
  && curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
     | dd of=/etc/apt/keyrings/githubcli-archive-keyring.gpg \
  && chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg \
  && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
     > /etc/apt/sources.list.d/github-cli.list \
  && apt-get update && apt-get install -y --no-install-recommends gh \
  && rm -rf /var/lib/apt/lists/*

# Install MCP server
RUN npm i -g gh-cli-mcp && corepack enable

# --- Runtime: slimmer base, copy only what we need ---
FROM node:${NODE_VERSION}-slim AS runtime
SHELL ["/bin/bash", "-lc"]
ENV DEBIAN_FRONTEND=noninteractive
# Install gh (runtime)
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl gnupg gh \
  && rm -rf /var/lib/apt/lists/*

# Bring in the globally-installed npm pkg from builder
# NOTE: this path contains global npm binaries (gh-cli-mcp)
COPY --from=builder /usr/local /usr/local

# Minimal, predictable env
ENV NODE_ENV=production \
    # optional: pass-through args to gh-cli-mcp (space-separated)
    ARGS="" \
    # optional: prefer env token over interactive auth
    GH_TOKEN=""

# Optional workspace path
WORKDIR /workspace

# Entry point: check gh + token, then run the MCP server
# - If gh missing: hard error (shouldn't happen)
# - If no GH_TOKEN: warn but continue (server may still work if public endpoints used)
RUN printf '%s\n' '#!/usr/bin/env bash' \
  'set -euo pipefail' \
  'command -v gh >/dev/null || { echo "[FATAL] gh CLI not found" >&2; exit 127; }' \
  'if [[ -z "${GH_TOKEN:-}" ]]; then' \
  '  echo "[WARN] GH_TOKEN not set. Public or pre-authd gh operations only." >&2' \
  'fi' \
  'echo "[INFO] Starting gh-cli-mcp ${ARGS:-}"' \
  'exec gh-cli-mcp ${ARGS:-}' > /usr/local/bin/entrypoint.sh \
  && chmod +x /usr/local/bin/entrypoint.sh

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]

