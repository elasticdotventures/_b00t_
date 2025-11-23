# b00t Agent Orchestrator

The **agent orchestrator** solves the chicken-egg problem where b00t commands require services to be running before they can execute.

## Problem Statement

When you run `b00t grok learn <url>`, the grok system needs:
- Qdrant vector database running
- Ollama API (optional, for local embeddings)
- Python environment with dependencies

Previously, if these weren't running, the command would fail with cryptic errors.

## Solution: Silent Dependency Management

The orchestrator automatically:
1. Detects command dependencies from datums
2. Checks if required services are running
3. Silently starts missing dependencies
4. Waits for services to be ready
5. Executes the command

## Architecture

```rust
// In commands/grok.rs
async fn ensure_grok_dependencies() -> Result<()> {
    let orchestrator = Orchestrator::new(&path)?;

    // Silently start Qdrant if needed
    orchestrator.ensure_dependencies("grok-guru.mcp").await?;

    Ok(())
}
```

### Datum Dependencies

Datums declare dependencies using `depends_on` field:

```toml
# grok-guru.mcp.toml
[b00t]
name = "grok-guru"
type = "mcp"
depends_on = ["qdrant.docker"]  # ⬅️ Dependency declaration
```

### Stack Orchestration

Complex systems use stack datums:

```toml
# grok.stack.toml
[b00t]
name = "grok"
type = "stack"
members = [
    "qdrant.docker",
    "grok-guru.mcp",
]
```

## Implementation Details

### Orchestrator Module

Located: `b00t-cli/src/orchestrator.rs`

**Key Functions:**
- `new(path)` - Load all datums from _b00t_/
- `ensure_dependencies(datum_key)` - Start dependencies if needed
- `needs_start(datum)` - Check if service requires starting
- `start_service(datum)` - Execute start command for datum type

### Docker Service Management

```rust
async fn start_docker_service(&self, datum: &BootDatum) -> Result<()> {
    let runtime = self.get_container_runtime()?; // docker or podman
    let image = datum.image.as_ref()?;

    // Build docker run command from datum.docker_args
    let mut args = vec!["run", "-d", "--name", &datum.name];

    // Add environment, ports, volumes from datum
    if let Some(docker_args) = &datum.docker_args {
        args.extend(docker_args.iter().map(|s| s.as_str()));
    }

    // Execute silently
    Command::new(&runtime).args(&args).output()?;

    // Wait for readiness
    self.wait_for_ready(datum).await?;

    Ok(())
}
```

## Usage Patterns

### For Command Authors

Add orchestration to any command that needs services:

```rust
use crate::orchestrator::Orchestrator;

pub async fn handle_my_command() -> Result<()> {
    // Ensure dependencies before execution
    ensure_dependencies("my-service.mcp").await?;

    // ... rest of command logic
}

async fn ensure_dependencies(datum_key: &str) -> Result<()> {
    let path = std::env::var("_B00T_Path")?;
    let orchestrator = Orchestrator::new(&path)?;
    orchestrator.ensure_dependencies(datum_key).await?;
    Ok(())
}
```

### For Datum Authors

Declare dependencies in your datum:

```toml
[b00t]
name = "my-service"
type = "mcp"
depends_on = ["postgres.docker", "redis.docker"]
```

## Silent Operation

By default, orchestrator operates **silently**:
- ✅ Services start in background
- ✅ No output unless error
- ✅ Transparent to user

Enable debug output:
```bash
export B00T_DEBUG=1
b00t grok learn <url>
# Output: 🚀 Started dependencies: qdrant.docker
```

## Future Enhancements

### Health Checks

Currently waits for container to start. Future:
```toml
[b00t.health_check]
http_url = "http://localhost:6333/health"
expected_status = 200
timeout_ms = 5000
```

### Graceful Shutdown

Add `b00t stop <datum>` command:
```rust
pub async fn stop_service(&self, datum_key: &str) -> Result<()> {
    let datum = self.datums.get(datum_key)?;
    match datum.datum_type {
        DatumType::Docker => {
            Command::new("docker")
                .args(&["stop", &datum.name])
                .output()?;
        }
        _ => {}
    }
    Ok(())
}
```

### Dependency Caching

Track running services to avoid repeated checks:
```rust
pub struct Orchestrator {
    datums: HashMap<String, BootDatum>,
    running_cache: Arc<Mutex<HashSet<String>>>,  // ⬅️ Cache
}
```

## Best Practices

1. **Minimal Dependencies** - Only declare direct dependencies
2. **Silent by Default** - Don't spam logs unless debugging
3. **Idempotent Start** - Starting an already-running service is no-op
4. **Fail Fast** - Error immediately if dependency can't start
5. **Type Safety** - Use datum types for orchestration logic

## Related Files

- `b00t-cli/src/orchestrator.rs` - Core implementation
- `b00t-cli/src/commands/grok.rs` - Example integration
- `b00t-cli/src/dependency_resolver.rs` - Topological sort
- `_b00t_/grok-guru.mcp.toml` - Datum with dependencies
- `_b00t_/grok.stack.toml` - Stack orchestration

## Philosophy

The orchestrator embodies b00t's principle of **invisible intelligence**:
- Agents shouldn't babysit infrastructure
- Dependencies should "just work"
- Complexity hidden behind simple interfaces

When you type `b00t grok learn`, you shouldn't think about Qdrant, Docker, or Python environments. The orchestrator handles that complexity silently.

That's alignment. 🍰
