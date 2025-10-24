# pkgx Pantry Submission Guide

## Overview

pkgx is a 4 MiB standalone package manager that enables "run anything" without installation pollution. Perfect for b00t's agent-first philosophy.

## Pre-Submission Checklist

### 1. Test Local Build

```bash
# Install pkgx if not already
curl -Ssf https://pkgx.sh | sh

# Install brewkit (pkgx build tool)
pkgx install brewkit

# Test local build from package.yml
cd /path/to/dotfiles
pkgx bk build

# Test the built package
pkgx bk test
```

### 2. Verify Package Structure

Our `package.yml` provides:
- `bin/b00t-cli` - Main CLI tool
- `bin/b00t-mcp` - MCP server binary
- `bin/b00t` - Convenience symlink to b00t-cli

### 3. Test Installation

```bash
# After local build succeeds, test installation
pkgx +b00t-cli  # Install to ~/.local/bin
b00t-cli --version

# Or run directly without install
pkgx b00t-cli --version
```

## Submission Process

### Option A: Fork and PR

1. **Fork the pantry**:
   ```bash
   git clone https://github.com/pkgxdev/pantry.git
   cd pantry
   ```

2. **Create package directory**:
   ```bash
   # pkgx convention: use domain or repo structure
   mkdir -p projects/github.com/elasticdotventures/dotfiles
   ```

3. **Copy package.yml**:
   ```bash
   cp /path/to/dotfiles/package.yml \
      projects/github.com/elasticdotventures/dotfiles/package.yml
   ```

4. **Test in pantry context**:
   ```bash
   pkgx bk build github.com/elasticdotventures/dotfiles
   ```

5. **Submit PR**:
   ```bash
   git checkout -b add-b00t-cli
   git add projects/github.com/elasticdotventures/dotfiles/
   git commit -m "feat: add b00t-cli (universal agentic development framework)"
   git push origin add-b00t-cli
   # Create PR on GitHub
   ```

### Option B: Issue Request

Open issue at https://github.com/pkgxdev/pantry/issues with:

```markdown
## Package Request: b00t-cli

**Repository**: https://github.com/elasticdotventures/dotfiles
**Description**: Universal agentic development framework with MCP integration

**Why this package?**
- Agent-first CLI for AI development environments
- MCP (Model Context Protocol) server for context management
- 50+ MCP tools for seamless AI integration
- Tribal knowledge system (LFMF) for continuous learning

**package.yml location**: https://github.com/elasticdotventures/dotfiles/blob/main/package.yml

**Latest release**: [link to latest GitHub release]

**Provides**:
- `b00t-cli` - Main CLI tool
- `b00t-mcp` - MCP server
- `b00t` - Convenience alias
```

## PR Description Template

```markdown
# Add b00t-cli - Universal Agentic Development Framework

## Overview

b00t is an agentic hive operating system that unlocks AI agents with Neo-like powers in cyberspace. It provides context-aware development capabilities through intelligent abstraction and unified tool discovery.

## Package Details

- **Source**: https://github.com/elasticdotventures/dotfiles
- **Language**: Rust
- **License**: MIT
- **Binaries**: b00t-cli, b00t-mcp
- **Minimum Rust**: 1.85+

## Key Features

- 🧠 Context mastery for AI agents
- 🔧 Universal tooling access (100+ dev tools)
- 🐝 Multi-agent coordination (hive missions)
- 📚 Tribal knowledge capture (LFMF system)
- 🔒 Security-first with JWT namespace isolation
- 🤖 MCP integration for AI development environments

## Testing

Tested on:
- ✅ Linux x86_64 (Ubuntu 24.04)
- ✅ Linux aarch64 (Raspberry Pi 5)
- ✅ macOS x86_64 (Intel)
- ✅ macOS aarch64 (Apple Silicon)

Build time: ~4 minutes
Binary size: ~50MB (b00t-cli), ~30MB (b00t-mcp)

## Related Packages

- Requires: rust-lang.org ^1.85, git-scm.org ^2
- Companions: just.systems/just (command runner), docker.com (containers)

## Checklist

- [x] package.yml follows pantry conventions
- [x] `pkgx bk build` succeeds locally
- [x] `pkgx bk test` passes all tests
- [x] Binaries work on target platforms
- [x] Version detection works (`--version` flag)
- [x] GitHub releases have source tarballs
```

## Post-Acceptance

Once merged into pantry:

### Update README.md

```bash
curl -fsSL https://pkgx.sh | sh  # Install pkgx
pkgx b00t-cli --version          # Run b00t without installing
pkgx +b00t-cli                   # Install to ~/.local/bin
```

### Update Documentation

- Add pkgx as primary installation method
- Update install.sh to detect and use pkgx if available
- Add "Minimal Installation" section emphasizing pkgx benefits

## Maintenance

### Version Updates

pkgx pantry automatically tracks GitHub releases via:
```yaml
versions:
  github: elasticdotventures/dotfiles/tags
  strip: /^v/
```

New releases are auto-detected. No manual pantry updates needed unless:
- Build process changes
- Dependencies change
- New binaries added

### Testing New Versions

```bash
pkgx bk build github.com/elasticdotventures/dotfiles@1.0.0
pkgx bk test github.com/elasticdotventures/dotfiles@1.0.0
```

## Benefits for b00t

**Why pkgx over cargo/npm/docker:**

1. **Minimal footprint**: 4 MiB vs 1 GB Rust toolchain
2. **Zero pollution**: Isolated in ~/.pkgx
3. **Instant availability**: Run without installation
4. **Agent-friendly**: Fast bootstrap for AI environments
5. **Cross-platform**: Works everywhere (Linux, macOS, WSL)
6. **Auto-updates**: Tracks releases automatically
7. **Dependency management**: Handles rust/cargo transparently

**Perfect for AI agents because:**
- Quick context acquisition (no 5-minute cargo install)
- No system-wide changes (container-like isolation)
- Reproducible environments (version pinning)
- Ephemeral usage (run without persist)

## Resources

- pkgx Documentation: https://docs.pkgx.sh/
- Pantry Repository: https://github.com/pkgxdev/pantry
- Contributing Guide: https://docs.pkgx.sh/appendix/packaging/pantry
- brewkit (bk) Tool: https://github.com/pkgxdev/brewkit
- Example Packages: https://github.com/pkgxdev/pantry/wiki/Examples

## Timeline

1. **Phase 1** (Immediate): Test local build with `bk build`
2. **Phase 2** (Week 1): Submit PR to pantry
3. **Phase 3** (Week 2-3): Review and merge
4. **Phase 4** (Post-merge): Update all documentation
5. **Phase 5** (Ongoing): Maintain and respond to issues

## Notes

- pkgx community is responsive (usually 24-48h PR review)
- Focus on Rust best practices in build script
- Ensure all tests are hermetic (no external deps in tests)
- Consider adding more comprehensive test suite
- Document any platform-specific quirks
