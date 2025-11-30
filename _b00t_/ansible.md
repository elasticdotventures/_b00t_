## Ansible

- Canonical install: `uvx ansible --version` (b00t installs a wrapper so `ansible` shells out to `uvx` and will install uv/uvx if missing).  
- Fallbacks: pkgx wrapper, then distro package managers (apt/dnf/brew) if uvx/pkgx are unavailable.  
- Verify: `ansible --version` or `UVX_TOOL_UPGRADE=1 uvx ansible --version` to refresh the cached tool.  
- Playbooks: stored under `ansible/playbooks`; use `b00t ansible run --datum <name>` to run datum-backed playbooks with inventory/vars applied.
