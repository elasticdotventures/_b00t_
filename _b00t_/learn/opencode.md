# opencode

## Current b00t status

- `b00t install opencode` SHOULD install the upstream **CLI** binary.
- Linux `.deb` assets in upstream releases are for **OpenCode Desktop**, not the CLI harness.
- If the binary installer path is unsuitable, use the alternate datum: `b00t install opencode-npm`.
- The datum does **not** point at a vendored `vendor/...` source tree; it points at upstream OpenCode references and install endpoints.

## Install commands

```bash
b00t install opencode
b00t install opencode-npm
```

## Asset split

- CLI:
  - `curl -fsSL https://opencode.ai/install | bash`
  - release assets like `opencode-linux-x64.tar.gz`
- Desktop:
  - release assets like `opencode-desktop-linux-amd64.deb`
  - install with `sudo apt install ./opencode-desktop-linux-amd64.deb`

## Notes

- Upstream GitHub moved from `sst/opencode` to `anomalyco/opencode`.
- After install, verify `opencode --version` resolves on `PATH`.
