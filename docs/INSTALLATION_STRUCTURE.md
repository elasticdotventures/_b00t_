# b00t Installation Structure

## Installation Layout

### Standard Installation (via install.sh or GitHub releases)

```
~/.b00t/                         # B00T_HOME (main installation)
├── b00t-cli                     # Main CLI binary
├── b00t-mcp                     # MCP server binary
├── b00t -> b00t-cli             # Convenience symlink
└── _b00t_/                      # Datums directory (8.6 MB, 137 files)
    ├── learn/                   # Gospel skills
    │   ├── bash.md
    │   ├── rust.md
    │   ├── just.md
    │   └── ...
    ├── *.mcp.toml               # MCP server datums
    ├── *.toml                   # Tool datums
    ├── b00t.just                # Core justfile recipes
    └── ...

~/.local/bin/                    # INSTALL_DIR (in PATH)
├── b00t-cli -> ~/.b00t/b00t-cli # Symlink to binary
├── b00t-mcp -> ~/.b00t/b00t-mcp # Symlink to binary
└── b00t -> ~/.b00t/b00t         # Symlink to symlink

~/.config/b00t/                  # CONFIG_DIR (user config)
└── config.toml                  # User configuration
```

### pkgx Installation

```
~/.pkgx/                                    # pkgx managed packages
└── pkgx.sh/v*/                            # Version-specific install
    ├── bin/
    │   ├── b00t-cli
    │   ├── b00t-mcp
    │   └── b00t -> b00t-cli
    ├── share/b00t/_b00t_/                 # Datums location
    │   ├── learn/
    │   ├── *.mcp.toml
    │   └── ...
    └── etc/profile.d/b00t.sh              # Auto-sets _B00T_Path
```

## Environment Variables

### _B00T_Path
**Purpose**: Tells b00t-cli where to find datums

**Standard installation**:
```bash
export _B00T_Path="$HOME/.b00t/_b00t_"
```

**pkgx installation**:
```bash
# Auto-set by pkgx via etc/profile.d/b00t.sh
export _B00T_Path="~/.pkgx/pkgx.sh/v{version}/share/b00t/_b00t_"
```

**Custom location**:
```bash
# For development or custom setups
export _B00T_Path="/custom/path/_b00t_"
```

### PATH
Added by installer to include `~/.local/bin` for binary access

## Installation Methods Comparison

| Method | Binary Location | Datums Location | _B00T_Path | Size |
|--------|----------------|-----------------|------------|------|
| **install.sh** | ~/.b00t/ | ~/.b00t/_b00t_/ | ~/.b00t/_b00t_ | ~60 MB |
| **pkgx** | ~/.pkgx/.../bin/ | ~/.pkgx/.../share/b00t/_b00t_/ | Auto-set | ~60 MB |
| **cargo install** | ~/.cargo/bin/ | Manual setup | Manual | Binary only |
| **docker** | Container | Container | N/A | ~500 MB |

## What Gets Installed

### Binaries (~50 MB combined)
- `b00t-cli` - Main CLI tool
- `b00t-mcp` - MCP server
- `b00t` - Symlink to b00t-cli

### Datums (~8.6 MB, 137 files)
Essential for functionality:

**Gospel Skills** (`learn/`):
- `bash.md`, `rust.md`, `just.md`, `cargo.md`, `git.md`
- Platform-specific knowledge
- Best practices and patterns

**MCP Datums** (`*.mcp.toml`):
- Pre-configured MCP servers
- Connection details, dependencies
- Examples: `filesystem.mcp.toml`, `github.mcp.toml`

**Tool Datums** (`*.toml`):
- Tool configurations
- Version detection patterns
- Installation recipes

**Core Resources**:
- `b00t.just` - Justfile recipes
- Scripts and utilities
- Documentation templates

## Migration & Compatibility

### From Legacy ~/.dotfiles/_b00t_

If you have an existing installation at `~/.dotfiles/_b00t_/`:

```bash
# Option 1: Symlink for compatibility
ln -s ~/.dotfiles/_b00t_ ~/.b00t/_b00t_

# Option 2: Move to new location
mv ~/.dotfiles/_b00t_ ~/.b00t/_b00t_

# Option 3: Set environment variable
export _B00T_Path="$HOME/.dotfiles/_b00t_"
```

### Custom Datum Location

```bash
# Point to custom location
export _B00T_Path="/path/to/your/_b00t_"

# Or use --path flag
b00t-cli --path /path/to/_b00t_ mcp list
```

## Verification

After installation, verify everything is working:

```bash
# Check binaries are accessible
which b00t-cli
which b00t-mcp
which b00t

# Check environment
echo $_B00T_Path
ls -l $_B00T_Path

# Test functionality
b00t-cli whoami
b00t-cli mcp list
b00t-cli learn bash
```

## Uninstallation

### Standard installation
```bash
# Remove binaries and datums
rm -rf ~/.b00t
rm ~/.local/bin/b00t-cli ~/.local/bin/b00t-mcp ~/.local/bin/b00t

# Remove shell configuration
# Edit ~/.bashrc or ~/.zshrc and remove:
# export PATH="$HOME/.local/bin:$PATH"
# export _B00T_Path="$HOME/.b00t/_b00t_"

# Remove config (optional)
rm -rf ~/.config/b00t
```

### pkgx installation
```bash
pkgx -b00t-cli  # Removes package
```

## Troubleshooting

### b00t-cli can't find datums

**Symptoms**:
- `b00t-cli mcp list` shows no servers
- `b00t-cli learn bash` fails
- `whoami` shows no gospel

**Solution**:
```bash
# Check _B00T_Path is set
echo $_B00T_Path

# If not set, add to shell config:
echo 'export _B00T_Path="$HOME/.b00t/_b00t_"' >> ~/.bashrc
source ~/.bashrc

# Verify datums exist
ls -l $HOME/.b00t/_b00t_/
```

### Permission errors

```bash
# Fix ownership
chown -R $USER:$USER ~/.b00t

# Fix permissions
chmod -R u+rw ~/.b00t
chmod +x ~/.b00t/b00t-cli ~/.b00t/b00t-mcp
```

### Symlinks broken

```bash
# Recreate symlinks
cd ~/.local/bin
ln -sf ~/.b00t/b00t-cli b00t-cli
ln -sf ~/.b00t/b00t-mcp b00t-mcp
ln -sf ~/.b00t/b00t b00t
```

## Development Setup

For development, keep source and datums separate:

```bash
# Clone repository
git clone https://github.com/elasticdotventures/dotfiles.git ~/dev/b00t
cd ~/dev/b00t

# Build binaries
cargo build --release

# Point to development datums
export _B00T_Path="$HOME/dev/b00t/_b00t_"

# Test
./target/release/b00t-cli whoami
```

## Notes

- **Datums are essential**: b00t-cli is non-functional without them
- **_B00T_Path must be set**: Either via environment variable or --path flag
- **Symlinks enable flexibility**: Binaries can be anywhere, datums separate
- **pkgx handles environment**: Auto-configures _B00T_Path via profile.d
- **Standard location**: `~/.b00t/` is the new recommended default
