#!/bin/bash
cd /home/brianh/.b00t
b00t hive run --dry-run "git push --force origin main" 2>&1
echo "EXIT: $?"
