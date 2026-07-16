---
sccache + incremental compilation conflict on rustc 1.96. Symptom: 'sccache: incremental compilation is prohibited'. Fix: comment out rustc-wrapper=sccache in ~/.cargo/config.toml. Workaround: CARGO_INCREMENTAL=0 RUSTC_WRAPPER=''
