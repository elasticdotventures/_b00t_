# AI fine-tuning module — QLoRA (local k8s) + HF Jobs (cloud frontier)
# 🧠 Load this skill: b00t learn ai-finetune
# Datum: _b00t_/ai-finetune.skill.toml  (depends_on: hf-cli, HF-JOBS, HF-DATASET, HF-HUB-MODELS)
#
# Local tier:  sm3lly RTX3090 via k8s Jobs (sm0l / ch0nky models)
# Cloud tier:  HF Jobs A100 80GB (30B+ models, no timeout risk)
#
# Workflow:
#   Dataset:   just ai-finetune::dataset [&& just ai-finetune::hf-push]
#   Local sm0l: just ai-finetune::train-smol
#   Cloud 30B:  just ai-finetune::hf-push → just ai-finetune::cloud-coder
#   Observe:   just ai-finetune::logs | just ai-finetune::sparkline
#   Test:      just ai-finetune::test
#   Retrieve:  just ai-finetune::hf-adapter-pull

# Paths are relative to repo root (justfile_directory())
set working-directory := ".."

# ── Dataset ────────────────────────────────────────────────────────────────────

# Generate fine-tune dataset from b00t corpus
dataset:
    uv run python3 fine-tune/generate_dataset.py

# Generate rustdoc JSON for key workspace crates (requires nightly)
rustdoc:
    ~/.cargo/bin/cargo +nightly rustdoc -p b00t-cli --lib -- --output-format json -Z unstable-options
    ~/.cargo/bin/cargo +nightly rustdoc -p b00t-c0re-lib --lib -- --output-format json -Z unstable-options
    ~/.cargo/bin/cargo +nightly rustdoc -p b00t-datum-core --lib -- --output-format json -Z unstable-options
    @echo "Rustdoc JSON written to target/doc/. Re-run dataset to include."

# ── Local k8s training ─────────────────────────────────────────────────────────

# Apply RBAC/namespace prerequisites for fine-tune Jobs (one-time)
k8s-setup:
    kubectl apply -f _b00t_/k8s.🚢/fine-tune/rbac.yaml

# ch0nky variant: scale down sm0l → train → scale up
train-kube: k8s-setup
    kubectl scale deployment sm0l --replicas=0 -n b00t-inference || true
    kubectl delete job unsloth-train -n b00t-finetune --ignore-not-found
    kubectl apply -f _b00t_/k8s.🚢/fine-tune/job.yaml
    @echo "Training started. Watch: just ai-finetune::logs"
    kubectl wait --for=condition=complete --timeout=7200s job/unsloth-train -n b00t-finetune
    kubectl scale deployment sm0l --replicas=1 -n b00t-inference

# sm0l variant: trains Qwen2.5-0.5B → updates b00t sm0l adapter
train-smol: k8s-setup
    kubectl scale deployment sm0l --replicas=0 -n b00t-inference || true
    kubectl delete job unsloth-train-smol -n b00t-finetune --ignore-not-found
    kubectl apply -f _b00t_/k8s.🚢/fine-tune/job-smol.yaml
    @echo "sm0l training started. Watch: just ai-finetune::logs"
    kubectl wait --for=condition=complete --timeout=3600s job/unsloth-train-smol -n b00t-finetune
    kubectl scale deployment sm0l --replicas=1 -n b00t-inference

# Launch ch0nky-tier (Qwen3-8B) fine-tune (prereq: scale ch0nky to 0 first)
train-ch0nky:
    #!/bin/bash
    VRAM_FREE=$(nvidia-smi --query-gpu=memory.free --format=csv,noheader,nounits | head -1)
    if (( VRAM_FREE < 12000 )); then
        echo "⚠️  only ${VRAM_FREE}MB VRAM free — need ~14GB"
        echo "   run: kubectl scale deployment ch0nky --replicas=0 -n b00t-inference"
        exit 1
    fi
    echo "✅ ${VRAM_FREE}MB free — launching Qwen3-8B fine-tune"
    # TODO: add job-finetune-ch0nky.yaml to _b00t_/k8s.🚢/fine-tune/
    echo "⚠️  job-finetune-ch0nky.yaml not yet created"

# Export trained LoRA adapter to GGUF
export:
    uv run python3 fine-tune/export_gguf.py

# ── Observability ──────────────────────────────────────────────────────────────

# Stream live training logs from active sm0l pod
logs:
    #!/bin/bash
    POD=$(kubectl get pod -n b00t-finetune -l b00t.io/model=sm0l --field-selector=status.phase=Running -o name 2>/dev/null | head -1)
    if [[ -z "$POD" ]]; then
        POD=$(kubectl get pod -n b00t-finetune -l b00t.io/model=sm0l -o name 2>/dev/null | head -1)
    fi
    if [[ -z "$POD" ]]; then
        echo "No sm0l training pod found. Run: kubectl get pods -n b00t-finetune"
        exit 1
    fi
    echo "📋 streaming logs from $POD"
    kubectl logs -n b00t-finetune "$POD" -c trainer -f --tail=40

# Compact loss/step/epoch watch (ctrl-c to stop)
watch:
    #!/bin/bash
    POD=$(kubectl get pod -n b00t-finetune -l b00t.io/model=sm0l -o name 2>/dev/null | head -1)
    [[ -z "$POD" ]] && { echo "no training pod"; exit 1; }
    echo "watching $POD  (ctrl-c to stop)"
    while true; do
        LAST=$(kubectl logs -n b00t-finetune "$POD" -c trainer --tail=200 2>/dev/null \
               | grep "'loss'" | tail -1)
        [[ -n "$LAST" ]] && printf "\r  %s  " "$LAST"
        sleep 10
    done

# Loss sparkline from live training logs
sparkline n="60":
    scripts/finetune-sparkline.sh {{ n }}

# Status of fine-tune Jobs
status:
    kubectl get jobs,pods -n b00t-finetune

# ── Adapter smoke-test ─────────────────────────────────────────────────────────

# Run b00t-awareness probes against the LoRA adapter (5/5 = pass)
# 🤓 GPU deadlock: pre-scale sm0l to 0 BEFORE applying job
test:
    #!/bin/bash
    set -euo pipefail
    echo "🧪 adapter smoke-test: pre-scaling sm0l→0"
    kubectl scale deployment sm0l --replicas=0 -n b00t-inference
    kubectl wait --for=delete pod -n b00t-inference -l app=sm0l --timeout=60s 2>/dev/null || true
    kubectl delete job b00t-adapter-test -n b00t-finetune 2>/dev/null || true
    kubectl apply -f _b00t_/k8s.🚢/fine-tune/job-test-adapter.yaml
    echo "⏳ waiting for test pod (up to 5m)..."
    kubectl wait --for=condition=complete job/b00t-adapter-test -n b00t-finetune --timeout=300s || {
        echo "  ❌ timed out — logs: kubectl logs -n b00t-finetune -l b00t.io/job=adapter-test"
        kubectl scale deployment sm0l --replicas=1 -n b00t-inference
        exit 1
    }
    echo "📋 results:"
    kubectl logs -n b00t-finetune -l b00t.io/job=adapter-test 2>/dev/null | tail -20
    echo "🔄 restoring sm0l (replicas=1)"
    kubectl scale deployment sm0l --replicas=1 -n b00t-inference

# ── MLflow (training observability) ───────────────────────────────────────────
# Web UI: http://mlflow.b00t.local:30080 (via Gateway)
# SDK:    MLFLOW_TRACKING_URI=http://mlflow.b00t-inference.svc.cluster.local:5000

mlflow-status:
    kubectl get pods -n b00t-inference -l app=mlflow

mlflow-logs:
    kubectl logs -n b00t-inference -l app=mlflow -f --tail=40

mlflow-open:
    @echo "http://mlflow.b00t.local:30080  (add to /etc/hosts: 192.168.1.137 mlflow.b00t.local)"

mlflow-remove:
    kubectl delete -f _b00t_/k8s.🚢/b00t-inference/mlflow-deployment.yaml || true

# ── HF Cloud — generic training tier (any model, any flavor) ──────────────────
# See: _b00t_/datums/HF-JOBS.job.tomllmd
# Workflow: dataset → hf-push → cloud-train → hf-job-logs <id> → hf-adapter-pull
#
# Blessing chain: b00t learn ai-finetune
#   └── hf-cli → HF-JOBS → HF-DATASET → HF-HUB-MODELS
#
# Generic path:  just ai-finetune::cloud-train <flavor> <config-filename>
# Coder alias:   just ai-finetune::cloud-coder

# One-shot sync: regenerate dataset from corpus + push everything to HF
# Run before any cloud training run to ensure HF dataset is current
hf-sync:
    #!/bin/bash
    set -euo pipefail
    echo "🔄 generating rustdoc JSON (nightly required)..."
    just ai-finetune::rustdoc || echo "⚠️  rustdoc skipped (nightly unavailable)"
    echo "🔄 regenerating dataset from b00t corpus..."
    uv run python3 fine-tune/generate_dataset.py
    echo "📤 pushing to hf://datasets/elasticdotventures/b00t-training..."
    just ai-finetune::hf-push

# Visualize training dataset: token-length distribution + category breakdown → SVG
# Useful for spotting outliers, padding waste, category imbalance before cloud submit
# Output: fine-tune/dataset-viz.svg (open in browser or VLM for inspection)
dataset-viz:
    uv run python3 fine-tune/generate_dataset.py --viz fine-tune/dataset-viz.svg
    @echo "✓ fine-tune/dataset-viz.svg — open in browser or: just viz-entangle"

# Upload dataset + trainer + all fine-tune/config-*.yaml to HF repo
# Run after: just ai-finetune::dataset
hf-push:
    #!/bin/bash
    set -euo pipefail
    hf upload elasticdotventures/b00t-training fine-tune/train.jsonl train.jsonl --type dataset --private
    hf upload elasticdotventures/b00t-training fine-tune/train_unsloth.py train_unsloth.py --type dataset --private
    for f in fine-tune/config-*.yaml; do
        name=$(basename "$f")
        hf upload elasticdotventures/b00t-training "$f" "$name" --type dataset --private
        echo "  ✓ $name"
    done
    echo "✓ pushed to https://huggingface.co/datasets/elasticdotventures/b00t-training"

# Push a single config file to HF training dataset (after editing locally)
hf-push-config config:
    hf upload elasticdotventures/b00t-training fine-tune/{{config}} {{config}} --type dataset --private
    @echo "✓ pushed fine-tune/{{config}} → hf://datasets/elasticdotventures/b00t-training/{{config}}"

# Generic cloud training recipe — model + hyperparams are entirely in the YAML config
# flavor: a100-large ($2.50/h, 80GB) | h200 ($5/h, 141GB) | a10g-large ($1.50/h, 24GB)
# config: filename in hf://datasets/elasticdotventures/b00t-training (e.g. config-cloud-coder.yaml)
# 🤓 /data mount is read-only dataset repo; /adapters is writable bucket for checkpoints
# 🤓 pip install unsloth required even in unsloth image — image has vLLM variant, not training
cloud-train flavor="a100-large" config="config-cloud-coder.yaml":
    hf jobs run docker.io/unsloth/unsloth:latest \
      --flavor {{flavor}} \
      --timeout 10h \
      -e HF_HOME=/tmp/hf-cache \
      --secrets HF_TOKEN \
      -v hf://datasets/elasticdotventures/b00t-training:/data:ro \
      -v hf://buckets/elasticdotventures/b00t-adapters:/adapters:rw \
      --detach \
      /bin/sh -c "pip install -q unsloth 'huggingface_hub[hf_xet]>=0.26' && python3 /data/train_unsloth.py --config /data/{{config}}"

# Convenience alias: Qwen3-Coder-30B on A100-large
cloud-coder:
    just ai-finetune::cloud-train a100-large config-cloud-coder.yaml

# Resume a cancelled/timed-out cloud job from a HF Hub checkpoint
# checkpoint: HF Hub path e.g. elasticdotventures/b00t-qwen3-coder-30b/checkpoint-500
cloud-resume flavor="a100-large" checkpoint="" config="config-cloud-coder.yaml":
    #!/bin/bash
    [[ -z "{{checkpoint}}" ]] && { echo "ERROR: checkpoint= required"; exit 1; }
    hf jobs run docker.io/unsloth/unsloth:latest \
      --flavor {{flavor}} \
      --timeout 10h \
      -e HF_HOME=/tmp/hf-cache \
      --secrets HF_TOKEN \
      -v hf://datasets/elasticdotventures/b00t-training:/data:ro \
      -v hf://buckets/elasticdotventures/b00t-adapters:/adapters:rw \
      --detach \
      /bin/sh -c "pip install -q unsloth 'huggingface_hub[hf_xet]>=0.26' && python3 /data/train_unsloth.py --config /data/{{config}} --resume {{checkpoint}}"

# List all HF Jobs (active + recent)
hf-jobs-list:
    hf jobs ps

# Poll status of a specific HF job
hf-job-status job_id:
    hf jobs inspect {{job_id}}

# Stream logs from a running or completed HF job
hf-job-logs job_id:
    hf jobs logs --follow {{job_id}}

# Cancel a running job
hf-job-cancel job_id:
    hf jobs cancel {{job_id}}

# Generic: pull any adapter from HF Hub to local path
# model_id: e.g. elasticdotventures/b00t-qwen3-coder-30b
# local_dir: destination (default: fine-tune/output-adapter)
hf-adapter-pull model_id local_dir="fine-tune/output-adapter":
    hf download {{model_id}} --repo-type model --local-dir {{local_dir}}
    @echo "✓ adapter at {{local_dir}}"

# Pull the coder-30b adapter to its canonical path
hf-adapter-pull-coder:
    just ai-finetune::hf-adapter-pull elasticdotventures/b00t-qwen3-coder-30b fine-tune/output-cloud-coder/lora-adapter
