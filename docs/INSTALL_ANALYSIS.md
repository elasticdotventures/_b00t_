# b00t Installation & Upgrade Mechanism Analysis

## Current State

### What README.md Promises
```bash
curl -fsSL https://raw.githubusercontent.com/elasticdotventures/dotfiles/main/install.sh | sh
```

### What install.sh Expects
- GitHub releases at `https://github.com/elasticdotventures/dotfiles/releases`
- Binary assets: `b00t-${PLATFORM}.tar.gz` format
  - `b00t-x86_64-unknown-linux-gnu.tar.gz`
  - `b00t-aarch64-unknown-linux-gnu.tar.gz`
  - `b00t-x86_64-apple-darwin.tar.gz`
  - etc.
- Fallback to Docker container if binary unavailable

### What Actually Exists
- ❌ **No GitHub releases with binary assets**
- ✅ Docker images at `ghcr.io/elasticdotventures/b00t-cli`
- ✅ NPM packages: `@b00t/cli`, `@b00t/mcp`
- ⚠️ `.github/workflows/release.yml` only runs `cog bump --auto` (no binary builds)

### Current Installation Methods That Work
1. **From source** (requires Rust toolchain):
   ```bash
   git clone https://github.com/elasticdotventures/dotfiles.git ~/.dotfiles
   cd ~/.dotfiles && cargo install --path b00t-cli
   ```

2. **Docker container**:
   ```bash
   docker pull ghcr.io/elasticdotventures/b00t-cli:latest
   ```

3. **NPM** (if published):
   ```bash
   npm install -g @b00t/cli
   ```

## Gap Analysis

### Missing Components
1. **GitHub Release Workflow**
   - Need cross-platform binary builds (Linux x86_64/aarch64/armv7, macOS x86_64/aarch64)
   - Need tar.gz packaging
   - Need upload to GitHub releases
   - Need triggered on git tags

2. **Binary Artifacts**
   - No pre-built binaries in releases
   - Users can't use curl install without Rust toolchain

3. **Version Synchronization**
   - Cargo.toml version (0.7.1)
   - Git tags (v0.7.1)
   - Release assets need to match

## Proposed Solution: Multi-Tier Installation Strategy

### Tier 1: pkgx (Recommended - Minimal)
**Why pkgx:**
- 4 MiB standalone binary
- Zero install pollution (`~/.pkgx` only)
- Cross-platform (Linux, macOS, Windows/WSL)
- Automatic dependency management
- Can run without prior installation

**User experience:**
```bash
# One-time pkgx install
curl -Ssf https://pkgx.sh | sh

# Use b00t (auto-downloads on first use)
pkgx b00t-cli --version
pkgx b00t-cli learn rust

# Or install permanently
pkgx +b00t-cli
b00t-cli --version
```

**What we need:**
1. Create `package.yml` for pkgx pantry
2. Submit PR to `pkgxdev/pantry`
3. Update README.md with pkgx instructions

### Tier 2: GitHub Releases (Cross-platform Binaries)
**User experience:**
```bash
# Universal installer (current promise)
curl -fsSL https://raw.githubusercontent.com/elasticdotventures/dotfiles/main/install.sh | sh
```

**What we need:**
1. GitHub Actions workflow for cross-compilation
2. Build matrix: Linux (x86_64, aarch64, armv7), macOS (x86_64, aarch64)
3. Package binaries as tar.gz
4. Upload to GitHub releases on git tag

### Tier 3: Cargo Install (Source Build)
**Status:** ✅ Already works
```bash
cargo install --git https://github.com/elasticdotventures/dotfiles b00t-cli
```

### Tier 4: NPM Package (Node.js ecosystem)
**Status:** ⚠️ Package exists but may need binary bundling
```bash
npm install -g @b00t/cli
```

### Tier 5: Container (Docker/Podman)
**Status:** ✅ Already works
```bash
docker run --rm -it ghcr.io/elasticdotventures/b00t-cli:latest
```

## Recommended Implementation Order

### Phase 1: Fix GitHub Releases (Immediate - enables install.sh)
- [ ] Create `.github/workflows/build-release.yml`
- [ ] Use `cargo-zigbuild` or cross-compilation for multi-platform
- [ ] Trigger on git tags (v*)
- [ ] Upload tar.gz assets to release

### Phase 2: pkgx Integration (Minimal Install)
- [ ] Create `package.yml` based on deno/cargo examples
- [ ] Test local build with `pkgx bk build`
- [ ] Submit to pkgxdev/pantry
- [ ] Update README.md

### Phase 3: Package Manager Enhancements
- [ ] Homebrew tap (macOS/Linux)
- [ ] Scoop bucket (Windows)
- [ ] APT/DEB repository (Ubuntu/Debian)

## pkgx package.yml Structure (Draft)

Based on deno.land and cargo examples:

```yaml
distributable:
  url: https://github.com/elasticdotventures/dotfiles/releases/download/v{{version}}/b00t-{{version}}-src.tar.gz
  strip-components: 1

versions:
  github: elasticdotventures/dotfiles/tags
  strip: /^v/

provides:
  - bin/b00t-cli
  - bin/b00t-mcp

dependencies:
  rust-lang.org: ^1.85  # Minimum for workspace features

build:
  dependencies:
    rust-lang.org/cargo: '*'
  script: |
    cargo install --path b00t-cli --root {{prefix}}
    cargo install --path b00t-mcp --root {{prefix}}

    # Create convenience symlink
    ln -sf {{prefix}}/bin/b00t-cli {{prefix}}/bin/b00t

test:
  script: |
    b00t-cli --version | grep {{version}}
    b00t-cli status --help
    b00t-mcp --help
```

## Upgrade Mechanism

### Current
```bash
# Requires Rust toolchain
cargo install --path b00t-cli --force
```

### Proposed (pkgx)
```bash
pkgx --sync  # Updates all pkgx packages
```

### Proposed (install.sh)
```bash
# Re-run installer
curl -fsSL https://raw.githubusercontent.com/elasticdotventures/dotfiles/main/install.sh | sh
```

### Proposed (b00t self-update)
Add to b00t-cli:
```bash
b00t upgrade  # Detects install method and upgrades
```

## Next Steps

1. **Immediate**: Create GitHub release workflow with binary builds
2. **Short-term**: Draft and test pkgx package.yml
3. **Medium-term**: Submit to pkgx pantry
4. **Long-term**: Add `b00t upgrade` self-update command

## References
- pkgx pantry: https://github.com/pkgxdev/pantry
- pkgx docs: https://docs.pkgx.sh/
- cargo-zigbuild: https://github.com/rust-cross/cargo-zigbuild
- cross: https://github.com/cross-rs/cross
