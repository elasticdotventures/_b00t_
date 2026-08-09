# Windows-Keystore Credential Proxy — Design

**Date:** 2026-08-09
**Status:** Approved for implementation planning
**Tracking:** b00t task #168 (parent mission); this spec covers sub-project 1 of 4
**Memory:** `project_telnyx_fax_service_ledgrrr.md`

## Background

This is the first of four sub-projects decomposed out of a larger operator
directive: build a bi-directional fax service inside ledgrrr-desktop
(`crates/ledgerr-tauri` in the vendored `ledgrrr` repo,
`~/.dotfiles/vendor/ledgrrr`), backed by Telnyx, fronted by a "wrangler"
middleware layer that b00t describes as a stateless MCP-proxy over
containerized services. The four sub-projects are:

1. **Windows-keystore credential proxy** (this spec) — how secrets (starting
   with `TELNYX_API_KEY`) get from the operator's Windows Credential Manager
   into podman-managed services without ever living in plaintext on disk.
2. Outbound fax MVP (ledgrrr → wrangler → Telnyx), bi-directional-capable
   architecture, P0 test loop is self-directed (send a fax to ourselves).
3. `wrangler`-cli-managed self-hosting of `cloudflare-os` (Cloudflare's
   Workers-based agent-workspace app) via podman, with `ledgerr-mcp` and
   `b00t-mcp` as sidecars.
4. MCP sidecar wiring between wrangler, ledgrrr, and b00t.

Sub-projects 2–4 are deferred; each gets its own spec. This document
addresses **only** the credential proxy — it is a dependency of the other
three, which is why it was chosen to design first.

## Problem

Telnyx (and future) API credentials should live in the operator's Windows
Credential Manager, not as plaintext files or env vars checked into any repo
or baked into any container image. Services that need those credentials
(wrangler, ledgrrr sidecars) run as podman-managed containers inside WSL2.
Something has to bridge Windows Credential Manager → WSL2-side containers,
without ever persisting the secret outside the Windows keystore and the
minimum-lifetime copy the consuming container needs.

## Architecture

**`credproxy`: a bare Windows service, not a container.** A native
`windows-rs`-based Rust binary calls Win32 Credential Manager APIs
(`Windows::Win32::Security::Credentials` — `CredRead`/`CredEnumerate`)
directly. It runs as a Windows service and exposes a narrow `localhost`-only
HTTP API shaped like a Cloudflare Wrangler secrets provider:

```
GET /secret/{name}  →  {"value": "..."}
```

gated by a local-only bearer token in a file readable only by the operator's
Windows user account.

This cannot become a container under either candidate runtime: `podman` (via
its WSL2/Hyper-V VM) and `wslc` (Microsoft's built-in WSL container CLI, via
its own dedicated Hyper-V VM) both run **Linux** containers exclusively.
Neither gives a container access to real Win32 APIs — that requires an
actual Windows process, full stop. (Native Windows Containers — process- or
Hyper-V-isolated, Windows Server Core–based images — are a third, distinct
technology that neither podman nor wslc implement, and which Podman Desktop
cannot manage either; see References.) This constraint holds regardless of
which runtime fronts the consumer side, so it does not factor into that
choice.

**Consumer container runtimes.** ledgrrr's own containers run under `wslc`
(the operator's preference, on its own merits — Docker-compatible CLI, no
separate runtime install, native WSL integration — not because it grants any
Win32 access). wrangler and other hive sidecars continue on `podman`, per
the hive's existing packaging convention. Both are plain HTTP clients of
`credproxy`; the choice between them doesn't change how `credproxy` is
reached.

**Reachability.** A WSL2-side caller tries `127.0.0.1` first (works if the
operator has opted into `.wslconfig`'s `networkingMode=mirrored`, Windows 11
22H2+), and falls back to a gateway-IP lookup (`ip route show | grep default
| awk '{print $3}'`), which works today under WSL2's default NAT mode with
no configuration changes. This fallback logic is verified for a standard
WSL2 Linux distro. **It is not yet verified for `wslc`'s containers**, which
run inside their own dedicated Hyper-V VM (`Session`/`WslcService` API) that
may or may not share the same host-networking path — Microsoft's docs don't
say. See Open Risks.

**b00t-cli's role.** b00t-cli holds no Win32 logic and no secret state. It
describes `credproxy` as a datum (service description, health-check command,
capability tags) and exposes one MCP tool, `b00t_secret_get(name)`, that
performs the reachability fallback above and proxies the HTTP call — a
stateless MCP-proxy in front of an external capability, the same shape b00t
already uses elsewhere in the hive.

**Delivery to containers.** Not live per-call. At container bootstrap, a
`just` recipe (or b00t-cli setup step) calls `b00t_secret_get` once per
required secret and seeds a `podman secret create` (for podman-run
consumers) or the `wslc` equivalent (for wslc-run consumers, e.g. `ledgrrr`)
— the hive's existing preference for podman-managed secrets over raw env
vars, matched to whichever runtime each consumer uses. Containers read the
secret normally for their lifetime; rotation means re-running bootstrap plus
a container restart.

**Error handling.** If `credproxy` is unreachable (service not running,
firewall blocking, unexpected networking mode), `b00t_secret_get` fails
loudly with a diagnostic naming the specific cause it detected — e.g.
"127.0.0.1:PORT refused, gateway 172.x.x.x:PORT refused — is credproxy
running?" — never a silent empty secret. This matches the hive's existing
no-silent-drop convention (see `feedback_datum_postel_tolerance` memory).

**Testing.** `credproxy`'s Win32 logic requires a real Windows test target
(CI: `windows-latest` runner, or manual verification on the operator's
machine) — it cannot be exercised from this Linux session or from CI running
on Linux. The b00t-cli `b00t_secret_get` MCP-proxy side is tested against a
mock HTTP server standing in for `credproxy`, independent of any real
Windows process, so that logic gets normal Linux-CI coverage.

## Open Risks

1. **`wslc` container → Windows host reachability is unverified.** Neither
   the WSL container tutorial nor the WSL container API reference document
   whether a `wslc`-run container's dedicated Hyper-V VM shares the gateway-
   IP/mirrored-localhost path documented for standard WSL2 distros.
   **First implementation task must be an empirical spike**: run a `wslc`
   container, attempt to reach a service on the Windows host via both
   `127.0.0.1` and the standard-WSL2 gateway-IP lookup, and record which (if
   either) works before writing any reachability-dependent code against it.
   If neither works, `credproxy` may need to publish itself differently for
   `wslc` consumers specifically (e.g., via `wslc run -p` port-forwarding
   configured from the Windows side, reversing the usual direction).

2. **Grok/Irontology backend is down** (`localhost:6969` connection
   refused, confirmed 2026-08-09) — unrelated to this design, but means any
   future semantic search over this spec's assimilated content
   (`wsl-containers` and `mcp` datum topics) won't surface until that
   separate, already-tracked mission (`project_irontology.md`) is resolved.
   The git-blob + datum-TOML storage this design's research was assimilated
   into is unaffected — only live query is degraded.

## Non-Goals (this spec)

- The fax send/receive flow itself (sub-project 2).
- Self-hosting `cloudflare-os` (sub-project 3).
- MCP sidecar wiring for wrangler (sub-project 4).
- Secrets other than `TELNYX_API_KEY` (the design generalizes to any named
  secret via `/secret/{name}`, but only Telnyx's key is in scope for the
  first working version).

## References

- [WSL container | Microsoft Learn](https://learn.microsoft.com/en-us/windows/wsl/wsl-container) — `wslc.exe`, Linux-only, dedicated Hyper-V VM, WSL ≥2.9.3 pre-release
- [Get started with containers on WSL](https://github.com/MicrosoftDocs/wsl/blob/main/WSL/tutorials/wsl-containers.md)
- [Accessing network applications with WSL](https://learn.microsoft.com/en-us/windows/wsl/networking) — NAT gateway-IP vs mirrored-mode `127.0.0.1`
- [podman-container-tools/podman#27842](https://github.com/podman-container-tools/podman/issues/27842) — native Windows Containers support in Podman, open/unresolved
- [microsoft/Windows-Containers](https://github.com/microsoft/Windows-Containers) — community hub for the actual (process/Hyper-V isolated) Windows Containers technology, distinct from both podman and wslc
- [microsoft/windows-container-tools](https://github.com/microsoft/windows-container-tools) — `LogMonitor` only; not a runtime, not related to wslc despite the name similarity
