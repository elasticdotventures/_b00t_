# Review-Gate — the deepwiki pattern's teeth

`_b00t_/learn/deepwiki.md`: **deepwiki = autoresearch = karpathy**. An agent
recursively reads source material *before* acting, and a **sm0l reviewer model
reads the retrieved doc and answers `RELEVANT | SKIP:<reason>`**. The gate fires
between **Orient** and **Act** in OODA. Without it, retrieval is keyword-match
and the agent acts on wrong context.

## How to apply it here

Before a Ralph commits a story:

1. Retrieve: the one wiki page for that story + the exact files it names.
2. Gate: `b00t-cli grok ask "does <page> actually describe what I changed in
   <files>? answer RELEVANT or SKIP:<reason>"` — or any sm0l model.
   `SKIP` → do not commit; write the reason to `progress.txt` and re-orient.
3. Act: run the story's verify block; commit only on `RELEVANT` + green verify.

## For external repos

Use the deepwiki MCP (`_b00t_/deepwiki.mcp.toml`):
`ask_question(repoName='owner/repo', question=…)` is the retrieval step; still
run it past the sm0l reviewer before acting on the answer.

## Handoff checklist

- [ ] `git -c core.hooksPath=/dev/null` for commits (pre-commit hook is broken)
- [ ] one commit per story, `feat: [<ID>] - <title>`
- [ ] `progress.txt` appended with learnings each iteration
- [ ] one opencode Ralph + one pi Ralph, no more ([[Harness-Notes]])
- [ ] reviewer gate ran for every commit

Back to [[Home]].
