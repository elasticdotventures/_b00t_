# syntax=docker/dockerfile:1
# ---------------------------------------------------------------------------
# b00t Azure Control Plane — multi-stage Dockerfile
#
# Stage 1 (builder): Rust workspace build — compiles only b00t-azure-cp.
# Stage 2 (runtime): Distroless, ~30MB final image.
#
# Build:
#   docker build -t ghcr.io/elasticdotventures/b00t-azure-cp:latest \
#     -f b00t-azure-cp/Dockerfile .
#   (run from ~/.b00t/ workspace root)
#
# Required env vars at runtime (injected by ACA via terraform):
#   B00T_NODE_ID, AZURE_STORAGE_ACCOUNT_NAME, AZURE_TABLE_NAME,
#   AZURE_RESOURCE_GROUP, AZURE_SUBSCRIPTION_ID, AZURE_CLIENT_ID,
#   LEASE_TTL_MINUTES, PORT
# ---------------------------------------------------------------------------

FROM rust:1.82-slim-bookworm AS builder

WORKDIR /workspace

# Cache dependency compilation — copy workspace manifests first.
COPY Cargo.toml Cargo.lock ./
COPY b00t-azure-cp/Cargo.toml b00t-azure-cp/
COPY b00t-c0re-lib/Cargo.toml b00t-c0re-lib/
COPY b00t-cli/Cargo.toml b00t-cli/
COPY b00t-mcp/Cargo.toml b00t-mcp/
COPY b00t-lib-chat/Cargo.toml b00t-lib-chat/
COPY b00t-grok/Cargo.toml b00t-grok/
COPY k0mmand3r/Cargo.toml k0mmand3r/
COPY b00t-pyverse/Cargo.toml b00t-pyverse/
COPY b00t-ipc/Cargo.toml b00t-ipc/
COPY tomllm/Cargo.toml tomllm/

# Create stub lib/bin files so cargo can resolve the dep graph.
RUN for dir in b00t-azure-cp b00t-c0re-lib b00t-cli b00t-mcp b00t-lib-chat \
               b00t-grok k0mmand3r b00t-pyverse b00t-ipc tomllm; do \
      mkdir -p "$dir/src" && \
      echo 'fn main() {}' > "$dir/src/main.rs" && \
      echo '' > "$dir/src/lib.rs"; \
    done

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Pre-warm the dependency cache.
RUN cargo build --release --bin b00t-azure-cp 2>/dev/null || true

# Copy the real sources.
COPY b00t-azure-cp/src b00t-azure-cp/src/
COPY b00t-c0re-lib/src b00t-c0re-lib/src/
COPY b00t-cli/src b00t-cli/src/
COPY b00t-mcp/src b00t-mcp/src/
COPY b00t-lib-chat/src b00t-lib-chat/src/
COPY b00t-grok/src b00t-grok/src/
COPY k0mmand3r/src k0mmand3r/src/
COPY b00t-pyverse/src b00t-pyverse/src/
COPY b00t-ipc/src b00t-ipc/src/
COPY tomllm/src tomllm/src/

# Touch to bust the stub-vs-real cache difference.
RUN touch b00t-azure-cp/src/main.rs

RUN cargo build --release --bin b00t-azure-cp

# ---------------------------------------------------------------------------
# Runtime stage — minimal image
# ---------------------------------------------------------------------------
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

COPY --from=builder /workspace/target/release/b00t-azure-cp /usr/local/bin/b00t-azure-cp
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/b00t-azure-cp"]
