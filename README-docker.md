
● 🐳 b00t-cli Docker Usage Instructions

  🚀 Quick Start

  Local Build & Test

  # Build the container locally
```
  ./build-b00t-cli.sh
```

  # Or with custom tag and version
```
  ./build-b00t-cli.sh my-b00t-cli:v1.0 1.0.0

  # Test the container
  docker run --rm ghcr.io/elasticdotventures/b00t-cli:latest b00t-cli --version

  Pull & Use Pre-built Image

  # Pull latest from registry
  docker pull ghcr.io/elasticdotventures/b00t-cli:latest

  # Run interactively
  docker run -it --rm ghcr.io/elasticdotventures/b00t-cli:latest

  # Run specific command
  docker run --rm ghcr.io/elasticdotventures/b00t-cli:latest b00t-cli status
```
  🔗 Integration with Your Containers

  Option A: Copy from Layer
```
  FROM your-base-image

  # Import b00t-cli from dedicated layer
  COPY --from=ghcr.io/elasticdotventures/b00t-cli:latest /usr/local/bin/b00t-cli /usr/local/bin/
  COPY --from=ghcr.io/elasticdotventures/b00t-cli:latest /opt/b00t/config/ /opt/b00t/config/

  # Set environment
  ENV _B00T_Path=/opt/b00t/config
  ENV PATH="/usr/local/bin:${PATH}"
```
  Option B: Multi-stage Build
```
  # Import stage
  FROM ghcr.io/elasticdotventures/b00t-cli:latest AS b00t-cli-layer

  # Your main image
  FROM ubuntu:24.04
  COPY --from=b00t-cli-layer /usr/local/bin/b00t-cli /usr/local/bin/
  COPY --from=b00t-cli-layer /opt/b00t/config/ /opt/b00t/config/
  ENV _B00T_Path=/opt/b00t/config
```
  🛠️ Development Workflow

  Building Locally
```
  # Standard build
  docker build -f Dockerfile.b00t-cli -t b00t-cli:dev .

  # With version info
  docker build -f Dockerfile.b00t-cli \
    --build-arg BUILD_VERSION=0.5.3 \
    --build-arg BUILD_COMMIT=$(git rev-parse HEAD) \
    --build-arg BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ") \
    -t b00t-cli:dev .
```
  Testing Your Build
```
  # Verify binary works
  docker run --rm b00t-cli:dev b00t-cli --version

  # Check environment
  docker run --rm b00t-cli:dev env | grep B00T

  # Interactive shell
  docker run -it --rm b00t-cli:dev /bin/bash
```
  📦 Available Tags

  - latest - Latest stable release
  - v0.5.3 - Specific version tags
  - main-abc123 - Branch builds with commit hash
  - pr-42 - Pull request builds

  🔧 Environment Variables

  | Variable            | Default          | Description                |
  |---------------------|------------------|----------------------------|
  | _B00T_Path          | /opt/b00t/config | Path to b00t configuration |
  | B00T_CLI_VERSION    | 0.0.1            | Build version info         |
  | B00T_CLI_COMMIT     | unknown          | Git commit hash            |
  | B00T_CLI_BUILD_DATE | -                | Build timestamp            |

  🧪 Testing & Verification
```
  # Quick health check
  docker run --rm ghcr.io/elasticdotventures/b00t-cli:latest b00t-cli --help

  # Status command test
  docker run --rm ghcr.io/elasticdotventures/b00t-cli:latest b00t-cli status

  # Mount workspace for development
  docker run -it --rm -v $(pwd):/workspace ghcr.io/elasticdotventures/b00t-cli:latest
```
  🏷️ Image Details

  - Base: Ubuntu 24.04 (minimal runtime)
  - Size: ~50MB (optimized layers)
  - Registry: ghcr.io/elasticdotventures/b00t-cli
  - Arch: linux/amd64
  - Auto-built: On push to main, releases, and PRs

  💡 Pro Tips

  - Use specific version tags in production: ghcr.io/elasticdotventures/b00t-cli:v0.5.3
  - Layer is cached-friendly - rebuilds are fast
  - Container includes symlink: /usr/local/bin/b00t → /usr/local/bin/b00t-cli
  - Mount /workspace for working with local files
