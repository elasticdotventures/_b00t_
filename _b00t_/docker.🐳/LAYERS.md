# Docker OCI Layer Architecture

## Design Principles

1. **Single Source of Truth**: Each tool maintains its own OCI layer
2. **Composable**: Layers can be composed into containers
3. **No Duplication**: Shared dependencies in base layers
4. **Metadata-Driven**: layer.toml defines what each layer provides
5. **Alias System**: Each layer defines its own aliases and environment

## Layer Structure

Each layer follows this pattern:

```
docker.🐳/{layer-name}/
├── Dockerfile           # Builds the layer
├── layer.toml          # Metadata (provides, requires, aliases)
├── env.sh              # Environment setup and aliases
└── README.md           # Documentation
```

## Layer Metadata (layer.toml)

```toml
[layer]
name = "rust-toolchain"
version = "1.75"
description = "Rust compiler and cargo toolchain"

[provides]
binaries = ["cargo", "rustc", "rustup", "rustfmt", "clippy"]
libraries = []
paths = ["/usr/local/cargo/bin"]

[requires]
# Other layers this depends on
layers = ["build-essential", "ca-certificates"]

[runtime]
# Runtime dependencies (for multi-stage builds)
layers = ["ca-certificates"]

[aliases]
cargo = "cargo"
rustc = "rustc"
rustup = "rustup"

[environment]
CARGO_HOME = "/usr/local/cargo"
RUSTUP_HOME = "/usr/local/rustup"
PATH = "/usr/local/cargo/bin:${PATH}"
```

## Base Layers

### Layer: debian-base
- Base: `debian:bookworm-slim`
- Provides: Minimal Debian system
- Size: ~80MB

### Layer: ca-certificates
- Base: debian-base
- Provides: CA certificates for HTTPS
- Size: ~1MB

### Layer: build-essential
- Base: debian-base
- Provides: gcc, make, libc-dev
- Size: ~150MB

### Layer: ssl-dev
- Base: build-essential
- Provides: libssl-dev, pkg-config for building
- Size: ~5MB

### Layer: ssl-runtime
- Base: debian-base
- Provides: libssl3 for runtime
- Size: ~2MB

## Tool Layers

### Layer: git
- Base: debian-base + ca-certificates
- Provides: git
- Size: ~30MB

### Layer: mosquitto-clients
- Base: debian-base
- Provides: mosquitto_pub, mosquitto_sub
- Size: ~1MB

### Layer: rust-toolchain
- Base: debian-base + build-essential + ssl-dev + ca-certificates
- Provides: cargo, rustc, rustup
- Version: 1.75
- Size: ~1.2GB

### Layer: node-toolchain
- Base: debian-base + ca-certificates
- Provides: node, npm, npx, corepack
- Version: 20.11
- Size: ~200MB

### Layer: python-toolchain
- Base: debian-base + ca-certificates
- Provides: python3, pip, venv
- Version: 3.12
- Size: ~150MB

## Composition Examples

### b00t-builder (Multi-stage)
```dockerfile
# Build stage - compose build layers
FROM debian-base AS base
COPY --from=build-essential / /
COPY --from=ssl-dev / /
COPY --from=ca-certificates / /
COPY --from=rust-toolchain / /

# Runtime stage - compose runtime layers
FROM debian-base AS runtime
COPY --from=ssl-runtime / /
COPY --from=ca-certificates / /
COPY --from=git / /
COPY --from=mosquitto-clients / /
COPY --from=base /build/target/release/b00t-cli /usr/local/bin/
COPY --from=base /build/target/release/b00t-mcp /usr/local/bin/
```

## Layer Registry

Layers are built and tagged in local Docker registry:

```
b00t-layer/debian-base:bookworm
b00t-layer/ca-certificates:bookworm
b00t-layer/build-essential:bookworm
b00t-layer/ssl-dev:bookworm
b00t-layer/ssl-runtime:bookworm
b00t-layer/git:2.43
b00t-layer/mosquitto-clients:2.0
b00t-layer/rust-toolchain:1.75
b00t-layer/node-toolchain:20.11
b00t-layer/python-toolchain:3.12
```

## Build System

### build-layers.sh
```bash
#!/bin/bash
# Build all layers in dependency order

set -euo pipefail

LAYERS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

build_layer() {
    local layer_name=$1
    local layer_dir="${LAYERS_DIR}/${layer_name}"

    echo "Building layer: ${layer_name}"

    # Read layer.toml for metadata
    local version=$(toml get "${layer_dir}/layer.toml" layer.version)
    local tag="b00t-layer/${layer_name}:${version}"

    # Build the layer
    docker build -t "${tag}" "${layer_dir}"

    echo "Built: ${tag}"
}

# Build in dependency order
build_layer "debian-base"
build_layer "ca-certificates"
build_layer "build-essential"
build_layer "ssl-dev"
build_layer "ssl-runtime"
build_layer "git"
build_layer "mosquitto-clients"
build_layer "rust-toolchain"
build_layer "node-toolchain"
build_layer "python-toolchain"
```

### compose-container.sh
```bash
#!/bin/bash
# Compose a container from layers

set -euo pipefail

compose() {
    local container_name=$1
    shift
    local layers=("$@")

    # Generate multi-stage Dockerfile
    cat > Dockerfile.composed <<EOF
FROM debian-base AS composed
EOF

    for layer in "${layers[@]}"; do
        echo "COPY --from=b00t-layer/${layer} / /" >> Dockerfile.composed
    done

    docker build -f Dockerfile.composed -t "${container_name}" .
}
```

## Environment Loading

Each layer provides an env.sh that can be sourced:

```bash
# Load rust environment
source docker.🐳/rust-toolchain/env.sh

# Load node environment
source docker.🐳/node-toolchain/env.sh

# All aliases now available
cargo --version
node --version
```

## Benefits

1. **Reusability**: Build rust-toolchain once, use in b00t, k0mmand3r, etc.
2. **Cache Efficiency**: Layer changes only rebuild affected layers
3. **Version Management**: Each layer versioned independently
4. **Size Optimization**: Only include what you need
5. **Development Speed**: Fast iteration with layer caching

## Implementation Plan

1. Create base layers (debian-base, ca-certificates, build-essential)
2. Create ssl layers (ssl-dev, ssl-runtime)
3. Create tool layers (git, mosquitto-clients)
4. Create rust-toolchain layer
5. Implement build-layers.sh
6. Compose b00t-builder using layers
7. Test compilation
8. Add node-toolchain and python-toolchain
9. Document and optimize

---

**Status**: Architecture designed
**Next**: Implement base layers
