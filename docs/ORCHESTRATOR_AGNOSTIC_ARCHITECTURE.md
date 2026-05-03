# Orchestrator-Agnostic Architecture: Entangling b00t Datums with MCP Servers

> **Philosophy**: b00t is never meant to be custom orchestration; it's a native plugin layer that describes intent and delegates execution to battle-tested orchestrators.

## The Problem

Current Job/Stack datums are too k8s-specific:
- `initContainers` assumes k8s
- `queue_name` assumes Kueue
- Service discovery patterns assume k8s Services
- Not portable to docker-compose, Nomad, or other orchestrators

## The Solution: Three-Layer Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  INTENT LAYER: b00t Datums (Orchestrator-Agnostic)         │
│  - Describe WHAT to run (Job, Stack)                        │
│  - Describe REQUIREMENTS (resources, dependencies)          │
│  - Describe CONSTRAINTS (budget, GPU affinity)              │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────┐
│  ADAPTER LAYER: Orchestrator-Specific Translation          │
│  - K8sAdapter: Job → k8s Job + initContainers               │
│  - ComposeAdapter: Stack → docker-compose services          │
│  - NomadAdapter: Job → Nomad job spec                       │
│  - DirectAdapter: No orchestrator, run locally              │
└──────────────────┬──────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────┐
│  EXECUTION LAYER: MCP Servers (Tool Calls)                  │
│  - kubernetes-mcp: kubectl operations                       │
│  - docker-mcp: docker-compose operations                    │
│  - filesystem-mcp: Local execution                          │
└─────────────────────────────────────────────────────────────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────────────┐
│  ORCHESTRATOR: Actual Runtime                               │
│  - k8s/k0s (via kubectl)                                    │
│  - docker-compose (via docker CLI)                          │
│  - Nomad (via nomad CLI)                                    │
│  - systemd (via systemctl)                                  │
└─────────────────────────────────────────────────────────────┘
```

## Intent Layer: Orchestrator-Agnostic Datums

### Job Datum (Abstract Intent)

```toml
[b00t]
name = "llm-batch-job"
type = "job"
hint = "Batch inference job for LLM pipeline"

# Container specification (portable across orchestrators)
[b00t.container]
image = "ghcr.io/my-org/llm-batch-processor:latest"
command = ["python"]
args = ["-m", "batch_processor", "--model", "llama-70b-q4"]

# Environment (portable)
[b00t.env]
MODEL_ENDPOINT = "${STACK:llm-inference-pipeline:n8n:endpoint}"  # Abstract reference
BATCH_SIZE = "32"
OUTPUT_PATH = "/data/results"

# Dependencies (abstract - no orchestrator-specific implementation)
[b00t.dependencies]
requires_stacks = ["llm-inference-pipeline"]  # Abstract: "needs these services running"
wait_for_services = ["n8n:5678", "python:8000"]  # Abstract: "wait until these are ready"

# Resource requirements (portable)
[b00t.resources]
cpu = "2"
memory = "8Gi"
gpu_count = 1
gpu_type = "nvidia-a100"
gpu_shared = true

# Orchestration hints (not implementation)
[b00t.orchestration]
schedule_type = "queue_based"  # Hint: use queuing if available
batch_group = "llama-70b"  # Hint: batch similar jobs
budget_daily_limit = 50.00
budget_cost_per_job = 1.25
on_budget_exceeded = "defer"
```

### Stack Datum (Abstract Intent)

```toml
[b00t]
name = "llm-inference-pipeline"
type = "stack"
hint = "LLM inference services"
members = ["python.cli", "n8n.docker"]

# Stack-level orchestration hints
[b00t.orchestration]
gpu_affinity = "llama-70b"  # Hint: co-locate on same GPU
budget_daily_limit = 100.00

# Service discovery metadata (abstract)
[b00t.services]
# These get exposed differently based on orchestrator:
# - k8s: Services with DNS
# - docker-compose: service names
# - local: localhost ports
n8n = { port = 5678, protocol = "http" }
python = { port = 8000, protocol = "http" }
```

## Adapter Layer: Orchestrator-Specific Translation

### K8sAdapter

Translates abstract datums → k8s primitives:

```rust
pub struct K8sAdapter {
    mcp_client: Option<McpClient>,  // Optional: use kubernetes-mcp
}

impl OrchestratorAdapter for K8sAdapter {
    fn translate_job(&self, job: &JobDatum) -> Result<AdapterOutput> {
        // Generate k8s Job manifest
        let mut manifest = k8s_job_template(job)?;

        // Translate abstract dependencies → initContainers
        if let Some(deps) = &job.dependencies {
            for service in &deps.wait_for_services {
                manifest.add_init_container(wait_for_service_init(service)?);
            }
        }

        // Translate orchestration hints → k8s annotations
        if let Some(orch) = &job.orchestration {
            if orch.schedule_type == Some("queue_based") {
                manifest.add_label("kueue.x-k8s.io/queue-name", "gpu-queue");
            }
        }

        AdapterOutput {
            orchestrator: "kubernetes",
            manifests: vec![manifest],
            mcp_commands: self.generate_mcp_commands(&manifest)?,
        }
    }

    fn translate_stack(&self, stack: &StackDatum) -> Result<AdapterOutput> {
        // Use existing kompose integration
        let compose = stack.generate_docker_compose()?;
        let k8s_manifests = kompose_convert(&compose)?;

        // Enhance with orchestration metadata
        let enhanced = enhance_with_budget_annotations(k8s_manifests, stack)?;

        AdapterOutput {
            orchestrator: "kubernetes",
            manifests: enhanced,
            mcp_commands: self.generate_mcp_commands(&enhanced)?,
        }
    }
}
```

**Output**:
- k8s YAML manifests
- MCP tool calls for kubernetes-mcp server: `kubectl_apply`, `kubectl_get`, etc.

### ComposeAdapter

Translates abstract datums → docker-compose:

```rust
pub struct ComposeAdapter;

impl OrchestratorAdapter for ComposeAdapter {
    fn translate_job(&self, job: &JobDatum) -> Result<AdapterOutput> {
        // Job → docker-compose service with restart: "no"
        let service = ComposeService {
            image: job.container.image.clone(),
            command: job.container.command.clone(),
            environment: job.env.clone(),
            restart: "no",  // Jobs run once
            depends_on: self.translate_dependencies(&job.dependencies)?,
        };

        AdapterOutput {
            orchestrator: "docker-compose",
            manifests: vec![compose_yaml(service)],
            mcp_commands: vec!["docker-compose up --abort-on-container-exit"],
        }
    }

    fn translate_dependencies(&self, deps: &Option<Dependencies>) -> Result<Vec<String>> {
        // Abstract dependencies → docker-compose depends_on
        if let Some(d) = deps {
            Ok(d.requires_stacks.clone())
        } else {
            Ok(vec![])
        }
    }
}
```

**Output**:
- docker-compose.yml
- Shell commands or docker-mcp tool calls

### NomadAdapter

Translates abstract datums → Nomad job spec:

```rust
pub struct NomadAdapter;

impl OrchestratorAdapter for NomadAdapter {
    fn translate_job(&self, job: &JobDatum) -> Result<AdapterOutput> {
        // Job → Nomad job spec (HCL or JSON)
        let nomad_job = NomadJob {
            job: NomadJobSpec {
                id: job.name.clone(),
                type_: "batch",  // One-time execution
                datacenters: vec!["dc1"],
                task_groups: vec![
                    TaskGroup {
                        name: &job.name,
                        tasks: vec![
                            Task {
                                name: &job.name,
                                driver: "docker",
                                config: {
                                    image: &job.container.image,
                                    command: &job.container.command,
                                },
                                resources: Resources {
                                    cpu: parse_cpu(&job.resources.cpu)?,
                                    memory: parse_memory(&job.resources.memory)?,
                                },
                            }
                        ],
                    }
                ],
            }
        };

        AdapterOutput {
            orchestrator: "nomad",
            manifests: vec![nomad_job.to_hcl()],
            mcp_commands: vec!["nomad job run job.hcl"],
        }
    }
}
```

**Output**:
- Nomad HCL job spec
- nomad CLI commands

### DirectAdapter (No Orchestrator)

Translates abstract datums → local execution:

```rust
pub struct DirectAdapter;

impl OrchestratorAdapter for DirectAdapter {
    fn translate_job(&self, job: &JobDatum) -> Result<AdapterOutput> {
        // Job → systemd unit or simple bash script
        let script = format!(
            "#!/bin/bash\n\
             export {env}\n\
             {command} {args}",
            env = format_env(&job.env),
            command = job.container.command.join(" "),
            args = job.container.args.join(" "),
        );

        AdapterOutput {
            orchestrator: "direct",
            manifests: vec![script],
            mcp_commands: vec!["bash run.sh"],
        }
    }
}
```

**Output**:
- Bash script or systemd unit
- Direct shell commands

## Execution Layer: MCP Server Integration

### Agent Workflow

When an agent wants to deploy a job:

```rust
// 1. Agent reads job datum (orchestrator-agnostic)
let job = JobDatum::from_config("llm-batch-job", "_b00t_")?;

// 2. Detect available orchestrator
let orchestrator = detect_orchestrator()?;  // k8s, docker-compose, nomad, direct

// 3. Select appropriate adapter
let adapter: Box<dyn OrchestratorAdapter> = match orchestrator {
    Orchestrator::Kubernetes => Box::new(K8sAdapter::new()),
    Orchestrator::DockerCompose => Box::new(ComposeAdapter::new()),
    Orchestrator::Nomad => Box::new(NomadAdapter::new()),
    Orchestrator::Direct => Box::new(DirectAdapter::new()),
};

// 4. Translate intent → orchestrator-specific format
let output = adapter.translate_job(&job)?;

// 5. Generate MCP tool calls for agent
println!("📋 Generated MCP commands:");
for cmd in &output.mcp_commands {
    println!("  {}", cmd);
}

// 6. Agent executes via MCP server
// For k8s:
//   mcp_call("kubernetes-mcp", "kubectl_apply", manifest)
// For docker-compose:
//   mcp_call("docker-mcp", "docker_compose_up", compose_file)
```

### Auto-Detection Logic

```rust
pub fn detect_orchestrator() -> Result<Orchestrator> {
    // 1. Check for kubernetes context
    if Command::new("kubectl").arg("config").arg("current-context").output().is_ok() {
        return Ok(Orchestrator::Kubernetes);
    }

    // 2. Check for docker-compose
    if Command::new("docker-compose").arg("version").output().is_ok() {
        return Ok(Orchestrator::DockerCompose);
    }

    // 3. Check for Nomad
    if Command::new("nomad").arg("version").output().is_ok() {
        return Ok(Orchestrator::Nomad);
    }

    // 4. Fallback to direct execution
    Ok(Orchestrator::Direct)
}
```

### MCP Tool Call Generation

```rust
pub struct McpCommand {
    pub server: String,      // "kubernetes-mcp", "docker-mcp"
    pub tool: String,        // "kubectl_apply", "docker_compose_up"
    pub arguments: Value,    // JSON arguments
}

impl K8sAdapter {
    fn generate_mcp_commands(&self, manifest: &str) -> Result<Vec<McpCommand>> {
        vec![
            McpCommand {
                server: "kubernetes-mcp".to_string(),
                tool: "kubectl_apply".to_string(),
                arguments: json!({
                    "manifest": manifest,
                    "namespace": "default",
                }),
            }
        ]
    }
}
```

## Integration with Existing b00t k8s Subsystem

### Current Capabilities (from k8s.🚢/README.md)

✅ Already implemented:
- MCP server deployment to k8s
- Pod management (list, logs, delete)
- kube-rs client wrapper
- CLI commands: `b00t-cli k8s deploy-mcp`, `b00t-cli k8s list`

### Enhanced Integration

```rust
// b00t-cli/src/commands/job.rs
pub fn job_deploy(name: &str, orchestrator: Option<String>) -> Result<()> {
    let job = JobDatum::from_config(name, "_b00t_")?;

    // Auto-detect or use specified orchestrator
    let orch = if let Some(o) = orchestrator {
        Orchestrator::from_str(&o)?
    } else {
        detect_orchestrator()?
    };

    // Select adapter
    let adapter = create_adapter(orch)?;

    // Translate
    let output = adapter.translate_job(&job)?;

    match orch {
        Orchestrator::Kubernetes => {
            // Use existing b00t k8s subsystem
            deploy_to_k8s_via_mcp(&output)?;
        }
        Orchestrator::DockerCompose => {
            // Write docker-compose.yml and run
            deploy_to_docker_compose(&output)?;
        }
        _ => {
            // Generic execution
            execute_manifests(&output)?;
        }
    }

    Ok(())
}

fn deploy_to_k8s_via_mcp(output: &AdapterOutput) -> Result<()> {
    // Option 1: Use kubernetes-mcp server
    if let Some(mcp) = McpClient::connect("kubernetes-mcp")? {
        for cmd in &output.mcp_commands {
            mcp.call_tool(&cmd.tool, &cmd.arguments)?;
        }
    }
    // Option 2: Use b00t-cli k8s subsystem directly
    else {
        use crate::k8s;
        let client = k8s::client::B00tK8sClient::new().await?;
        for manifest in &output.manifests {
            client.apply_manifest(manifest).await?;
        }
    }
    Ok(())
}
```

## CLI Commands (Orchestrator-Agnostic)

```bash
# Auto-detect orchestrator and deploy
b00t job deploy llm-batch-job

# Specify orchestrator explicitly
b00t job deploy llm-batch-job --orchestrator kubernetes
b00t job deploy llm-batch-job --orchestrator docker-compose
b00t job deploy llm-batch-job --orchestrator nomad

# Generate manifests without deploying (for inspection)
b00t job to-manifest llm-batch-job --orchestrator kubernetes
# Output: llm-batch-job-k8s.yaml

b00t job to-manifest llm-batch-job --orchestrator docker-compose
# Output: llm-batch-job-compose.yaml

# Stack deployment (same pattern)
b00t stack deploy llm-inference-pipeline
b00t stack deploy llm-inference-pipeline --orchestrator kubernetes

# List available orchestrators
b00t orchestrator list
# Output:
# ✅ kubernetes (kubectl configured)
# ✅ docker-compose (docker installed)
# ❌ nomad (not available)
# ✅ direct (always available)

# Set default orchestrator
b00t orchestrator set-default kubernetes
```

## Benefits of This Architecture

| Aspect | Before (k8s-specific) | After (orchestrator-agnostic) |
|--------|----------------------|-------------------------------|
| **Portability** | Locked to k8s | Works with k8s, docker-compose, Nomad, systemd |
| **Intent** | Mixed with implementation | Pure intent, no orchestrator details |
| **Adapters** | N/A | Pluggable adapters for any orchestrator |
| **MCP Integration** | Manual kubectl calls | Generated MCP tool calls |
| **Agent-Friendly** | Agent needs k8s knowledge | Agent uses abstract datums |
| **Testing** | Requires k8s cluster | Can test with docker-compose or direct |
| **Development** | Heavy k8s setup | Lightweight local testing |

## Implementation Plan

### Phase 1: Core Abstraction
1. ✅ Define orchestrator-agnostic datum schema
2. Create `OrchestratorAdapter` trait
3. Implement `detect_orchestrator()`
4. Create `AdapterOutput` struct

### Phase 2: Adapters
1. Implement `K8sAdapter` (priority - already have k8s subsystem)
2. Implement `ComposeAdapter` (leverage existing kompose)
3. Implement `DirectAdapter` (simple, for testing)
4. Implement `NomadAdapter` (future)

### Phase 3: MCP Integration
1. Create `McpCommand` struct
2. Implement MCP command generation in adapters
3. Integrate with kubernetes-mcp server
4. Create docker-mcp integration (if needed)

### Phase 4: CLI & Documentation
1. Add `b00t job deploy` command
2. Add `b00t stack deploy` command
3. Add `b00t orchestrator` commands
4. Update Job/Stack datum examples to be orchestrator-agnostic
5. Document entanglement architecture (THIS DOC)

## Reference Implementations

### Abstract Service Discovery

Instead of k8s-specific:
```yaml
# ❌ k8s-specific
env:
  - name: MODEL_ENDPOINT
    value: http://n8n:5678  # Assumes k8s DNS
```

Use abstract references:
```toml
# ✅ Orchestrator-agnostic
[b00t.env]
MODEL_ENDPOINT = "${STACK:llm-inference-pipeline:n8n:endpoint}"
```

Adapters resolve:
- K8s: `http://n8n:5678` (Service DNS)
- docker-compose: `http://n8n:5678` (service name)
- Direct: `http://localhost:5678` (port mapping)

### Abstract Dependency Waiting

Instead of k8s-specific:
```yaml
# ❌ k8s-specific initContainer
initContainers:
  - name: wait-for-n8n
    image: busybox
    command: ["sh", "-c", "until nc -z n8n 5678; do sleep 2; done"]
```

Use abstract dependencies:
```toml
# ✅ Orchestrator-agnostic
[b00t.dependencies]
wait_for_services = ["n8n:5678", "python:8000"]
```

Adapters implement:
- K8s: initContainer with `nc -z` check
- docker-compose: `depends_on` with healthcheck
- Direct: Simple curl retry loop

### Abstract Resource Constraints

Instead of k8s-specific:
```yaml
# ❌ k8s-specific
resources:
  requests:
    nvidia.com/gpu: 1
```

Use abstract resources:
```toml
# ✅ Orchestrator-agnostic
[b00t.resources]
gpu_count = 1
gpu_type = "nvidia-a100"
```

Adapters translate:
- K8s: `nvidia.com/gpu: 1` + node selector
- docker-compose: `--gpus 1`
- Nomad: `device "nvidia/gpu" { count = 1 }`

## Entanglement Summary

```
b00t Datum (Intent)
    ↓ [reads]
b00t-cli (Adapter Selection)
    ↓ [translates via]
OrchestratorAdapter (K8s/Compose/Nomad/Direct)
    ↓ [generates]
Manifests + MCP Commands
    ↓ [executes via]
MCP Server (kubernetes-mcp, docker-mcp, etc.)
    ↓ [calls]
Orchestrator CLI (kubectl, docker-compose, nomad)
    ↓ [deploys to]
Runtime (k8s cluster, docker, systemd)
```

**Key Principle**: Each layer is loosely coupled via well-defined interfaces. b00t provides the abstraction layer that makes AI agents orchestrator-agnostic while leveraging battle-tested tools at every level.

---

**Next Steps**: Implement `OrchestratorAdapter` trait and K8sAdapter to prove the concept works with existing b00t k8s subsystem and kubernetes-mcp server.
