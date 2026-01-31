# syntax=docker/dockerfile:1.7
# Rust build environment for b00t on aarch64

FROM rust:1.75-bookworm

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    ca-certificates \
    git \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /build

# Default command
CMD ["cargo", "build", "--release"]
