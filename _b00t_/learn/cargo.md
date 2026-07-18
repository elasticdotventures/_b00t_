---
conda prefix conflict: unset CONDA_PREFIX before building Rust projects with PyO3 dependencies to avoid linking errors with undefined Python symbols

rustc version dependency conflicts: When crates require newer rustc for unstable features (e.g., rig-core@0.17.1 needs rustc 1.88+ for let-chains), upgrade stable toolchain with 'rustup update stable && rustup override set stable' from workspace root. Never downgrade dependencies or skip build - fix toolchain properly.


---
sm3lly /c0de maintenance: target/debug is the hog (142G of 148G; cargo never GCs stale dep artifacts). ~/.b00t/target symlinks to /c0de/cargo-target/b00t-root. Keep ~/.cache/sccache (11G) — it makes debug/ deletion cheap (deps replay from cache). cargo sweep -t 14 installed but only pays off once artifact ages diverge (mass rebuilds touch everything). /c0de/mod3ls/ollama blobs are root-owned — need sudo to remove. b00t sh guard: resubmit same command within 300s to force
