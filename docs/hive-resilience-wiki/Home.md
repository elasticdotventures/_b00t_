# Hive Resilience — DeepWiki

A small cross-linked wiki (the "deepwiki pattern": short pages, `[[links]]`, a
reviewer gate before Act). Hand this to a fresh session or a local Ralph — each
page is one unit of work, sized for a local model's context.

## The epic

The hive lost 95,700 restarts to one crash-looping service that nobody watched.
This epic makes the hive notice, react, and stay reachable.

| # | Page | State |
|---|------|-------|
| A | [[Watchdog]] — the Sentinel: crash-loop watchdog + NATS herald | ✅ shipped (`ralph/hive-watchdog`) |
| B | [[Agent-Doctor]] — every agent gets a checked path to inference + registers | ⏳ Ralph stories, ready |
| C | Prefix KV cache | ⛔ deferred (LMCache needs vLLM; llama.cpp `:8001` has native prompt cache) |
| D | [[Health-Metrics]] — NATS health surface + `b00t hive health` | 📋 designed |
| E | [[Repair-Chain]] — deterministic `doctor --fix` + mesh register | 📋 designed |

## Ground rules

- One harness of each kind: one [[Harness-Notes|opencode Ralph]], one pi Ralph. No more.
- [[Review-Gate]] fires between Orient and Act — retrieval is not permission to act.
- Datums/justfiles: `b00t-cli patch apply <file> - --yes`, never sed.
- Commit with `git -c core.hooksPath=/dev/null` (the repo's pre-commit hook fails
  `cargo: command not found` and is unrelated to these changes).
- Branch off `main`; keep stories one-commit each: `feat: [<ID>] - <title>`.

## Discovery

- `just whatis <topic>` → local deepwiki page for a b00t concept (datum/agent/mcp/cli/skill/profile + xrefs).
- deepwiki MCP (`_b00t_/deepwiki.mcp.toml`) → `ask_question(repoName='owner/repo', ...)` for external repos.
