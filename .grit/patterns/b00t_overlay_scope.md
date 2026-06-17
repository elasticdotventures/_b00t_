---
level: warn
tags: [b00t, overlay, validation]
---
# b00t overlay — validate enclave commit scope

Detects overlay datum files (`.overlay.toml`) referenced in non-enclave
source code. Overlay files should only be committed to the enclave branch
(`b00t/node/<host>/overlay`), never to origin/main.

This pattern flags Rust source files that hardcode `.overlay.toml` filenames
outside of the project module — enforcing that overlay awareness is scoped
to the enclave infrastructure.

```grit
language rust

`$str` where {
  $str <: regex("\\.overlay\\.toml"),
  $str <: not within `mod project`,
  $str <: not within `fn write_config`,
}
```

## hardcoded overlay path in wrong module — flag

```rust
let path = "models.overlay.toml";
```

## overlay reference in project module — OK (negative test)

```rust
mod project {
    let f = "models.overlay.toml";
}
```

```rust
mod project {
    let f = "models.overlay.toml";
}
```
