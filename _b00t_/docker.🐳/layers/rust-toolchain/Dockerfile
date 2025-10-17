# syntax=docker/dockerfile:1.7
# Layer: rust-toolchain
# Rust compiler, cargo, and toolchain

ARG RUST_VERSION=1.75
FROM rust:${RUST_VERSION}-bookworm

# Already includes rust, cargo, rustup
# Just label it as a layer

LABEL layer.name="rust-toolchain" \
      layer.version="1.75" \
      layer.description="Rust compiler and cargo toolchain"

# Set environment
ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:${PATH}
