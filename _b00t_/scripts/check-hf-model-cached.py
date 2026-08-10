#!/usr/bin/env python3
"""Verify a HuggingFace model repo is cached locally before starting an inference service."""
import sys
from huggingface_hub import try_to_load_from_cache

repo_id, filename = sys.argv[1], sys.argv[2]
if not try_to_load_from_cache(repo_id, filename):
    print(f"model not found in cache — run: hf download {repo_id}", file=sys.stderr)
    sys.exit(1)
