#!/bin/bash
cd "$HOME/.b00t"
b00t hive run --dry-run "git push --force origin main" 2>&1
echo "EXIT: $?"
