#!/usr/bin/env python3
"""Quick smoke-test for the fine-tuned LoRA adapter."""
import sys
from pathlib import Path

ADAPTER = Path(__file__).parent / "output-smol/lora-adapter"
BASE_MODEL = "unsloth/Qwen2.5-0.5B-Instruct-bnb-4bit"

PROBES = [
    ("b00t basics",
     "What command do I run to orient my role and load blessings in b00t?"),
    ("b00t task",
     "How do I add a new task in the b00t task system?"),
    ("b00t grok",
     "What is the b00t grok command used for and what is the correct syntax?"),
    ("soul kv",
     "How do I read and write values to the b00t soul KV store?"),
    ("install mode",
     "What modes does just install support and how does it remember past choices?"),
]

def run_probe(model, tokenizer, question: str) -> str:
    messages = [{"role": "user", "content": question}]
    prompt = tokenizer.apply_chat_template(messages, tokenize=False, add_generation_prompt=True)
    inputs = tokenizer(prompt, return_tensors="pt").to(model.device)
    import torch
    with torch.no_grad():
        out = model.generate(
            **inputs,
            max_new_tokens=120,
            temperature=0.3,
            do_sample=True,
            pad_token_id=tokenizer.eos_token_id,
        )
    gen = out[0][inputs["input_ids"].shape[1]:]
    return tokenizer.decode(gen, skip_special_tokens=True).strip()

def main():
    from unsloth import FastLanguageModel
    print(f"Loading base model + LoRA adapter from {ADAPTER}")
    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=str(ADAPTER),
        max_seq_length=512,
        load_in_4bit=True,
    )
    FastLanguageModel.for_inference(model)
    print(f"Model loaded. Running {len(PROBES)} probes...\n{'─'*60}")

    passed = 0
    for label, question in PROBES:
        print(f"\n[{label}] {question}")
        answer = run_probe(model, tokenizer, question)
        print(f"→ {answer}")
        # basic signal: answer mentions b00t-cli / b00t / just
        ok = any(tok in answer.lower() for tok in ["b00t", "just", "soul", "mcp", "grok"])
        print("  PASS" if ok else "  FAIL (no b00t signal)")
        passed += ok

    print(f"\n{'─'*60}")
    print(f"Result: {passed}/{len(PROBES)} probes returned b00t-aware responses")
    sys.exit(0 if passed >= len(PROBES) // 2 else 1)

if __name__ == "__main__":
    main()
