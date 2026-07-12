#!/usr/bin/env python3
"""Register b00t reviewer capabilities in opencode-b00t agent-metadata.json.
Idempotent — safe to re-run. Reads from the canonical capability registry.
Run via: just reviewer install-opencode"""

import json
import os
import subprocess
import sys

# Resolve repo root via git — reliable regardless of CWD
repo_root = subprocess.check_output(
    ["git", "rev-parse", "--show-toplevel"],
    text=True
).strip()

META = os.path.join(repo_root, "vendor", "opencode-b00t", ".opencode", "agent-metadata.json")

if not os.path.exists(META):
    print("⚠️  opencode-b00t not initialized — run: git submodule update --init vendor/opencode-b00t")
    sys.exit(1)

with open(META) as f:
    data = json.load(f)

agents = {
    "b00t-reviewer": {
        "id": "b00t-reviewer", "name": "B00tReviewer",
        "category": "subagents/code", "type": "subagent",
        "version": "1.0.0", "author": "b00t",
        "tags": ["reviewer", "b00t", "multi-framework", "mece", "triz", "eureka", "governance"],
        "dependencies": [
            "subagent:mece-analyzer", "subagent:triz-analyzer",
            "subagent:eureka-analyzer", "subagent:synthesis-agent",
            "skill:b00t-integration"
        ]
    },
    "mece-analyzer": {
        "id": "mece-analyzer", "name": "MeceAnalyzer",
        "category": "subagents/code", "type": "subagent",
        "version": "1.0.0", "author": "b00t",
        "tags": ["analysis", "mece", "decomposition"],
        "dependencies": ["mcp:codebase-memory"]
    },
    "triz-analyzer": {
        "id": "triz-analyzer", "name": "TrizAnalyzer",
        "category": "subagents/code", "type": "subagent",
        "version": "1.0.0", "author": "b00t",
        "tags": ["analysis", "triz", "contradiction"],
        "dependencies": ["mcp:codebase-memory", "mcp:context7"]
    },
    "eureka-analyzer": {
        "id": "eureka-analyzer", "name": "EurekaAnalyzer",
        "category": "subagents/code", "type": "subagent",
        "version": "1.0.0", "author": "b00t",
        "tags": ["analysis", "eureka", "insight"],
        "dependencies": ["mcp:codebase-memory"]
    },
    "synthesis-agent": {
        "id": "synthesis-agent", "name": "SynthesisAgent",
        "category": "subagents/code", "type": "subagent",
        "version": "1.0.0", "author": "b00t",
        "tags": ["synthesis", "triangulation"],
        "dependencies": ["mcp:b00t-mcp", "mcp:github"]
    }
}

added = 0
for key, val in agents.items():
    if key not in data["agents"]:
        data["agents"][key] = val
        added += 1

opencoder_deps = data["agents"].get("opencoder", {}).get("dependencies", [])
if "subagent:b00t-reviewer" not in opencoder_deps:
    opencoder_deps.append("subagent:b00t-reviewer")
    added += 1

if added > 0:
    with open(META, "w") as f:
        json.dump(data, f, indent=2)
        f.write("\n")

print(f"✅ {added} capability entries registered in opencode-b00t")
