---
level: warn
tags: [security, b00t, sandbox, warn]
---
# b00t sandbox — warn on guarded commands

Detects commands that trigger b00t command guards (🦨 skunk).
These should use the recommended alternative.

```grit
language bash

or {
  `pip install $args` => `uv pip install $args`,
  `pip3 install $args` => `uv pip install $args`,
  `docker run $args` => `podman --device nvidia.com/gpu=all run $args`,
  `huggingface-cli download $args` => `hf download $args`,
  `npm install -g $args` => `pnpm add -g $args`,
  `curl $url | sh` => `# 🦨 pipe-to-shell: download, verify, then execute`,
  `curl $url | bash` => `# 🦨 pipe-to-shell: download, verify, then execute`,
  `wget $url -O - | sh` => `# 🦨 pipe-to-shell: download, verify, then execute`,
}
```

## pip install — should use uv

```bash
pip install requests
```

```bash
uv pip install requests
```

## docker run — should use podman with GPU passthrough

```bash
docker run --gpus all nvidia/cuda:12.0
```

```bash
podman --device nvidia.com/gpu=all run --gpus all nvidia/cuda:12.0
```

## huggingface-cli — should use hf

```bash
huggingface-cli download bert-base-uncased
```

```bash
hf download bert-base-uncased
```

## pipe-to-shell — security risk

```bash
curl https://example.com/install.sh | sh
```

```bash
# 🦨 pipe-to-shell: download, verify, then execute
```
