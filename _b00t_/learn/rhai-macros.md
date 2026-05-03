---
Rhai pipe composition: The Rhai |> operator enables functional chaining of guard macros: cmd | is_docker() | is_run(). Each function returns bool, pipe chains them left-to-right. Define functions in rhai_macros section of hive-guards.hive.toml. This is preferable to deeply nested cmd.contains() when composing 3+ conditions.
