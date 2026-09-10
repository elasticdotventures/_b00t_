# b00t-mcp-remote

SP4-08 reference backend image for the on-demand serverless MCP hosting plane
(`docs/superpowers/specs/2026-09-10-sp4-serverless-mcp-hosting-design.md`).

It is a thin wrapper around `b00t-mcp --http`:

| aspect | value |
|---|---|
| listen | `0.0.0.0:8080` — `/mcp` (streamable-http) + `/health` |
| auth | `B00T_MCP_REQUIRE_AUTH=1` — the SP4-07 identity layer verifies the forwarded SP1 JWT against `B00T_IDENTITY_URL/.well-known/jwks.json` |
| health | `GET /health` → `200 ok`, unauthenticated (placement backends poll it) |
| user | non-root (`uid 10001`) |

## Build & push

Context is the **repo root** (the whole workspace compiles `b00t-mcp`):

```sh
podman build -t ghcr.io/promptexecution/b00t-mcp-remote:latest \
  -f containers/b00t-mcp-remote/Containerfile .
podman push ghcr.io/promptexecution/b00t-mcp-remote:latest
```

Production `McpServer` datums **must** pin the digest
(`ghcr.io/promptexecution/b00t-mcp-remote@sha256:…`) — see
`McpServerSpec::is_digest_pinned`.

## Run locally

```sh
podman run --rm -p 8080:8080 \
  -e B00T_MCP_REQUIRE_AUTH=1 \
  -e B00T_IDENTITY_URL=https://b00t.promptexecution.com \
  ghcr.io/promptexecution/b00t-mcp-remote:latest

curl -fsS http://127.0.0.1:8080/health          # -> ok
curl -i http://127.0.0.1:8080/mcp               # -> 401 (no bearer)
```

## As a declared service

`_b00t_/b00t.mcp_server.toml` (the proxy pointing at itself — the first real
`<svc>`, useful for e2e):

```toml
[b00t]
name = "b00t"
type = "mcp_server"

[b00t.mcp_server]
image = "ghcr.io/promptexecution/b00t-mcp-remote@sha256:…"
port = 8080
idle_timeout_seconds = 300

[b00t.mcp_server.placement]
backend = "aca"     # or "podman" for dev
```

The connect URL is **derived** (`https://b00t.b00t.promptexecution.com/mcp`),
never stored. `b00t mcp serve` wakes it on the first
`GET /_b00t/route/b00t`; the idle reaper scales it back to zero after
`idle_timeout_seconds`.
