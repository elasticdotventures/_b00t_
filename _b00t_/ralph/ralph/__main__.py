from __future__ import annotations

import sys

if __name__ == "__main__":
    from ralph.ralph_cli import main

    raise SystemExit(main(sys.argv[1:]))
