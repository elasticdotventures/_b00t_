# b00t-grok PyO3/Maturin Build Guide

## 🤓 Critical Build Requirements

### Environment Conflicts Resolution
The **#1 issue** with PyO3/Maturin builds is environment conflicts between `uv` and `conda`:

```bash
# ❌ FAILS: "Both VIRTUAL_ENV and CONDA_PREFIX are set"
uv run maturin develop

# ✅ WORKS: Unset conda first
unset CONDA_PREFIX && uv run maturin develop
```

### Build Commands (Memoized)

```bash
# 🦀 Quick build (from project root)
just grok-build

# 🚀 Build + run development server
just grok-dev

# 🧹 Clean build artifacts
just grok-clean
```

### Manual Build Steps

If `just` commands fail, use these manual steps:

```bash
# 1. Navigate to Python project
cd b00t-grok-py

# 2. 🤓 CRITICAL: Resolve environment conflicts
unset CONDA_PREFIX

# 3. Build with Maturin
uv run maturin develop

# 4. Test the build
uv run python -c "import b00t_grok; print('✅ Import successful')"
```

## Environment Setup

### Required Components
- **uv**: Python package manager (`curl -LsSf https://astral.sh/uv/install.sh | sh`)
- **Rust**: Latest stable (`rustup update`)
- **Maturin**: Python wheel builder (auto-installed by uv)
- **Python 3.12**: Used by uv virtual environment

### Project Structure
```
/home/brianh/.dotfiles/
├── b00t-grok/                   # Rust crate with PyO3 bindings
│   ├── Cargo.toml              # PyO3 dependencies & features
│   └── src/lib.rs              # Rust code with #[pyfunction] exports
└── b00t-grok-py/               # Python project
    ├── pyproject.toml          # Maturin configuration
    ├── .venv/                  # uv virtual environment (Python 3.12)
    └── python/                 # Python wrapper code
```

## Troubleshooting

### Common Errors & Solutions

| Error | Cause | Solution |
|-------|-------|----------|
| `Both VIRTUAL_ENV and CONDA_PREFIX are set` | Environment conflict | `unset CONDA_PREFIX` |
| `undefined reference to _Py_Dealloc` | Missing Python shared library | Use Maturin instead of direct cargo |
| `No such file or directory: python-config` | uv doesn't include python-config | Use system Python or Maturin |
| `Failed to execute 'patchelf'` | Missing rpath tool | `pip install maturin[patchelf]` (optional) |

### Verification Steps

```bash
# 1. Check Python version
uv run python --version
# Expected: Python 3.12.8

# 2. Verify uv environment
echo $VIRTUAL_ENV
# Expected: /home/brianh/.dotfiles/b00t-grok-py/.venv

# 3. Check conda conflicts
echo $CONDA_PREFIX
# Expected: (empty or unset)

# 4. Test import
uv run python -c "import b00t_grok; print('✅ Success')"
```

## Integration with b00t-cli

The b00t-grok Rust library is consumed by:
1. **b00t-cli** grok commands (`b00t-cli grok ask/learn/digest`)
2. **b00t-mcp** MCP server tools
3. **b00t-grok-py** FastAPI/MCP server (this project)

### Architecture Flow
```
b00t-cli (Rust) → b00t-grok (Rust) ← PyO3 bindings ← b00t-grok-py (Python/FastAPI)
```

## Best Practices

1. **Always use `just grok-build`** - handles environment setup automatically
2. **Never manually set PyO3 environment variables** - let Maturin handle it
3. **Use uv for Python dependencies** - maintains consistent environment
4. **Unset CONDA_PREFIX** - prevents environment conflicts
5. **Test immediately after build** - catch linking issues early

## Future Considerations

- **Container builds**: Consider Docker for consistent environments
- **CI/CD**: GitHub Actions should use `unset CONDA_PREFIX` step
- **Windows support**: May need different environment handling
- **Multiple Python versions**: Currently locked to 3.12 via uv

---

🤓 **Remember**: The `unset CONDA_PREFIX` step is **CRITICAL** and must be included in all build scripts and documentation.