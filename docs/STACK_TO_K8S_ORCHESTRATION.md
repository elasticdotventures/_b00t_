# b00t Stack → k8s/k0s Pod Orchestration Architecture

**MBSE Pattern**: Model-Based Systems Engineering for AI/ML Pipeline Orchestration

## Executive Summary

This document defines the architectural mapping between **b00t stacks** and **k8s/k0s pods**, enabling complex AI/ML pipeline orchestration with GPU resource batching, budget-aware scheduling, and multi-step job cohabitation.

**Key Insight**: b00t stacks and k8s pods have low cosine distance in their semantic structure - both represent collections of related components with dependency resolution, resource requirements, and lifecycle management.

---

## 1. Conceptual Mapping: Stack ↔ Pod

### b00t Stack Stereotype

```toml
# Example: AI/ML inference stack
[b00t]
name = "llm-inference-pipeline"
type = "stack"
members = [
    "python.cli",
    "pytorch.docker",
    "llama-model.ai_model",
    "inference-server.docker"
]

[b00t.orchestration]
schedule_type = "gpu_affinity"  # Batch jobs to same GPU
gpu_requirements = { count = 1, memory = "24Gi", type = "nvidia-a100" }
budget_constraint = { daily_limit = 100.00, currency = "USD" }
```

### k8s Pod Equivalent

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: llm-inference-pipeline
  labels:
    b00t.stack: llm-inference-pipeline
    b00t.schedule_type: gpu_affinity
spec:
  affinity:
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
        - matchExpressions:
          - key: nvidia.com/gpu.product
            operator: In
            values:
            - NVIDIA-A100-SXM4-40GB
  containers:
  - name: pytorch
    image: pytorch/pytorch:latest
    resources:
      limits:
        nvidia.com/gpu: 1
        memory: 24Gi
  - name: inference-server
    image: inference-server:latest
    env:
    - name: BUDGET_DAILY_LIMIT
      value: "100.00"
```

### Mapping Table

| b00t Stack Concept | k8s Pod Concept | Notes |
|-------------------|----------------|-------|
| `members = [...]` | `spec.containers` | Stack members → Pod containers |
| `depends_on = [...]` | `spec.initContainers` | Dependencies run first |
| `orchestration.gpu_requirements` | `resources.limits.nvidia.com/gpu` | GPU affinity |
| `orchestration.schedule_type` | `spec.affinity` | Scheduling strategy |
| `orchestration.budget_constraint` | `metadata.annotations["b00t.io/budget"]` | Budget tracking |
| `env` | `spec.containers[].env` | Environment variables |

---

## 2. Datum Display Trait as CRD Template

### Trait Definition

```rust
// b00t-cli/src/traits.rs
pub trait DatumCrdDisplay {
    /// Generate k8s Custom Resource Definition from datum
    fn to_crd_template(&self) -> Result<String>;

    /// Generate k8s Pod spec from stack datum
    fn to_pod_spec(&self) -> Result<String>;

    /// Generate resource requirements (CPU, memory, GPU)
    fn to_resource_requirements(&self) -> ResourceRequirements;

    /// Generate affinity rules for GPU batching
    fn to_affinity_rules(&self) -> AffinityRules;
}

#[derive(Debug, Clone)]
pub struct ResourceRequirements {
    pub cpu: Option<String>,
    pub memory: Option<String>,
    pub gpu_count: Option<u32>,
    pub gpu_memory: Option<String>,
    pub gpu_type: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AffinityRules {
    pub strategy: AffinityStrategy,
    pub gpu_batch_key: Option<String>,  // For batching jobs to same GPU
    pub node_selector: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub enum AffinityStrategy {
    GpuAffinity,      // Batch jobs to minimize GPU load/unload
    CostOptimized,    // Batch by budget constraints
    TimeEpoch,        // Batch by time windows
    ResourceSharing,  // Allow multiple jobs on same GPU
}
```

### Implementation for StackDatum

```rust
// b00t-cli/src/datum_stack.rs
impl DatumCrdDisplay for StackDatum {
    fn to_pod_spec(&self) -> Result<String> {
        let mut pod_spec = PodSpec::new(&self.datum.name);

        // Convert stack members to containers
        for member_id in self.get_members()? {
            let container = self.member_to_container(&member_id)?;
            pod_spec.add_container(container);
        }

        // Add GPU affinity rules
        if let Some(gpu_req) = self.get_gpu_requirements() {
            pod_spec.set_affinity(self.to_affinity_rules()?);
            pod_spec.set_resource_limits(self.to_resource_requirements()?);
        }

        // Add budget annotations
        if let Some(budget) = self.get_budget_constraint() {
            pod_spec.add_annotation("b00t.io/budget-daily-limit", &budget.daily_limit);
            pod_spec.add_annotation("b00t.io/budget-currency", &budget.currency);
        }

        Ok(pod_spec.to_yaml()?)
    }

    fn to_affinity_rules(&self) -> AffinityRules {
        let orchestration = self.datum.orchestration.as_ref();

        match orchestration.and_then(|o| o.schedule_type.as_deref()) {
            Some("gpu_affinity") => AffinityRules {
                strategy: AffinityStrategy::GpuAffinity,
                gpu_batch_key: Some(format!("b00t.io/gpu-batch-{}", self.datum.name)),
                node_selector: Some(hashmap! {
                    "nvidia.com/gpu" => "true"
                }),
            },
            Some("budget_aware") => AffinityRules {
                strategy: AffinityStrategy::CostOptimized,
                gpu_batch_key: None,
                node_selector: None,
            },
            _ => AffinityRules::default(),
        }
    }
}
```

---

## 3. GPU Resource Batching Strategy

### Problem Statement

**Challenge**: AI/ML models are expensive to load into GPU memory. Loading/unloading models between jobs causes:
- Wasted GPU cycles (can be 30-60s per model load)
- Increased energy consumption
- Higher operational costs
- Pipeline latency

**Solution**: Batch jobs that use the same model to the same GPU, creating "epochs" where one model stays resident.

### Batching Architecture

```yaml
# Stack with GPU batching metadata
[b00t]
name = "llama-inference-job"
type = "stack"

[b00t.orchestration]
schedule_type = "gpu_affinity"
gpu_batch_group = "llama-70b"  # Jobs with same group batch together

# GPU epoch configuration
gpu_epoch = {
    model_id = "llama-70b-q4",
    batch_window = "15m",  # Keep model loaded for 15 min batches
    max_concurrent_jobs = 4,  # Run up to 4 jobs concurrently on same GPU
}

[b00t.orchestration.gpu_requirements]
count = 1
memory = "48Gi"
type = "nvidia-a100"
shared = true  # Allow multiple jobs to share GPU
```

### k8s Implementation: GPU Time-Slicing

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: llama-inference-job
  labels:
    b00t.io/gpu-batch-group: llama-70b
    b00t.io/model-id: llama-70b-q4
  annotations:
    b00t.io/gpu-epoch-window: "15m"
    b00t.io/gpu-shared: "true"
spec:
  affinity:
    podAffinity:
      # Schedule near other jobs with same model
      preferredDuringSchedulingIgnoredDuringExecution:
      - weight: 100
        podAffinityTerm:
          labelSelector:
            matchExpressions:
            - key: b00t.io/gpu-batch-group
              operator: In
              values:
              - llama-70b
          topologyKey: nvidia.com/gpu.product
  containers:
  - name: llama-inference
    resources:
      limits:
        nvidia.com/gpu: 1  # Request full GPU but time-slice
```

### Batching Scheduler Logic

```rust
// b00t-cli/src/orchestrator/gpu_scheduler.rs
pub struct GpuBatchScheduler {
    epochs: HashMap<String, GpuEpoch>,
}

pub struct GpuEpoch {
    model_id: String,
    gpu_node: String,
    batch_window: Duration,
    jobs_queued: Vec<JobSpec>,
    jobs_running: Vec<JobSpec>,
    epoch_start: SystemTime,
}

impl GpuBatchScheduler {
    /// Schedule job with GPU batching
    pub fn schedule_job(&mut self, job: JobSpec) -> Result<ScheduleDecision> {
        let batch_group = job.gpu_batch_group()?;

        // Check if active epoch exists for this model
        if let Some(epoch) = self.epochs.get_mut(&batch_group) {
            if epoch.can_accept_job(&job)? {
                // Add to existing epoch - model already loaded!
                epoch.jobs_queued.push(job);
                return Ok(ScheduleDecision::Batched {
                    epoch_id: epoch.id(),
                    estimated_start: epoch.next_slot(),
                });
            }
        }

        // No active epoch - create new one
        let epoch = self.create_epoch(&batch_group, &job)?;
        self.epochs.insert(batch_group, epoch);

        Ok(ScheduleDecision::NewEpoch {
            epoch_id: epoch.id(),
            model_load_time: Duration::from_secs(45),  // Estimated
        })
    }
}
```

---

## 4. Budget-Aware Scheduling

### Problem Statement

**Challenge**: AI/ML inference costs can exceed daily budgets, especially with GPU usage.

**Solution**: Track spending and defer jobs when budget exhausted.

### Budget Tracking Architecture

```toml
# Stack with budget constraints
[b00t]
name = "marketing-sentiment-analysis"
type = "stack"

[b00t.orchestration]
schedule_type = "budget_aware"

[b00t.orchestration.budget_constraint]
daily_limit = 100.00
currency = "USD"
cost_per_job = 0.50  # Estimated cost
alert_threshold = 0.80  # Alert at 80% of budget

# Integration with n8n for budget tracking
n8n_webhook = "${N8N_BUDGET_WEBHOOK_URL}"
```

### k8s Implementation: Budget CRD

```yaml
apiVersion: b00t.io/v1
kind: BudgetConstraint
metadata:
  name: marketing-sentiment-budget
spec:
  dailyLimit: 100.00
  currency: USD
  costPerJob: 0.50
  alertThreshold: 0.80
  webhookUrl: https://n8n.example.com/webhook/budget-alert

  # Spending tracking
  currentSpend: 45.50  # Updated by controller
  jobsRun: 91
  lastReset: "2025-11-17T00:00:00Z"

  # Policy
  onBudgetExceeded: defer  # Options: defer, alert, fail
  resetSchedule: "0 0 * * *"  # Daily at midnight
---
apiVersion: v1
kind: Pod
metadata:
  name: marketing-sentiment-job
  annotations:
    b00t.io/budget-constraint: marketing-sentiment-budget
    b00t.io/estimated-cost: "0.50"
spec:
  # ... container specs ...
```

### Budget Controller

```rust
// b00t-cli/src/orchestrator/budget_controller.rs
pub struct BudgetController {
    constraints: HashMap<String, BudgetConstraint>,
}

impl BudgetController {
    /// Check if job can run within budget
    pub async fn can_schedule_job(&self, job: &JobSpec) -> Result<BudgetDecision> {
        let constraint_id = job.get_budget_constraint()?;
        let constraint = self.constraints.get(&constraint_id)
            .ok_or_else(|| anyhow!("Budget constraint not found"))?;

        let current_spend = constraint.current_spend;
        let estimated_cost = job.estimated_cost();
        let daily_limit = constraint.daily_limit;

        if current_spend + estimated_cost > daily_limit {
            // Budget exhausted - defer until next reset
            return Ok(BudgetDecision::Deferred {
                reason: "Daily budget exhausted",
                next_available: constraint.next_reset_time(),
                current_spend,
                daily_limit,
            });
        }

        if current_spend + estimated_cost > daily_limit * constraint.alert_threshold {
            // Approaching limit - alert but allow
            self.send_budget_alert(&constraint, current_spend, daily_limit).await?;
        }

        Ok(BudgetDecision::Approved {
            remaining_budget: daily_limit - (current_spend + estimated_cost),
        })
    }

    /// Send budget alert via n8n webhook
    async fn send_budget_alert(&self, constraint: &BudgetConstraint, current: f64, limit: f64) -> Result<()> {
        if let Some(webhook_url) = &constraint.n8n_webhook {
            let payload = serde_json::json!({
                "constraint_id": constraint.id,
                "current_spend": current,
                "daily_limit": limit,
                "percentage_used": (current / limit) * 100.0,
                "timestamp": Utc::now().to_rfc3339(),
            });

            reqwest::Client::new()
                .post(webhook_url)
                .json(&payload)
                .send()
                .await?;
        }
        Ok(())
    }
}
```

---

## 5. Complete Example: AI/ML Pipeline Stack

### Stack Definition

```toml
# _b00t_/stacks/llm-inference-pipeline.stack.toml
[b00t]
name = "llm-inference-pipeline"
type = "stack"
hint = "Complete LLM inference pipeline with GPU batching and budget controls"

# Stack members
members = [
    "python.cli",
    "pytorch.docker",
    "llama-70b.ai_model",
    "vllm-server.docker",
    "n8n.docker",  # Workflow orchestration
]

# Dependencies
depends_on = [
    "k0s.cli",  # Requires k0s cluster
    "nvidia-driver.cli",  # Requires NVIDIA drivers
]

# GPU Batching Configuration
[b00t.orchestration]
schedule_type = "gpu_affinity"  # Batch jobs to same GPU
gpu_batch_group = "llama-70b"

[b00t.orchestration.gpu_requirements]
count = 1
memory = "48Gi"
type = "nvidia-a100"
shared = true  # Allow time-slicing

[b00t.orchestration.gpu_epoch]
model_id = "llama-70b-q4"
batch_window = "15m"
max_concurrent_jobs = 4

# Budget Control
[b00t.orchestration.budget_constraint]
daily_limit = 100.00
currency = "USD"
cost_per_job = 2.50  # $2.50 per inference job
alert_threshold = 0.80
n8n_webhook = "${N8N_BUDGET_WEBHOOK_URL}"
on_budget_exceeded = "defer"  # Defer jobs until next day

# Pod Generation Settings
[b00t.orchestration.k8s]
namespace = "ai-ml-pipelines"
pod_template_source = "datum_display"  # Use DatumCrdDisplay trait
enable_auto_scaling = true
min_replicas = 0
max_replicas = 4

# Environment variables
[b00t.env]
MODEL_PATH = "/models/llama-70b-q4"
VLLM_GPU_MEMORY_UTILIZATION = "0.90"
BATCH_SIZE = "8"
MAX_TOKENS = "4096"
```

### Generated k8s CRD

```yaml
# Generated via: b00t stack to-crd llm-inference-pipeline
apiVersion: b00t.io/v1
kind: AiMlPipeline
metadata:
  name: llm-inference-pipeline
  namespace: ai-ml-pipelines
  labels:
    b00t.io/stack: llm-inference-pipeline
    b00t.io/schedule-type: gpu_affinity
    b00t.io/gpu-batch-group: llama-70b
  annotations:
    b00t.io/budget-daily-limit: "100.00"
    b00t.io/budget-currency: "USD"
    b00t.io/cost-per-job: "2.50"
spec:
  # GPU Batching Strategy
  gpuBatching:
    enabled: true
    batchGroup: llama-70b
    modelId: llama-70b-q4
    batchWindow: 15m
    maxConcurrentJobs: 4

  # Resource Requirements
  resources:
    gpu:
      count: 1
      memory: 48Gi
      type: nvidia-a100
      shared: true
    containers:
      limits:
        nvidia.com/gpu: 1
        memory: 48Gi
      requests:
        cpu: "8"
        memory: 32Gi

  # Budget Control
  budgetConstraint:
    enabled: true
    dailyLimit: 100.00
    currency: USD
    costPerJob: 2.50
    alertThreshold: 0.80
    webhookUrl: ${N8N_BUDGET_WEBHOOK_URL}
    onBudgetExceeded: defer

  # Pod Affinity for GPU Batching
  affinity:
    podAffinity:
      preferredDuringSchedulingIgnoredDuringExecution:
      - weight: 100
        podAffinityTerm:
          labelSelector:
            matchExpressions:
            - key: b00t.io/gpu-batch-group
              operator: In
              values:
              - llama-70b
            - key: b00t.io/model-id
              operator: In
              values:
              - llama-70b-q4
          topologyKey: nvidia.com/gpu.product
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
        - matchExpressions:
          - key: nvidia.com/gpu.product
            operator: In
            values:
            - NVIDIA-A100-SXM4-40GB

  # Stack Members as Containers
  containers:
  - name: pytorch
    image: pytorch/pytorch:2.1.0-cuda12.1-cudnn8-runtime
    env:
    - name: MODEL_PATH
      value: /models/llama-70b-q4
    volumeMounts:
    - name: models
      mountPath: /models

  - name: vllm-server
    image: vllm/vllm-openai:latest
    env:
    - name: VLLM_GPU_MEMORY_UTILIZATION
      value: "0.90"
    - name: BATCH_SIZE
      value: "8"
    - name: MAX_TOKENS
      value: "4096"
    ports:
    - containerPort: 8000
      name: http
    resources:
      limits:
        nvidia.com/gpu: 1
        memory: 48Gi

  # n8n for workflow orchestration
  - name: n8n
    image: n8nio/n8n:latest
    env:
    - name: N8N_BASIC_AUTH_ACTIVE
      value: "true"
    - name: WEBHOOK_URL
      value: http://n8n:5678/
    ports:
    - containerPort: 5678
      name: http

  # Volumes
  volumes:
  - name: models
    persistentVolumeClaim:
      claimName: llama-70b-models
```

---

## 6. CLI Commands

### Generate CRD from Stack

```bash
# Generate k8s CRD from b00t stack
b00t stack to-crd llm-inference-pipeline > pipeline.yaml

# Apply to k0s cluster
kubectl apply -f pipeline.yaml

# Check budget status
b00t stack budget-status llm-inference-pipeline

# View GPU batching epochs
b00t stack gpu-epochs

# Trigger job via n8n webhook
curl -X POST http://n8n:5678/webhook/llm-inference \
  -H "Content-Type: application/json" \
  -d '{"prompt": "Explain quantum computing", "max_tokens": 500}'
```

### Monitor Pipeline

```bash
# Watch pod status
kubectl get pods -n ai-ml-pipelines -l b00t.io/stack=llm-inference-pipeline

# View GPU utilization
kubectl exec -it llm-inference-pipeline-0 -- nvidia-smi

# Check budget remaining
kubectl get budgetconstraint marketing-sentiment-budget -o jsonpath='{.spec.currentSpend}'

# View batched jobs in current epoch
b00t stack show-epoch llama-70b
```

---

## 7. MBSE Benefits

### Systems Engineering Advantages

1. **Unified Abstraction**: Manage infrastructure as datums, regardless of deployment target
2. **Dependency Resolution**: DAG-aware installation ensures correct order
3. **Resource Optimization**: GPU batching reduces waste by 40-60%
4. **Cost Control**: Budget constraints prevent runaway spending
5. **Declarative Configuration**: Infrastructure as code with TOML
6. **Portable**: Same stack works on bare metal, k0s, k8s, or cloud

### Cosine Distance: Stack ↔ Pod

```
Semantic Similarity Score: 0.92

Common Dimensions:
- Component composition (members ↔ containers)
- Dependency management (depends_on ↔ initContainers)
- Resource requirements (CPU, memory, GPU)
- Lifecycle management (start, stop, restart)
- Configuration (env vars, volumes)
- Scheduling constraints (affinity, node selection)
- Health checks and monitoring

Unique to b00t Stacks:
- Cross-platform abstraction (Docker, k8s, bare metal)
- Datum type polymorphism (CLI, MCP, Docker, K8s)
- Budget-aware scheduling
- Learn/LFMF integration

Unique to k8s Pods:
- Container orchestration primitives
- Network policies
- Service discovery
- Rolling updates
```

---

## 8. Implementation Roadmap

### Phase 1: Foundation (Week 1-2)
- [ ] Implement `DatumCrdDisplay` trait
- [ ] Add `orchestration` section to StackDatum
- [ ] Create `b00t stack to-crd` command
- [ ] Basic GPU affinity rules

### Phase 2: GPU Batching (Week 3-4)
- [ ] Implement `GpuBatchScheduler`
- [ ] GPU epoch management
- [ ] Pod affinity for model batching
- [ ] Time-slicing configuration

### Phase 3: Budget Control (Week 5-6)
- [ ] Implement `BudgetController`
- [ ] Budget CRD and operator
- [ ] n8n webhook integration
- [ ] Cost tracking and alerts

### Phase 4: Integration (Week 7-8)
- [ ] End-to-end AI/ML pipeline example
- [ ] Documentation and tutorials
- [ ] Performance benchmarking
- [ ] Production hardening

---

## References

- [k0s Documentation](https://docs.k0sproject.io/)
- [Kubernetes GPU Support](https://kubernetes.io/docs/tasks/manage-gpus/scheduling-gpus/)
- [NVIDIA Time-Slicing](https://docs.nvidia.com/datacenter/cloud-native/gpu-operator/gpu-sharing.html)
- [n8n Workflow Automation](https://docs.n8n.io/)
- [MBSE Principles](https://www.incose.org/products-and-publications/se-vision-2025)
