# Stage 1: Rust toolchain from official Rust image
FROM rust:latest AS rust-layer
# Rust installed in /usr/local/cargo & /usr/local/rustup

# Stage 2: Godot binary from official Godot image
# https://github.com/godotengine/build-containers
FROM godotengine/godot:latest AS godot-layer
# Godot binary assumed at /usr/local/bin/godot

# Stage 3: bun binary from official oven/bun image
FROM oven/bun AS bun-layer
# bun binary assumed at /usr/local/bin/bun

# Stage 4: Final image based on Ubuntu
FROM ubuntu:latest

# Install minimal dependencies required for runtime
RUN apt-get update && \
    apt-get install -y curl libssl-dev ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Import Rust toolchain from rust-layer
COPY --from=rust-layer /usr/local/cargo/ /usr/local/cargo/
COPY --from=rust-layer /usr/local/rustup/ /usr/local/rustup/
ENV PATH="/usr/local/cargo/bin:${PATH}"

# Import Godot binary from godot-layer
COPY --from=godot-layer /usr/local/bin/godot /usr/local/bin/godot
RUN chmod +x /usr/local/bin/godot

# Import bun binary from bun-layer
COPY --from=bun-layer /usr/local/bin/bun /usr/local/bin/bun
RUN chmod +x /usr/local/bin/bun

# Verification: outputs versions for rustc, cargo, godot and bun
RUN rustc --version && cargo --version && godot --version && bun --version

