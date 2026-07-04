# NGC — NVIDIA GPU Cloud CLI + model API recipes
# 🧠 Load: b00t learn ngc
# Datum: _b00t_/datums/ngc.cli.tomllm
#
# Prereqs:
#   1. NGC account: https://org.ngc.nvidia.com/setup/api-key
#   2. Add to ~/.b00t/.env:  NGC_API_KEY=<key>  NVIDIA_API_KEY=<key>
#   3. Install NGC CLI: just ngc::install
#
# Container naming: nvcr.io/nvidia/<image>:<yy.mm>-py3
# Model API base:   https://integrate.api.nvidia.com/v1  (OpenAI-compat)

set working-directory := ".."

# Install NGC CLI via pip (uvx managed, no global install)
install:
    uv tool install ngccli
    ngc config set
    @echo "✅ NGC CLI ready — run: ngc --version"

# Show NGC auth + org info
auth-check:
    #!/usr/bin/env python3
    import os, urllib.request, urllib.error, json, sys
    def load_key():
        for src in [os.environ, {}]:
            v = src.get("NGC_API_KEY", "") or src.get("NVIDIA_API_KEY", "")
            if v: return v
        env_path = os.path.expanduser("~/.b00t/.env")
        if not os.path.exists(env_path): return ""
        for line in open(env_path):
            if line.startswith("NGC_API_KEY=") or line.startswith("NVIDIA_API_KEY="):
                return line.split("=", 1)[1].strip().strip("\"'")
        return ""
    key = load_key()
    if not key:
        print("❌ NGC_API_KEY not set — add to ~/.b00t/.env"); sys.exit(1)
    req = urllib.request.Request(
        "https://authn.nvidia.com/token?service=ngc&scope=group/ngc:*",
        headers={"Authorization": f"ApiKey {key}"}
    )
    try:
        with urllib.request.urlopen(req, timeout=8) as r:
            d = json.load(r)
            org = d.get("user", {}).get("defaultOrgName", "?")
            print(f"✅ NGC auth OK — org: {org}")
    except urllib.error.HTTPError as e:
        print(f"❌ NGC auth FAIL: {e.code}"); sys.exit(1)

# List recent NGC PyTorch training containers
container-list:
    #!/usr/bin/env python3
    import os, urllib.request, urllib.error, json, sys
    env_path = os.path.expanduser("~/.b00t/.env")
    key = os.environ.get("NGC_API_KEY", "")
    if not key and os.path.exists(env_path):
        for line in open(env_path):
            if line.startswith("NGC_API_KEY="):
                key = line.split("=", 1)[1].strip().strip("\"'")
                break
    if not key:
        print("NGC_API_KEY not set"); sys.exit(1)
    url = "https://api.ngc.nvidia.com/v2/orgs/nvidia/containers/pytorch/tags?page_size=15&sort_by=name"
    req = urllib.request.Request(url, headers={"Authorization": f"ApiKey {key}"})
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            d = json.load(r)
            tags = sorted(d.get("tags") or [], key=lambda x: x.get("name", ""), reverse=True)
            print("=== Recent NGC PyTorch containers ===")
            for t in tags[:12]:
                name = t.get("name", "?")
                print(f"  nvcr.io/nvidia/pytorch:{name}")
    except urllib.error.HTTPError as e:
        print(f"API error {e.code}"); sys.exit(1)

# Pull a specific NGC container to local podman cache
container-pull image="pytorch:24.12-py3":
    #!/usr/bin/env bash
    KEY=$(grep -m1 '^NGC_API_KEY=' ~/.b00t/.env 2>/dev/null | cut -d= -f2- | tr -d "\"'")
    podman login nvcr.io --username '$oauthtoken' --password "$KEY"
    podman pull "nvcr.io/nvidia/{{image}}"

# List available free models on integrate.api.nvidia.com
model-list:
    #!/usr/bin/env python3
    import os, urllib.request, urllib.error, json, sys
    env_path = os.path.expanduser("~/.b00t/.env")
    key = os.environ.get("NVIDIA_API_KEY", "") or os.environ.get("NGC_API_KEY", "")
    if not key and os.path.exists(env_path):
        for line in open(env_path):
            if line.startswith("NVIDIA_API_KEY=") or line.startswith("NGC_API_KEY="):
                key = line.split("=", 1)[1].strip().strip("\"'")
                break
    if not key:
        print("❌ NVIDIA_API_KEY not set — add to ~/.b00t/.env"); sys.exit(1)
    req = urllib.request.Request(
        "https://integrate.api.nvidia.com/v1/models",
        headers={"Authorization": f"Bearer {key}"}
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            d = json.load(r)
            models = d.get("data") or []
            print(f"=== {len(models)} models on NVIDIA API ===")
            for m in models[:25]:
                print(f"  {m.get('id', '?')}")
    except urllib.error.HTTPError as e:
        print(f"❌ {e.code}"); sys.exit(1)

# Quick prompt test against NVIDIA model API (OpenAI-compat)
model-smoke model="nvidia/llama-3.1-nemotron-70b-instruct" prompt="What is b00t?":
    #!/usr/bin/env python3
    import os, json, urllib.request, urllib.error, sys
    env_path = os.path.expanduser("~/.b00t/.env")
    key = os.environ.get("NVIDIA_API_KEY", "") or os.environ.get("NGC_API_KEY", "")
    if not key and os.path.exists(env_path):
        for line in open(env_path):
            if line.startswith("NVIDIA_API_KEY=") or line.startswith("NGC_API_KEY="):
                key = line.split("=", 1)[1].strip().strip("\"'")
                break
    if not key:
        print("❌ NVIDIA_API_KEY not set"); sys.exit(1)
    payload = json.dumps({
        "model": "{{model}}",
        "messages": [{"role": "user", "content": "{{prompt}}"}],
        "max_tokens": 256,
        "temperature": 0.2,
    }).encode()
    req = urllib.request.Request(
        "https://integrate.api.nvidia.com/v1/chat/completions",
        data=payload,
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as r:
            d = json.load(r)
            print(d["choices"][0]["message"]["content"])
    except urllib.error.HTTPError as e:
        body = e.read().decode()
        print(f"❌ {e.code}: {body[:200]}"); sys.exit(1)

# Show recommended NGC image for RunPod training (no supervisord)
training-image:
    @echo "nvcr.io/nvidia/pytorch:24.12-py3"
    @echo "  PyTorch 2.5 | CUDA 12.6 | Python 3.10 | standard bash entrypoint"
    @echo "  docker_args CMD works — no supervisord override"
    @echo "  Pull: just ngc::container-pull pytorch:24.12-py3"
