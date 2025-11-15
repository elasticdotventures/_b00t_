# RHAI MCP Server Validation Pattern

Idiomatic pattern for validating MCP server prerequisites using RHAI scripts.

## Overview

RHAI pre-start validation scripts run before MCP servers start, providing:
- **Fail-fast validation** - Catch configuration errors early
- **Clear error messages** - Actionable guidance vs cryptic server errors
- **Credential management** - Dynamic token retrieval from secure sources
- **Environment setup** - Automatic configuration from multiple sources

## Pattern: pre_start Field

Add `pre_start` field to MCP server TOML configuration:

```toml
[[b00t.mcp.stdio]]
priority = 0
command = "npx"
args = ["-y", "some-mcp-server"]
pre_start = "validation-script.rhai"  # Runs before server starts
```

**Execution flow:**
1. b00t reads MCP server configuration
2. Detects `pre_start` field
3. Executes RHAI script from `_b00t_/scripts/`
4. If script succeeds → Start MCP server
5. If script fails → Stop with error message (no server start)

## Use Cases

### 1. Credential Validation (GitHub Example)

**Problem**: GitHub MCP server requires `GITHUB_PERSONAL_ACCESS_TOKEN`, but:
- Token might not be set in environment
- User might have gh CLI authenticated
- PAT storage is security vector
- Server errors are cryptic ("401 Unauthorized")

**Solution**: Pre-start script validates/retrieves token

```rhai
// _b00t_/scripts/github-auth-validate.rhai

// Check if token already set
let token = get_env("GITHUB_PERSONAL_ACCESS_TOKEN");
if token != "" {
    log_success("Token found in environment");
    return token;
}

// Check .env file
let workspace_root = get_env("WORKSPACE_ROOT");
if workspace_root == "" {
    workspace_root = get_env("HOME");
}
let env_file = workspace_root + "/.env";
if file_exists(env_file) {
    // ... parse .env for token ...
}

// Try gh CLI
if !command_exists("gh") {
    log_error("❌ gh CLI not found");
    log_error("ℹ️  Install: https://cli.github.com/manual/installation");
    throw "No GitHub credentials available";
}

// Check gh auth status
let status = run_cmd("gh auth status 2>&1");
if !status.contains("Logged in") {
    log_error("❌ gh not authenticated");
    log_error("ℹ️  Run: gh auth login");
    throw "GitHub CLI not authenticated";
}

// Get token from gh CLI
let token = run_cmd("gh auth token").trim();
set_env("GITHUB_PERSONAL_ACCESS_TOKEN", token);
log_success("✅ Token retrieved from gh CLI");
return token;
```

**TOML configuration:**
```toml
[[b00t.mcp.stdio]]
priority = 0
command = "npx"
args = ["-y", "github-mcp-server"]
requires = ["node", "gh"]
pre_start = "github-auth-validate.rhai"
```

### 2. API Key Validation (Generic Pattern)

Validate API keys from multiple sources with priority:

```rhai
// _b00t_/scripts/api-key-validate.rhai

let service = "OPENAI";  // Customize per service
let env_var = service + "_API_KEY";

// 1. Check environment
let key = get_env(env_var);
if key != "" {
    log_success(`✅ ${env_var} found in environment`);
    return key;
}

// 2. Check .env file
let env_file = WORKSPACE_ROOT + "/.env";
if file_exists(env_file) {
    let content = read_file(env_file);
    let lines = content.split("\n");

    for line in lines {
        if line.starts_with(env_var + "=") {
            key = line.sub_string(env_var.len() + 1).trim();
            set_env(env_var, key);
            log_success(`✅ ${env_var} loaded from .env`);
            return key;
        }
    }
}

// 3. Check platform-specific secret managers
if command_exists("gh") {
    let secret = run_cmd(`gh secret list | grep ${env_var}`);
    if secret != "" {
        // Platform-specific secret retrieval logic
    }
}

// Fail with actionable error
log_error(`❌ ${env_var} not found`);
log_error(`ℹ️  Set via: export ${env_var}=sk-...`);
log_error(`ℹ️  Or add to .env: ${env_var}=sk-...`);
throw `${env_var} not configured`;
```

### 3. Docker/Service Health Check

Verify external services before starting MCP server:

```rhai
// _b00t_/scripts/docker-health-check.rhai

log_info("🐳 Checking Docker service...");

if !command_exists("docker") {
    log_error("❌ Docker not installed");
    log_error("ℹ️  Install: https://docs.docker.com/get-docker/");
    throw "Docker required but not installed";
}

// Check Docker daemon
let docker_ps = try {
    run_cmd("docker ps 2>&1")
} catch(e) {
    log_error("❌ Docker daemon not running");
    log_error("ℹ️  Start: sudo systemctl start docker");
    throw "Docker daemon not accessible";
};

// Check specific container if needed
if !docker_ps.contains("redis-mcp") {
    log_warn("⚠️  redis-mcp container not running");
    log_info("ℹ️  Starting container...");
    run_cmd("docker run -d --name redis-mcp redis:latest");
}

log_success("✅ Docker services ready");
return true;
```

### 4. Configuration File Validation

Validate required configuration files exist and are valid:

```rhai
// _b00t_/scripts/config-validate.rhai

let config_file = get_env("WORKSPACE_ROOT") + "/config.json";

if !file_exists(config_file) {
    log_error(`❌ Configuration file not found: ${config_file}`);
    log_error("ℹ️  Create from template: cp config.example.json config.json");
    throw "Configuration file missing";
}

let config = read_file(config_file);

// Basic JSON validation
if !config.contains("{") || !config.contains("}") {
    log_error("❌ Configuration file is not valid JSON");
    throw "Invalid configuration format";
}

// Check required fields (simplified example)
if !config.contains("\"api_endpoint\"") {
    log_error("❌ Missing required field: api_endpoint");
    throw "Incomplete configuration";
}

log_success("✅ Configuration validated");
return true;
```

## Best Practices

### 1. Fail Fast with Actionable Errors

❌ **Bad**: Vague error
```rhai
throw "Authentication failed";
```

✅ **Good**: Actionable guidance
```rhai
log_error("❌ GitHub CLI not authenticated");
log_error("ℹ️  Run: gh auth login");
log_error("ℹ️  Or set: export GITHUB_PERSONAL_ACCESS_TOKEN=ghp_xxx");
throw "GitHub authentication failed: gh CLI not authenticated (run 'gh auth login')";
```

### 2. Multiple Fallback Sources

Check credentials in priority order:
1. Environment variable (fastest)
2. .env file (project-specific)
3. Platform CLI (gh, aws, gcloud)
4. Secret manager (last resort)

```rhai
let token = get_env("API_KEY");
if token == "" {
    token = check_env_file("API_KEY");
}
if token == "" {
    token = get_from_cli();
}
if token == "" {
    throw "No credentials found";
}
```

### 3. Validate Format/Scopes

Don't just check if credentials exist - validate they're usable:

```rhai
// Validate token format
if !token.starts_with("ghp_") {
    log_warn("⚠️  Unexpected token format");
}

// Validate scopes (if possible)
let scopes = run_cmd("gh auth status 2>&1");
if !scopes.contains("repo") {
    log_warn("⚠️  Token may lack 'repo' scope");
    log_info("ℹ️  Refresh: gh auth refresh -s repo");
}
```

### 4. Set Environment for Server

Always set the environment variable after validation:

```rhai
let token = run_cmd("gh auth token").trim();
set_env("GITHUB_PERSONAL_ACCESS_TOKEN", token);  // Server will use this
return token;
```

### 5. Return Useful Values

Script return value can be used by b00t:

```rhai
// Return token for logging/debugging
return token;  // b00t can log: "Token retrieved: ghp_xxx..."

// Return status object
return #{
    success: true,
    source: "gh_cli",
    token_prefix: token.sub_string(0, 8)
};
```

## RHAI Functions Available

Pre-start scripts have access to all b00t RHAI functions:

### Environment
- `get_env(var)` - Get environment variable
- `set_env(var, value)` - Set environment variable

### Commands
- `command_exists(cmd)` - Check if command available
- `run_cmd(cmd)` - Execute shell command (throws on error)

### File Operations
- `file_exists(path)` - Check file exists
- `read_file(path)` - Read file contents
- `write_file(path, content)` - Write file

### Logging
- `log_info(msg)` - Info message (ℹ️)
- `log_warn(msg)` - Warning message (⚠️)
- `log_error(msg)` - Error message (❌)
- `log_success(msg)` - Success message (✅)

### Context Variables (access via `get_env()`)
- `get_env("WORKSPACE_ROOT")` - Current workspace path
- `get_env("USER")` - Current user
- `get_env("IS_CI")` - Running in CI environment
- `get_env("IS_DOCKER")` - Running in Docker container
- `get_env("HOSTNAME")` - Current hostname

## Error Handling

### Throw Errors to Fail Fast

```rhai
if !condition {
    throw "Error message";  // Stops server startup
}
```

### Try/Catch for Optional Operations

```rhai
let result = try {
    run_cmd("optional-command")
} catch(e) {
    log_warn(`⚠️  Optional step failed: ${e}`);
    ""  // Continue anyway
};
```

## Testing Validation Scripts

Test scripts manually before deploying:

```bash
# Run validation script
b00t script run github-auth-validate

# Should output:
# 🔐 Validating GitHub authentication for MCP server...
# ✅ Token retrieved successfully from gh CLI
# ℹ️  Token prefix: ghp_abcd...
```

## Common Patterns

### Credential Priority Chain

```rhai
fn get_credential(var_name) {
    // 1. Environment
    let val = get_env(var_name);
    if val != "" { return val; }

    // 2. .env file
    val = read_from_env_file(var_name);
    if val != "" { return val; }

    // 3. CLI tool
    val = get_from_cli(var_name);
    if val != "" { return val; }

    // 4. Fail
    throw `${var_name} not found`;
}
```

### Service Dependency Check

```rhai
fn check_service(name, cmd) {
    log_info(`Checking ${name}...`);

    if !command_exists(cmd) {
        log_error(`❌ ${name} not installed`);
        throw `${name} required`;
    }

    let status = run_cmd(`${cmd} --version 2>&1`);
    log_success(`✅ ${name} available`);
}

check_service("Docker", "docker");
check_service("Kubectl", "kubectl");
```

## Integration with TOML Schema

The `pre_start` field is part of the MCP stdio configuration:

```toml
[[b00t.mcp.stdio]]
priority = 0           # Server priority (0 = highest)
command = "npx"        # Command to run
args = ["-y", "server"] # Command arguments
transport = "stdio"    # Transport protocol
requires = ["node"]    # Required dependencies
pre_start = "validate.rhai"  # ← Validation script

[b00t.mcp.stdio.env]
# Environment variables for server
VAR = "value"
```

## LFMF Integration

Record lessons about validation patterns:

```bash
b00t lfmf rhai "pre-start validation: Use pre_start field in MCP TOML for fail-fast credential checks"
b00t lfmf rhai "credential priority: Check env → .env → CLI → fail (in that order)"
b00t lfmf rhai "error messages: Always provide actionable guidance (install, configure, run commands)"
b00t lfmf github-mcp "gh CLI validation: Check gh auth status before calling gh auth token to avoid confusing errors"
```

## Future Enhancements

Potential extensions to the pattern:

1. **Async validation**: Run multiple validations in parallel
2. **Caching**: Cache successful validations to speed up restarts
3. **Retry logic**: Auto-retry failed validations with backoff
4. **Schema validation**: Validate TOML configuration itself
5. **Health monitoring**: Continuous validation while server runs

## References

- [RHAI Language Guide](https://rhai.rs/book/)
- [b00t RHAI Engine](../../b00t-c0re-lib/src/rhai_engine.rs)
- [GitHub MCP Validation](github-mcp.md#rhai-validation-script)
- [Example Scripts](_b00t_/scripts/)
