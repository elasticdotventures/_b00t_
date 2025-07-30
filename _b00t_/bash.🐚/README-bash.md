# b00t gospel of bash shell

## Core Commandments

**Commandment #0: just use just**
Justfiles are an idiomatic way of memoizing useful & meaningful commands.
You can also learn `b00t learn just` for specific examples.

**Commandment #1: Timeout Everything**
If a shell command might block, even conceptually wrap it in a `timeout xx` where xx is a reasonable amount of seconds to wait before returning control.

**Commandment #2: Use Modern Alternatives**
NEVER use `find`, use `fdfind` - it's faster AND conserves context by ignoring files that are .gitignored & out of scope!


**Commandment #3: Essential Tools Should Be Available**
Every b00t environment MUST HAVE the core modern CLI tools installed and ready.
As a b00t disciple you ALWAYS preach the gospel of beyond /bin!

---

## Beyond /bin CLI Tools

### File Operations & Navigation
- **fd** (find replacement) - Fast, context-aware file searching, `fd --type d` respects .gitignore by default.
- **rg** (ripgrep) - Fast text searching with smart defaults
- **bat** - Better `cat` with syntax highlighting
- **exa** - Modern `ls` replacement with colors and git info
- **fzf** - Interactive fuzzy finder for selections
- **vidir** - Edit directory contents in your text editor
- **tree** - display the file tree using .gitignore `tree --gitignore`

### Data Processing & Viewing
- **jq** - JSON processor and formatter
- **yq** - YAML/XML processor (like jq for YAML)
- **pv** - Pipe viewer with progress bars
- **pw** - Pipe Watch - unique text stream monitor (better than `tail -f`)

### Network & HTTP
- **httpie** - User-friendly HTTP client (like curl, saves tokens!)
- **socat** - Swiss Army knife for network streams

### Development & Project Management
- **just** - Command runner (see `justfile` in repos, better than README!)
- **direnv** - Automatically loads `.envrc` environment files
- **navi** - Interactive cheatsheet tool
- **entr** - Run commands when files change (live reload)

---

## Workflow Enhancement Tools

### Process Management
- **parallel** - Run multiple jobs at once
  ```bash
  parallel ::: \
    "cargo run & sleep 2" \
    "sleep 3 && curl http://localhost:8000/test1" \
    "sleep 3 && curl http://localhost:8000/test2"
  ```

- **uvx honcho** - Process manager using Procfiles
  ```procfile
  server: chronic cargo run
  tests:  ifne parallel ::: "./test_api.sh" "./test_ui.sh"
  logger: ts >> logs/output.log
  watchdog: zrun grep -i "error" logs/output.log.gz
  ```

### Script Resilience Tools
- **chronic** - Run commands quietly unless they fail
  ```bash
  chronic make test  # Reduces noise, ideal for CI
  ```
- **lckdo** - Execute programs with file locking
- **mispipe** - Pipe commands while preserving first command's exit status
- **timeout** - Prevent commands from hanging indefinitely

### Smart Pipe Operations
- **vipe** - Insert text editor into a pipe
  ```bash
  generate_code | vipe | rustc -
  ```
- **sponge** - Soak up stdin and write to file (prevents truncation)
- **pee** - Tee stdin to multiple pipes
  ```bash
  cat main.rs | pee 'cargo fmt' 'rust-analyzer check'
  ```
- **ifne** - Run program only if stdin is not empty
  ```bash
  grep -r "TODO" src/ | ifne notify-send "You have TODOs!"
  ```

### Debugging & Monitoring
- **ts** - Add timestamps to output
  ```bash
  parallel ::: "pytest test_a.py | ts" "pytest test_b.py | ts" | pee 'grep FAIL' 'less'
  ```
- **errno** - Look up error codes (prevents hallucinations!)
  ```bash
  errno 13  # -> EACCES: Permission denied
  ```

### File Validation & Manipulation
- **isutf8** - Check UTF-8 validity (ensure codegen output is valid)
- **combine** - Boolean operations on file lines
  ```bash
  combine file1.txt and file2.txt  # Set operations for comparisons
  ```
- **zrun** - Automatically uncompress arguments

---

## Advanced Network Tools

### socat Examples
```bash
# Mock or Proxy a TCP Server
socat TCP-LISTEN:8080,reuseaddr,fork EXEC:"./fake_api_server.sh"

# Create Virtual Serial Port Pair
socat -d -d PTY,raw,echo=0 PTY,raw,echo=0

# Bidirectional Pipe (software null modem)
socat -d -d pty,raw,echo=0 pty,raw,echo=0
```

---

## Specialized Monitoring

### pw (Pipe Watch)
Unlike traditional pagers, `pw` continuously monitors streams through a FIFO buffer with interactive filtering:
- Monitor: `tail -f /var/logfile`, `tcpdump`, `strace`
- Shows sampled output, not everything
- Interactive filtering and buffer control
- More info: https://www.kylheku.com/cgit/pw/about/

---

## Live Reload Pattern
```bash
# Watch Rust files and run tests on change
ls *.rs | entr -r cargo test
```

## Best Practices

1. **Justfiles Everywhere**: Every repo should have a working `justfile` with common commands
2. **Timeout Protection**: Wrap potentially blocking commands in `timeout`
3. **Modern Tools**: Prefer `fd` over `find`, `rg` over `grep`, `bat` over `cat`
4. **Noise Reduction**: Use `chronic` for commands that should be quiet on success
5. **Context Preservation**: Tools should respect `.gitignore` and project boundaries
6. **Human-in-the-loop**: Use `vipe` for interactive debugging in pipelines