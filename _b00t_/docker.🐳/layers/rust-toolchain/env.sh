#!/bin/bash
# rust-toolchain layer environment

export RUST_VERSION="1.75"
export CARGO_HOME="${HOME}/.cargo"

# Shared cache directories
mkdir -p "$CARGO_HOME" "$HOME/.cache/cargo-target"

# Docker wrapper for cargo
_cargo_docker() {
    docker run --rm -it \
        -v "$PWD":"$PWD" -w "$PWD" \
        -v "$CARGO_HOME":/usr/local/cargo \
        -v "$HOME/.cache/cargo-target":/build/target \
        --user "$(id -u)":"$(id -g)" \
        b00t-layer/rust-toolchain:1.75 \
        cargo "$@"
}

# Docker wrapper for rustc
_rustc_docker() {
    docker run --rm -it \
        -v "$PWD":"$PWD" -w "$PWD" \
        --user "$(id -u)":"$(id -g)" \
        b00t-layer/rust-toolchain:1.75 \
        rustc "$@"
}

alias cargo='_cargo_docker'
alias rustc='_rustc_docker'
alias rustup='_cargo_docker rustup'
alias rustfmt='_cargo_docker rustfmt'
