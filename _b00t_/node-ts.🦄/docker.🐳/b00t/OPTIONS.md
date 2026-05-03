# b00t Container Build Options

Choose the best approach for your needs.

## Summary

| Option | Speed | What You Get | When to Use |
|--------|-------|--------------|-------------|
| **A: Pull from GHCR** | 30 sec | b00t-cli only | Quick test, CLI only needed |
| **B: Build Unified** | 10-15 min | b00t-cli + b00t-mcp | **Recommended** - Full setup |
| **C: Build CLI Only** | 10-15 min | b00t-cli only | Legacy, not recommended |

## Option A: Pull Pre-built from GHCR (Fastest)

**What it does:** Pulls b00t-cli from GitHub Container Registry (built by CI/CD)

**Pros:**
- ⚡ Very fast (30 seconds)
- ✅ Pre-tested by CI/CD
- ✅ Same build as production

**Cons:**
- ❌ CLI only (no b00t-mcp)
- ❌ May not be latest local changes
- ❌ Requires network access

**Command:**
```bash
./docker.🐳/b00t/pull-from-ghcr.sh
```

**Use when:**
- You only need b00t-cli
- You want to test quickly
- You trust the CI/CD builds

## Option B: Build Unified Container (Recommended)

**What it does:** Builds both b00t-cli and b00t-mcp from g0spell source

**Pros:**
- ✅ Both binaries in one container
- ✅ b00t-cli, b00t-mcp, and b00t alias
- ✅ Build from local g0spell (latest changes)
- ✅ Full MCP server support
- ✅ Native aarch64 build

**Cons:**
- ⏱️ Slower (10-15 min first build)
- 💾 Uses more disk space during build

**Command:**
```bash
./docker.🐳/b00t/build-unified.sh
```

**Use when:**
- You need b00t-mcp MCP server
- You want Claude → b00t integration
- You have local g0spell changes
- You want complete setup

**Recommended for most users.**

## Option C: Build CLI Only (Legacy)

**What it does:** Builds only b00t-cli from g0spell

**Pros:**
- ✅ Native build from g0spell
- ✅ Mirrors GitHub Actions workflow

**Cons:**
- ❌ No b00t-mcp (can't use MCP server)
- ⏱️ Same build time as unified
- ❌ Less functionality than unified

**Commands:**
```bash
./docker.🐳/b00t/quick-build-aarch64.sh  # Fast version
./docker.🐳/b00t/build-from-g0spell.sh    # DRY version
```

**Use when:**
- You specifically don't want b00t-mcp
- You're testing CLI-only workflows

**Not recommended - use Option B instead.**

## Comparison Matrix

|  | GHCR Pull (A) | Unified Build (B) | CLI Only (C) |
|---|:---:|:---:|:---:|
| **Time** | 30s | 10-15m | 10-15m |
| **b00t-cli** | ✅ | ✅ | ✅ |
| **b00t-mcp** | ❌ | ✅ | ❌ |
| **b00t alias** | ❌ | ✅ | ✅ |
| **Local changes** | ❌ | ✅ | ✅ |
| **MCP support** | ❌ | ✅ | ❌ |
| **Network required** | ✅ | ❌ | ❌ |
| **Disk space** | Low | High | High |

## What Each Produces

### Option A: GHCR Pull
```
ghcr.io/elasticdotventures/b00t-cli:latest
  → b00t-cli:latest
  → b00t:latest
```

### Option B: Unified Build
```
b00t:latest (unified)
  ├── b00t-cli (CLI tool)
  ├── b00t-mcp (MCP server)
  └── b00t → b00t-cli (alias)

Also tagged as:
  - b00t:v0.7.0
  - b00t:aarch64
  - b00t-cli:latest
  - b00t-mcp:latest
```

### Option C: CLI Only
```
b00t-cli:latest
  ├── b00t-cli (CLI tool)
  └── b00t → b00t-cli (alias)

Also tagged as:
  - b00t-cli:v0.7.0
  - b00t-cli:aarch64
```

## Recommendation Decision Tree

```
Do you need b00t-mcp MCP server?
  ├─ Yes → Option B (Build Unified)
  └─ No
      ├─ Need latest g0spell changes?
      │   ├─ Yes → Option B or C (build locally)
      │   └─ No → Option A (Pull GHCR)
      └─ Want fastest setup?
          ├─ Yes → Option A (Pull GHCR)
          └─ No → Option B (Build Unified)
```

**TL;DR: Use Option B (Build Unified) for complete setup.**

## Next Steps After Building

1. **Verify build:**
   ```bash
   docker run --rm b00t:latest b00t-cli --version
   docker run --rm b00t:latest b00t-mcp --version  # Unified only
   ```

2. **Install MCP to Claude:**
   ```bash
   ./docker.🐳/b00t/install-mcp.sh  # Unified only
   ```

3. **Test integration:**
   ```bash
   source claude.🐳/env.sh
   claude
   /mcp  # Check for b00t server
   ```

## References

- **Unified Build:** `./build-unified.sh`
- **GHCR Pull:** `./pull-from-ghcr.sh`
- **MCP Installer:** `./install-mcp.sh`
- **Full Guide:** `./README.md`
- **Quick Start:** `../../QUICKSTART.md`
