# syntax=docker/dockerfile:1
# ---------------------------------------------------------------------------
# b00t Azure Control Plane — multi-stage Dockerfile (cargo-chef pattern)
#
# Stage 1 (chef):    base image + cargo-chef install.
# Stage 2 (planner): `cargo chef prepare` -> recipe.json (deps-only fingerprint).
# Stage 3 (builder): `cargo chef cook` builds ONLY dependencies as a
#                     cacheable layer, then the real source is copied in and
#                     `cargo build` compiles just b00t-azure-cp against the
#                     already-warm dependency cache.
# Stage 4 (runtime): Distroless, ~30MB final image.
#
# Build:
#   podman build -t ghcr.io/elasticdotventures/b00t-azure-cp:latest \
#     -f b00t-azure-cp/Dockerfile .
#   (run from ~/.b00t/ workspace root)
#
# Required env vars at runtime (injected by ACA via terraform):
#   B00T_NODE_ID, AZURE_STORAGE_ACCOUNT_NAME, AZURE_TABLE_NAME,
#   AZURE_RESOURCE_GROUP, AZURE_SUBSCRIPTION_ID, AZURE_CLIENT_ID,
#   LEASE_TTL_MINUTES, PORT
#
# 🤓 b00t-azure-cp itself only depends on workspace-external crates (azure_*,
#    axum, tokio, ...) — no path deps on sibling b00t-* crates. But this repo
#    is one big Cargo virtual workspace (see root Cargo.toml `[workspace]
#    members`), and `cargo` refuses to resolve *any* member — even to build
#    just one bin — unless every listed member's Cargo.toml is present on
#    disk, and several of those members path-depend on crates living outside
#    the workspace `members` list (vendor/*). So the manifest COPY list below
#    must cover the *entire* workspace, matching Dockerfile.b00t-cli's list —
#    this is not b00t-azure-cp-specific bloat, it's a workspace resolution
#    requirement. The previous non-chef version of this Dockerfile only
#    copied a 10-crate subset (and had a stale `tomllm/` path instead of
#    `vendor/tomllm/`), which would have failed to resolve against the full
#    25-member workspace.
# ---------------------------------------------------------------------------

# 🤓 rust:1.82 predates edition2024 stabilization (Rust 1.85+); the root
#    Cargo.toml sets `edition = "2024"` workspace-wide, so anything older
#    fails to even parse the workspace manifest graph. Pinned to the same
#    tag as Dockerfile.b00t-cli for consistency.
FROM rust:1.96-slim AS chef
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates && \
    rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked
WORKDIR /workspace

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY _b00t_/ ./_b00t_/
COPY b00t-admin/ ./b00t-admin/
COPY b00t-ast/ ./b00t-ast/
COPY b00t-azure-cp/ ./b00t-azure-cp/
COPY b00t-bouncer/ ./b00t-bouncer/
COPY b00t-cad/ ./b00t-cad/
COPY b00t-cli/ ./b00t-cli/
COPY b00t-crate/ ./b00t-crate/
COPY b00t-c0re-a2a/ ./b00t-c0re-a2a/
COPY b00t-c0re-gov/ ./b00t-c0re-gov/
COPY b00t-c0re-hierarchy/ ./b00t-c0re-hierarchy/
COPY b00t-c0re-lib/ ./b00t-c0re-lib/
COPY b00t-datum-core/ ./b00t-datum-core/
COPY b00t-embed/ ./b00t-embed/
COPY b00t-grok/ ./b00t-grok/
COPY b00t-ipc/ ./b00t-ipc/
COPY b00t-l3dg3rr-viz/ ./b00t-l3dg3rr-viz/
COPY b00t-lib-chat/ ./b00t-lib-chat/
COPY b00t-lsp/ ./b00t-lsp/
COPY b00t-mcp/ ./b00t-mcp/
COPY b00t-mcp-npm/ ./b00t-mcp-npm/
COPY b00t-mermaid-wasm/ ./b00t-mermaid-wasm/
COPY b00t-pyverse/ ./b00t-pyverse/
COPY b00t-ui-wasm/ ./b00t-ui-wasm/
COPY b00t-zellij/ ./b00t-zellij/
COPY blessed/ ./blessed/
COPY k0mmand3r/ ./k0mmand3r/
COPY pm2-tasker/ ./pm2-tasker/
COPY vendor/embed-anything-b00t/rust/ ./vendor/embed-anything-b00t/rust/
COPY vendor/embed-anything-b00t/processors/ ./vendor/embed-anything-b00t/processors/
COPY vendor/irontology-mcp/ ./vendor/irontology-mcp/
COPY vendor/ledgrrr/crates/b00t-reflect-types/ ./vendor/ledgrrr/crates/b00t-reflect-types/
COPY vendor/runpod-sdk/ ./vendor/runpod-sdk/
COPY vendor/tomllm/ ./vendor/tomllm/

# 🤓 No `--bin` scoping here: this is a virtual workspace where several
#    members path-depend on crates *outside* the `[workspace] members` list
#    (e.g. b00t-cli -> vendor/runpod-sdk, which is in root Cargo.toml's
#    `exclude`). A `--bin`-scoped recipe only reconstructs the dependency
#    subgraph reachable from that target, which silently drops manifests
#    for out-of-workspace path deps pulled in by *other* members — cargo
#    chef cook then fails trying to read them back out of the recipe.
#    Preparing the full workspace (matching vendor/ledgrrr/Dockerfile's
#    unscoped `cargo chef prepare`) keeps recipe.json self-contained.
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /workspace/recipe.json recipe.json

# 🤓 cargo-chef's `cook` only reconstructs dummy sources for crates in
#    `[workspace] members`. vendor/embed-anything-b00t and vendor/runpod-sdk
#    are deliberately excluded from workspace membership (root Cargo.toml
#    `exclude`) yet are real path dependencies of workspace members
#    (b00t-embed, b00t-cli respectively) — chef silently drops them from the
#    reconstructed tree, so `cook` fails trying to read their Cargo.toml back
#    out. Copying their real source in before cook — instead of relying on
#    chef's reconstruction — fixes this. It narrows the dependency-cache
#    benefit slightly for these two (rarely-changing) vendored submodules,
#    but every other workspace member still caches cleanly.
COPY vendor/embed-anything-b00t/rust/ ./vendor/embed-anything-b00t/rust/
COPY vendor/embed-anything-b00t/processors/ ./vendor/embed-anything-b00t/processors/
COPY vendor/runpod-sdk/ ./vendor/runpod-sdk/

RUN cargo chef cook --release --recipe-path recipe.json

# Copy the real sources (same set as the planner stage above).
COPY Cargo.toml Cargo.lock ./
COPY _b00t_/ ./_b00t_/
COPY b00t-admin/ ./b00t-admin/
COPY b00t-ast/ ./b00t-ast/
COPY b00t-azure-cp/ ./b00t-azure-cp/
COPY b00t-bouncer/ ./b00t-bouncer/
COPY b00t-cad/ ./b00t-cad/
COPY b00t-cli/ ./b00t-cli/
COPY b00t-crate/ ./b00t-crate/
COPY b00t-c0re-a2a/ ./b00t-c0re-a2a/
COPY b00t-c0re-gov/ ./b00t-c0re-gov/
COPY b00t-c0re-hierarchy/ ./b00t-c0re-hierarchy/
COPY b00t-c0re-lib/ ./b00t-c0re-lib/
COPY b00t-datum-core/ ./b00t-datum-core/
COPY b00t-embed/ ./b00t-embed/
COPY b00t-grok/ ./b00t-grok/
COPY b00t-ipc/ ./b00t-ipc/
COPY b00t-l3dg3rr-viz/ ./b00t-l3dg3rr-viz/
COPY b00t-lib-chat/ ./b00t-lib-chat/
COPY b00t-lsp/ ./b00t-lsp/
COPY b00t-mcp/ ./b00t-mcp/
COPY b00t-mcp-npm/ ./b00t-mcp-npm/
COPY b00t-mermaid-wasm/ ./b00t-mermaid-wasm/
COPY b00t-pyverse/ ./b00t-pyverse/
COPY b00t-ui-wasm/ ./b00t-ui-wasm/
COPY b00t-zellij/ ./b00t-zellij/
COPY blessed/ ./blessed/
COPY k0mmand3r/ ./k0mmand3r/
COPY pm2-tasker/ ./pm2-tasker/
COPY vendor/embed-anything-b00t/rust/ ./vendor/embed-anything-b00t/rust/
COPY vendor/embed-anything-b00t/processors/ ./vendor/embed-anything-b00t/processors/
COPY vendor/irontology-mcp/ ./vendor/irontology-mcp/
COPY vendor/ledgrrr/crates/b00t-reflect-types/ ./vendor/ledgrrr/crates/b00t-reflect-types/
COPY vendor/runpod-sdk/ ./vendor/runpod-sdk/
COPY vendor/tomllm/ ./vendor/tomllm/

RUN cargo build --release --bin b00t-azure-cp

# ---------------------------------------------------------------------------
# Runtime stage — minimal image
# ---------------------------------------------------------------------------
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime

COPY --from=builder /workspace/target/release/b00t-azure-cp /usr/local/bin/b00t-azure-cp
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/b00t-azure-cp"]
