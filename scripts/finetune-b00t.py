#!/usr/bin/env python3
"""b00t unsloth finetune — QLoRA fine-tune on b00t source code.

Deterministic pipeline:
  1. Load base model (Qwen3.5-4B via unsloth)
  2. Load b00t training data (~/.b00t/training/b00t-corpus.jsonl)
  3. QLoRA train (4-bit base + LoRA adapters, fits 4-6GB VRAM)
  4. Save LoRA adapter
  5. Merge + export GGUF

Usage:
  uv pip install unsloth transformers datasets
  python3 scripts/finetune-b00t.py

Env vars:
  B00T_BASE_MODEL    huggingface model ID (default: unsloth/Qwen3.5-4B-GGUF → need base HF repo)
  B00T_TRAIN_DATA    path to training JSONL
  B00T_OUTPUT_DIR    output directory for LoRA + GGUF
"""

import os, json, sys
from pathlib import Path

B00T_ROOT = Path(os.environ.get("B00T_ROOT", os.path.expanduser("~/.b00t")))
BASE_MODEL = os.environ.get("B00T_BASE_MODEL", "unsloth/Qwen3.5-0.8B")
TRAIN_DATA = Path(os.environ.get("B00T_TRAIN_DATA", B00T_ROOT / "training" / "b00t-corpus.jsonl"))
OUTPUT_DIR = Path(os.environ.get("B00T_OUTPUT_DIR", B00T_ROOT / "training" / "output"))
LORA_NAME = "b00t-lora"


def install_deps():
    """Ensure required packages are available."""
    try:
        import unsloth, datasets, transformers, torch
    except ImportError:
        print("📦 Installing dependencies (uv pip install)...")
        os.system("uv pip install unsloth datasets transformers torch accelerate peft bitsandbytes")
        print("✅ Dependencies installed. Restart this script.")
        sys.exit(0)


def load_training_data(path: Path) -> list[dict]:
    """Load ChatML-formatted training data."""
    data = []
    with open(path) as f:
        for line in f:
            record = json.loads(line)
            if "messages" in record:
                data.append(record)
    print(f"📊 Loaded {len(data)} training examples from {path}")
    return data


def finetune():
    import torch
    from unsloth import FastLanguageModel
    from datasets import Dataset
    from transformers import TrainingArguments
    from trl import SFTTrainer

    install_deps()

    # ── Load model with QLoRA ──
    print(f"🔧 Loading base model: {BASE_MODEL}")
    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=BASE_MODEL,
        max_seq_length=1024,
        dtype=None,  # auto-detect
        load_in_4bit=True,  # QLoRA: 4-bit base
    )

    # ── LoRA config ──
    model = FastLanguageModel.get_peft_model(
        model,
        r=16,
        target_modules=["q_proj", "k_proj", "v_proj", "o_proj",
                        "gate_proj", "up_proj", "down_proj"],
        lora_alpha=16,
        lora_dropout=0,
        bias="none",
        use_gradient_checkpointing="unsloth",
        random_state=42,
    )

    # ── Format data ──
    data = load_training_data(TRAIN_DATA)

    def formatting_func(example):
        """Convert ChatML messages to tokenizer format."""
        return tokenizer.apply_chat_template(
            example["messages"],
            tokenize=False,
            add_generation_prompt=False,
        )

    dataset = Dataset.from_list(data)
    dataset = dataset.map(
        lambda x: {"text": formatting_func(x)},
        remove_columns=dataset.column_names,
    )

    # ── Train ──
    trainer = SFTTrainer(
        model=model,
        tokenizer=tokenizer,
        train_dataset=dataset,
        dataset_text_field="text",
        max_seq_length=1024,
        args=TrainingArguments(
            per_device_train_batch_size=1,
            gradient_accumulation_steps=4,
            warmup_steps=5,
            max_steps=200,
            learning_rate=2e-4,
            fp16=not torch.cuda.is_bf16_supported(),
            bf16=torch.cuda.is_bf16_supported(),
            logging_steps=10,
            optim="adamw_8bit",
            weight_decay=0.01,
            lr_scheduler_type="linear",
            seed=42,
            output_dir=str(OUTPUT_DIR / "checkpoints"),
            report_to="none",
        ),
    )

    print(f"🚀 Starting finetune — {len(dataset)} examples, {trainer.args.max_steps} steps")
    trainer.train()

    # ── Save LoRA ──
    lora_path = str(OUTPUT_DIR / LORA_NAME)
    model.save_pretrained(lora_path)
    tokenizer.save_pretrained(lora_path)
    print(f"💾 LoRA adapter saved to {lora_path}")

    # ── Merge + export GGUF ──
    print("🔨 Merging LoRA into base model...")
    merged = model.merge_and_unload()

    # 🤓 Patch missing non-LoRA tensors (LayerNorm weights).
    #    merge_and_unload() preserves LoRA targets; some base-model tensors
    #    stored as bnb.nn.Linear4bit may lose their state_dict keys.
    #    Symptom: llama-server reports "missing tensor 'blk.24.attn_norm.weight'"
    print("🔍 Verifying tensor completeness...")
    import torch.nn as nn
    state = merged.state_dict()
    missing = []
    for module_name, module in merged.named_modules():
        if isinstance(module, nn.LayerNorm):
            key = f"{module_name}.weight"
            if key not in state and module_name.count('.') >= 1:
                missing.append(module_name)
    if missing:
        print(f"   ⚠️  {len(missing)} LayerNorm tensors missing — patching from base model...")
        from transformers import AutoModelForCausalLM
        base = AutoModelForCausalLM.from_pretrained(
            "unsloth/Qwen3.5-0.8B", torch_dtype="auto", device_map="cpu")
        for module_name in missing:
            parts = module_name.split(".")
            src, dst = base, merged
            for p in parts:
                src = getattr(src, p)
                dst = getattr(dst, p)
            dst.weight.data.copy_(src.weight.data)
            print(f"   ✅ patched {module_name}.weight")
        del base
    else:
        print("   ✅ all tensors present")
    merged_path = str(OUTPUT_DIR / "merged")
    merged.save_pretrained(merged_path)
    tokenizer.save_pretrained(merged_path)
    print(f"💾 Merged model saved to {merged_path}")

    # ── Convert to GGUF via llama.cpp (not unsloth save_gguf — format mismatch) ──
    gguf_path = str(OUTPUT_DIR / "b00t-finetuned.Q4_K_M.gguf")
    print(f"🔨 Converting to GGUF: {gguf_path}")
    # Use llama.cpp's convert + quantize pipeline (compatible with stock server)
    import subprocess
    subprocess.run([
        "python3", os.path.expanduser("~/.unsloth/llama.cpp/convert_hf_to_gguf.py"),
        "--outfile", str(OUTPUT_DIR / "b00t-finetuned.BF16.gguf"),
        "--outtype", "bf16",
        merged_path,
    ], check=True)
    subprocess.run([
        os.path.expanduser("~/.unsloth/llama.cpp/llama-quantize"),
        str(OUTPUT_DIR / "b00t-finetuned.BF16.gguf"),
        gguf_path,
        "q4_k_m",
    ], check=True)
    print(f"✅ GGUF saved to {gguf_path} ({os.path.getsize(gguf_path)//1024//1024}MB)")

    return gguf_path


if __name__ == "__main__":
    print("=== b00t unsloth finetune pipeline ===")
    print(f"   Base model: {BASE_MODEL}")
    print(f"   Training data: {TRAIN_DATA} ({TRAIN_DATA.stat().st_size if TRAIN_DATA.exists() else 0} bytes)")
    print(f"   Output dir: {OUTPUT_DIR}")
    print()
    finetune()
