---
sccache + incremental compilation conflict on rustc 1.96. Symptom: 'sccache: incremental compilation is prohibited'. Fix: comment out rustc-wrapper=sccache in ~/.cargo/config.toml. Workaround: CARGO_INCREMENTAL=0 RUSTC_WRAPPER=''

---
b00t build plane (plan gleaming-jingling-nygaard, Phase 6). The remote build
box (`dev-env/b00t-build.dev-environment.yaml`) sets, via the dev-env `env:`:
  RUSTC_WRAPPER=sccache
  SCCACHE_DIR=/mnt/cache/sccache        # a LOCAL-DIR cache on the object mount
  SCCACHE_CACHE_SIZE=40G
  CARGO_INCREMENTAL=0                   # required — sccache rejects incremental
/mnt/cache is GCS-backed (dev-env/cache-up.sh), so the sccache dir survives an
idle-stop the same way target/ does. `just remote-build`/`remote-test` run
`sccache --start-server` before cargo. This is COMPLEMENTARY to CARGO_TARGET_DIR
(see worktree lesson) — never point SCCACHE_DIR at CARGO_TARGET_DIR. Confirm
with `sccache --show-stats` before/after two warm runs; hit-rate should climb.
Local adoption on the workstation stays opt-in (scripts/install-rustc-wrappers.sh);
`session.start.sh` is untouched.
