#!/usr/bin/env python3
"""Unsloth QLoRA fine-tuning for b00t-aligned Qwen3.6-27B.

Trains on the b00t corpus (datums, learn files, justfile, AGENTS) to produce
a ch0nky-tier model that understands b00t idioms natively.

Usage:
    uv run python fine-tune/train_unsloth.py --config fine-tune/config.yaml
"""

import argparse
import json
import os
import sys
from pathlib import Path

os.environ["HF_HOME"] = os.environ.get("HF_HOME", os.path.expanduser("~/.cache/huggingface"))


def load_config(config_path: str) -> dict:
    """Load training configuration from YAML or return defaults."""
    try:
        import yaml
        with open(config_path) as f:
            return yaml.safe_load(f)
    except (ImportError, FileNotFoundError) as e:
        print(f"  ⚠️  Using default config ({e})", file=sys.stderr)
        return {}


def train(config: dict):
    """Run the unsloth QLoRA fine-tuning loop."""
    model_name = config.get("base_model", "unsloth/Qwen3.6-27B-GGUF")
    adapter_name = config.get("adapter_name", "b00t-aligned-qwen36-27b")
    dataset_path = config.get("dataset_path", config.get("dataset", "fine-tune/train.jsonl"))
    output_dir = config.get("output_dir", "./fine-tune/output")
    lora_r = config.get("lora_r", 16)
    lora_alpha = config.get("lora_alpha", 32)
    lora_dropout = config.get("lora_dropout", 0.05)
    learning_rate = float(config.get("learning_rate", 2e-4))
    num_epochs = int(config.get("num_epochs", 3))
    per_device_batch_size = int(config.get("batch_size", 2))
    gradient_accumulation_steps = int(config.get("grad_accum", 8))
    max_seq_length = int(config.get("max_seq_length", 2048))

    print(f"🚀 Unsloth QLoRA Fine-Tuning")
    print(f"   Base model: {model_name}")
    print(f"   Dataset: {dataset_path}")
    print(f"   LoRA r={lora_r} alpha={lora_alpha} dropout={lora_dropout}")
    print(f"   Epochs: {num_epochs}, Batch: {per_device_batch_size}, Grad Accum: {gradient_accumulation_steps}")
    print(f"   Max seq length: {max_seq_length}")
    print()

    # ─── Load model with unsloth ───────────────────────────────────────────────
    print("Loading model...", end=" ", flush=True)
    try:
        from unsloth import FastLanguageModel
    except ImportError:
        print("\n  ❌ unsloth not installed. Run: uv pip install unsloth")
        sys.exit(1)

    try:
        model, tokenizer = FastLanguageModel.from_pretrained(
            model_name=model_name,
            max_seq_length=max_seq_length,
            load_in_4bit=config.get("load_in_4bit", True),
            dtype=None,
        )
    except Exception as e:
        print(f"\n  ❌ Failed to load base model: {e}")
        print("  Make sure the GGUF file is downloaded:")
        print("  hf download unsloth/Qwen3.6-27B-GGUF --include 'Qwen3.6-27B-Q4_K_M.gguf'")
        sys.exit(1)

    print("✓")

    # ─── Apply LoRA ────────────────────────────────────────────────────────────
    print("Applying LoRA adapters...", end=" ", flush=True)
    model = FastLanguageModel.get_peft_model(
        model,
        r=lora_r,
        target_modules=["q_proj", "v_proj", "k_proj", "o_proj", "gate_proj", "up_proj", "down_proj"],
        lora_alpha=lora_alpha,
        lora_dropout=lora_dropout,
        bias="none",
        use_gradient_checkpointing="unsloth",
        random_state=42,
    )
    print("✓")

    # ─── Load dataset ──────────────────────────────────────────────────────────
    print("Loading dataset...", end=" ", flush=True)
    if not os.path.exists(dataset_path):
        print(f"\n  ❌ Dataset not found: {dataset_path}")
        print("  Run: uv run python fine-tune/generate_dataset.py")
        sys.exit(1)

    try:
        from datasets import Dataset as HFDataset, load_dataset
        from trl import SFTTrainer
        from transformers import TrainingArguments
    except ImportError as e:
        print(f"\n  ❌ Missing dependency: {e}")
        print("  Run: uv pip install unsloth datasets trl transformers accelerate")
        sys.exit(1)

    # Load JSONL
    data = []
    with open(dataset_path) as f:
        for line in f:
            row = json.loads(line)
            instruction = row.get("instruction", "")
            input_text = row.get("input", "")
            response = row.get("response", "")
            if input_text.strip():
                text = f"### Instruction:\n{instruction}\n\n### Input:\n{input_text}\n\n### Response:\n{response}"
            else:
                text = f"### Instruction:\n{instruction}\n\n### Response:\n{response}"
            data.append({"text": text})

    dataset = HFDataset.from_list(data)
    print(f"✓ ({len(dataset)} examples)")

    # ─── Training ──────────────────────────────────────────────────────────────
    print("Starting training...")
    print()

    trainer = SFTTrainer(
        model=model,
        tokenizer=tokenizer,
        train_dataset=dataset,
        dataset_text_field="text",
        max_seq_length=max_seq_length,
        dataset_num_proc=2,
        packing=False,
        args=TrainingArguments(
            per_device_train_batch_size=per_device_batch_size,
            gradient_accumulation_steps=gradient_accumulation_steps,
            warmup_ratio=float(config.get("warmup_ratio", 0.05)),
            num_train_epochs=num_epochs,
            learning_rate=learning_rate,
            fp16=not model.dtype.is_floating_point,
            bf16=model.dtype.is_floating_point,
            logging_steps=int(config.get("logging_steps", 10)),
            optim=config.get("optim", "adamw_8bit"),
            weight_decay=float(config.get("weight_decay", 0.01)),
            lr_scheduler_type=config.get("lr_scheduler", "linear"),
            seed=42,
            output_dir=output_dir,
            save_steps=int(config.get("save_steps", 0)) or None,
            report_to=config.get("report_to", "none"),
        ),
    )

    trainer.train()

    # ─── Export ────────────────────────────────────────────────────────────────
    print("\nSaving LoRA adapter...")
    model.save_pretrained(f"{output_dir}/lora-adapter")
    tokenizer.save_pretrained(f"{output_dir}/lora-adapter")

    if config.get("push_to_hub") and config.get("hub_model_id"):
        hub_id = config["hub_model_id"]
        print(f"\nPushing adapter to HF Hub: {hub_id}...")
        model.push_to_hub(hub_id)
        tokenizer.push_to_hub(hub_id)
        print(f"✓ https://huggingface.co/{hub_id}")

    print(f"\n✅ Fine-tuning complete!")
    print(f"   Adapter: {output_dir}/lora-adapter")
    print(f"   To export to GGUF: uv run python fine-tune/export_gguf.py --adapter {output_dir}/lora-adapter")
    print(f"   To test: b00t hive activate inference-qwen36-27b-llamacpp")


def main():
    parser = argparse.ArgumentParser(description="Unsloth QLoRA fine-tuning for b00t")
    parser.add_argument("--config", default="fine-tune/config.yaml", help="Training config YAML")
    args = parser.parse_args()

    config = load_config(args.config)
    train(config)


if __name__ == "__main__":
    main()
