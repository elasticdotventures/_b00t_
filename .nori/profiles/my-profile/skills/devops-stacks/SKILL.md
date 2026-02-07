---
name: devops-stacks
description: Datum-driven infrastructure composition using b00t component manifests. Activates when assembling application stacks, composing services, or managing infrastructure-as-code from declarative datums.
tags: [infrastructure, stacks, composition, datums, iac, docker, k8s]
---

# DevOps Stacks Skill

**Workflow Capability**: Compose application infrastructure from reusable b00t datums—combining Docker, Kubernetes, databases, caches, and services into validated, production-ready stacks following compounding engineering principles.

## When This Skill Activates

- Composing application stacks
- Infrastructure setup/teardown
- Service arrangement from datums
- Multi-environment deployment
- Stack validation/testing
- Datum-driven GitOps workflows

## B00t Datum System for Infrastructure

### Datum Types

**1. Service Datums** (`.docker.toml`, `.k8s.toml`)
```toml
# _b00t_/postgres-enhanced.docker.toml
[service]
name = "postgres"
image = "postgres:16-alpine"
environment = [
  "POSTGRES_PASSWORD=${DB_PASSWORD}",
  "POSTGRES_DB=${DB_NAME}"
]

[healthcheck]
test = ["CMD", "pg_isready", "-U", "postgres"]
interval = "5s"
timeout = "3s"
retries = 3

[volumes]
data = "./data/postgres:/var/lib/postgresql/data"

[networks]
- "backend"
```

**2. Stack Datums** (`.stack.toml`)
```toml
# _b00t_/ai-dev-stack.stack.toml
[stack]
name = "ai-dev"
description = "AI development environment"

[components]
database = { datum = "postgres-enhanced.docker.toml" }
cache = { datum = "valkey.k8s.toml" }
api = { datum = "fastapi.docker.toml" }
frontend = { datum = "nextjs.docker.toml" }

[networks]
backend = { driver = "bridge" }
frontend = { driver = "bridge" }

[dependencies]
api.depends_on = ["database", "cache"]
frontend.depends_on = ["api"]
```

**3. Environment Datums** (`.env.toml`)
```toml
# _b00t_/envs/development.toml
[environment]
name = "development"
DB_PASSWORD = "dev_password"
DB_NAME = "app_dev"
API_PORT = "3000"
LOG_LEVEL = "debug"
```

## Core Operations

### Compose Stack

```bash
# From stack datum
b00t stack compose \
  --datum ai-dev-stack.stack.toml \
  --env development.toml \
  --output docker-compose.yaml

# Generated docker-compose.yaml includes:
# - All services from datums
# - Proper dependencies (healthchecks)
# - Network configuration
# - Environment variables
# - Volume mounts
```

### Validate Stack

```bash
# Before deployment
b00t stack validate docker-compose.yaml

# Checks:
- All referenced datums exist
- Dependencies form DAG (no cycles)
- Healthchecks defined
- Required env vars present
- Ports don't conflict
- Volumes paths valid
```

### Deploy Stack

```bash
# Development
just stack-up development

# Behind the scenes:
b00t stack compose --datum ai-dev-stack --env dev
docker-compose up -d
b00t stack healthcheck --timeout 60s
```

### Update Component

```bash
# Update single service
b00t stack update \
  --component postgres \
  --datum postgres-enhanced.docker.toml \
  --version v2

# Preserves data, restarts service
```

## Stack Composition Patterns

### Pattern 1: Microservices Stack

```
Stack: microservices-app
├─ API Gateway (nginx.docker.toml)
├─ Auth Service (keycloak.docker.toml)
├─ User Service (user-api.docker.toml)
├─ Product Service (product-api.docker.toml)
├─ Database (postgres-enhanced.docker.toml)
└─ Cache (valkey.k8s.toml)

Dependencies:
  gateway → [auth, user, product]
  auth → [database]
  user → [database, cache]
  product → [database, cache]
```

### Pattern 2: AI/ML Stack

```
Stack: ml-pipeline
├─ Jupyter (jupyter-scipy.docker.toml)
├─ MLflow (mlflow-tracking.docker.toml)
├─ Postgres (postgres-mlflow.docker.toml)
├─ MinIO (minio-artifacts.docker.toml)
└─ Ray Cluster (ray-cluster.k8s.toml)

Dependencies:
  jupyter → [mlflow, ray]
  mlflow → [postgres, minio]
```

### Pattern 3: Full-Stack App

```
Stack: fullstack-app
├─ Next.js Frontend (nextjs.docker.toml)
├─ FastAPI Backend (fastapi.docker.toml)
├─ Postgres DB (postgres-enhanced.docker.toml)
├─ Redis Cache (valkey.k8s.toml)
└─ Nginx Proxy (nginx-proxy.docker.toml)

Dependencies:
  proxy → [frontend, backend]
  frontend → [backend]
  backend → [database, cache]
```

## Datum Inheritance

```toml
# Base postgres datum
# _b00t_/postgres-base.docker.toml
[service]
image = "postgres:16-alpine"

[healthcheck]
test = ["CMD", "pg_isready"]

# Development variant (inherits base)
# _b00t_/postgres-dev.docker.toml
[extends]
datum = "postgres-base.docker.toml"

[service]
environment = ["LOG_STATEMENT=all"]  # Extra logging for dev
ports = ["5432:5432"]  # Expose for local access

# Production variant (inherits base)
# _b00t_/postgres-prod.docker.toml
[extends]
datum = "postgres-base.docker.toml"

[service]
environment = [
  "POSTGRES_PASSWORD_FILE=/run/secrets/db_password"
]

[deploy]
replicas = 3
resources.limits.memory = "2GB"
```

## Environment Management

### Multi-Environment Pattern

```bash
# Development
b00t stack compose --env dev
→ Uses: _b00t_/envs/development.toml
→ Features: Debug logging, exposed ports, hot reload

# Staging
b00t stack compose --env staging
→ Uses: _b00t_/envs/staging.toml
→ Features: Production-like, test data, monitoring

# Production
b00t stack compose --env production
→ Uses: _b00t_/envs/production.toml
→ Features: Secrets from vault, replicas, health checks
```

### Secrets Management

```toml
# Development: Plain text OK
[environment]
DB_PASSWORD = "dev_password"

# Staging/Production: Reference secrets
[environment]
DB_PASSWORD = { secret = "db-password", provider = "vault" }
API_KEY = { secret = "api-key", provider = "k8s-secrets" }
```

## Integration with Other Skills

### With Agent-Orchestration

```typescript
// Gemini researches optimal stack configuration
const stackResearch = await gemini.research({
  query: "PostgreSQL + Redis caching patterns for high-traffic API 2025"
});

// Claude designs stack architecture
const stackDesign = await claude.designStack({
  requirements: "requirements/api-stack.md",
  research: stackResearch
});

// Generate datums from design
await b00t.stack.generateDatums({
  design: stackDesign,
  output: "_b00t_/"
});

// Codex generates Kubernetes manifests
const k8sManifests = await codex.generateK8s({
  datums: stackDesign.components
});
```

### With Systems-Engineering

```bash
# Each stack component traced to requirements
b00t stack compose \
  --with-traceability \
  --requirement-matrix traceability.md

# Generated stack includes:
# - REQ-001 → postgres-enhanced.docker.toml
# - REQ-002 → valkey.k8s.toml
# - REQ-003 → fastapi.docker.toml
```

### With Hive-Memory

```bash
# Capture stack composition learnings
b00t lfmf datum abstract <<EOF
Lesson: Postgres + Redis requires connection pooling

Context: High-traffic API stack
Problem: Connection exhaustion under load
Solution: Added pgbouncer datum between API and Postgres

Pattern: API → pgbouncer → Postgres + Redis
Codified: _b00t_/pgbouncer.docker.toml

🤓 Direct Postgres connections = connection exhaustion. Always pool.

Tags: #lfmf #postgres #redis #connection-pooling
EOF
```

## Stack Validation Gates

### Gate 1: Datum Validation

```bash
just validate-datums

# Checks:
- TOML syntax valid
- Required fields present
- Referenced datums exist
- No circular dependencies
```

### Gate 2: Composition Validation

```bash
just validate-composition

# Checks:
- Dependencies form DAG
- Healthchecks defined
- Networks configured
- Volumes accessible
```

### Gate 3: Deployment Validation

```bash
just validate-deployment

# Checks:
- All services started
- Healthchecks passing
- Connectivity between services
- Performance baselines met
```

## Datum Library

### Database Datums

```
_b00t_/
├── postgres-base.docker.toml
├── postgres-enhanced.docker.toml
├── postgres-mlflow.docker.toml
├── mysql-base.docker.toml
└── mongodb-replica.k8s.toml
```

### Cache Datums

```
_b00t_/
├── redis-base.docker.toml
├── valkey.k8s.toml
├── memcached.docker.toml
└── dragonfly.docker.toml
```

### Service Datums

```
_b00t_/
├── nginx-proxy.docker.toml
├── traefik.k8s.toml
├── fastapi.docker.toml
├── nextjs.docker.toml
└── keycloak.docker.toml
```

### Observability Datums

```
_b00t_/
├── prometheus.k8s.toml
├── grafana.docker.toml
├── loki.k8s.toml
└── tempo.k8s.toml
```

## Compounding Through Stacks

### Iteration 1: Manual Composition

```bash
# Manual docker-compose.yaml creation
→ 4 hours of configuration
→ Multiple trial-error cycles
→ Documentation scattered
```

### Iteration 2: Datum-ize Components

```bash
# Extract to datums
b00t datum extract docker-compose.yaml

# Generated:
# - _b00t_/postgres.docker.toml
# - _b00t_/redis.docker.toml
# - _b00t_/api.docker.toml

→ 30 minutes to compose similar stack
```

### Iteration 3: Stack Templates

```bash
# Create reusable stack
b00t stack template create \
  --name "fullstack-api" \
  --components postgres,redis,fastapi,nextjs

→ 10 minutes to deploy new project
```

### Iteration 4: Automated Generation

```bash
# From requirements → Stack
b00t stack generate \
  --from-requirements requirements.md \
  --output ai-dev-stack.stack.toml

→ 5 minutes automated generation
```

**Compounding: 4 hours → 30 min → 10 min → 5 min**

## GitOps Integration

### Pattern: Datum-Driven Deployments

```yaml
# .github/workflows/deploy.yaml
name: Deploy Stack

on:
  push:
    paths:
      - '_b00t_/**/*.toml'
      - 'environments/**'

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Validate Datums
        run: b00t stack validate --all

      - name: Compose Stack
        run: b00t stack compose \
          --datum ai-dev-stack.stack.toml \
          --env ${{ github.ref_name }} \
          --output stack.yaml

      - name: Deploy to Kubernetes
        run: kubectl apply -f stack.yaml

      - name: Verify Deployment
        run: b00t stack healthcheck --timeout 120s
```

## References

Detailed patterns in `references/`:

- **`docker-patterns.md`** - Docker Compose datum patterns
- **`k8s-patterns.md`** - Kubernetes manifest datums
- **`composition-strategies.md`** - Stack assembly approaches
- **`datum-schema.md`** - Datum structure reference
- **`validation-gates.md`** - Quality checkpoints

---

*Infrastructure as reusable datums. Compose once, deploy everywhere.* 🐳
