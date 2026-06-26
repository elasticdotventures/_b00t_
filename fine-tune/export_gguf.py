#!/usr/bin/env python3
"""Export fine-tuned LoRA adapter to GGUF for llama.cpp/vLLM inference.

Usage:
    uv run python fine-tune/export_gguf.py --adapter ./fine-tune/output/lora-adapter --quant Q4_K_M
"""

import argparse
import os
import sys


def export(adapter_path: str, quant: str, output_path: str):
    """Merge LoRA adapter into base model and export as GGUF."""
    import json
    print(f"🚀 Exporting LoRA adapter to GGUF ({quant})")
    print(f"   Adapter: {adapter_path}")
    print(f"   Output: {output_path}")

    # Read base model from adapter_config.json
    adapter_cfg_path = os.path.join(adapter_path, "adapter_config.json")
    if not os.path.exists(adapter_cfg_path):
        print(f"\n  ❌ adapter_config.json not found at {adapter_cfg_path}")
        sys.exit(1)
    with open(adapter_cfg_path) as f:
        adapter_cfg = json.load(f)
    base_model = adapter_cfg.get("base_model_name_or_path", "unsloth/Qwen3.6-27B-GGUF")
    print(f"   Base model: {base_model}")

    # Load base model + adapter
    print("Loading base model with adapter...", end=" ", flush=True)
    try:
        from unsloth import FastLanguageModel
        from peft import PeftModel
    except ImportError as e:
        print(f"\n  ❌ Missing dependency: {e}. Run: uv pip install unsloth peft")
        sys.exit(1)

    try:
        model, tokenizer = FastLanguageModel.from_pretrained(
            model_name=base_model,
            max_seq_length=2048,
            load_in_4bit=True,
        )
        model = PeftModel.from_pretrained(model, adapter_path)
        model = model.merge_and_unload()
    except Exception as e:
        print(f"\n  ❌ Failed to load model with adapter: {e}")
        sys.exit(1)
    print("✓")

    # Export to GGUF
    print(f"Exporting to GGUF ({quant})...")
    try:
        model.save_pretrained_gguf(
            output_path,
            tokenizer,
            quantization_method=quant,
        )
    except Exception as e:
        print(f"\n  ❌ GGUF export failed: {e}")
        print("  Try: Q4_K_M (default), Q5_K_M, Q8_0, or F16")
        sys.exit(1)

    print(f"\n✅ GGUF model exported to: {output_path}")
    print(f"   To use with llama.cpp:")
    print(f"   cp {output_path} /opt/b00t/models/")
    print(f"   b00t hive activate inference-qwen36-27b-llamacpp")


def main():
    parser = argparse.ArgumentParser(description="Export LoRA to GGUF")
    parser.add_argument("--adapter", default="./fine-tune/output/lora-adapter", help="LoRA adapter path")
    parser.add_argument("--quant", default="Q4_K_M", help="Quantization method (Q4_K_M, Q5_K_M, Q8_0, F16)")
    parser.add_argument("--output", default="./fine-tune/output/b00t-aligned-qwen36-27b.gguf", help="Output path")
    args = parser.parse_args()

    export(args.adapter, args.quant, args.output)


if __name__ == "__main__":
    main()
