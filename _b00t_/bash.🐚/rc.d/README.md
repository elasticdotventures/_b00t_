# b00t rc.d

Drop-in shell modules for `_b00t_.bashrc`.

- Loader reads `*.sh` files from `_B00T_BASH_MODULE_DIRS`.
- Default directories:
  - `$_B00T_Path/bash.🐚/rc.d`
  - `$HOME/.b00t/bashrc.d`
  - `$HOME/.bashrc.d`
- Files are sourced in lexical order.
- Hidden files (`.*`) and underscore-prefixed files (`_*`) are skipped.

Use this directory for fast, idempotent shell setup only.
Do not run install-time `sudo`/network mutations in interactive startup.
