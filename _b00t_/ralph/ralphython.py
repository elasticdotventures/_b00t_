#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = [
#   "fastmcp>=3.0.0b1",
# ]
# ///

import sys

if __name__ == "__main__":
    from ralph.entrypoint import main

    raise SystemExit(main(sys.argv[1:]))
