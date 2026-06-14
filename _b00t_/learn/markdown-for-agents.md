# Markdown for Agents — Content Negotiation Protocol

## What It Is
Cloudflare's [Markdown for Agents](https://blog.cloudflare.com/markdown-for-agents/) (Feb 2026):
send `Accept: text/markdown` — enabled sites auto-convert HTML → markdown at the edge.
**80% token reduction** (16,180 → 3,150 tokens for a typical blog post).

## How to Use

```bash
# curl — any Cloudflare-proxied site with feature enabled
curl https://developers.cloudflare.com/fundamentals/reference/markdown-for-agents/ \
  -H "Accept: text/markdown"

# Response headers include:
# content-type: text/markdown; charset=utf-8
# x-markdown-tokens: 725          ← estimated token count
# content-signal: ai-train=yes, search=yes, ai-input=yes
```

## In b00t learn

`b00t learn <topic> --digest <URL>` auto-negotiates markdown when given a URL:

```bash
# fetch + digest to RAG — markdown negotiated automatically
b00t learn stripe --digest https://docs.stripe.com/api

# query after digest
b00t learn stripe --ask "how do idempotency keys work"
```

## In Rust (reqwest)

```rust
let resp = client.get(url)
    .header("Accept", "text/markdown, text/html;q=0.9, */*;q=0.8")
    .send().await?;

// check if server honoured it
let is_markdown = resp.headers()
    .get("content-type")
    .and_then(|v| v.to_str().ok())
    .map(|ct| ct.contains("text/markdown"))
    .unwrap_or(false);

// token hint
let tokens = resp.headers().get("x-markdown-tokens")
    .and_then(|v| v.to_str().ok());
```

## Content Signals Header
`content-signal: ai-train=yes, search=yes, ai-input=yes` — publisher expresses consent.
Parse this before ingesting content into RAG to respect publisher preferences.

## Sites Supporting It (2026)
- `developers.cloudflare.com` ✅
- `blog.cloudflare.com` ✅
- Claude Code + OpenCode already send `Accept: text/markdown` on all requests

## Try It
```bash
curl https://blog.cloudflare.com/markdown-for-agents/ -H "Accept: text/markdown" | head -20
```

# b00t:map v1
# summary: Markdown-for-Agents — CF content negotiation, 80% token reduction, Accept: text/markdown header protocol
# tags: markdown, http, agents, cloudflare, content-negotiation, rag, learn, token-efficiency
# tier: sm0l
# cmds: b00t learn <topic> --digest <url>, curl -H "Accept: text/markdown" <url>
# complexity: 2
