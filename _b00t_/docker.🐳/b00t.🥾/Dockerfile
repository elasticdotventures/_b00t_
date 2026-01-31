# syntax=docker/dockerfile:1.7
# Multi-stage Dockerfile for b00t-cli and b00t-mcp runtime

ARG RUST_VERSION=1.75
FROM rust:${RUST_VERSION}-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    ca-certificates \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy source
WORKDIR /build
COPY . .

# Build b00t-cli
RUN cargo build --release --bin b00t-cli

# Build b00t-mcp
RUN cargo build --release --bin b00t-mcp

# --- Runtime stage ---
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 \
    ca-certificates \
    mosquitto-clients \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy binaries from builder
COPY --from=builder /build/target/release/b00t-cli /usr/local/bin/
COPY --from=builder /build/target/release/b00t-mcp /usr/local/bin/

# Create b00t user
RUN groupadd -r b00t && \
    useradd -r -g b00t -d /home/b00t -m b00t

# Set working directory
WORKDIR /workspace

# Default to b00t-cli
ENTRYPOINT ["/usr/local/bin/b00t-cli"]
CMD ["--help"]
