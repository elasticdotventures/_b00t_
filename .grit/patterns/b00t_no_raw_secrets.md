---
level: error
tags: [security, b00t, overlay, secrets]
---
# b00t overlay — no raw secrets in source

Detects hardcoded API keys, tokens, and passwords in source code.
Overlay datums MUST reference secrets via env vars or keyring — never literals.

```grit
language bash

`export $key="$value"` where {
  $key <: or {
    `API_KEY`, `api_key`,
    `SECRET`, `secret`,
    `TOKEN`, `token`,
    `PASSWORD`, `password`, `PASSWD`, `passwd`,
    `PRIVATE_KEY`, `private_key`,
    `ACCESS_KEY`, `access_key`,
    `AWS_SECRET_ACCESS_KEY`,
    `OPENAI_API_KEY`,
    `ANTHROPIC_API_KEY`,
  },
  $value <: regex("[A-Za-z0-9+/=_-]{12,}"),
  $value <: not regex("^\\$\\{?"),
}
```

## hardcoded API key

```bash
export OPENAI_API_KEY="sk-proj-abc123XYZdefinitelyakey456"
```

## hardcoded password

```bash
export PASSWORD="supersecret12345678"
```

## env var reference — OK (negative test)

```bash
export OPENAI_API_KEY="${OPENAI_API_KEY}"
```

```bash
export OPENAI_API_KEY="${OPENAI_API_KEY}"
```
