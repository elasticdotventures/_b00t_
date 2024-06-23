#!/bin/bash

git config --global user.email "brianh@elastic.ventures"
git config --global user.name "Brian H"


curl -sS https://starship.rs/install.sh | sh

echo eval "$(starship init bash)" >> ~/.bashrc


# 🦀 rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# tree but ignores .git (useful for chatgpt dumps)
cargo install itree
cargo install ripgrep
alias itree='rg --files | tree --fromfile'


# just is a command runner
curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | sudo bash -s -- --to /usr/local/bin

