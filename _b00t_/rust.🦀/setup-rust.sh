#!/bin/bash

    if ! command -v rust-analyzer > /dev/null; then
        echo "Rust Analyzer not found. Installing..."
        cargo install rust-analyzer --locked
    else
        echo "Rust Analyzer is already installed."
    fi

