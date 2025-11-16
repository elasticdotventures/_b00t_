---
conda prefix conflict: unset CONDA_PREFIX before building Rust projects with PyO3 dependencies to avoid linking errors with undefined Python symbols

rustc version dependency conflicts: When crates require newer rustc for unstable features (e.g., rig-core@0.17.1 needs rustc 1.88+ for let-chains), upgrade stable toolchain with 'rustup update stable && rustup override set stable' from workspace root. Never downgrade dependencies or skip build - fix toolchain properly.

