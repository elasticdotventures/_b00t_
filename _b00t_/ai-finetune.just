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

# ── Inlined training script (was train_unsloth.py) ───────────────────────────
# Entry point for cloud jobs: just -f /data/ai-finetune.just train /data/config.yaml
# sm0l output contract: prints step progress; exit 0 on success, 1 on error
train config="/data/config-cloud-coder.yaml" resume="":
    #!/usr/bin/env python3
    import argparse, json, os, sys
    from pathlib import Path
    os.environ.setdefault("HF_HOME", os.path.expanduser("~/.cache/huggingface"))

    config_path = "{{config}}"
    resume_from = "{{resume}}" or None

    try:
        import yaml
        with open(config_path) as f:
            cfg = yaml.safe_load(f)
    except Exception as e:
        print(f"❌ config load failed: {e}"); sys.exit(1)

    model_name   = cfg.get("base_model", "Qwen/Qwen3-Coder-30B-A3B-Instruct")
    dataset_path = cfg.get("dataset_path", cfg.get("dataset", "/data/train.jsonl"))
    output_dir   = cfg.get("output_dir", "/tmp/output")
    lora_r       = cfg.get("lora_r", 16)
    lora_alpha   = cfg.get("lora_alpha", 32)
    lora_dropout = cfg.get("lora_dropout", 0.0)
    lr           = float(cfg.get("learning_rate", 1e-4))
    num_epochs   = int(cfg.get("num_epochs", 1))
    batch_size   = int(cfg.get("batch_size", 2))
    grad_accum   = int(cfg.get("grad_accum", 8))
    max_seq_len  = int(cfg.get("max_seq_length", 2048))

    print(f"🚀 b00t QLoRA  model={model_name}  r={lora_r}  epochs={num_epochs}  seq={max_seq_len}")

    try:
        from unsloth import FastLanguageModel
    except ImportError:
        print("❌ unsloth not installed"); sys.exit(1)

    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=model_name,
        max_seq_length=max_seq_len,
        load_in_4bit=cfg.get("load_in_4bit", False),
        dtype=None,
    )
    print("✓ model loaded")

    model = FastLanguageModel.get_peft_model(
        model, r=lora_r,
        target_modules=cfg.get("target_modules", ["q_proj","v_proj","k_proj","o_proj"]),
        lora_alpha=lora_alpha, lora_dropout=lora_dropout,
        bias="none", use_gradient_checkpointing="unsloth", random_state=42,
    )
    print("✓ LoRA applied")

    if not os.path.exists(dataset_path):
        print(f"❌ dataset not found: {dataset_path}"); sys.exit(1)
    from datasets import Dataset as HFDataset
    from trl import SFTTrainer
    from transformers import TrainingArguments

    rows = []
    with open(dataset_path) as f:
        for line in f:
            r = json.loads(line)
            inp = r.get("input","")
            text = (f"### Instruction:\n{r['instruction']}\n\n### Input:\n{inp}\n\n### Response:\n{r.get('response','')}"
                    if inp.strip() else
                    f"### Instruction:\n{r['instruction']}\n\n### Response:\n{r.get('response','')}")
            rows.append({"text": text})

    dataset = HFDataset.from_list(rows)
    print(f"✓ dataset loaded ({len(dataset)} examples)")

    has_hub = bool(cfg.get("hub_model_id") and int(cfg.get("save_steps",0)) > 0)
    trainer = SFTTrainer(
        model=model, tokenizer=tokenizer, train_dataset=dataset,
        dataset_text_field="text", max_seq_length=max_seq_len,
        dataset_num_proc=2, packing=bool(cfg.get("packing", False)),
        args=TrainingArguments(
            per_device_train_batch_size=batch_size,
            gradient_accumulation_steps=grad_accum,
            warmup_ratio=float(cfg.get("warmup_ratio",0.05)),
            num_train_epochs=num_epochs, learning_rate=lr,
            fp16=not model.dtype.is_floating_point,
            bf16=model.dtype.is_floating_point,
            logging_steps=int(cfg.get("logging_steps",10)),
            optim=cfg.get("optim","adamw_8bit"),
            weight_decay=float(cfg.get("weight_decay",0.01)),
            lr_scheduler_type=cfg.get("lr_scheduler","cosine"),
            seed=42, output_dir=output_dir,
            save_steps=int(cfg.get("save_steps",0)) or None,
            report_to=cfg.get("report_to","none"),
            push_to_hub=has_hub,
            hub_model_id=cfg.get("hub_model_id","") or "",
            hub_strategy="checkpoint",
            hub_private_repo=cfg.get("private", True),
        ),
    )
    trainer.train(resume_from_checkpoint=resume_from or None)

    model.save_pretrained(f"{output_dir}/lora-adapter")
    tokenizer.save_pretrained(f"{output_dir}/lora-adapter")
    print(f"✓ adapter saved → {output_dir}/lora-adapter")

    if cfg.get("push_to_hub") and cfg.get("hub_model_id"):
        hub_id = cfg["hub_model_id"]
        private = cfg.get("private", True)
        model.push_to_hub(hub_id, private=private)
        tokenizer.push_to_hub(hub_id, private=private)
        print(f"✓ pushed → https://huggingface.co/{hub_id}")

    print("✅ training complete")

# ── Inlined adapter smoke-test (was test_adapter.py) ─────────────────────────
# Generic: adapter path + base model from args; prints PASS/FAIL per probe
# sm0l output contract: exit 0 if >=3/5 probes return b00t-aware responses
smoke-test adapter="fine-tune/output-smol/lora-adapter" base="unsloth/Qwen2.5-0.5B-Instruct-bnb-4bit":
    #!/usr/bin/env python3
    import sys

    adapter = "{{adapter}}"
    base    = "{{base}}" or None

    PROBES = [
        ("b00t basics",    "What command do I run to orient my role and load blessings in b00t?"),
        ("b00t task",      "How do I add a new task in the b00t task system?"),
        ("b00t grok",      "What is b00t grok used for and what is the correct syntax?"),
        ("soul kv",        "How do I read and write values to the b00t soul KV store?"),
        ("install mode",   "What modes does just install support and how does it remember past choices?"),
    ]

    try:
        from unsloth import FastLanguageModel
    except ImportError:
        print("❌ unsloth not installed"); sys.exit(1)

    model_path = adapter if not base else adapter
    print(f"Loading adapter: {adapter}")
    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=adapter, max_seq_length=512, load_in_4bit=True,
    )
    FastLanguageModel.for_inference(model)

    import torch
    passed = 0
    for label, question in PROBES:
        msgs = [{"role":"user","content":question}]
        prompt = tokenizer.apply_chat_template(msgs, tokenize=False, add_generation_prompt=True)
        inputs = tokenizer(prompt, return_tensors="pt").to(model.device)
        with torch.no_grad():
            out = model.generate(**inputs, max_new_tokens=120, temperature=0.3,
                                  do_sample=True, pad_token_id=tokenizer.eos_token_id)
        answer = tokenizer.decode(out[0][inputs["input_ids"].shape[1]:], skip_special_tokens=True).strip()
        ok = any(t in answer.lower() for t in ["b00t","just","soul","mcp","grok","datum"])
        print(f"{'PASS' if ok else 'FAIL'} [{label}]: {answer[:80]}")
        passed += ok

    print(f"\n{passed}/{len(PROBES)} probes b00t-aware")
    sys.exit(0 if passed >= 3 else 1)

# Upload dataset + trainer + all fine-tune/config-*.yaml to HF repo
# Run after: just ai-finetune::dataset
hf-push:
    #!/bin/bash
    set -euo pipefail
    hf upload elasticdotventures/b00t-training fine-tune/train.jsonl train.jsonl --type dataset --private
    # 🤓 justfile IS the training script — no separate .py file needed in cloud
    hf upload elasticdotventures/b00t-training _b00t_/ai-finetune.just ai-finetune.just --type dataset --private
    hf upload elasticdotventures/b00t-training fine-tune/generate_dataset.py generate_dataset.py --type dataset --private
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
      /bin/sh -c "pip install -q unsloth 'huggingface_hub[hf_xet]>=0.26' && \
        curl -qL https://github.com/casey/just/releases/download/1.40.0/just-1.40.0-x86_64-unknown-linux-musl.tar.gz \
          | tar -xz -C /usr/local/bin just && \
        just --version && \
        just -f /data/ai-finetune.just train /data/{{config}}"

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
      /bin/sh -c "pip install -q unsloth 'huggingface_hub[hf_xet]>=0.26' && \
        curl -qL https://github.com/casey/just/releases/download/1.40.0/just-1.40.0-x86_64-unknown-linux-musl.tar.gz \
          | tar -xz -C /usr/local/bin just && \
        just -f /data/ai-finetune.just train /data/{{config}} {{checkpoint}}"

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

# ── Handoff: open live log stream for human monitoring ────────────────────────
# Agent calls this after submitting — then STOPS. Human watches + cancels.
# Protocol: submit → watch-run → agent exits; human monitors; agent re-invoked on completion.
# Uses zellij if $ZELLIJ is set; otherwise prints the command to run.
watch-run job_id:
    #!/bin/bash
    CMD="hf jobs logs --follow elasticdotventures/{{job_id}}"
    CANCEL="just ai-finetune::hf-job-cancel {{job_id}}"
    if [[ -n "${ZELLIJ:-}" ]]; then
        zellij run --name "hf-logs-{{job_id}}" --close-on-exit -- $CMD
        echo "PASS: zellij pane opened — watch live; cancel with: $CANCEL"
    else
        echo ""
        echo "▶  STREAM LOGS (run in your terminal):"
        echo "   $CMD"
        echo ""
        echo "✗  CANCEL JOB:"
        echo "   $CANCEL"
        echo ""
    fi

# ── Monitor: sm0l output contract ────────────────────────────────────────────
# Blessing evidentiary handler — command syntax IS the proof.
# Output contract: PASS: <stage> step=<N> rate=<X>s/it ETA=<HH:MM>
#                  FAIL: <reason>
# Thresholds (A100): warn>90s/it fail>180s/it  (H200): warn>40s/it fail>90s/it
monitor-run job_id threshold_s="90":
    #!/usr/bin/env python3
    import subprocess, re, sys, os
    job_id   = "{{job_id}}"
    threshold = float("{{threshold_s}}")
    owner    = "elasticdotventures"

    def run(cmd):
        r = subprocess.run(cmd, shell=True, capture_output=True, text=True)
        return r.stdout + r.stderr

    # Stage check
    raw = run(f"hf jobs inspect {owner}/{job_id} 2>/dev/null")
    m = re.search(r'"stage":\s*"([^"]+)"', raw)
    stage = m.group(1) if m else "UNKNOWN"

    if stage in ("FAILED", "CANCELLED"):
        print(f"FAIL: job {job_id} stage={stage}"); sys.exit(1)
    if stage == "SCHEDULING":
        print(f"PASS: SCHEDULING — awaiting node allocation"); sys.exit(0)

    # Step timing from logs
    logs = run(f"hf jobs logs {owner}/{job_id} 2>/dev/null")
    matches = re.findall(r'(\d+)/(\d+) \[[\d:]+<[\d:]+, ([\d.]+)s/it\]', logs)
    if not matches:
        print(f"PASS: {stage} — no step data yet (model loading)"); sys.exit(0)

    step, total, rate_s = matches[-1]
    rate = float(rate_s)
    eta_m = re.findall(r'<([\d:]+),', logs)
    eta = eta_m[-1] if eta_m else "?"

    if rate > threshold:
        print(f"FAIL: step_time={rate}s/it > threshold={threshold}s — step={step}/{total} ETA={eta} (cancel+resubmit faster flavor)")
        sys.exit(1)
    print(f"PASS: {stage} step={step}/{total} rate={rate}s/it ETA={eta}")

# Generic: pull any adapter from HF Hub to local path
# model_id: e.g. elasticdotventures/b00t-qwen3-coder-30b
# local_dir: destination (default: fine-tune/output-adapter)
hf-adapter-pull model_id local_dir="fine-tune/output-adapter":
    hf download {{model_id}} --repo-type model --local-dir {{local_dir}}
    @echo "✓ adapter at {{local_dir}}"

# Pull the coder-30b adapter to its canonical path
hf-adapter-pull-coder:
    just ai-finetune::hf-adapter-pull elasticdotventures/b00t-qwen3-coder-30b fine-tune/output-cloud-coder/lora-adapter

# ── Budget + Cost Forecasting ────────────────────────────────────────────────
# Config: _b00t_/cloud-budget.tomllmd
# FOCUS v1.3 custom columns: x_ExperimentId, x_ModelVariant, x_StepTime

# Pre-flight cost estimate — prints expected cost + hours before submit
# Reads dataset row count, config params, and flavor rate from cloud-budget.tomllmd
# sm0l output contract: "ESTIMATE: $N.NN (~Nh) — budget OK|WARN|GATE"
cost-estimate flavor="a100-large" config="fine-tune/config-cloud-coder.yaml":
    #!/usr/bin/env -S uv run --with pyyaml python3
    import json, sys, math
    try:
        import yaml
        with open("{{config}}") as f:
            cfg = yaml.safe_load(f)
        import tomllib
        with open("_b00t_/cloud-budget.tomllmd", "rb") as f:
            budget = tomllib.load(f)
    except Exception as e:
        print(f"FAIL: config load — {e}"); sys.exit(1)

    # dataset rows
    dataset_path = cfg.get("dataset_path", "fine-tune/train.jsonl").replace("/data/","fine-tune/")
    if dataset_path.startswith("/"):
        dataset_path = "fine-tune/train.jsonl"
    try:
        rows = sum(1 for _ in open(dataset_path))
    except FileNotFoundError:
        rows = 1200   # fallback estimate
        print(f"  ⚠️  dataset not found locally; using estimate of {rows} rows")

    batch      = int(cfg.get("batch_size", 2))
    grad_accum = int(cfg.get("grad_accum", 8))
    epochs     = int(cfg.get("num_epochs", 1))
    packing    = cfg.get("packing", False)

    steps_per_epoch = math.ceil(rows / (batch * grad_accum * (2 if packing else 1)))
    total_steps     = steps_per_epoch * epochs

    # step time from budget datum
    flavor_key = "{{flavor}}".replace("-","_")
    step_time_map = budget.get("budget",{}).get("step_time_estimates",{})
    step_time_s   = step_time_map.get(f"{flavor_key}_30b_moe", 130)

    wall_hours = (total_steps * step_time_s) / 3600
    rates      = budget.get("budget",{}).get("hf_jobs",{})
    rate       = rates.get(flavor_key, 2.50)
    cost_cake  = wall_hours * rate

    # budget gate check
    cap        = budget.get("budget",{}).get("monthly_cap_usd", 200.0)
    warn_frac  = budget.get("budget",{}).get("warn_threshold", 0.75)
    gate_frac  = budget.get("budget",{}).get("gate_threshold", 0.90)

    # actual spend this month from hf jobs history (best effort)
    import subprocess, re as regex
    raw = subprocess.run("hf jobs ps --all 2>/dev/null", shell=True, capture_output=True, text=True).stdout
    spent = 0.0
    for line in raw.splitlines()[1:]:
        parts = line.split("\t")
        if len(parts) >= 6 and parts[4] not in ("CANCELED","ERROR"):
            dur = parts[5].strip()
            m = regex.match(r"(?:(\d+)h\s*)?(?:(\d+)m\s*)?(?:(\d+)s)?", dur)
            if m:
                h = int(m.group(1) or 0)
                mn = int(m.group(2) or 0)
                s = int(m.group(3) or 0)
                spent += (h + mn/60 + s/3600) * rate

    remaining = cap - spent - cost_cake
    frac_after = (spent + cost_cake) / cap

    if frac_after >= gate_frac:
        gate = "GATE ❌"
    elif frac_after >= warn_frac:
        gate = "WARN ⚠️"
    else:
        gate = "OK ✅"

    print(f"ESTIMATE: {cost_cake:.2f}🎂 (~{wall_hours:.1f}h) on {flavor_key} — budget {gate}")
    print(f"  steps={total_steps} ({steps_per_epoch}/epoch × {epochs} epoch)  step_time={step_time_s}s  rate={rate}🎂/h")
    print(f"  monthly: spent={spent:.2f}🎂  this_run={cost_cake:.2f}🎂  remaining={remaining:.2f}🎂  cap={cap:.0f}🎂")

# Actual spend report from HF Jobs history (all completed/running jobs this session)
# Parses hf jobs ps --all durations × flavor rate → FOCUS x_ExperimentId records
# sm0l output contract: table of job costs + monthly total
cost-report:
    #!/usr/bin/env python3
    import subprocess, re, sys
    try:
        import tomllib
        with open("_b00t_/cloud-budget.tomllmd", "rb") as f:
            budget = tomllib.load(f)
    except Exception as e:
        print(f"FAIL: budget config — {e}"); sys.exit(1)

    rates = budget.get("budget",{}).get("hf_jobs",{})
    cap   = budget.get("budget",{}).get("monthly_cap_usd", 200.0)

    raw = subprocess.run("hf jobs ps --all 2>/dev/null", shell=True, capture_output=True, text=True).stdout
    lines = raw.strip().splitlines()
    if len(lines) <= 1:
        print("PASS: no job history"); sys.exit(0)

    total = 0.0
    print(f"{'JOB_ID':28} {'STATUS':12} {'DURATION':10} {'FLAVOR':14} {'COST':>8}")
    print("─" * 80)
    for line in lines[1:]:
        parts = line.split("\t")
        if len(parts) < 6:
            continue
        job_id, _, _, _, status, dur = parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]
        dur = dur.strip()
        m = re.match(r"(?:(\d+)h\s*)?(?:(\d+)m\s*)?(?:(\d+)s)?", dur)
        h = int(m.group(1) or 0) if m else 0
        mn = int(m.group(2) or 0) if m else 0
        s = int(m.group(3) or 0) if m else 0
        hours = h + mn/60 + s/3600
        # infer flavor from duration heuristic (a100 is default)
        rate  = rates.get("a100_large", 2.50)
        cost  = hours * rate if status not in ("CANCELED","ERROR") else 0.0
        total += cost
        cost_s = f"{cost:.3f}🎂" if cost > 0 else "--"
        print(f"{job_id[:26]:28} {status:12} {dur:10} {'a100-large':14} {cost_s:>8}")

    pct = (total / cap) * 100
    print("─" * 80)
    print(f"  Total billed: {total:.2f}🎂  /  cap {cap:.0f}🎂  ({pct:.1f}%)")
    forecast = budget.get("forecast",{})
    monthly_proj = forecast.get("runs_per_month",4) * forecast.get("hours_per_run",7) * rates.get("a100_large",2.50)
    print(f"  Monthly forecast ({forecast.get('runs_per_month',4)} runs × {forecast.get('hours_per_run',7)}h): {monthly_proj:.0f}🎂")

# Budget gate: exit 1 with GATE if estimated cost would exceed gate_threshold of monthly cap
# Use as pre-condition before cloud-train: just ai-finetune::budget-gate a100-large config.yaml && just cloud-train
budget-gate flavor="a100-large" config="fine-tune/config-cloud-coder.yaml":
    #!/bin/bash
    set -euo pipefail
    result=$(just ai-finetune::cost-estimate {{flavor}} {{config}})
    echo "$result"
    if echo "$result" | grep -q "GATE"; then
        echo "BLOCKED: monthly budget gate exceeded — adjust budget or use cheaper flavor"
        exit 1
    fi
