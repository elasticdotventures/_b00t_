# grit — git-next-gen Rust reimplementation

Source: https://grit-scm.com/ | https://github.com/gitbutlerapp/grit

## Install

```bash
b00t install grit
```

## Two binaries

- `grit` — drop-in git replacement (same interface; `alias git=grit` works)
- `gs` — grit shell: simplified git UX optimized for agent + human workflows

## gs key commands

```bash
gs status          # branch, ahead/behind, recent commits
gs shortlog        # commits on THIS branch not on main (changelog audit)
gs log             # paginated log with "→ more:" pointer
gs add [paths]     # stage (no path = stage everything)
gs commit -m "..."
gs push
```

## b00t idiom: pre-PR changelog sequencing

```bash
gs shortlog                             # audit what's on the branch
git diff --name-only HEAD~5 HEAD        # files changed in last 5 commits  
cog changelog                           # preview cocogitto changelog entry
```

## grit extensions over git

- Simplified UX for `gs status` (terse one-line branch context)
- `gs shortlog` = commits ahead of default branch (no args needed)
- Paginated `gs log` with continuation pointer
- Agents can parse `gs status` output for compact branch context

## Changelog sequencing pattern (b00t praxis)

1. `gs shortlog` — see what's on branch
2. Group uncommitted changes by capability area
3. Commit each group with `fix/feat/chore/docs(scope): message` (cocogitto)
4. `cog changelog` preview → PR body

## b00t:map v1

```
# summary: grit v0.4.7 + gs companion — git-next-gen Rust CLI for branch audit
# tags: git, grit, gs, changelog, branch, commit-organization
# tier: sm0l
# cmds: gs status, gs shortlog, grit log --oneline -10
# complexity: 2
```
