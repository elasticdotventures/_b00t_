# NTFS Reserved Characters — Cross-Platform Compatibility Policy

## Overview

Filenames containing certain characters are legal on Linux ext4 but **fatal** on Windows NTFS.
This document tracks the policy and remediation for cross-platform repo compatibility.

## Reserved Characters on NTFS

The following characters **cannot** appear in filenames on Windows NTFS:

| Character | Hex | Description |
|----------|-----|------------|
| `:` | 0x3A | Drive separator (used for D:) |
| `|` | 0x7C | Pipe (command pipeline) |
| `?` | 0x3F | Wildcard single char |
| `*` | 0x2A | Wildcard multi-char |
| `<` | 0x3C | Redirection input |
| `>` | 0x3E | Redirection output |
| `"` | 0x22 | Quote delimiter |

## Known Offenders in This Repository

```bash
# Run to scan:
git ls-tree -r HEAD --name-only | grep -E '[:|?*<>""]'
```

### Current (2 offenders)

- `b00t-c0re-lib/src/mcp_registry.rs:494:13`  ← **COLON**
- `b00t-lib-chat/src/security.rs:10:5`     ← **COLON**

> 🤓 These are git notes refs, not actual files — checkout fails on Windows.

## Policy

### Pre-flight Path Scan

Before any Windows-local materialization of a foreign repo:

```bash
# Scan for reserved chars
git ls-tree -r HEAD --name-only | grep -E '[:|?*<>""]' && {
  echo "BLOCKED: NTFS-invalid paths detected"
  exit 1
}
```

### Pivot to Remote Inspection

Use raw GitHub content URLs instead of clone:

```bash
# Fetch single file via raw URL
curl -sL "https://raw.githubusercontent.com/elasticdotventures/_b00t_/main/path/to/file.toml"
```

### Git Config Workaround (Optional)

```bash
# Disable git notes refs (prevents the :494:13 refs)
git config core.checkRebase false
# Or globally disable:
git config --global transfer.fsckobjects true
```

## History

- **2026-05-03**: Policy created after 3 failed clone attempts on Windows pwsh
- **Root cause**: Git notes refs stored as `mcp_registry.rs:494:13` format
- **User**: Will add lint/datum to enforce cross-platform validity in future releases