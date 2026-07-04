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


# ⚠️ DEPRECATED — Qwen2.5-Coder-14B-Instruct not desired; config archived for reference
# Do not run. Create a new recipe + config for the target model instead.
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


# ── MLflow public tunnel (required for RunPod training) ───────────────────────
# Exposes local MLflow (NodePort 30803) via Cloudflare Tunnel.
# Run once before `b00t-cli runpod train`; copy the https URL printed and:
#   export MLFLOW_TRACKING_URI=https://xxx.trycloudflare.com
# Keep this terminal open for the duration of training.
mlflow-tunnel:
    #!/usr/bin/env bash
    echo "Starting Cloudflare tunnel → MLflow (port 30803)"
    echo "Copy the https://xxx.trycloudflare.com URL and:"
    echo "  export MLFLOW_TRACKING_URI=https://xxx.trycloudflare.com"
    echo ""
    cloudflared tunnel --url http://localhost:30803

# ── Local smoke test (5 steps, verifies full pipeline without full training) ──
# Prerequisites: just ai-finetune::inference-down  (frees GPU)
# After:         just ai-finetune::inference-up    (restores inference)
# 🤓 Uses podman + unsloth image — no local Python package installs; same env as RunPod.
#    --network=host lets the container reach the k8s MLflow NodePort (localhost:30803).
#    --userns=keep-id: rootless podman bind mount (not --user uid:gid).
train-smoke-test:
    #!/usr/bin/env bash
    set -euo pipefail
    export MLFLOW_TRACKING_URI="${MLFLOW_TRACKING_URI:-http://localhost:30803}"
    echo "GPU free before:"
    nvidia-smi --query-gpu=memory.free --format=csv,noheader
    echo "MLflow: $MLFLOW_TRACKING_URI"
    echo ""
    # 🤓 unsloth:latest uses supervisord entrypoint (studio UI) — must override.
    #    Python lives in /opt/venv; uv mounted from host for installs.
    #    --userns=keep-id conflicts with nvidia-container-toolkit hook → omit it.
    # 🤓 HF cache is mounted so the model survives container restarts.
    #    unsloth writes to /root/.cache/huggingface — keep-id would conflict with nvidia hook,
    #    so we mount host cache path directly (same uid=root inside container is fine for read).
    podman run --rm \
        --device nvidia.com/gpu=all \
        --security-opt=label=disable \
        --entrypoint bash \
        --network=host \
        -v "$(pwd):/workspace/b00t:ro" \
        -v "$(which uv):/usr/local/bin/uv:ro" \
        -v "$HOME/.cache/huggingface:/root/.cache/huggingface:Z" \
        -e MLFLOW_TRACKING_URI="$MLFLOW_TRACKING_URI" \
        -e MLFLOW_EXPERIMENT_NAME="${MLFLOW_EXPERIMENT_NAME:-b00t-qwen36-finetune}" \
        -e HF_TOKEN="${HF_TOKEN:-}" \
        docker.io/unsloth/unsloth:latest \
        -c "uv pip install --python /opt/venv/bin/python3 -q 'transformers>=5.2.0' mlflow pyyaml datasets trl 2>&1 | tail -3 && \
            cd /workspace/b00t && \
            /opt/venv/bin/python3 fine-tune/train_unsloth.py --config fine-tune/config.yaml --max-steps 5"
    echo ""
    echo "✅ smoke-test done — http://localhost:30803"

# Scale down inference to free GPU for local training, then restore after
inference-down:
    kubectl scale deployment ch0nky --replicas=0 -n b00t-inference
    kubectl scale deployment sm0l --replicas=0 -n b00t-inference
    @echo "⏳ waiting for pods to terminate..."
    kubectl wait --for=delete pod -l app=ch0nky -n b00t-inference --timeout=60s || true
    kubectl wait --for=delete pod -l app=sm0l -n b00t-inference --timeout=60s || true
    nvidia-smi --query-gpu=memory.free --format=csv,noheader

inference-up:
    kubectl scale deployment ch0nky --replicas=1 -n b00t-inference
    kubectl scale deployment sm0l --replicas=1 -n b00t-inference
    @echo "inference restored"

# ── RunPod training lifecycle ──────────────────────────────────────────────────
# Launch a training pod on RunPod A100 80GB (on-demand, SECURE cloud).
# Requires: RUNPOD_API_KEY, HF_TOKEN, MLFLOW_TRACKING_URI env vars set.
# Set MLFLOW_TRACKING_URI to the cloudflared tunnel URL before running.
# Usage: just ai-finetune::runpod-train
runpod-train:
    #!/usr/bin/env bash
    set -euo pipefail
    : "${RUNPOD_API_KEY:?RUNPOD_API_KEY required — add to ~/.b00t/.env}"
    : "${HF_TOKEN:?HF_TOKEN required for hub push}"
    : "${MLFLOW_TRACKING_URI:?MLFLOW_TRACKING_URI required — run mlflow-tunnel first}"
    export MLFLOW_EXPERIMENT_NAME="${MLFLOW_EXPERIMENT_NAME:-b00t-qwen36-finetune}"
    b00t-cli runpod train --config fine-tune/config.yaml --gpu "NVIDIA A100-SXM4-80GB"

# Tail logs for a running RunPod training pod.
# Usage: just ai-finetune::runpod-logs <pod-id>
runpod-logs pod_id:
    b00t-cli runpod logs {{pod_id}}

# Show status of all b00t RunPod pods.
runpod-pods:
    b00t-cli runpod pods

# Stop a RunPod training pod via b00t-cli (does not delete — preserves logs/volumes).
# Usage: just ai-finetune::runpod-cli-stop <pod-id>
runpod-cli-stop pod_id:
    b00t-cli runpod stop {{pod_id}}

# Delete a RunPod training pod (permanent).
# Usage: just ai-finetune::runpod-delete <pod-id>
runpod-delete pod_id:
    b00t-cli runpod delete {{pod_id}}

# Dry-run: print pod spec without launching.
runpod-dry-run:
    #!/usr/bin/env bash
    export MLFLOW_TRACKING_URI="${MLFLOW_TRACKING_URI:-http://192.168.1.137:30803}"
    export MLFLOW_EXPERIMENT_NAME="${MLFLOW_EXPERIMENT_NAME:-b00t-qwen36-finetune}"
    b00t-cli runpod train --config fine-tune/config.yaml --gpu "NVIDIA A100-SXM4-80GB" --dry-run

# ── SAM3 local provider ────────────────────────────────────────────────────────
# Build the app4dog/sam3-runner:local container image.
sam3-build:
    podman build -t app4dog/sam3-runner:local app4dog/sam3-runner/
    @echo "✅ app4dog/sam3-runner:local built"

# Run SAM3 on a request JSON file.
# Usage: just ai-finetune::sam3-run /path/to/request.json /path/to/output/
sam3-run request_path output_dir:
    #!/usr/bin/env bash
    set -euo pipefail
    [[ -f "{{request_path}}" ]] || { echo "ERROR: request file not found: {{request_path}}"; exit 1; }
    mkdir -p "{{output_dir}}"
    podman run --rm \
        --device nvidia.com/gpu=all \
        --security-opt=label=disable \
        --entrypoint python3 \
        -v "{{request_path}}:/workspace/request.json:ro" \
        -v "{{output_dir}}:/workspace:rw" \
        app4dog/sam3-runner:local \
        /app/run.py /workspace/request.json
    @echo "✅ result → {{output_dir}}/result.json"

# Quick smoke-test: segment a test image with a text prompt.
sam3-smoke:
    #!/usr/bin/env bash
    set -euo pipefail
    TMP=$(mktemp -d)
    # Download a small test image
    curl -sL "https://upload.wikimedia.org/wikipedia/commons/thumb/4/43/Cute_dog.jpg/320px-Cute_dog.jpg" -o "$TMP/dog.jpg"
    # write request.json via base64 to avoid just parser issues with float literals
    echo 'eyJzb3VyY2VfdHlwZSI6InBhdGgiLCJpbWFnZV9wYXRoIjoiL3dvcmtzcGFjZS9kb2cuanBnIiwib3V0cHV0X3BhdGgiOiIvd29ya3NwYWNlL3Jlc3VsdC5qc29uIiwicHJvbXB0cyI6W3sidHlwZSI6InRleHQiLCJ2YWx1ZSI6ImRvZyJ9XSwibW9kZWxfaWQiOiJmYWNlYm9vay9zYW0zIiwiZGV2aWNlIjoiY3VkYSIsInNjb3JlX3RocmVzaG9sZCI6MC4zLCJvdXRwdXRfZm9ybWF0IjoicmxlIn0=' | base64 -d > "$TMP/request.json"
    podman run --rm \
        --device nvidia.com/gpu=all \
        --security-opt=label=disable \
        -v "$TMP:/workspace:rw" \
        app4dog/sam3-runner:local
    echo "--- result ---"
    python3 -c "import json; d=json.load(open('$TMP/result.json')); print(f'{d[\"segment_count\"]} segments, timing={d[\"timing\"]}')"
    echo "✅ SAM3 smoke test passed → $TMP/result.json"

# ── RunPod session monitoring (recurring tasks) ────────────────────────────────
# 🤓 These are the recurring checks needed every RunPod training session.
#    GPU=0% after model-download phase means either training completed or failed.
#    HF Hub is ground truth for completion; pod status only tells you it's alive.
#    Train log is at /workspace/train.log inside the container — no direct access
#    without supportPublicIp=true (not in runpod crate v0.1.30).

# Show all running pods with GPU utilization via RunPod GraphQL.
# 🤓 just tokenizes every line of shebang recipe body — use #!/usr/bin/env python3 to avoid
#    multiline python3 -c "..." failures; Python reads .env directly.
runpod-gpu-status:
    #!/usr/bin/env python3
    import json, os, sys, urllib.request
    env_path = os.path.expanduser("~/.b00t/.env")
    env = {}
    if os.path.exists(env_path):
        for line in open(env_path):
            line = line.strip()
            if "=" in line and not line.startswith("#"):
                k, _, v = line.partition("=")
                env[k.strip()] = v.strip().strip('"').strip("'")
    key = env.get("RUNPOD_API_KEY") or os.environ.get("RUNPOD_API_KEY", "")
    if not key:
        print("ERROR: RUNPOD_API_KEY not in ~/.b00t/.env"); sys.exit(1)
    req = urllib.request.Request(
        "https://rest.runpod.io/v1/pods",
        headers={"Authorization": f"Bearer {key}"},
    )
    with urllib.request.urlopen(req, timeout=15) as r:
        pods = json.load(r)
    # 🚩 REST /v1/pods exposes env vars incl HF_TOKEN in plaintext — never log raw response
    if not pods:
        print("no pods"); sys.exit(0)
    for p in pods:
        created = p.get("lastStartedAt", "?")[:19]
        cost = p.get("costPerHr", "?")
        gpus = p.get("gpuCount", "?")
        print(f"{p['id']}  {p.get('name',''):30s}  {p.get('desiredStatus',''):10s}  GPUs={gpus}  ${cost}/hr  started={created}")

# Check whether HF Hub adapter repo exists and show latest files.
# Usage: just ai-finetune::runpod-hf-check repo="elasticdotventures/b00t-qwen36-35b-a3b"
runpod-hf-check repo="elasticdotventures/b00t-qwen36-35b-a3b":
    #!/usr/bin/env python3
    import json, os, sys, urllib.request, urllib.error
    repo = "{{repo}}"
    tok = os.environ.get("HUGGING_FACE_HUB_TOKEN", "")
    if not tok:
        print("ERROR: HUGGING_FACE_HUB_TOKEN not set"); sys.exit(1)
    req = urllib.request.Request(
        f"https://huggingface.co/api/models/{repo}",
        headers={"Authorization": f"Bearer {tok}"},
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            d = json.load(r)
        print(f"✅ MODEL EXISTS: {d.get('id')}")
        print(f"   lastModified: {d.get('lastModified')}")
        sibs = d.get("siblings") or []
        print(f"   files: {[s['rfilename'] for s in sibs[:20]]}")
    except urllib.error.HTTPError as e:
        if e.code == 404:
            print(f"❌ NOT FOUND: {repo} — training not yet complete or HF push failed")
        else:
            print(f"HTTP {e.code}: {e.reason}")

# Poll pod status + HF Hub every N seconds until adapter appears or pod stops.
# Usage: just ai-finetune::runpod-wait pod_id="dsmiude96elaym" repo="elasticdotventures/b00t-qwen36-35b-a3b" interval="120"
runpod-wait pod_id="dsmiude96elaym" repo="elasticdotventures/b00t-qwen36-35b-a3b" interval="120":
    #!/usr/bin/env bash
    set -euo pipefail
    KEY=$(grep -m1 'RUNPOD_API_KEY' ~/.b00t/.env 2>/dev/null | cut -d= -f2- | tr -d "\"'")
    i=0
    while true; do
        i=$((i+1))
        TS=$(date '+%H:%M:%S')
        STATUS=$(curl -s --max-time 10 "https://rest.runpod.io/v1/pods/{{pod_id}}" -H "Authorization: Bearer $KEY" 2>/dev/null | python3 -c "import json,sys; raw=sys.stdin.read(); d=json.loads(raw) if raw.strip() else {}; print(d.get('desiredStatus') or d.get('error','curl-fail'))" 2>/dev/null || echo "curl-fail")
        HF=$(python3 -c "import urllib.request,os,sys; urllib.request.urlopen(urllib.request.Request('https://huggingface.co/api/models/{{repo}}',headers={'Authorization':'Bearer '+os.environ.get('HUGGING_FACE_HUB_TOKEN','')}),timeout=8); print('FOUND')" 2>/dev/null || echo "NOT_FOUND")
        echo "$TS [$i] {{pod_id}} — $STATUS — HF:$HF"
        [[ "$HF" == "FOUND" ]] && { echo "✅ Adapter on HF Hub — training complete."; exit 0; }
        [[ "$STATUS" == EXITED* || "$STATUS" == STOPPED* ]] && { echo "⚠️  Pod stopped before HF push"; exit 1; }
        sleep {{interval}}
    done

# Terminate a specific pod to stop billing.
# Usage: just ai-finetune::runpod-stop pod_id="dsmiude96elaym"
runpod-stop pod_id="dsmiude96elaym":
    #!/usr/bin/env bash
    set -euo pipefail
    KEY=$(grep -m1 'RUNPOD_API_KEY' ~/.b00t/.env 2>/dev/null | cut -d= -f2- | tr -d "\"'")
    echo "⚠️  Stopping pod {{pod_id}} (billing will end)"
    curl -s -X DELETE "https://rest.runpod.io/v1/pods/{{pod_id}}" -H "Authorization: Bearer $KEY" | python3 -c "import json,sys; print(json.load(sys.stdin))"


# ── Composable RunPod training pipeline — Kaizen auto-remediation ──────────────
# 🤓 Each stage is a discrete recipe with structured exit codes:
#    0 = PASS, 1 = FAIL (fatal), 2 = RETRY (transient — caller should retry)
#    Chain: runpod-preflight → runpod-train → runpod-wait → runpod-verify
#    On stage FAIL: agent-remediate is called with stage name + last log lines.
#    b00t kaizen: every failure writes a lfmf entry for future sessions.

# Stage 0: Pre-flight — validate env, costs, disk, GPU availability before launch.
# Exit 0=OK, 1=BLOCKED (operator must resolve), 2=WARN (proceed with caution)
runpod-preflight gpu="A100 SXM4 80GB" max_cost_hr="3.0":
    #!/usr/bin/env python3
    import os, sys, urllib.request, json
    errors = []
    warnings = []
    tok = os.environ.get("HUGGING_FACE_HUB_TOKEN", "")
    if not tok:
        errors.append("HUGGING_FACE_HUB_TOKEN not set — HF push will fail")
    env_path = os.path.expanduser("~/.b00t/.env")
    env = {}
    if os.path.exists(env_path):
        for line in open(env_path):
            if "=" in line and not line.startswith("#"):
                k, _, v = line.partition("=")
                env[k.strip()] = v.strip().strip('"').strip("'")
    key = env.get("RUNPOD_API_KEY") or os.environ.get("RUNPOD_API_KEY", "")
    if not key:
        errors.append("RUNPOD_API_KEY not in ~/.b00t/.env")
    if not env.get("MLFLOW_TRACKING_URI"):
        warnings.append("MLFLOW_TRACKING_URI not set — training won't log to MLflow")
    if float("{{max_cost_hr}}") > 5.0:
        errors.append(f"max_cost_hr={{max_cost_hr}} exceeds safety cap of $5.00/hr")
    for e in errors:
        print(f"🚫 BLOCKED: {e}")
    for w in warnings:
        print(f"⚠️  WARN: {w}")
    if errors:
        sys.exit(1)
    print(f"✅ preflight OK — GPU={{gpu}} max=${{'{{max_cost_hr}}'}}/hr tok={tok[:8]}***")
    sys.exit(2 if warnings else 0)

# Stage 1: Launch training pod (wraps b00t-cli runpod train).
# Emits pod_id to stdout on success for downstream stages.
runpod-launch config="fine-tune/config.yaml" gpu="A100 SXM4 80GB" spot="false":
    #!/usr/bin/env bash
    set -euo pipefail
    just --justfile {{justfile()}} runpod-preflight || { echo "FAIL: preflight blocked"; exit 1; }
    POD_ID=$(b00t-cli runpod train --config "{{config}}" --gpu-type "{{gpu}}" --spot {{spot}} --dry-run=false 2>&1 | grep -oP 'pod_id=\K[a-z0-9]+' || true)
    if [[ -z "$POD_ID" ]]; then
        echo "FAIL: b00t-cli runpod train did not emit pod_id — check b00t-cli runpod train output"
        b00t-cli runpod train --config "{{config}}" --gpu-type "{{gpu}}" --spot {{spot}} 2>&1 | tail -10
        exit 1
    fi
    echo "pod_id=${POD_ID}"
    echo "${POD_ID}" > /tmp/b00t-runpod-active-pod-id
    echo "✅ launched: ${POD_ID}"

# Stage 2: Block until HF adapter appears or pod stops (calls runpod-wait).
# Reads pod_id from /tmp/b00t-runpod-active-pod-id if not provided.
runpod-await pod_id="" repo="elasticdotventures/b00t-qwen36-35b-a3b" timeout_min="180":
    #!/usr/bin/env bash
    set -euo pipefail
    PID="{{pod_id}}"
    [[ -z "$PID" && -f /tmp/b00t-runpod-active-pod-id ]] && PID=$(cat /tmp/b00t-runpod-active-pod-id)
    [[ -z "$PID" ]] && { echo "FAIL: no pod_id — run runpod-launch first or pass pod_id=<id>"; exit 1; }
    DEADLINE=$(( $(date +%s) + {{timeout_min}} * 60 ))
    echo "⏳ waiting for ${PID} → HF:{{repo}} (timeout={{timeout_min}}min)"
    just --justfile {{justfile()}} runpod-wait pod_id="$PID" repo="{{repo}}" interval="90" &
    WAIT_PID=$!
    while kill -0 $WAIT_PID 2>/dev/null; do
        [[ $(date +%s) -gt $DEADLINE ]] && { kill $WAIT_PID 2>/dev/null; echo "FAIL: timeout after {{timeout_min}}min"; exit 1; }
        sleep 15
    done
    wait $WAIT_PID
    EXIT=$?
    if [[ $EXIT -eq 0 ]]; then
        echo "✅ await complete — adapter on HF Hub"
    else
        echo "FAIL: runpod-wait exited $EXIT"
        exit $EXIT
    fi

# Stage 3: Verify adapter is usable (HF Hub exists + files present).
runpod-verify repo="elasticdotventures/b00t-qwen36-35b-a3b":
    #!/usr/bin/env python3
    import json, os, sys, urllib.request, urllib.error
    repo = "{{repo}}"
    tok = os.environ.get("HUGGING_FACE_HUB_TOKEN", "")
    req = urllib.request.Request(
        f"https://huggingface.co/api/models/{repo}",
        headers={"Authorization": f"Bearer {tok}"},
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            d = json.load(r)
    except urllib.error.HTTPError as e:
        print(f"FAIL: HF Hub {e.code} — {repo} not found"); sys.exit(1)
    sibs = [s["rfilename"] for s in (d.get("siblings") or [])]
    required = ["adapter_config.json", "adapter_model.safetensors"]
    missing = [f for f in required if not any(f in s for s in sibs)]
    if missing:
        print(f"FAIL: adapter incomplete — missing {missing}"); print(f"  found: {sibs}"); sys.exit(1)
    print(f"✅ verify PASS — {repo}")
    print(f"   lastModified: {d.get('lastModified')}")
    print(f"   files: {sibs[:10]}")

# Full end-to-end pipeline: preflight → launch → await → verify.
# 🤓 Kaizen agent-fix-first on stage failure:
#    1. consult lfmf FIRST (prior lessons inform the fix agent)
#    2. spawn fix agent via `b00t-cli ooda run` (3 iterations)
#    3. agent fixed it → retry the failed stage ONCE
#    4. only if the agent exhausted its iterations → record a NEW lfmf lesson
#    This is fail → consult → agent-fix → (unfixable) → lfmf; NOT fail → lfmf.
runpod-pipeline config="fine-tune/config.yaml" repo="elasticdotventures/b00t-qwen36-35b-a3b" gpu="A100 SXM4 80GB" spot="false" timeout_min="180":
    #!/usr/bin/env bash
    set -euo pipefail
    # fail_stage: Kaizen agent-fix-first. Returns 0 if the agent fixed the
    # stage (caller retries it once); exits 1 after recording a lesson if not.
    fail_stage() {
        local stage=$1 msg=$2
        echo "❌ STAGE FAILED: ${stage} — ${msg}"
        # 1. Consult prior lessons FIRST so the fix agent benefits from them
        local advice
        advice=$(b00t-cli lfmf advice runpod 2>/dev/null | tr '\n' ' ' | head -c 800)
        [[ -z "${advice// /}" ]] && advice="no prior lessons"
        echo "📚 prior lfmf advice: ${advice}"
        # 2. Create fix task carrying stage context + advice
        local task_id
        task_id=$(b00t-cli task add "fix: runpod ${stage} failed — ${msg}. Advice: ${advice}" 2>/dev/null | grep -oP 'added #\K[0-9]+' || echo "")
        # 3. Agent attempts the fix — 3 iterations max
        if [[ -n "$task_id" ]]; then
            echo "🤖 dispatching fix agent (task #${task_id}, max-iter=3)"
            if b00t-cli ooda run --agent=claude --task="$task_id" --max-iter=3; then
                echo "✅ Agent fixed ${stage} — retrying stage"
                return 0
            fi
        else
            echo "⚠️ could not create fix task — skipping agent remediation"
        fi
        # 4. Agent could not fix — NOW record the lesson for future agents
        b00t-cli lfmf runpod "${stage}: ${msg} (agent exhausted 3 iterations)" 2>/dev/null || true
        exit 1
    }
    # run_stage <stage> <fail-msg> <cmd...>: run cmd; on failure invoke
    # fail_stage; if the agent fixed it, retry the stage ONCE before giving up.
    run_stage() {
        local stage=$1 msg=$2; shift 2
        if "$@"; then return 0; fi
        fail_stage "$stage" "$msg"   # exits 1 unless the agent fixed it
        echo "🔁 retrying ${stage} after agent fix"
        if "$@"; then return 0; fi
        echo "❌ ${stage} failed again after agent fix"
        b00t-cli lfmf runpod "${stage}: ${msg} (failed again after agent fix)" 2>/dev/null || true
        exit 1
    }
    echo "=== b00t RunPod Training Pipeline ==="
    echo "  config:  {{config}}"
    echo "  repo:    {{repo}}"
    echo "  gpu:     {{gpu}}"
    echo "  spot:    {{spot}}"
    echo "  timeout: {{timeout_min}}min"
    echo ""
    run_stage preflight "env/cost gate" just --justfile {{justfile()}} runpod-preflight gpu="{{gpu}}"
    run_stage launch "pod did not start" just --justfile {{justfile()}} runpod-launch config="{{config}}" gpu="{{gpu}}" spot="{{spot}}"
    POD_ID=$(cat /tmp/b00t-runpod-active-pod-id 2>/dev/null || echo "")
    run_stage await "training timeout or pod stopped" just --justfile {{justfile()}} runpod-await pod_id="$POD_ID" repo="{{repo}}" timeout_min="{{timeout_min}}"
    run_stage verify "adapter missing from HF Hub" just --justfile {{justfile()}} runpod-verify repo="{{repo}}"
    echo ""
    echo "✅ Pipeline COMPLETE — adapter at https://huggingface.co/{{repo}}"
