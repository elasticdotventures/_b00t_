# Job-Stack Dependencies: Leveraging k8s-Native Patterns

## Philosophy: Don't Reinvent the Wheel

Instead of building custom orchestration, we leverage existing k8s ecosystem tools:

- **Kueue**: Job queuing, quotas, GPU sharing
- **k8s Jobs**: One-time execution
- **k8s Services**: Service discovery
- **initContainers**: Service readiness checks

## Pattern: Jobs Requiring Stacks

### Problem
You have a batch job that needs certain services running (e.g., inference server, database, cache).

### b00t Solution
1. Define a **Stack** datum (collection of services)
2. Define a **Job** datum that requires that stack
3. Generate k8s manifests with proper dependencies

### Example: LLM Batch Processing

#### Stack: llm-inference-pipeline.stack.toml
```toml
[b00t]
name = "llm-inference-pipeline"
type = "stack"
members = ["python.cli", "n8n.docker"]

[b00t.orchestration]
schedule_type = "gpu_affinity"
gpu_batch_group = "llama-70b"
```

#### Job: llm-batch-job.job.toml
```toml
[b00t]
name = "llm-batch-job"
type = "job"
image = "my-batch-processor:latest"

[b00t.orchestration]
requires_stacks = ["llm-inference-pipeline"]  # Stack must be running
queue_name = "gpu-queue"  # Kueue queue for scheduling
```

## Generated k8s Manifests

### Stack → Deployments + Services
```bash
b00t stack to-k8s llm-inference-pipeline
# Generates:
# - n8n-deployment.yaml
# - n8n-service.yaml  ← Service endpoint
# - python-deployment.yaml
```

### Job → Job + initContainer
```bash
b00t job to-k8s llm-batch-job
# Generates job.yaml:
```

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: llm-batch-job
  labels:
    kueue.x-k8s.io/queue-name: gpu-queue  # Kueue integration
spec:
  template:
    spec:
      # Wait for stack services to be ready
      initContainers:
        - name: wait-for-n8n
          image: busybox:1.36
          command:
            - sh
            - -c
            - |
              until nc -z n8n 5678; do
                echo "Waiting for n8n service..."
                sleep 2
              done
              echo "n8n is ready"

      # Main job container
      containers:
        - name: batch-processor
          image: my-batch-processor:latest
          env:
            - name: MODEL_ENDPOINT
              value: http://n8n:5678  # Stack service
          resources:
            requests:
              nvidia.com/gpu: 1  # Share GPU with stack
              memory: 8Gi

      restartPolicy: Never
```

## Benefits of This Approach

| Aspect | Custom Orchestrator | k8s-Native Pattern |
|--------|---------------------|-------------------|
| Service Discovery | Build DNS/registry | ✅ k8s Services (free) |
| Readiness Checks | Custom health checks | ✅ initContainers (built-in) |
| Job Queuing | Build queue system | ✅ Kueue (battle-tested) |
| GPU Sharing | Custom time-slicing | ✅ Kueue GPU support |
| Budget Quotas | Custom budget tracker | ✅ Kueue ResourceQuotas |
| Dependency DAG | Custom DAG engine | ✅ Job dependencies + Argo |

## Integration with Existing Tools

### Kueue for Job Scheduling
```yaml
# Kueue ClusterQueue with budget
apiVersion: kueue.x-k8s.io/v1beta1
kind: ClusterQueue
metadata:
  name: gpu-queue
spec:
  cohort: main
  resourceGroups:
    - coveredResources: ["cpu", "memory", "nvidia.com/gpu"]
      flavors:
        - name: a100-gpu
          resources:
            - name: "nvidia.com/gpu"
              nominalQuota: 8  # Total GPU quota
```

b00t orchestration metadata → Kueue ResourceQuota:
```bash
b00t budget check llm-inference-pipeline
# Daily Limit: $100 USD
# → Translates to Kueue quota: 40 jobs/day @ $2.50/job
```

### Argo Workflows for Complex DAGs
For multi-step pipelines:
```bash
b00t stack to-argo llm-inference-pipeline
# Generates Argo Workflow with:
# - Stack deployment as first step
# - Job steps with dependencies
# - DAG visualization
```

### Tekton for CI/CD Pipelines
```bash
b00t stack to-tekton ml-training-pipeline
# Generates Tekton Pipeline with:
# - Data prep task
# - Training task (requires data prep)
# - Evaluation task (requires training)
```

## Simplified Architecture

```
b00t Datum → k8s Primitives → Ecosystem Tools
     ↓              ↓                  ↓
   Stack    → Deployment      → kubectl apply
              + Service
     ↓              ↓                  ↓
    Job      → Job              → Kueue (queuing)
              + initContainer      Argo (DAGs)
                                   Tekton (CI/CD)
```

## Commands

```bash
# Deploy stack (services)
b00t stack to-k8s llm-inference-pipeline
kubectl apply -f llm-inference-pipeline-k8s/

# Generate job with stack dependency
b00t job to-k8s llm-batch-job
kubectl apply -f llm-batch-job-k8s/job.yaml

# Job waits for stack services via initContainer
# Kueue manages queuing and GPU quotas
# Budget controller tracks spending
```

## Why This is Better

1. **DRY**: Leverage k8s primitives instead of reinventing
2. **Battle-tested**: Kueue/Argo used by thousands
3. **Standard**: Works with any k8s cluster (k0s, k3s, k8s)
4. **Composable**: Mix with other k8s tools (Istio, Linkerd, etc.)
5. **Observable**: Standard k8s metrics and logging
6. **Portable**: No vendor lock-in, just YAML

## Next Steps

1. ✅ Generate Job manifests with initContainer dependencies
2. ✅ Integrate with Kueue for queuing
3. ⏳ Generate Argo Workflows for DAGs
4. ⏳ Budget limits → Kueue ResourceQuotas
5. ⏳ GPU affinity → Kueue ClusterQueue topology
