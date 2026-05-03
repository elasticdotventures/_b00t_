# Docker OCI Layer Composition System

Composable, single-source-of-truth Docker layer architecture for b00t and related tools.

## Architecture

### Design Principles

1. **Single Source of Truth**: Each tool maintains its own OCI layer
2. **Composable**: Layers combine to create containers
3. **No Duplication**: Shared dependencies in base layers
4. **Metadata-Driven**: `layer.toml` defines capabilities
5. **Alias System**: Each layer defines environment and aliases

### Directory Structure

```
docker.🐳/
├── layers/                      # OCI layer definitions
│   ├── debian-base/            # Base Debian system
│   ├── ca-certificates/        # HTTPS certificates
│   ├── build-essential/        # GCC, make, pkg-config
│   ├── ssl-dev/                # OpenSSL development libs
│   ├── ssl-runtime/            # OpenSSL runtime libs
│   ├── git/                    # Git VCS
│   ├── mosquitto-clients/      # MQTT clients
│   └── rust-toolchain/         # Rust compiler & cargo
├── b00t/                        # b00t container
│   ├── Dockerfile.composed     # Multi-stage build from layers
│   ├── build.sh                # Build script
│   └── env.sh                  # Docker wrapper
├── rust/                        # Legacy (replaced by layers)
├── build-layers.sh             # Build all layers
├── test-layers.sh              # Test layer composition
├── LAYERS.md                   # Layer architecture docs
└── README.md                   # This file
```

## Quick Start

### 1. Clone Gospel

```bash
git clone https://github.com/elasticdotventures/dotfiles.git ~/.b00t
```

### 2. Build Layers

```bash
cd /home/brianh/homeassistant/_b00t_/node-ts.🦄
./docker.🐳/build-layers.sh
```

This builds all layers in dependency order:
- `b00t-layer/debian-base:bookworm`
- `b00t-layer/ca-certificates:bookworm`
- `b00t-layer/build-essential:bookworm`
- `b00t-layer/ssl-dev:bookworm`
- `b00t-layer/ssl-runtime:bookworm`
- `b00t-layer/git:2.43`
- `b00t-layer/mosquitto-clients:2.0`
- `b00t-layer/rust-toolchain:1.75`

### 3. Build b00t

```bash
./docker.🐳/b00t/build.sh
```

Composes b00t from layers (multi-stage build):
- **Builder**: rust-toolchain + build-essential + ssl-dev
- **Runtime**: debian-base + ssl-runtime + ca-certificates + git + mosquitto-clients

### 4. Test Everything

```bash
./docker.🐳/test-layers.sh
```

Tests each layer individually and the composed b00t container.

### 5. Use b00t

```bash
# Source environment
source docker.🐳/b00t/env.sh

# Run b00t-cli
b00t --version
b00t-cli --help

# Run b00t-mcp
b00t-mcp --help
```

## Layer Details

### Base Layers

**debian-base** (~80MB)
- Minimal Debian bookworm-slim
- No dependencies
- Foundation for all layers

**ca-certificates** (~1MB)
- CA certificates for HTTPS
- Depends on: debian-base
- Required by: git, rust-toolchain

**build-essential** (~150MB)
- gcc, g++, make, pkg-config
- Depends on: debian-base
- Required by: rust-toolchain (build only)

### SSL Layers

**ssl-dev** (~5MB)
- libssl-dev for building
- Depends on: debian-base
- Build-time only

**ssl-runtime** (~2MB)
- libssl3 for runtime
- Depends on: debian-base
- Runtime dependency

### Tool Layers

**git** (~30MB)
- Git version control
- Depends on: debian-base, ca-certificates
- Provides: `git`
- Has env.sh wrapper

**mosquitto-clients** (~1MB)
- MQTT client tools
- Depends on: debian-base
- Provides: `mosquitto_pub`, `mosquitto_sub`
- Has env.sh wrapper

**rust-toolchain** (~1.2GB)
- Rust 1.75 compiler and cargo
- Depends on: debian-base, build-essential, ssl-dev, ca-certificates
- Provides: `cargo`, `rustc`, `rustup`, `rustfmt`
- Has env.sh wrapper

## Layer Composition

### Multi-Stage Build Pattern

```dockerfile
# Build stage - compose build layers
FROM b00t-layer/rust-toolchain:1.75 AS builder
COPY --from=b00t-layer/build-essential:bookworm /usr /usr
COPY --from=b00t-layer/ssl-dev:bookworm /usr /usr
# ... build application ...

# Runtime stage - compose runtime layers
FROM b00t-layer/debian-base:bookworm AS runtime
COPY --from=b00t-layer/ssl-runtime:bookworm /usr /usr
COPY --from=b00t-layer/git:2.43 /usr /usr
# ... copy built artifacts ...
```

### Benefits

1. **Reusability**: Build rust-toolchain once, use everywhere
2. **Cache Efficiency**: Layer changes only rebuild affected components
3. **Version Management**: Independent layer versioning
4. **Size Optimization**: Only include needed layers
5. **Development Speed**: Fast iteration with layer caching

## Individual Layer Usage

Each layer can be used standalone via env.sh:

```bash
# Use git layer
source docker.🐳/layers/git/env.sh
git clone https://github.com/example/repo.git

# Use rust layer
source docker.🐳/layers/rust-toolchain/env.sh
cargo build --release

# Use mosquitto layer
source docker.🐳/layers/mosquitto-clients/env.sh
mosquitto_sub -h localhost -t "b00t/#"
```

## Adding New Layers

### 1. Create Layer Directory

```bash
mkdir -p docker.🐳/layers/my-tool
```

### 2. Create Dockerfile

```dockerfile
# docker.🐳/layers/my-tool/Dockerfile
FROM b00t-layer/debian-base:bookworm

RUN apt-get update && \
    apt-get install -y --no-install-recommends my-tool && \
    rm -rf /var/lib/apt/lists/*

LABEL layer.name="my-tool" \
      layer.version="1.0" \
      layer.description="My tool description"
```

### 3. Create layer.toml

```toml
[layer]
name = "my-tool"
version = "1.0"
description = "My tool description"

[provides]
binaries = ["my-tool"]
libraries = []
paths = ["/usr/bin"]

[requires]
layers = ["debian-base"]

[runtime]
layers = ["debian-base"]

[aliases]
my-tool = "my-tool"
```

### 4. Create env.sh (Optional)

```bash
#!/bin/bash
_my_tool_docker() {
    docker run --rm -it \
        -v "$PWD":"$PWD" -w "$PWD" \
        b00t-layer/my-tool:1.0 \
        my-tool "$@"
}
alias my-tool='_my_tool_docker'
```

### 5. Add to build-layers.sh

```bash
build_layer "my-tool" || exit 1
```

## Maintenance

### Rebuild Specific Layer

```bash
cd docker.🐳/layers/rust-toolchain
docker build -t b00t-layer/rust-toolchain:1.75 .
```

### Rebuild All Layers

```bash
./docker.🐳/build-layers.sh
```

### Clean Up

```bash
# Remove all b00t layers
docker images | grep "b00t-layer/" | awk '{print $3}' | xargs docker rmi

# Remove b00t containers
docker rmi b00t:aarch64 b00t:latest
```

## Integration with b00t Architecture

This layer system supports the b00t architecture:

- **Gospel Convention**: `~/.b00t` mounted read-only
- **Agent Workspace**: `~/_b00t_` mounted read-write
- **Skills**: Each skill (rust, node, python) is a layer
- **MQTT**: mosquitto-clients layer for coordination
- **Git**: git layer for memoization

See [B00T-ARCHITECTURE.md](../B00T-ARCHITECTURE.md) for complete system design.

## Troubleshooting

### Layer Build Fails

```bash
# Check layer dependencies
docker images | grep "b00t-layer/"

# Rebuild dependencies first
./docker.🐳/build-layers.sh
```

### b00t Build Fails

```bash
# Check gospel exists
ls -la ~/.b00t

# Rebuild layers
./docker.🐳/build-layers.sh

# Try build again
./docker.🐳/b00t/build.sh
```

### Permission Errors

Ensure `--user "$(id -u):$(id -g)"` in env.sh wrappers.

---

**Status**: Layer system implemented and ready for testing
**Next**: Run `./docker.🐳/test-layers.sh` to validate
