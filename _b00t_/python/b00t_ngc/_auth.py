"""Key resolution for NGC / NVIDIA API — single source of truth."""
from __future__ import annotations
import os
from pathlib import Path
from functools import lru_cache


_ENV_NAMES = ("NGC_API_KEY", "NVIDIA_API_KEY")
_DOT_ENV   = Path.home() / ".b00t" / ".env"


@lru_cache(maxsize=1)
def load_key() -> str:
    """Return the first non-empty NGC/NVIDIA API key found.

    Search order: process env → ~/.b00t/.env
    Raises RuntimeError if none found.
    """
    for name in _ENV_NAMES:
        if v := os.environ.get(name, "").strip():
            return v

    if _DOT_ENV.exists():
        for line in _DOT_ENV.read_text().splitlines():
            for name in _ENV_NAMES:
                if line.startswith(f"{name}="):
                    v = line.split("=", 1)[1].strip().strip("\"'")
                    if v:
                        return v

    raise RuntimeError(
        f"No NGC/NVIDIA API key found.\n"
        f"Add to ~/.b00t/.env:  NGC_API_KEY=<your-key>\n"
        f"Get a key: https://org.ngc.nvidia.com/setup/api-key"
    )
