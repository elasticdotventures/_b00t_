#!/usr/bin/env python3
"""
b00t sm0l-translator — semantic pipe for frontier→b00t CLI translation.

Pattern: Frontier model (not b00t-trained) sends NL task description →
         fine-tuned sm0l adapter translates to b00t CLI invocation.

Modes:
  --serve       OpenAI-compat API on port 8002 (default)
  --query "...""  One-shot translation from command line
  --smoke        Run smoke-test probes against adapter

VRAM: ~800MB (0.5B 4bit base + LoRA adapter + inference)
Runs concurrently with llama-server on same GPU.
"""

import argparse, json, os, sys
from pathlib import Path
import torch

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
ADAPTER = REPO_ROOT / "fine-tune" / "output-smol" / "lora-adapter"

SYSTEM_PROMPT = """You are a b00t CLI translator. Given a natural language task description, output the EXACT b00t CLI command(s) to accomplish it.

Available commands and their subcommands:
- b00t task add "<description>"  — create a new task
- b00t task list  — list all pending tasks
- b00t task next  — show next task to work on
- b00t task done <id>  — mark task complete
- b00t learn <topic>  — load a skill/knowledge datum
- b00t lfmf <tool> "<lesson>"  — memoize tribal knowledge
- b00t grok ask "<query>" --topic <topic>  — search knowledgebase
- b00t grok learn --source <url> --content "<content>"  — learn from source
- b00t grok digest --topic <topic> --content "<content>"  — digest content
- b00t soul get <key>  — read soul KV value
- b00t soul set <key> <value>  — write soul KV value
- b00t whoami  — agent identity and context
- b00t hive status  — RAM/GPU/CPU snapshot
- b00t hive list  — available .hive.toml profiles
- b00t hive activate=<profile>  — transition system state
- b00t checkpoint  — commit all + run tests
- b00t status  — tool availability status
- b00t mcp list  — list MCP servers
- just install <name>  — install a tool/service
- just compile-agent <role> <count> <output>  — compile sandbox agent
- just -l  — list available just recipes

Output format: Return ONLY the command(s), one per line. No explanations, no commentary, no markdown formatting, no backticks."""


def load_model():
    os.environ.setdefault("CUDA_VISIBLE_DEVICES", "0")
    os.environ.setdefault("HF_HOME", os.path.expanduser("~/.cache/huggingface"))

    from unsloth import FastLanguageModel

    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=str(ADAPTER),
        max_seq_length=1024,
        load_in_4bit=True,
        dtype=None,
    )
    FastLanguageModel.for_inference(model)

    if hasattr(torch.cuda, "set_per_process_memory_fraction"):
        torch.cuda.set_per_process_memory_fraction(0.04)

    return model, tokenizer


def translate(model, tokenizer, query: str, temperature: float = 0.2) -> str:
    msgs = [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": query},
    ]
    prompt = tokenizer.apply_chat_template(
        msgs, tokenize=False, add_generation_prompt=True
    )
    inputs = tokenizer(prompt, return_tensors="pt").to(model.device)
    with torch.no_grad():
        out = model.generate(
            **inputs,
            max_new_tokens=200,
            temperature=temperature,
            do_sample=True,
            pad_token_id=tokenizer.eos_token_id,
        )
    answer = tokenizer.decode(
        out[0][inputs["input_ids"].shape[1] :], skip_special_tokens=True
    ).strip()
    return answer


def smoke_test(model, tokenizer):
    probes = [
        ("task-add", "add a new task to track the login bug"),
        ("task-list", "show me all my pending tasks"),
        ("b00t-learn", "I need to understand how to use the grok command"),
        ("soul-get", "what is the current cake economy rate"),
        ("whoami", "what agent am I and what blessings do I have"),
    ]
    results = []
    for label, query in probes:
        cmd = translate(model, tokenizer, query)
        ok = any(
            kw in cmd.lower()
            for kw in ["b00t", "just"]
        )
        results.append((label, query, cmd, ok))
    return results


def serve(model, tokenizer, port: int = 8002):
    try:
        from flask import Flask, request, jsonify
    except ImportError:
        print("⚠️  flask not installed. Run: uv pip install flask")
        sys.exit(1)

    app = Flask(__name__)

    @app.route("/v1/chat/completions", methods=["POST"])
    def chat():
        data = request.get_json(force=True)
        messages = data.get("messages", [])
        temp = data.get("temperature", 0.2)

        user_msg = messages[-1]["content"] if messages else ""
        if messages and messages[0].get("role") == "system":
            sys_prompt = messages[0]["content"]
            if sys_prompt.strip() and sys_prompt.strip() != SYSTEM_PROMPT.strip():
                user_msg = f"SYSTEM: {sys_prompt}\n\nUSER: {user_msg}"

        cmd = translate(model, tokenizer, user_msg, temperature=temp)

        return jsonify({
            "id": "sm0l-translator",
            "object": "chat.completion",
            "model": "b00t-sm0l-translator",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": cmd},
                "finish_reason": "stop",
            }],
            "usage": {"completion_tokens": len(cmd.split())},
        })

    @app.route("/health", methods=["GET"])
    def health():
        return jsonify({"status": "ok", "model": "b00t-sm0l-translator"})

    print(f"sm0l-translator serving on :{port}")
    app.run(host="0.0.0.0", port=port, threaded=False)


def main():
    parser = argparse.ArgumentParser(description="b00t sm0l-translator")
    parser.add_argument("--serve", action="store_true", help="Start OpenAI-compat server")
    parser.add_argument("--port", type=int, default=8002, help="Server port (default: 8002)")
    parser.add_argument("--query", type=str, help="One-shot translation")
    parser.add_argument("--smoke", action="store_true", help="Run smoke-test probes")

    args = parser.parse_args()

    if not (args.serve or args.query or args.smoke):
        parser.print_help()
        sys.exit(1)

    print("loading sm0l adapter...", file=sys.stderr)
    model, tokenizer = load_model()
    print("ready.", file=sys.stderr)

    if args.smoke:
        results = smoke_test(model, tokenizer)
        passed = sum(1 for _, _, _, ok in results if ok)
        print(f"\nsmoke test: {passed}/{len(results)} b00t-aware probes\n")
        for label, query, cmd, ok in results:
            status = "PASS" if ok else "FAIL"
            print(f"  {status} [{label}]: {query}")
            print(f"         → {cmd}\n")
        return 0 if passed >= 3 else 1

    if args.query:
        cmd = translate(model, tokenizer, args.query)
        print(cmd)
        return 0

    if args.serve:
        serve(model, tokenizer, port=args.port)


if __name__ == "__main__":
    sys.exit(main())
