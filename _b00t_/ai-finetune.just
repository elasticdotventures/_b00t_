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

    config_path = "{{ config }}"
    resume_from = "{{ resume }}" or None

    try:
        import yaml
        with open(config_path) as f:
            cfg = yaml.safe_load(f)
    except Exception as e:
        print(f"❌ config load failed: {e}"); sys.exit(1)

    if cfg.get("mlflow_tracking_uri"):
        os.environ["MLFLOW_TRACKING_URI"] = cfg["mlflow_tracking_uri"]
    if cfg.get("mlflow_experiment"):
        os.environ["MLFLOW_EXPERIMENT_NAME"] = cfg["mlflow_experiment"]

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

    adapter = "{{ adapter }}"
    base    = "{{ base }}" or None

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
    hf upload elasticdotventures/b00t-training fine-tune/{{ config }} {{ config }} --type dataset --private
    @echo "✓ pushed fine-tune/{{ config }} → hf://datasets/elasticdotventures/b00t-training/{{ config }}"

# Generic cloud training recipe — model + hyperparams are entirely in the YAML config
# flavor: a100-large ($2.50/h, 80GB) | h200 ($5/h, 141GB) | a10g-large ($1.50/h, 24GB)
# config: filename in hf://datasets/elasticdotventures/b00t-training (e.g. config-cloud-coder.yaml)
# 🤓 /data mount is read-only dataset repo; /adapters is writable bucket for checkpoints
# 🤓 pip install unsloth required even in unsloth image — image has vLLM variant, not training
# 🤓 just download: versioned URL avoids GitHub /latest/ redirect → 404 on renamed assets

# 🤓 timeout_hours: run `just ai-finetune::cost-estimate <flavor> <config>` first → wall_h + 1
cloud-train flavor="h200" config="config-cloud-coder.yaml" timeout_hours="10.0":
    hf jobs run ghcr.io/elasticdotventures/b00t-training-image:latest \
      --flavor {{ flavor }} \
      --timeout {{ timeout_hours }}h \
      -e HF_HOME=/tmp/hf-cache \
      -e PYTORCH_ALLOC_CONF=expandable_segments:True \
      -e TORCHDYNAMO_DISABLE=1 \
      -e UNSLOTH_COMPILE_LOCATION=/tmp/unsloth_compiled_cache \
      --secrets HF_TOKEN \
      -v hf://datasets/elasticdotventures/b00t-training:/data:ro \
      -v hf://buckets/elasticdotventures/b00t-adapters:/adapters:rw \
      --detach \
      /bin/sh -c "command -v just 2>/dev/null || { curl -sSL -o /tmp/just.tar.gz https://github.com/casey/just/releases/download/1.54.0/just-1.54.0-x86_64-unknown-linux-musl.tar.gz && tar -xzf /tmp/just.tar.gz -C /tmp just; } && \
        \$(command -v just 2>/dev/null || echo /tmp/just) -f /data/ai-finetune.just train /data/{{ config }}"

# Convenience alias: Qwen3-Coder-30B on A100-large — budget-gated
# 🤓 Gate runs first: calls ledgrrr_b00t_delegate_datum via b00t-mcp; exits non-zero if over cap.
#    resume_token is stored in .b00t/last-delegate-token and passed as HF job label for billing reconciliation.
cloud-coder:
    #!/usr/bin/env python3
    import subprocess, sys, json, os, pathlib, datetime
    # 1. Budget gate
    gate = subprocess.run(
        ["just", "ai-finetune::budget-gate", "h200", "fine-tune/config-cloud-coder.yaml"],
        capture_output=True, text=True
    )
    if gate.returncode != 0:
        print(gate.stdout + gate.stderr, end="", flush=True)
        print("❌ cloud-coder blocked: cake budget gate denied", file=sys.stderr)
        sys.exit(gate.returncode)
    # 2. Mint resume token (ledgrrr side handles real one; local shim for label round-trip)
    ts = int(datetime.datetime.now(datetime.timezone.utc).timestamp() * 1000)
    resume_token = f"b00t-hf-coder-{ts:016x}"
    pathlib.Path(".b00t/last-delegate-token").write_text(resume_token)
    # 3. Submit job with resume token as label for billing reconciliation
    result = subprocess.run([
        "hf", "jobs", "run", "ghcr.io/elasticdotventures/b00t-training-image:latest",
        "--flavor", "h200",
        "--timeout", "10.0h",
        "-e", "HF_HOME=/tmp/hf-cache",
        "-e", "PYTORCH_ALLOC_CONF=expandable_segments:True",
        "-e", "TORCHDYNAMO_DISABLE=1",
        "-e", "UNSLOTH_COMPILE_LOCATION=/tmp/unsloth_compiled_cache",
        "--secrets", "HF_TOKEN",
        "-v", "hf://datasets/elasticdotventures/b00t-training:/data:ro",
        "-v", "hf://buckets/elasticdotventures/b00t-adapters:/adapters:rw",
        "--label", f"ledgrrr_resume_token={resume_token}",
        "--detach",
        "/bin/sh", "-c",
        "command -v just 2>/dev/null || { curl -sSL -o /tmp/just.tar.gz "
        "https://github.com/casey/just/releases/download/1.54.0/just-1.54.0-x86_64-unknown-linux-musl.tar.gz "
        "&& tar -xzf /tmp/just.tar.gz -C /tmp just; } && "
        "$(command -v just 2>/dev/null || echo /tmp/just) -f /data/ai-finetune.just train /data/config-cloud-coder.yaml"
    ])
    sys.exit(result.returncode)


# Dense 14B Coder — no MoE, avoids triton_kernels.routing requirement
# 🤓 Qwen3-Coder-30B-A3B (MoE) requires triton_kernels.routing which is NOT on PyPI.
#    Use this recipe for reliable training: Qwen3-Coder-14B-Instruct (dense) on A10g-large.
#    14B 4-bit: ~7GB weights, ~20GB total → fits A10g-large 24GB. ~15-25s/step = ~3h = ~.50.
cloud-coder-14b:
    #!/usr/bin/env python3
    import subprocess, sys, pathlib, datetime
    gate = subprocess.run(
        ["just", "ai-finetune::budget-gate", "a10g-large", "fine-tune/config-cloud-coder-14b.yaml"],
        capture_output=True, text=True
    )
    if gate.returncode != 0:
        print(gate.stdout + gate.stderr, end="", flush=True)
        print("❌ cloud-coder-14b blocked: cake budget gate denied", file=sys.stderr)
        sys.exit(gate.returncode)
    ts = int(datetime.datetime.now(datetime.timezone.utc).timestamp() * 1000)
    resume_token = f"b00t-hf-coder14-{ts:016x}"
    pathlib.Path(".b00t/last-delegate-token").write_text(resume_token)
    result = subprocess.run([
        "hf", "jobs", "run", "ghcr.io/elasticdotventures/b00t-training-image:latest",
        "--flavor", "a10g-large",
        "--timeout", "5.0h",
        "-e", "HF_HOME=/tmp/hf-cache",
        "-e", "PYTORCH_ALLOC_CONF=expandable_segments:True",
        "-e", "TORCHDYNAMO_DISABLE=1",
        "-e", "UNSLOTH_COMPILE_LOCATION=/tmp/unsloth_compiled_cache",
        # 🤓 base unsloth image sets UNSLOTH_VLLM_STANDBY=1; when=1 it removes
        #    expandable_segments → memory fragmentation → OOM on 14B backward pass
        "-e", "UNSLOTH_VLLM_STANDBY=0",
        "--secrets", "HF_TOKEN",
        "-v", "hf://datasets/elasticdotventures/b00t-training:/data:ro",
        "--label", f"ledgrrr_resume_token={resume_token}",
        "--detach",
        "/bin/sh", "-c",
        "command -v just 2>/dev/null || { curl -sSL -o /tmp/just.tar.gz "
        "https://github.com/casey/just/releases/download/1.54.0/just-1.54.0-x86_64-unknown-linux-musl.tar.gz "
        "&& tar -xzf /tmp/just.tar.gz -C /tmp just; } && "
        "$(command -v just 2>/dev/null || echo /tmp/just) -f /data/ai-finetune.just train /data/config-cloud-coder-14b.yaml"
    ])
    sys.exit(result.returncode)

# Resume a cancelled/timed-out cloud job from a HF Hub checkpoint

# checkpoint: HF Hub path e.g. elasticdotventures/b00t-qwen3-coder-30b/checkpoint-500
cloud-resume flavor="a100-large" checkpoint="" config="config-cloud-coder.yaml" timeout_hours="13.0":
    #!/bin/bash
    [[ -z "{{ checkpoint }}" ]] && { echo "ERROR: checkpoint= required"; exit 1; }
    hf jobs run ghcr.io/elasticdotventures/b00t-training-image:latest \
      --flavor {{ flavor }} \
      --timeout {{ timeout_hours }}h \
      -e HF_HOME=/tmp/hf-cache \
      -e PYTORCH_ALLOC_CONF=expandable_segments:True \
      -e TORCHDYNAMO_DISABLE=1 \
      -e UNSLOTH_COMPILE_LOCATION=/tmp/unsloth_compiled_cache \
      --secrets HF_TOKEN \
      -v hf://datasets/elasticdotventures/b00t-training:/data:ro \
      -v hf://buckets/elasticdotventures/b00t-adapters:/adapters:rw \
      --detach \
      /bin/sh -c "command -v just 2>/dev/null || { curl -sSL -o /tmp/just.tar.gz https://github.com/casey/just/releases/download/1.54.0/just-1.54.0-x86_64-unknown-linux-musl.tar.gz && tar -xzf /tmp/just.tar.gz -C /tmp just; } && \
        \$(command -v just 2>/dev/null || echo /tmp/just) -f /data/ai-finetune.just train /data/{{ config }} {{ checkpoint }}"

# List all HF Jobs (active + recent)
hf-jobs-list:
    hf jobs ps

# Poll status of a specific HF job
hf-job-status job_id:
    hf jobs inspect {{ job_id }}

# Stream logs from a running or completed HF job
hf-job-logs job_id:
    hf jobs logs --follow {{ job_id }}

# Cancel a running job
hf-job-cancel job_id:
    hf jobs cancel {{ job_id }}

# ── Handoff: open live log stream for human monitoring ────────────────────────
# Agent calls this after submitting — then STOPS. Human watches + cancels.
# Protocol: submit → watch-run → agent exits; human monitors; agent re-invoked on completion.

# Uses zellij if $ZELLIJ is set; otherwise prints the command to run.
watch-run job_id:
    #!/bin/bash
    CMD="hf jobs logs --follow elasticdotventures/{{ job_id }}"
    CANCEL="just ai-finetune::hf-job-cancel {{ job_id }}"
    if [[ -n "${ZELLIJ:-}" ]]; then
        zellij run --name "hf-logs-{{ job_id }}" --close-on-exit -- $CMD
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
    job_id   = "{{ job_id }}"
    threshold = float("{{ threshold_s }}")
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
hf-adapter-pull model_id local_dir="fine-tune/output-adapter":
    hf download {{ model_id }} --repo-type model --local-dir {{ local_dir }}
    @echo "✓ adapter at {{ local_dir }}"

# Pull the coder-30b adapter to its canonical path
hf-adapter-pull-coder:
    just ai-finetune::hf-adapter-pull elasticdotventures/b00t-qwen3-coder-30b fine-tune/output-cloud-coder/lora-adapter

# Pull adapter from Hub + patch ch0nky k8s deployment → b00t-aligned model live in hive
# 🤓 ch0nky reads adapter from pod /adapters/current; rollout is zero-downtime
chonky-rollout:
    #!/bin/bash
    set -euo pipefail
    ADAPTER_DIR="fine-tune/output-cloud-coder/lora-adapter"
    echo "1/4 pulling adapter from HF Hub..."
    just ai-finetune::hf-adapter-pull-coder
    echo "2/4 syncing adapter to ch0nky pod..."
    CHONKY_POD=$(kubectl get pod -n b00t-inference -l app=ch0nky -o jsonpath='{.items[0].metadata.name}')
    kubectl cp "${ADAPTER_DIR}/." "b00t-inference/${CHONKY_POD}:/adapters/current/"
    echo "3/4 rolling restart..."
    kubectl rollout restart deployment/ch0nky -n b00t-inference
    kubectl rollout status deployment/ch0nky -n b00t-inference --timeout=120s
    echo "4/4 logging to soul ops register..."
    b00t soul log "ch0nky rollout complete — adapter elasticdotventures/b00t-qwen3-coder-30b" --scope global --result ok
    echo "✅ ch0nky serving b00t-aligned adapter"

# ── Training Image (Option B: baked deps, no runtime pip) ───────────────────
# 🤓 b00t uses uv not pip; image layers mlflow+uv on unsloth:latest via GHCR
# Build locally (requires podman):
training-image-build:
    podman build -t ghcr.io/elasticdotventures/b00t-training-image:latest containers/training/

# Push to GHCR (requires: podman login ghcr.io)
training-image-push:
    podman push ghcr.io/elasticdotventures/b00t-training-image:latest

# Build + push in one step
training-image-release: training-image-build training-image-push

# ── Budget + Cost Forecasting ────────────────────────────────────────────────
# Config + commands embedded in _b00t_/cloud-budget.tomllmd (datum IS the object).
# These shims extract and run the embedded scripts; b00t run cloud-budget::<name> will replace them.

cost-estimate flavor="a100-large" config="fine-tune/config-cloud-coder.yaml":
    #!/usr/bin/env python3
    import tomllib, subprocess, sys, os
    def soul(key, fallback):
        r = subprocess.run(["b00t-cli","soul","get",key], capture_output=True, text=True)
        v = r.stdout.strip()
        return v if r.returncode == 0 and v else fallback
    with open("_b00t_/cloud-budget.tomllmd", "rb") as f:
        d = tomllib.load(f)
    script = next(c["script"] for c in d["command"] if c["name"] == "cost-estimate")
    lines = script.strip().splitlines()
    code  = "\n".join(l for l in lines if not l.startswith("#!"))
    env   = {
        **os.environ,
        "FLAVOR": "{{ flavor }}", "CONFIG": "{{ config }}",
        "CAKE_USD_RATE":    soul("economy.cake.usd_per_cake",    "10"),
        "CAKE_MONTHLY_CAP": soul("economy.cake.monthly_cap_cake", "200"),
        "SLICE_USD_RATE":   soul("economy.cake.usd_per_slice",   "1"),
    }
    sys.exit(subprocess.run(["uv","run","--with","pyyaml","python3","-c",code], env=env).returncode)

cost-report:
    #!/usr/bin/env python3
    import tomllib, subprocess, sys
    with open("_b00t_/cloud-budget.tomllmd", "rb") as f:
        d = tomllib.load(f)
    script = next(c["script"] for c in d["command"] if c["name"] == "cost-report")
    lines = script.strip().splitlines()
    code  = "\n".join(l for l in lines if not l.startswith("#!"))
    sys.exit(subprocess.run(["python3", "-c", code]).returncode)

budget-reconcile:
    #!/usr/bin/env python3
    import tomllib, subprocess, sys
    with open("_b00t_/cloud-budget.tomllmd", "rb") as f:
        d = tomllib.load(f)
    script = next(c["script"] for c in d["command"] if c["name"] == "budget-reconcile")
    lines = script.strip().splitlines()
    code  = "\n".join(l for l in lines if not l.startswith("#!"))
    sys.exit(subprocess.run(["bash", "-c", code]).returncode)

budget-gate flavor="a100-large" config="fine-tune/config-cloud-coder.yaml":
    #!/usr/bin/env python3
    import tomllib, subprocess, sys, os
    def soul(key, fallback):
        r = subprocess.run(["b00t-cli","soul","get",key], capture_output=True, text=True)
        v = r.stdout.strip()
        return v if r.returncode == 0 and v else fallback
    with open("_b00t_/cloud-budget.tomllmd", "rb") as f:
        d = tomllib.load(f)
    script = next(c["script"] for c in d["command"] if c["name"] == "budget-gate")
    lines = script.strip().splitlines()
    code  = "\n".join(l for l in lines if not l.startswith("#!"))
    sys.exit(subprocess.run(["bash", "-c", code], env={
        **os.environ,
        "FLAVOR": "{{ flavor }}", "CONFIG": "{{ config }}",
        "CAKE_USD_RATE":    soul("economy.cake.usd_per_cake",    "10"),
        "CAKE_MONTHLY_CAP": soul("economy.cake.monthly_cap_cake", "200"),
        "SLICE_USD_RATE":   soul("economy.cake.usd_per_slice",   "1"),
    }).returncode)

# ── Sm0l Translator (semantic pipe: frontier → b00t CLI) ──────────────────────
# The fine-tuned sm0l adapter translates natural language to b00t CLI commands.
# Frontier model (not b00t-trained) → sm0l translator → b00t CLI → execution.

# Smoke-test the sm0l translator adapter (5 b00t-aware probes, k8s job)
# 🤓 GPU deadlock: pre-scale ch0nky+sm0l to 0 BEFORE applying
translator-test:
    #!/bin/bash
    set -euo pipefail
    echo "🧪 translator smoke-test: pre-scaling inference →0"
    kubectl scale deployment ch0nky --replicas=0 -n b00t-inference
    kubectl scale deployment sm0l --replicas=0 -n b00t-inference
    kubectl wait --for=delete pod -n b00t-inference -l app=ch0nky --timeout=60s 2>/dev/null || true
    kubectl wait --for=delete pod -n b00t-inference -l app=sm0l --timeout=60s 2>/dev/null || true
    kubectl delete job b00t-translator-test -n b00t-finetune 2>/dev/null || true
    kubectl apply -f _b00t_/k8s.🚢/fine-tune/job-translator-test.yaml
    echo "⏳ waiting for translator test pod (up to 5m)..."
    kubectl wait --for=condition=complete job/b00t-translator-test -n b00t-finetune --timeout=300s || {
        echo "  ❌ timed out — logs: kubectl logs -n b00t-finetune -l b00t.io/job=translator-test"
        kubectl scale deployment ch0nky --replicas=1 -n b00t-inference
        kubectl scale deployment sm0l --replicas=1 -n b00t-inference
        exit 1
    }
    echo "📋 results:"
    kubectl logs -n b00t-finetune -l b00t.io/job=translator-test 2>/dev/null | tail -30
    echo "🔄 restoring inference (ch0nky=1, sm0l=1)"
    kubectl scale deployment ch0nky --replicas=1 -n b00t-inference
    kubectl scale deployment sm0l --replicas=1 -n b00t-inference

# One-shot translator query via k8s job (returns b00t CLI command)
# Usage: just ai-finetune::translator-query "add a task to track the login bug"
translator-query query:
    #!/bin/bash
    set -euo pipefail
    kubectl scale deployment sm0l --replicas=0 -n b00t-inference
    kubectl wait --for=delete pod -n b00t-inference -l app=sm0l --timeout=60s 2>/dev/null || true
    kubectl delete job b00t-translator-query -n b00t-finetune 2>/dev/null || true
    # Patch the job template with query before applying
    sed "s/__QUERY__/$(echo '{{ query }}' | sed 's/[&/\]/\\&/g')/g" _b00t_/k8s.🚢/fine-tune/job-translator-query.yaml | kubectl apply -f -
    kubectl wait --for=condition=complete job/b00t-translator-query -n b00t-finetune --timeout=300s
    kubectl logs -n b00t-finetune -l b00t.io/job=translator-query --tail=5
    kubectl scale deployment sm0l --replicas=1 -n b00t-inference

# Show translator test marks — output contract: PASS/FAIL per probe
translator-marks:
    kubectl logs -n b00t-finetune -l b00t.io/job=translator-test 2>/dev/null || echo "no test results — run: just ai-finetune::translator-test"

# ── Grok failback (keyword RAG over learn/ + datums/) ─────────────────────────
# When raglite (Qdrant VPN down) + irontology (repo.index missing) are both dead.
# Zero dependencies — greps 89 learn/ files + all tomllmd datums.

# Keyword search over b00t knowledge corpus
grok-fallback query="":
    bash _b00t_/scripts/grok-fallback.sh {{ query }}

# Keyword search with topic filter
grok-fallback-topic topic query:
    bash _b00t_/scripts/grok-fallback.sh --topic {{ topic }} {{ query }}

# Transparent grok wrapper — tries b00t-cli grok first, falls back to keyword RAG
grok-ask query="":
    bash _b00t_/scripts/b00t-grok.sh ask {{ query }}

grok-ask-topic topic query:
    bash _b00t_/scripts/b00t-grok.sh ask --topic {{ topic }} {{ query }}

# Translator via keyword: extract cmds: from best-matching datum or cheatsheet
# Usage: just ai-finetune::translate "add a task to track the login bug"
translate query:
    #!/usr/bin/env python3
    import subprocess, sys, re, tomllib, os

    query = "{{ query }}"

    # ── 1. Search b00t-cli-cheatsheet (highest priority) ────────────────────
    cheatsheet = os.path.expanduser("~/.b00t/_b00t_/datums/b00t-cli-cheatsheet.tomllmd")
    try:
        with open(cheatsheet, "rb") as f:
            data = tomllib.load(f)
        best_score = 0
        best_cmd = ""
        best_name = ""
        for key, section in data.items():
            if not isinstance(section, dict):
                continue
            action = section.get("action", "")
            command = section.get("command", "")
            if not action or not command:
                continue
            qwords = set(query.lower().split())
            awords = set(action.lower().replace(",", " ").replace("/", " ").split())
            # filter stop words from query
            stops = {"a","an","the","to","of","in","is","it","for","on","and","or","not","how","do","what","can","should","would","could","i","me","my","we","you","that","this","be","are","was","were","been","has","have","had","will","shall","may","might","must","does","did","doing","please","need","want"}
            qwords = qwords - stops
            score = len(qwords & awords) * 10 + len(qwords & {key.lower().split("_")[-1]}) * 5
            if score > best_score:
                best_score = score
                best_cmd = command
                best_name = key
        if best_score >= 5:
            print(best_cmd)
            sys.exit(0)
    except Exception:
        pass

    # ── 2. Fallback: search all datums ──────────────────────────────────────
    import glob
    datums_dir = os.path.expanduser("~/.b00t/_b00t_/datums")
    scored = []
    for f in glob.glob(f"{datums_dir}/*.tomllm*") + glob.glob(f"{datums_dir}/*.toml"):
        fname = os.path.basename(f).replace(".tomllmd","").replace(".toml","")
        score = 0
        for w in query.lower().split():
            if w in fname.lower():
                score += 10
        try:
            with open(f) as fh:
                content = fh.read()
        except:
            continue
        hint = re.search(r'^#\s*summary:\s*(.+)$', content, re.MULTILINE)
        hint_text = hint.group(1) if hint else ""
        for w in query.lower().split():
            if w in hint_text.lower():
                score += 5
        cmds_m = re.search(r'^#\s*cmds:\s*(.+)$', content, re.MULTILINE)
        cmds_text = cmds_m.group(1) if cmds_m else ""
        for w in query.lower().split():
            if w in cmds_text.lower():
                score += 8
        if score > 0:
            scored.append((score, fname, hint_text, cmds_text))
    scored.sort(key=lambda x: x[0], reverse=True)
    if scored:
        top = scored[0]
        if top[3]:
            print(top[3])
        else:
            print(f"# datum: {top[1]} — no cmds: tail-map")
        sys.exit(0)

    print(f"# no match for: {query}")
    sys.exit(1)

## ── Inference (provider-agnostic) ────────────────────────────────────────────
## Usage:
##   just ai-finetune::inference-deploy                        # default: k8s
##   INFERENCE_PROVIDER=runpod just ai-finetune::inference-deploy
##
## Providers: k8s (local ch0nky), runpod (serverless endpoint)
## Config: PROVIDER-RUNPOD.provider.tomllmd / PROVIDER-HF.provider.tomllmd

INFERENCE_PROVIDER := env_var_or_default("INFERENCE_PROVIDER", "k8s")
FRAMEWORK          := env_var_or_default("FRAMEWORK", "unsloth")
ORCHESTRATOR       := env_var_or_default("ORCHESTRATOR", "hf")

# Deploy inference endpoint — routes to provider impl below
inference-deploy:
    @echo "→ deploying inference via provider={{ INFERENCE_PROVIDER }}"
    @just ai-finetune::inference-deploy-{{ INFERENCE_PROVIDER }}

# Alias: get endpoint/pod status for active inference provider
inference-status:
    @just ai-finetune::inference-status-{{ INFERENCE_PROVIDER }}

# Alias: tear down inference endpoint for active provider
inference-teardown:
    @just ai-finetune::inference-teardown-{{ INFERENCE_PROVIDER }}

# ── k8s (local sm3lly ch0nky) ─────────────────────────────────────────────────

# Deploy ch0nky inference on local k8s (existing Qwen3 pod)
inference-deploy-k8s:
    kubectl rollout status deployment/ch0nky -n b00t-inference
    @echo "✓ k8s ch0nky running"

# k8s ch0nky status
inference-status-k8s:
    kubectl get pods -n b00t-inference -l app=ch0nky -o wide

# k8s: rolling restart to pick up new adapter (calls chonky-rollout)
inference-teardown-k8s:
    @echo "⚠️  k8s: teardown not supported — use: kubectl scale deployment/ch0nky --replicas=0 -n b00t-inference"

# ── RunPod Serverless ─────────────────────────────────────────────────────────

# Deploy ch0nky as RunPod serverless endpoint (scale-to-zero; requires RUNPOD_API_KEY)
# Endpoint config defaults from PROVIDER-RUNPOD.provider.tomllmd
# Pass RUNPOD_TEMPLATE_ID=<id> for the inference worker template
inference-deploy-runpod:
    @echo "→ deploying RunPod serverless endpoint"
    b00t-cli provider endpoint deploy \
      --provider runpod \
      --name b00t-ch0nky \
      --gpu-types "NVIDIA A40,NVIDIA RTX A6000"

# RunPod endpoint status by ID (set RUNPOD_ENDPOINT_ID after deploy)
inference-status-runpod endpoint_id="":
    #!/usr/bin/env bash
    set -euo pipefail
    id="{{ endpoint_id }}"
    if [[ -z "$id" ]]; then
        b00t-cli provider endpoint list --provider runpod
    else
        b00t-cli provider endpoint status --provider runpod "$id"
    fi

# Tear down RunPod endpoint (stops billing)
inference-teardown-runpod endpoint_id:
    b00t-cli provider endpoint teardown --provider runpod {{ endpoint_id }}

# List all RunPod endpoints
inference-list-runpod:
    b00t-cli provider endpoint list --provider runpod

# ── Training provider dispatch (future: ORCHESTRATOR=dstack) ──────────────────

# Submit training job via configured ORCHESTRATOR (hf|runpod; dstack in sprint 2)
provider-train config="config-cloud-coder.yaml" flavor="a100-large" timeout_hours="10.0":
    #!/usr/bin/env bash
    set -euo pipefail
    echo "→ training: FRAMEWORK={{ FRAMEWORK }} ORCHESTRATOR={{ ORCHESTRATOR }}"
    b00t-cli provider job submit \
      --provider {{ ORCHESTRATOR }} \
      --config fine-tune/{{ config }} \
      --image ghcr.io/elasticdotventures/b00t-training-image:latest \
      --flavor {{ flavor }} \
      --timeout-hours {{ timeout_hours }}

# List training jobs for active ORCHESTRATOR
provider-train-list:
    b00t-cli provider job list --provider {{ ORCHESTRATOR }}

# Cancel a training job
provider-train-cancel job_id:
    b00t-cli provider job cancel --provider {{ ORCHESTRATOR }} {{ job_id }}
