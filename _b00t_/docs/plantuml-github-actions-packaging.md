# PlantUML Server b00t Fork - GitHub Actions Packaging Strategy

## Repository
**Fork:** https://github.com/PromptExecution/plantuml-server--b00t--togaf--mcp
**Upstream:** https://github.com/plantuml/plantuml-server

## Objective
Create automated GitHub Actions workflow to build and publish a **single container image** combining:
1. **PlantUML Server** (Java-based rendering engine)
2. **PlantUML MCP Server** (Node.js-based MCP protocol server)
3. **TOGAF integration** (pre-loaded templates, C4 diagrams support)

## Container Architecture

### Multi-Stage Dockerfile Strategy

```dockerfile
# Stage 1: Build PlantUML Server (Java)
FROM eclipse-temurin:17-jre-alpine AS plantuml-builder
ARG PLANTUML_VERSION=1.2024.3
WORKDIR /app
# Download PlantUML JAR
RUN wget -O plantuml.jar \
    "https://github.com/plantuml/plantuml/releases/download/v${PLANTUML_VERSION}/plantuml-${PLANTUML_VERSION}.jar"

# Stage 2: Node.js MCP Server
FROM node:18-alpine AS mcp-builder
WORKDIR /mcp
# Install infobip/plantuml-mcp-server
RUN npm install -g plantuml-mcp-server@0.1.11

# Stage 3: Runtime Image
FROM eclipse-temurin:17-jre-alpine
# Install Node.js runtime alongside Java
RUN apk add --no-cache nodejs npm supervisor

# Copy PlantUML JAR from builder
COPY --from=plantuml-builder /app/plantuml.jar /opt/plantuml/plantuml.jar

# Copy MCP server from builder
COPY --from=mcp-builder /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s /usr/local/lib/node_modules/plantuml-mcp-server/bin/plantuml-mcp-server \
    /usr/local/bin/plantuml-mcp-server

# Supervisor configuration for running both services
COPY supervisord.conf /etc/supervisord.conf

# Environment configuration
ENV PLANTUML_SERVER_URL=http://localhost:8080/plantuml \
    PLANTUML_LIMIT_SIZE=16384

# Expose ports
EXPOSE 8080 3000

# Health check for PlantUML server
HEALTHCHECK --interval=30s --timeout=3s \
    CMD wget -q --spider http://localhost:8080/plantuml || exit 1

# Start supervisor to manage both services
CMD ["/usr/bin/supervisord", "-c", "/etc/supervisord.conf"]
```

### Supervisord Configuration

```ini
# supervisord.conf
[supervisord]
nodaemon=true
user=root

[program:plantuml-server]
command=java -jar /opt/plantuml/plantuml.jar -Djava.net.preferIPv4Stack=true
autostart=true
autorestart=true
stdout_logfile=/dev/stdout
stdout_logfile_maxbytes=0
stderr_logfile=/dev/stderr
stderr_logfile_maxbytes=0

[program:mcp-server]
command=plantuml-mcp-server
autostart=true
autorestart=true
environment=PLANTUML_SERVER_URL="http://localhost:8080/plantuml"
stdout_logfile=/dev/stdout
stdout_logfile_maxbytes=0
stderr_logfile=/dev/stderr
stderr_logfile_maxbytes=0
```

## GitHub Actions Workflow

### File: `.github/workflows/build-and-publish.yml`

```yaml
name: Build and Publish PlantUML b00t Container

on:
  push:
    branches:
      - main
      - develop
    tags:
      - 'v*'
  pull_request:
    branches:
      - main
  workflow_dispatch:
    inputs:
      plantuml_version:
        description: 'PlantUML version to build'
        required: false
        default: '1.2024.3'

env:
  REGISTRY: ghcr.io
  IMAGE_NAME: ${{ github.repository }}

jobs:
  build-and-push:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write

    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Set up QEMU
        uses: docker/setup-qemu-action@v3
        with:
          platforms: linux/amd64,linux/arm64

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to GitHub Container Registry
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}
          tags: |
            type=ref,event=branch
            type=ref,event=pr
            type=semver,pattern={{version}}
            type=semver,pattern={{major}}.{{minor}}
            type=sha,prefix={{branch}}-
            type=raw,value=latest,enable={{is_default_branch}}

      - name: Build and push Docker image
        uses: docker/build-push-action@v5
        with:
          context: .
          platforms: linux/amd64,linux/arm64
          push: ${{ github.event_name != 'pull_request' }}
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          build-args: |
            PLANTUML_VERSION=${{ github.event.inputs.plantuml_version || '1.2024.3' }}
            MCP_SERVER_VERSION=0.1.11
            NODE_VERSION=18-alpine
          cache-from: type=gha
          cache-to: type=gha,mode=max

      - name: Generate SBOM
        uses: anchore/sbom-action@v0
        with:
          image: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}:latest
          format: spdx-json
          output-file: sbom.spdx.json

      - name: Upload SBOM artifact
        uses: actions/upload-artifact@v4
        with:
          name: sbom
          path: sbom.spdx.json
```

## Repository Setup Requirements

### 1. GitHub Secrets (Not Required - Uses GITHUB_TOKEN)
The workflow uses the built-in `GITHUB_TOKEN` for authentication.

### 2. GitHub Container Registry Permissions
Enable GitHub Container Registry in repository settings:
- Settings → Packages → "Inherit access from repository"

### 3. Branch Protection Rules
- Protect `main` branch
- Require PR reviews before merging
- Run Actions on PR to validate builds

## Build Triggers

| Event | Trigger | Image Tag |
|-------|---------|-----------|
| Push to `main` | Automatic | `latest` |
| Push to `develop` | Automatic | `develop` |
| Git tag `v1.0.0` | Automatic | `1.0.0`, `1.0` |
| Pull Request | Automatic | `pr-123` |
| Manual dispatch | Button in Actions tab | Custom + SHA |

## Multi-Architecture Support

The workflow builds for:
- **linux/amd64** (x86_64 servers, most cloud instances)
- **linux/arm64** (ARM servers, Apple Silicon, Raspberry Pi)

## Usage Examples

### Pull Latest Image
```bash
docker pull ghcr.io/promptexecution/plantuml-server--b00t--togaf--mcp:latest
```

### Run Container
```bash
docker run -d \
  -p 8080:8080 \
  -p 3000:3000 \
  --name plantuml-b00t \
  ghcr.io/promptexecution/plantuml-server--b00t--togaf--mcp:latest
```

### Use in b00t
```bash
# Install MCP server datum
b00t-cli mcp install plantuml claudecode

# Start PlantUML server container
docker run -d -p 8080:8080 ghcr.io/promptexecution/plantuml-server--b00t--togaf--mcp:latest
```

### Docker Compose (with TOGAF volumes)
```yaml
services:
  plantuml-b00t:
    image: ghcr.io/promptexecution/plantuml-server--b00t--togaf--mcp:latest
    ports:
      - "8080:8080"
      - "3000:3000"
    volumes:
      - ~/.b00t/data/togaf/datasheets:/togaf/datasheets:ro
      - diagrams_output:/diagrams
    environment:
      - PLANTUML_SERVER_URL=http://localhost:8080/plantuml
      - PLANTUML_LIMIT_SIZE=16384

volumes:
  diagrams_output:
```

## Validation Tests

### Test PlantUML Server (HTTP)
```bash
curl -X POST http://localhost:8080/plantuml/svg \
  -H "Content-Type: text/plain" \
  -d '@startuml
Alice -> Bob: Hello
@enduml'
```

### Test MCP Server (via npx)
```bash
# With local server
PLANTUML_SERVER_URL=http://localhost:8080/plantuml npx plantuml-mcp-server
```

### Test via b00t Job
```bash
# Generate TOGAF BPMN diagram
b00t job run togaf-generate-diagram \
  --input togaf-process.puml \
  --output diagrams/process.svg \
  --format svg
```

## Continuous Improvement

### Future Enhancements
1. **Pre-loaded TOGAF Templates**
   - Include C4 PlantUML libraries
   - TOGAF standard diagram templates
   - ArchiMate notation support

2. **Performance Optimization**
   - Cache rendered diagrams
   - Parallel rendering for batch jobs
   - Redis integration for MCP state

3. **Security Hardening**
   - Non-root user execution
   - Read-only root filesystem
   - Security scanning in CI

4. **Observability**
   - Prometheus metrics export
   - Structured JSON logging
   - OpenTelemetry tracing

## References

- [infobip/plantuml-mcp-server](https://github.com/infobip/plantuml-mcp-server)
- [PlantUML Server Upstream](https://github.com/plantuml/plantuml-server)
- [GitHub Actions Docker Build](https://docs.github.com/en/actions/publishing-packages/publishing-docker-images)
- [GitHub Container Registry](https://docs.github.com/en/packages/working-with-a-github-packages-registry/working-with-the-container-registry)
