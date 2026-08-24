# Build Acceleration
- sccache 0.16.0 — ~7-10x faster cargo builds
- cranelift (nightly) — ~2x faster debug builds via `CARGO_CODEGEN_BACKEND=cranelift`
- Self-upgrade check in `b00t up` via rhai script
- evidence/ untracked from git
- runpod.rs compile fix
- version check now includes workspace version
- str_trim→str_strip (was shadowing built-in .trim())
