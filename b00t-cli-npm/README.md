# b00t-cli

Universal package manager / hive agent CLI (Rust). See the [full
implementation](https://github.com/elasticdotventures/dotfiles/tree/main/b00t-cli)
for documentation.

## Install

```bash
npm install -g b00t-cli
```

This currently installs via `cargo install` — **requires a Rust toolchain**
(get one at https://rustup.rs). There is no prebuilt-binary distribution
yet, so first install compiles the workspace and can take 10-30+ minutes.

## Usage

```bash
b00t-cli --help
b00t whoami
```

## License

MIT
