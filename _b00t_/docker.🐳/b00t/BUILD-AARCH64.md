# Building b00t-cli on aarch64 (ARM64)

Quick start guide for building b00t-cli on ARM64 architecture using Docker/Podman and Rust.

## Prerequisites

- Docker or Podman installed
- Gospel cloned to `~/.b00t` from elasticdotventures/dotfiles
- At least 2GB free disk space for Rust toolchain

## Quick Build

The existing build script works on aarch64:

```bash
cd ~/homeassistant/_b00t_/node-ts.🦄
./docker.🐳/b00t/build-from-gospel.sh
```

This script:
- Auto-detects docker or podman
- Uses gospel's `Dockerfile.b00t-cli` (multi-arch compatible)
- Builds native aarch64 binary via rust:1.88-slim
- Tags as `b00t-cli:aarch64`

## Architecture Detection

The gospel Dockerfile uses `FROM rust:1.88-slim` which automatically:
- Pulls the correct aarch64 image on ARM64 hosts
- Uses native ARM64 Rust toolchain
- Produces native ARM64 binaries

No cross-compilation needed - it builds natively on aarch64.

## Verify Architecture

After building:

```bash
docker run --rm b00t-cli:latest uname -m
# Should output: aarch64

docker run --rm b00t-cli:latest file /usr/local/bin/b00t-cli
# Should show: ELF 64-bit LSB pie executable, ARM aarch64
```

## Build Output

```
b00t-cli:latest     # Latest build (aarch64 on ARM64 host)
b00t-cli:v0.7.0     # Version tag
b00t-cli:aarch64    # Explicit architecture tag
```

## Troubleshooting

### Slow build on first run

First build downloads Rust toolchain (~1.2GB). Subsequent builds are cached:

```bash
# Check layer cache
docker images | grep rust
```

### Out of memory

If build fails with OOM, increase Docker memory:

```bash
# For Docker Desktop
# Settings → Resources → Memory → 4GB minimum

# For Podman
podman system info | grep -i memory
```

### Force rebuild

```bash
# Clear cache and rebuild
docker system prune -a
./docker.🐳/b00t/build-from-gospel.sh
```

## Platform Override

To build for different architecture (requires QEMU):

```bash
# Build for x86_64 on aarch64 host
docker buildx build --platform linux/amd64 \
  -f ~/.b00t/Dockerfile.b00t-cli \
  -t b00t-cli:amd64 \
  ~/.b00t
```

But for native aarch64, just use the regular build script.

## Running b00t-cli

```bash
# Direct run
docker run --rm b00t-cli:latest b00t-cli --version

# With workspace mount
docker run --rm \
  -v $PWD:$PWD -w $PWD \
  b00t-cli:latest b00t-cli status

# Interactive
docker run --rm -it \
  -v $PWD:$PWD -w $PWD \
  b00t-cli:latest bash
```

## Next Steps

After building b00t-cli:

1. Build b00t-mcp container (similar process)
2. Test b00t-mcp MCP server
3. Configure Claude to use b00t-mcp
4. Test multi-agent coordination

## References

- Gospel Dockerfile: `~/.b00t/Dockerfile.b00t-cli`
- Build script: `./docker.🐳/b00t/build-from-gospel.sh`
- GitHub workflow: `~/.b00t/.github/workflows/b00t-cli-container.yml`
