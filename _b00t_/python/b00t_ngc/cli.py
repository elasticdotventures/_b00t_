"""b00t-ngc CLI — thin argparse shell over NvidiaClient."""
from __future__ import annotations
import argparse
import sys
import urllib.error

from .client import NvidiaClient


def _client() -> NvidiaClient:
    try:
        return NvidiaClient()
    except RuntimeError as e:
        print(f"❌ {e}", file=sys.stderr)
        sys.exit(1)


def cmd_auth(_args) -> None:
    c = _client()
    try:
        org = c.whoami()
        print(f"✅ NGC auth OK — org: {org}")
    except urllib.error.HTTPError as e:
        print(f"❌ auth failed: {e.code}", file=sys.stderr)
        sys.exit(1)


def cmd_containers(args) -> None:
    c = _client()
    tags = c.containers(image=args.image, n=args.n)
    for t in tags:
        print(t.image)


def cmd_models(args) -> None:
    c = _client()
    models = c.models()
    if args.filter:
        models = [m for m in models if args.filter.lower() in m.id.lower()]
    print(f"=== {len(models)} model(s) ===")
    for m in models:
        print(f"  {m.id}")


def cmd_chat(args) -> None:
    c = _client()
    prompt = " ".join(args.prompt)
    try:
        if args.stream:
            for token in c.stream_chat(prompt, model=args.model, max_tokens=args.max_tokens):
                print(token, end="", flush=True)
            print()
        else:
            reply = c.chat(
                prompt,
                model=args.model,
                system=args.system,
                max_tokens=args.max_tokens,
                temperature=args.temperature,
            )
            print(reply)
    except urllib.error.HTTPError as e:
        body = e.read().decode()
        print(f"❌ {e.code}: {body[:300]}", file=sys.stderr)
        sys.exit(1)


def main() -> None:
    p = argparse.ArgumentParser(prog="b00t-ngc", description="NGC + NVIDIA model API client")
    sub = p.add_subparsers(dest="cmd", required=True)

    sub.add_parser("auth", help="Verify NGC API key and show org")

    pc = sub.add_parser("containers", help="List NGC container tags")
    pc.add_argument("--image", default="pytorch", help="NGC image name (default: pytorch)")
    pc.add_argument("-n", type=int, default=12, help="Max results")

    pm = sub.add_parser("models", help="List available NVIDIA API models")
    pm.add_argument("--filter", default="", help="Filter by substring")

    pch = sub.add_parser("chat", help="Chat with a NVIDIA-hosted model")
    pch.add_argument("prompt", nargs="+", help="Prompt text")
    pch.add_argument("--model", default="nvidia/llama-3.1-nemotron-70b-instruct")
    pch.add_argument("--system", default="", help="System prompt")
    pch.add_argument("--max-tokens", type=int, default=512, dest="max_tokens")
    pch.add_argument("--temperature", type=float, default=0.2)
    pch.add_argument("--stream", action="store_true")

    args = p.parse_args()
    {"auth": cmd_auth, "containers": cmd_containers, "models": cmd_models, "chat": cmd_chat}[args.cmd](args)


if __name__ == "__main__":
    main()
