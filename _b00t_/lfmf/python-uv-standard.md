# LFMF: uv Tool Management in b00t

**Topic**: python, uv, package-management, b00t

**Lesson Learned**: b00t ALWAYS uses `uv` for Python package management, never pipx/pip

## Problem

Initially implemented crawl4ai and LangChain documentation with `pipx` and `pip` commands, violating b00t's DRY principle and tool standardization.

## Solution

### Universal Python Package Management with uv

b00t uses `uv` exclusively for ALL Python package management:

```bash
# ❌ NEVER use pipx
pipx install crawl4ai[all]

# ✅ ALWAYS use uv tool
uv tool install crawl4ai[all]

# ❌ NEVER use pip
pip install langchain

# ✅ ALWAYS use uv
uv add langchain
```

### uv Commands Reference

**Global Tools (CLI applications):**
```bash
# Install global CLI tool
uv tool install crawl4ai[all]

# Upgrade global tool
uv tool upgrade crawl4ai

# Run tool directly
uv tool run crawl4ai https://example.com

# List installed tools
uv tool list

# Uninstall tool
uv tool uninstall crawl4ai
```

**Project Dependencies:**
```bash
# Install project dependencies
uv sync

# Add package to project
uv add langchain langchain-anthropic langgraph

# Add dev dependency
uv add --dev pytest pytest-asyncio

# Remove package
uv remove langchain

# Run command in project environment
uv run python script.py
uv run pytest
```

**Playwright Browser Installation:**
```bash
# ❌ NEVER run directly
playwright install chromium

# ✅ ALWAYS use uv tool run
uv tool run playwright install chromium
```

### Datum Patterns

**CLI Tool Datum:**
```toml
[b00t]
name = "crawl4ai"
type = "cli"

install = """
uv tool install crawl4ai[all]
"""

update = """
uv tool upgrade crawl4ai
"""
```

**Python Library Datum:**
```toml
[b00t]
name = "langchain"
type = "ai"

[b00t.usage]
install = "uv add langchain langchain-anthropic langgraph"
```

### Why uv Over pipx/pip

1. **Faster**: Rust-based, 10-100x faster than pip
2. **Unified**: One tool for everything (pip + pipx + venv + pyenv)
3. **Lockfile**: `uv.lock` ensures reproducible builds
4. **b00t Standard**: Consistency across all Python projects

### Migration Pattern

When updating documentation:

```diff
- pipx install <package>
+ uv tool install <package>

- pip install <package>
+ uv add <package>

- python -m <module>
+ uv run python -m <module>

- playwright install
+ uv tool run playwright install
```

## Benefits

1. **Consistency**: Single tool for all Python needs
2. **Speed**: Sub-second dependency resolution
3. **Reproducibility**: Lockfiles ensure same versions everywhere
4. **DRY**: Don't maintain multiple package managers
5. **Modern**: uv is the future of Python packaging

## Related

- b00t gospel: DRY principle (Don't Repeat Yourself)
- LFMF: Python package management patterns
- Documentation: `_b00t_/python.🐍/README.md`

## Date

2025-11-17

## Category

python, uv, package-management, standards
