# k8s.⛵ - b00t Kubernetes Subsystem

> **Status**: ✅ MCP DEPLOYMENT READY  
> **Version**: 0.2.0  
> **Target**: minikube + kube-rs ecosystem  

## 🎯 Overview

The b00t k8s subsystem provides Kubernetes orchestration capabilities through a curated, batteries-included approach. Built on the kube-rs ecosystem, it enables seamless Docker→k8s translation, pod lifecycle management, and MCP server deployment.

### ✅ Current Working Features
- **MCP Server Deployment**: Deploy any b00t MCP server as a Kubernetes pod
- **Pod Management**: List, monitor, and manage b00t-managed pods  
- **CLI Integration**: Full `b00t-cli k8s` command suite with help and examples
- **Real-time Status**: JSON and table output formats with live pod status
- **Kubernetes Integration**: Works with any kubectl-configured cluster (minikube, etc.)

### 🚀 Quick Start

```bash
# 1. Ensure kubectl is configured for your cluster
kubectl config current-context

# 2. Create an MCP server config 
echo '[b00t]
name = "filesystem"
type = "mcp"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem"]' > ~/.dotfiles/_b00t_/filesystem.mcp.toml

# 3. Deploy MCP server to Kubernetes
b00t-cli k8s deploy-mcp --server filesystem

# 4. List running pods
b00t-cli k8s list

# 5. See help for all commands  
b00t-cli k8s --help
```

### Core Philosophy
- **NO wheel reinvention** - leverage proven kube-rs patterns
- **Agent-friendly** - transparent resource discovery & hints  
- **DWIW approach** - "do what i want" with minimal configuration
- **Data convergence ready** - designed for b00t datum graph integration

## 🏗️ Architecture

```
b00t-cli k8s
├── client/          # kube-rs client wrapper
├── resources/       # k8s resource management
├── translate/       # Docker→k8s translation engine
├── lifecycle/       # pod/resource lifecycle ops
├── discovery/       # resource discovery & sharing
└── mcp/            # MCP server deployment
```

### Component Responsibilities

#### 🔌 Client Wrapper (`client/`)
- Simplified kube-rs client interface
- Connection management & auth
- Error handling with snafu
- Minikube-specific optimizations

#### 📦 Resource Management (`resources/`)
- Pod deployment to default namespace
- Resource lifecycle (CRUD operations)
- Validation & health checks
- Cleanup & garbage collection

#### 🔄 Translation Engine (`translate/`)
- Dockerfile → Pod/Deployment specs
- docker-compose → Pod collections
- Helm chart ingestion
- LLM-powered smart transformations

#### 🔄 Lifecycle Operations (`lifecycle/`)
- Cluster setup/teardown
- Resource provisioning
- Dependency resolution
- Rollback capabilities

#### 🔍 Discovery & Sharing (`discovery/`)
- Resource inventory management
- Cross-instance communication
- Dependency hints for agents
- "Yahoo directory" of available resources

#### 🤖 MCP Integration (`mcp/`)
- MCP server pod deployment
- Service discovery & routing
- Agent development workflows
- Hot reload capabilities

## 🛠️ Technical Specification

### Dependencies
```toml
[dependencies]
kube = { version = "0.87.1", features = ["runtime", "derive"] }
k8s-openapi = { version = "0.20.0", features = ["v1_27"] }
kube-runtime = "0.87.1"
snafu = "0.7.5"
tokio = { version = "1.34.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
tracing = "0.1"
uuid = { version = "1.0", features = ["v4"] }
```

### Error Handling Strategy
Using snafu for structured error management:

```rust
use snafu::prelude::*;

#[derive(Debug, Snafu)]
pub enum K8sError {
    #[snafu(display("Failed to connect to cluster: {source}"))]
    ClusterConnection { source: kube::Error },
    
    #[snafu(display("Pod deployment failed for {name}: {reason}"))]
    PodDeployment { name: String, reason: String },
    
    #[snafu(display("Resource translation failed: {source}"))]
    Translation { source: TranslationError },
}
```

### API Design

#### Core Client Interface
```rust
pub struct B00tK8sClient {
    client: kube::Client,
    namespace: String,
}

impl B00tK8sClient {
    pub async fn new() -> Result<Self, K8sError>;
    pub async fn deploy_pod(&self, spec: PodSpec) -> Result<Pod, K8sError>;
    pub async fn list_pods(&self) -> Result<Vec<Pod>, K8sError>;
    pub async fn delete_pod(&self, name: &str) -> Result<(), K8sError>;
    pub async fn get_logs(&self, name: &str) -> Result<String, K8sError>;
}
```

#### Translation Engine Interface
```rust
pub trait ResourceTranslator {
    type Input;
    type Output;
    
    async fn translate(&self, input: Self::Input) -> Result<Self::Output, TranslationError>;
}

pub struct DockerfileTranslator;
impl ResourceTranslator for DockerfileTranslator {
    type Input = Dockerfile;
    type Output = PodSpec;
}

pub struct ComposeTranslator;
impl ResourceTranslator for ComposeTranslator {
    type Input = DockerCompose;
    type Output = Vec<k8s_openapi::api::core::v1::Pod>;
}
```

## 🔄 Data Flow Diagrams

### Pod Deployment Flow
```
[b00t-cli] → [Translator] → [Validator] → [K8s Client] → [minikube]
     ↓             ↓            ↓            ↓            ↓
[User Input] → [PodSpec] → [Validated] → [API Call] → [Running Pod]
```

### Resource Discovery Flow
```
[Agent Query] → [Discovery Service] → [Resource Inventory] 
                       ↓
                [Available Resources] → [Usage Hints] → [Agent Context]
```

## 🧪 Testing Strategy

### Test Structure
```
tests/
├── unit/           # Individual component tests
├── integration/    # kube-rs integration tests  
├── e2e/           # Full workflow tests with minikube
└── fixtures/      # Test data (JSON-based per b00t gospel)
```

### Coverage Goals
- **Unit Tests**: 90%+ coverage
- **Integration**: All API endpoints
- **E2E**: Complete workflows (deploy→validate→cleanup)
- **Error Cases**: All error paths tested

### Test Data Management
Per b00t gospel - all test data stored in JSON files:
```
tests/fixtures/
├── pods/
│   ├── simple-pod.json
│   └── multi-container.json
├── dockerfiles/
│   └── sample-app.dockerfile
└── compose/
    └── web-db-stack.yaml
```

## 🚀 Performance Targets

### Benchmarks
- **Pod Deployment**: <5s for simple pod
- **Resource Discovery**: <100ms for inventory query  
- **Translation**: <2s for typical Dockerfile
- **Cleanup**: <10s for complete teardown

### Resource Limits
- **Memory**: <50MB baseline, <200MB peak
- **CPU**: <5% sustained, <20% burst
- **Network**: Batch operations, connection pooling

## 🔒 Security Considerations

### Authentication & Authorization
- ServiceAccount-based auth for pods
- RBAC policies for least-privilege access
- Secret management via k8s secrets
- No hardcoded credentials

### Network Security
- Pod-to-pod communication via services
- Network policies for isolation
- TLS for all external communication
- Ingress gateway validation

### Vulnerability Management
- Regular dependency scanning
- Container image security scanning
- SAST/DAST integration in CI
- CVE monitoring and response

## 🔧 Integration Points

### b00t-cli Integration - ✅ IMPLEMENTED
```bash
# MCP Server Deployment - ✅ WORKING
b00t-cli k8s deploy-mcp --server <mcp-server-name>
b00t-cli k8s deploy-mcp --server filesystem
b00t-cli k8s deploy-mcp --server brave-search --namespace mcp-servers

# Pod Management - ✅ WORKING  
b00t-cli k8s list                    # Show b00t-managed pods
b00t-cli k8s list --all              # Show all pods 
b00t-cli k8s list --json             # JSON output
b00t-cli k8s list --namespace test   # Specific namespace

# Pod Operations - 🚧 PLANNED
b00t-cli k8s logs <pod-name>         # View logs
b00t-cli k8s delete <pod-name>       # Delete pod
b00t-cli k8s restart-mcp <server>    # Restart MCP server

# Deployment - 🚧 PLANNED
b00t-cli k8s deploy --from-dockerfile ./Dockerfile
b00t-cli k8s deploy --from-compose ./docker-compose.yaml  
b00t-cli k8s deploy --image nginx:latest --name web-server

# Cluster Management - 🚧 PLANNED
b00t-cli k8s status                  # Cluster status
b00t-cli k8s setup                   # Setup minikube
b00t-cli k8s teardown                # Cleanup resources
```

## 📋 Concrete Examples

### Example 0: MCP Server Deployment (✅ WORKING)

**Deploy an MCP server to Kubernetes:**

```bash
# Create MCP server configuration
echo '[b00t]
name = "filesystem"
type = "mcp"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem"]
hint = "File system access MCP server"

[b00t.env]
FILESYSTEM_ROOT = "/tmp"' > ~/.dotfiles/_b00t_/filesystem.mcp.toml

# Deploy to Kubernetes
b00t-cli k8s deploy-mcp --server filesystem
```

**Output:**
```
🤖 Deploying MCP server to Kubernetes:
   Server: filesystem
   Pod name: filesystem-mcp
   Namespace: default
✅ Loaded MCP server configuration
✅ Connected to Kubernetes cluster
🚀 Successfully deployed MCP server pod
   Pod name: filesystem-mcp
   Namespace: default
💡 Use 'b00t-cli k8s logs filesystem-mcp' to view logs
```

**List deployed pods:**
```bash
b00t-cli k8s list
```

**Output:**
```
📋 Listing b00t-managed pods in all namespaces
PODS:
✅ filesystem-mcp       | Running         | default    | filesystem-mcp
```

**JSON output:**
```bash
b00t-cli k8s list --json
```

**Output:**
```json
[
  {
    "app": "filesystem-mcp",
    "name": "filesystem-mcp",
    "namespace": "default",
    "ready": true,
    "restarts": 0,
    "running": true,
    "status": "Running"
  }
]
```

**Verify with kubectl:**
```bash
kubectl get pods -l app.kubernetes.io/managed-by=b00t
# Output: filesystem-mcp   1/1     Running   0          2m
```

### Example 1: Deploy a Simple Web Server (🚧 PLANNED)

**Input Dockerfile:**
```dockerfile
FROM nginx:alpine
COPY ./html /usr/share/nginx/html
EXPOSE 80
ENV NODE_ENV=production
```

**b00t Translation:**
```bash
b00t k8s deploy --from-dockerfile ./Dockerfile --name web-server
```

**Generated Kubernetes Resources:**
```yaml
# Pod: web-server
apiVersion: v1
kind: Pod
metadata:
  name: web-server
  namespace: default
  labels:
    app.kubernetes.io/name: web-server
    app.kubernetes.io/managed-by: b00t
    b00t.elastic.ventures/app: web-server
spec:
  containers:
  - name: web-server
    image: nginx:alpine
    ports:
    - containerPort: 80
      name: port-80
    env:
    - name: NODE_ENV
      value: production
    resources:
      requests:
        cpu: "100m"
        memory: "128Mi"
      limits:
        cpu: "500m"
        memory: "512Mi"
  restartPolicy: Always

---
# Service: web-server-service (auto-generated)
apiVersion: v1
kind: Service
metadata:
  name: web-server-service
  namespace: default
  labels:
    app.kubernetes.io/managed-by: b00t
spec:
  selector:
    app.kubernetes.io/name: web-server
  ports:
  - port: 80
    targetPort: 80
    name: port-80
  type: ClusterIP
```

### Example 2: Deploy a Multi-Service Application

**Input docker-compose.yaml:**
```yaml
version: '3.8'
services:
  web:
    image: nginx:alpine
    ports:
      - "80:8080"
    environment:
      - API_URL=http://api:3000
    depends_on:
      - api
  
  api:
    image: node:18-alpine
    ports:
      - "3000:3000"
    environment:
      - DATABASE_URL=postgres://user:pass@db:5432/myapp
    depends_on:
      - db
  
  db:
    image: postgres:15-alpine
    environment:
      - POSTGRES_DB=myapp
      - POSTGRES_USER=user
      - POSTGRES_PASSWORD=pass
    volumes:
      - db_data:/var/lib/postgresql/data

volumes:
  db_data:
```

**b00t Translation:**
```bash
b00t k8s deploy --from-compose ./docker-compose.yaml
```

**Generated Resources:**
```yaml
# Generated: 3 Pods + 3 Services
# - web pod/service (nginx:alpine)
# - api pod/service (node:18-alpine) 
# - db pod/service (postgres:15-alpine)

# Warning: Volume mounts need manual PersistentVolume setup
# Warning: Service dependencies noted for startup order
```

### Example 3: Deploy MCP Server

**MCP Server Configuration:**
```toml
# contextual-engineering.mcp.toml
[b00t]
name = "contextual-engineering"
type = "mcp"
command = "npx"
args = ["-y", "@anysphere/contextual-engineering"]
hint = "Engineering context and code analysis MCP server"
desires = "latest"

[b00t.env]
CONTEXT_DEPTH = "5"
ANALYSIS_MODE = "deep"
```

**Deploy as Pod:**
```bash
b00t k8s deploy-mcp --server contextual-engineering
```

**Generated Pod:**
```yaml
apiVersion: v1
kind: Pod
metadata:
  name: contextual-engineering-mcp
  namespace: default
  labels:
    app.kubernetes.io/name: contextual-engineering
    app.kubernetes.io/managed-by: b00t
    b00t.elastic.ventures/type: mcp-server
spec:
  containers:
  - name: contextual-engineering
    image: node:18-alpine
    command: ["npx"]
    args: ["-y", "@anysphere/contextual-engineering"]
    env:
    - name: CONTEXT_DEPTH
      value: "5"
    - name: ANALYSIS_MODE
      value: "deep"
    ports:
    - containerPort: 3000
      name: mcp-port
    resources:
      requests:
        cpu: "200m"
        memory: "256Mi"
      limits:
        cpu: "1000m"
        memory: "1Gi"
  restartPolicy: Always
```

### Example 4: Rust Application with Custom Build

**Dockerfile:**
```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/myapp /usr/local/bin/myapp
EXPOSE 8080
CMD ["myapp"]
```

**Deploy:**
```bash
b00t k8s deploy --from-dockerfile ./Dockerfile --name rust-app
```

**Usage Example:**
```bash
# Check deployment status
b00t k8s status rust-app
# Output: ✅ rust-app | Running | 1/1 ready | 0 restarts

# Get logs
b00t k8s logs rust-app
# Output: [2025-01-27T07:30:00Z] Starting server on 0.0.0.0:8080

# Scale manually (future feature)
kubectl scale --replicas=3 pod rust-app

# Port forward for testing
kubectl port-forward pod/rust-app 8080:8080
```

### Example 5: Development Workflow

**Typical Agent Development Session:**
```bash
# 1. Setup development environment
b00t k8s cluster status
# Output: ✅ minikube running | kubectl configured | 4 nodes ready

# 2. Deploy MCP server for development
b00t k8s deploy-mcp --server taskmaster-ai
# Output: ✅ Deployed taskmaster-ai-mcp | Pod running in 15s

# 3. Quick iteration: restart after code changes
b00t k8s restart-mcp taskmaster-ai
# Output: ✅ Restarted taskmaster-ai-mcp | Ready in 8s

# 4. Monitor logs during development
b00t k8s logs taskmaster-ai-mcp --follow
# Output: [streaming logs...]

# 5. Deploy a test application
echo 'FROM nginx:alpine
EXPOSE 80' | b00t k8s deploy --from-stdin --name test-nginx

# 6. List all b00t-managed resources
b00t k8s list
# Output:
# PODS:
# ✅ taskmaster-ai-mcp    | Running  | mcp-server
# ✅ test-nginx           | Running  | web-server
# 
# SERVICES:
# ✅ taskmaster-ai-service | ClusterIP | 3000/TCP
# ✅ test-nginx-service    | ClusterIP | 80/TCP

# 7. Cleanup when done
b00t k8s delete test-nginx
# Output: ✅ Deleted test-nginx pod and service
```

### Example 6: Real-World Debugging Scenario

**Problem:** MCP server fails to start
```bash
# Check pod status
b00t k8s status my-mcp-server
# Output: ❌ my-mcp-server | CrashLoopBackOff | 0/1 ready | 5 restarts

# Get detailed pod information
kubectl describe pod my-mcp-server
# Shows: Image pull failed, network issues, etc.

# Check logs for errors
b00t k8s logs my-mcp-server --previous
# Output: Error: Cannot find module '@anysphere/contextual-engineering'

# Fix: Update the MCP configuration
vim ~/.dotfiles/_b00t_/my-mcp-server.mcp.toml
# Change: args = ["-y", "@anysphere/contextual-engineering@latest"]

# Redeploy with fixed configuration
b00t k8s restart-mcp my-mcp-server
# Output: ✅ Restarted my-mcp-server | Ready in 12s
```

### Example 7: Resource Discovery and Sharing

**Agent discovers existing resources:**
```bash
# Agent checks what's available before starting its own database
b00t k8s list --type database
# Output:
# ✅ postgres-shared     | Running  | Available for connection
# ✅ redis-cache        | Running  | Available for sessions

# Agent uses existing postgres instead of creating new one
export DATABASE_URL="postgresql://postgres-shared:5432/mydb"
b00t k8s deploy --from-dockerfile ./my-app/Dockerfile --env DATABASE_URL
```

### Example 8: Error Handling and Recovery

**Translation with warnings:**
```bash
b00t k8s deploy --from-compose ./complex-app/docker-compose.yaml
```

**Output:**
```
⚠️  Translation completed with warnings:

✅ Created pods: web, api, worker (3)
✅ Created services: web-service, api-service (2)

⚠️  Warnings:
  - Service 'db' has volume mounts - you may need to create PersistentVolumes manually
  - Service 'web' has dependencies [api] - ensure proper startup order
  - Service 'worker' uses build context - you'll need to build and push the image first

📋 Next steps:
  1. Build and push custom images: docker build -t myregistry/worker:latest ./worker
  2. Create PersistentVolume for database: kubectl apply -f db-pv.yaml
  3. Update image references in generated pods
  
🔍 Generated resources saved to: .b00t/k8s/complex-app-resources.yaml
```

### MCP Server Integration
- Automatic service discovery
- Health check endpoints
- Hot reload during development  
- Agent context preservation

### Git Workflow Integration
- Resource specs stored in `.b00t/k8s/`
- GitOps-friendly YAML generation
- Conventional commit integration
- Automated testing in CI

## 📊 Monitoring & Observability

### Metrics Collection
- Pod deployment success/failure rates
- Resource utilization tracking
- Translation performance metrics
- Error rate monitoring

### Logging Strategy
- Structured logging with tracing
- Log aggregation for troubleshooting
- Audit trail for security events
- Developer-friendly log formatting

### Health Checks
- Cluster connectivity validation
- Resource health monitoring
- Dependency availability checks
- Automated recovery procedures

## 🎯 Implementation Phases

### Phase 1: Foundation ✅ COMPLETE
- [x] Research kube-rs ecosystem
- [x] Create technical specification (THIS DOC)
- [x] Implement core module structure
- [x] Basic client wrapper

### Phase 2: Core Functionality ✅ PARTIAL
- [x] Pod deployment capabilities (MCP servers)
- [x] Resource lifecycle management (basic CRUD)
- [x] Error handling implementation (snafu-based)
- [x] CLI command structure
- [ ] Basic translation engine (Docker→k8s)

### Phase 3: Advanced Features 🚧 IN PROGRESS  
- [x] MCP server deployment ✅ **WORKING**
- [x] Pod listing and management ✅ **WORKING**
- [ ] Resource discovery system
- [ ] LLM integration for translations
- [ ] Performance optimizations

### Phase 4: Production Ready 📋 PLANNED
- [x] Basic testing (manual verification)
- [ ] Comprehensive testing suite
- [ ] Security hardening
- [ ] Documentation completion
- [ ] Performance benchmarking

## 🤝 Contributing

### Development Workflow
1. Create feature branch: `git checkout -b feature/k8s-<feature>`
2. Follow TDD: write tests first
3. Implement with error handling
4. Ensure tests pass: `cargo test`
5. Format code: `cargo fmt`
6. Check lints: `cargo clippy`
7. Conventional commit: `git commit -m "feat(k8s): add pod deployment"`

### Code Standards
- Follow b00t Rust best practices
- Use `cargo add` for dependencies
- Implement proper error handling with snafu
- Document all public APIs
- Include examples in documentation

## 📚 References

### kube-rs Ecosystem
- [kube-rs/kube](https://github.com/kube-rs/kube) - Primary k8s client
- [kube-rs/controller-runtime](https://github.com/kube-rs/controller-runtime) - Controller patterns
- [stackabletech/operator-rs](https://github.com/stackabletech/operator-rs) - Operator utilities

### Documentation
- [Kubernetes API Reference](https://kubernetes.io/docs/reference/kubernetes-api/)
- [minikube Documentation](https://minikube.sigs.k8s.io/docs/)
- [Rust Kubernetes Book](https://kube.rs/controllers/intro/)

---

**🥾 b00t k8s.⛵ - Kubernetes orchestration, the agent-friendly way**