# git core.fileMode on WSL/NTFS-backed checkouts

WSL checkouts on an NTFS-backed mount (`/mnt/c/...`, or any Windows-side volume) leak
spurious executable-bit flips: files round-trip between Windows and WSL as 644→755 (or
back) with zero content change, and `git status`/`git diff` show every touched file as
"modified" even though nothing in the file actually changed.

**Fix, once per clone:**
```bash
git config core.fileMode false
```

Confirmed live 2026-09-05 reviewing `PromptExecution/xero-mcp-server-b00t#1`: a PR's
own body had to explain away "~130 modified files" as pure exec-bit noise before the
real 114-line diff could be reviewed — `core.fileMode false` avoids the noise at the
source. Per-repo, not global — set it in every fresh clone made from a WSL/NTFS mount,
same class of gotcha as this repo's own worktree-discipline note (`b00t learn worktree`).

<!-- b00t:map v1
summary: git core.fileMode false — WSL/NTFS exec-bit leak fix
tags: git, wsl, ntfs, filemode, hygiene
tier: sm0l
cmds: git config core.fileMode false
complexity: 1
-->
