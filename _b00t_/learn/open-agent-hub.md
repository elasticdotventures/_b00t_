# open-agent-hub

## Current b00t status

- `b00t install open-agent-hub` SHOULD install a local Git checkout plus CLI symlinks.
- Upstream is **not** published on the npm registry, so `npm install -g open-agent-hub` is not the right default path.
- The upstream CLI is a zero-dependency Node script with aliases: `open-agent`, `open-agent-hub`, `oah`, and `ahub`.

## Install model

```bash
b00t install open-agent-hub
```

What b00t does:

- clones `https://github.com/guanyang/open-agent-hub.git` into `~/.local/share/open-agent-hub`
- symlinks `open-agent`, `open-agent-hub`, `oah`, and `ahub` into `~/.local/bin`
- avoids npm global-prefix ambiguity and keeps the upstream content tree available for inspection and `oah sync`

## Key commands

```bash
oah list
oah status
oah enable --target=opencode
oah enable --global --target=all
oah sync
```

## Supported targets

- Claude Code
- Gemini CLI
- Codex
- Cursor
- Trae
- OpenCode
- Kiro
- Antigravity

## Notes

- Default target is `claude`; use `--target=opencode` when linking into `.opencode/` or `~/.config/opencode/`.
- The repo stores reusable capability content under `skills/`, `agents/`, and `commands/`.
- `oah sync` pulls updated skills from configured upstream sources in `skills_sources.json`.
