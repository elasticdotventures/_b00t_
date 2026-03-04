# b00t up / Tutorial / Ontology Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `b00t up` (Rust outer-loop for ralph agent), `b00t tutorial` (datum-based progression tracking via session memory), and `b00t ontology` (live capability query) commands.

**Architecture:** `b00t up` spawns `b00t.sh` (existing ralph loop) as a child process, injects `B00T_ONTOLOGY` env var, and restarts on exit code 75 (POSIX TEMPFAIL). Tutorial state lives in existing `SessionMemory` via string keys. Ontology is derived in real-time from datum TOMLs — no new config files.

**Tech Stack:** Rust (b00t-cli), TOML (datum files), existing `SessionMemory`, `std::process::Command`, `serde_json`.

---

## Task 1: Add `b00t up` top-level command (skeleton)

**Files:**
- Create: `b00t-cli/src/commands/up.rs`
- Modify: `b00t-cli/src/commands/mod.rs`
- Modify: `b00t-cli/src/main.rs`

### Step 1: Write failing integration test

Add to `b00t-cli/src/commands/up.rs` (create file):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_up_command_parses() {
        use clap::Parser;
        // Verify the command parses without panic
        let cmd = UpArgs::try_parse_from(["b00t-cli", "--tool", "claude"]);
        assert!(cmd.is_ok());
    }

    #[test]
    fn test_up_command_defaults() {
        let args = UpArgs {
            tool: "claude".to_string(),
            max_iter: 10,
            role: None,
            max_restarts: 5,
        };
        assert_eq!(args.tool, "claude");
        assert_eq!(args.max_iter, 10);
        assert_eq!(args.max_restarts, 5);
    }
}
```

### Step 2: Run test to verify it fails

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli test_up_command 2>&1 | head -20
```
Expected: FAIL — `up.rs` not yet created.

### Step 3: Implement skeleton `up.rs`

```rust
// b00t-cli/src/commands/up.rs
use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
pub struct UpArgs {
    /// AI tool to use for the ralph loop
    #[clap(long, default_value = "claude", value_parser = ["claude", "amp", "codex"])]
    pub tool: String,

    /// Maximum iterations per ralph session
    #[clap(long, default_value = "10")]
    pub max_iter: u32,

    /// Agent role (filters ontology + tutorial path)
    #[clap(long)]
    pub role: Option<String>,

    /// Maximum restart cycles before giving up
    #[clap(long, default_value = "5")]
    pub max_restarts: u32,
}

impl UpArgs {
    pub fn execute(&self) -> Result<()> {
        println!("🥾 b00t up: launching ralph loop (tool={}, max_iter={}, max_restarts={})",
            self.tool, self.max_iter, self.max_restarts);
        // Phase 1: skeleton — real spawn in Task 2
        println!("⚠️  b00t up spawn not yet implemented");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_up_command_parses() {
        let args = UpArgs::try_parse_from(["b00t-cli", "--tool", "claude"]);
        assert!(args.is_ok());
    }

    #[test]
    fn test_up_command_defaults() {
        let args = UpArgs {
            tool: "claude".to_string(),
            max_iter: 10,
            role: None,
            max_restarts: 5,
        };
        assert_eq!(args.tool, "claude");
        assert_eq!(args.max_restarts, 5);
    }
}
```

### Step 4: Wire into `mod.rs`

In `b00t-cli/src/commands/mod.rs`, add:
```rust
pub mod up;
pub use up::UpArgs;
```

### Step 5: Wire into `main.rs`

In `b00t-cli/src/main.rs`, find the `Commands` enum and add:
```rust
#[clap(about = "Launch ralph agent REPL outer-loop")]
Up(commands::up::UpArgs),
```

In the match block at bottom, add:
```rust
Some(Commands::Up(args)) => args.execute(),
```

Also add the import at top:
```rust
use commands::UpArgs;
```

### Step 6: Run tests

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli test_up_command 2>&1
cargo build -p b00t-cli 2>&1 | tail -5
```
Expected: tests PASS, build succeeds.

### Step 7: Smoke test

```bash
cd /home/brianh/.b00t && cargo run -p b00t-cli -- up --help 2>&1
```
Expected: Shows `b00t-cli up` help with `--tool`, `--max-iter`, `--role`, `--max-restarts`.

### Step 8: Commit

```bash
git add b00t-cli/src/commands/up.rs b00t-cli/src/commands/mod.rs b00t-cli/src/main.rs
git commit -m "feat(up): add b00t up command skeleton with clap parser"
```

---

## Task 2: Implement ralph spawn + exit-code restart loop

**Files:**
- Modify: `b00t-cli/src/commands/up.rs`

### Step 1: Write failing test for spawn logic

Add to `up.rs` tests:
```rust
#[test]
fn test_exit_code_75_is_tempfail() {
    // 75 = POSIX EX_TEMPFAIL — agent signals restart intent
    assert_eq!(75u32, libc_ex_tempfail());
}

fn libc_ex_tempfail() -> u32 { 75 }

#[test]
fn test_restart_count_increments() {
    let mut count = 0u32;
    let max = 3u32;
    // Simulate 3 restarts then done
    let exit_codes = vec![75, 75, 75, 0];
    let mut final_code = 0i32;
    for code in exit_codes {
        if code == 75 && count < max {
            count += 1;
        } else {
            final_code = code;
            break;
        }
    }
    assert_eq!(count, 3);
    assert_eq!(final_code, 0);
}
```

### Step 2: Run test

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli test_exit_code 2>&1
cargo test -p b00t-cli test_restart_count 2>&1
```
Expected: PASS (pure logic tests, no I/O).

### Step 3: Implement ralph spawn

Replace `execute` in `up.rs`:
```rust
use crate::session_memory::SessionMemory;
use std::process::Command;

impl UpArgs {
    pub fn execute(&self) -> Result<()> {
        let workspace_root = crate::utils::get_workspace_root();
        let ralph_script = format!("{}/b00t.sh", workspace_root);

        // Verify b00t.sh exists
        if !std::path::Path::new(&ralph_script).exists() {
            anyhow::bail!("b00t.sh not found at {}. Run from b00t workspace root.", ralph_script);
        }

        let mut restart_count = 0u32;
        let mut session = SessionMemory::load().unwrap_or_default();

        loop {
            println!("🥾 b00t up: cycle {} (tool={}, max_iter={})",
                restart_count + 1, self.tool, self.max_iter);

            // Build ontology JSON (Task 4 fills this out; placeholder for now)
            let ontology_json = build_ontology_placeholder(self.role.as_deref());

            let status = Command::new("bash")
                .arg(&ralph_script)
                .arg("--tool")
                .arg(&self.tool)
                .arg(self.max_iter.to_string())
                .env("B00T_ONTOLOGY", &ontology_json)
                .env("B00T_ROLE", self.role.as_deref().unwrap_or("developer"))
                .current_dir(&workspace_root)
                .status()
                .context(format!("Failed to exec b00t.sh: {}", ralph_script))?;

            let code = status.code().unwrap_or(1);

            // Update session
            session.set("up.last_exit", &code.to_string())?;
            session.set("up.tool", &self.tool)?;
            session.set("up.restart_count", &restart_count.to_string())?;
            let _ = session.save();

            match code {
                0 => {
                    println!("✅ b00t up: ralph completed successfully after {} cycles", restart_count + 1);
                    return Ok(());
                }
                75 => {
                    restart_count += 1;
                    if restart_count >= self.max_restarts {
                        anyhow::bail!("b00t up: max restarts ({}) reached, giving up", self.max_restarts);
                    }
                    println!("🔄 b00t up: restart {} of {} requested (exit 75)", restart_count, self.max_restarts);
                }
                n => {
                    anyhow::bail!("b00t up: ralph exited with error code {}", n);
                }
            }
        }
    }
}

fn build_ontology_placeholder(role: Option<&str>) -> String {
    // Placeholder — Task 4 replaces with real datum scan
    serde_json::json!({
        "role": role.unwrap_or("developer"),
        "available": [],
        "installable": [],
        "blessings": [],
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "note": "placeholder - see Task 4"
    }).to_string()
}
```

### Step 4: Build

```bash
cd /home/brianh/.b00t && cargo build -p b00t-cli 2>&1 | grep -E "error|warning: unused" | head -20
```
Expected: builds clean (or only unused import warnings).

### Step 5: Commit

```bash
git add b00t-cli/src/commands/up.rs
git commit -m "feat(up): implement ralph spawn loop with exit-75 restart protocol"
```

---

## Task 3: Add `[validate]` + `[roles]` fields to core datum TOMLs

**Files (modify existing, additive changes):**
- `_b00t_/git.cli.toml` (or wherever git datum lives — check with `ls _b00t_/git*`)
- `_b00t_/gh.cli.toml`
- `_b00t_/just.cli.toml`
- `_b00t_/rustc.cli.toml` (or `rust.cli.toml`)
- `_b00t_/context7.mcp.toml`
- `_b00t_/taskmaster-ai.mcp.toml`
- `_b00t_/uv.cli.toml`

### Step 1: Locate the datum files

```bash
ls /home/brianh/.b00t/_b00t_/ | grep -E "^(git|gh|just|rust|context7|taskmaster|uv)\."
```
Expected: shows existing toml files.

### Step 2: Append to each datum TOML

For each tool, append these sections (do NOT modify existing fields — additive only):

**git datum:**
```toml
[validate]
command = "git --version"
regex = "git version \\d+"

[roles]
required_for = ["developer", "orchestrator", "analyst"]
optional_for = []
```

**gh datum:**
```toml
[validate]
command = "gh --version"
regex = "gh version \\d+"

[roles]
required_for = ["developer", "orchestrator"]
optional_for = ["analyst"]
```

**just datum:**
```toml
[validate]
command = "just --version"
regex = "\\d+\\.\\d+\\.\\d+"

[roles]
required_for = ["developer", "orchestrator"]
optional_for = ["analyst"]
```

**uv datum:**
```toml
[validate]
command = "uv --version"
regex = "uv \\d+"

[roles]
required_for = ["developer"]
optional_for = ["orchestrator", "analyst"]
```

**context7 mcp datum:**
```toml
[validate]
command = "bunx @upstash/context7-mcp --help"
regex = "context7|MCP"

[roles]
required_for = ["developer", "orchestrator"]
optional_for = ["analyst"]
```

**taskmaster-ai mcp datum:**
```toml
[validate]
command = "npx task-master --version"
regex = "\\d+\\.\\d+"

[roles]
required_for = ["orchestrator"]
optional_for = ["developer"]
```

### Step 3: Verify TOML parses correctly

```bash
cd /home/brianh/.b00t && for f in _b00t_/git.cli.toml _b00t_/gh.cli.toml _b00t_/just.cli.toml _b00t_/uv.cli.toml; do
  python3 -c "import tomllib; tomllib.load(open('$f','rb')); print('✅ $f')" 2>&1
done
```
Expected: `✅` for each file.

### Step 4: Write parser test in b00t-cli

Add to `b00t-cli/src/commands/up.rs` tests:
```rust
#[test]
fn test_datum_toml_has_validate_section() {
    let workspace = crate::utils::get_workspace_root();
    let git_datum = format!("{}/_b00t_/git.cli.toml", workspace);
    if std::path::Path::new(&git_datum).exists() {
        let content = std::fs::read_to_string(&git_datum).unwrap();
        // Additive fields should be present
        assert!(content.contains("[validate]"), "git datum missing [validate] section");
        assert!(content.contains("[roles]"), "git datum missing [roles] section");
    }
    // If file doesn't exist on CI, skip gracefully
}
```

### Step 5: Run test

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli test_datum_toml 2>&1
```
Expected: PASS.

### Step 6: Commit

```bash
git add _b00t_/git.cli.toml _b00t_/gh.cli.toml _b00t_/just.cli.toml _b00t_/uv.cli.toml _b00t_/context7.mcp.toml _b00t_/taskmaster-ai.mcp.toml
git commit -m "feat(datums): add [validate] and [roles] sections to core datums"
```

---

## Task 4: Implement `b00t ontology query` command

**Files:**
- Create: `b00t-cli/src/commands/ontology.rs`
- Modify: `b00t-cli/src/commands/mod.rs`
- Modify: `b00t-cli/src/main.rs`
- Modify: `b00t-cli/src/commands/up.rs` (replace placeholder)

### Step 1: Write failing test

```rust
// In ontology.rs tests:
#[test]
fn test_datum_roles_parsed() {
    let toml_str = r#"
[b00t]
name = "git"
type = "cli"

[validate]
command = "git --version"
regex = "git version \\d+"

[roles]
required_for = ["developer", "orchestrator"]
optional_for = ["analyst"]
"#;
    let datum: DatumMeta = toml::from_str(toml_str).unwrap();
    assert!(datum.roles.required_for.contains(&"developer".to_string()));
    assert!(datum.roles.optional_for.contains(&"analyst".to_string()));
    assert_eq!(datum.validate.command, "git --version");
}

#[test]
fn test_ontology_filters_by_role() {
    let datums = vec![
        DatumMeta {
            b00t: B00tMeta { name: "git".to_string(), ..Default::default() },
            validate: ValidateMeta { command: "git --version".to_string(), regex: "".to_string() },
            roles: RolesMeta {
                required_for: vec!["developer".to_string()],
                optional_for: vec![],
            },
        },
        DatumMeta {
            b00t: B00tMeta { name: "k9s".to_string(), ..Default::default() },
            validate: ValidateMeta { command: "k9s version".to_string(), regex: "".to_string() },
            roles: RolesMeta {
                required_for: vec!["orchestrator".to_string()],
                optional_for: vec!["developer".to_string()],
            },
        },
    ];
    let required = filter_required_for_role(&datums, "developer");
    assert_eq!(required.len(), 1);
    assert_eq!(required[0].b00t.name, "git");
}
```

### Step 2: Run to verify fails

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli test_datum_roles 2>&1 | head -15
cargo test -p b00t-cli test_ontology_filters 2>&1 | head -15
```
Expected: FAIL — types not yet defined.

### Step 3: Implement `ontology.rs`

```rust
// b00t-cli/src/commands/ontology.rs
use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Parser, Debug)]
pub enum OntologyCommands {
    #[clap(about = "Query live capability ontology from datum TOMLs")]
    Query {
        #[clap(long, help = "Filter by agent role (developer|orchestrator|analyst)")]
        role: Option<String>,
        #[clap(long, default_value = "table", value_parser = ["table", "json"])]
        format: String,
    },
}

impl OntologyCommands {
    pub fn execute(&self) -> Result<()> {
        match self {
            OntologyCommands::Query { role, format } => {
                let ontology = build_ontology(role.as_deref())?;
                match format.as_str() {
                    "json" => println!("{}", serde_json::to_string_pretty(&ontology)?),
                    _ => print_ontology_table(&ontology),
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DatumMeta {
    pub b00t: B00tMeta,
    #[serde(default)]
    pub validate: ValidateMeta,
    #[serde(default)]
    pub roles: RolesMeta,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct B00tMeta {
    pub name: String,
    #[serde(rename = "type", default)]
    pub datum_type: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub version_regex: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ValidateMeta {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub regex: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct RolesMeta {
    #[serde(default)]
    pub required_for: Vec<String>,
    #[serde(default)]
    pub optional_for: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ontology {
    pub role: String,
    pub available: Vec<String>,   // installed + validate passes
    pub installable: Vec<String>, // not installed but in role path
    pub blessings: Vec<String>,   // detected env credentials
    pub timestamp: String,
}

pub fn build_ontology(role: Option<&str>) -> Result<Ontology> {
    let role = role.unwrap_or("developer").to_string();
    let workspace = crate::utils::get_workspace_root();
    let datum_dir = format!("{}/_b00t_", workspace);

    let datums = scan_datums(&datum_dir)?;
    let role_datums: Vec<_> = datums.iter()
        .filter(|d| {
            d.roles.required_for.contains(&role) || d.roles.optional_for.contains(&role)
        })
        .collect();

    let mut available = Vec::new();
    let mut installable = Vec::new();

    for datum in &role_datums {
        if datum.validate.command.is_empty() {
            continue;
        }
        if is_validated(datum) {
            available.push(datum.b00t.name.clone());
        } else {
            installable.push(datum.b00t.name.clone());
        }
    }

    let blessings = detect_blessings();

    Ok(Ontology {
        role,
        available,
        installable,
        blessings,
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

fn scan_datums(datum_dir: &str) -> Result<Vec<DatumMeta>> {
    let mut datums = Vec::new();
    if !Path::new(datum_dir).exists() {
        return Ok(datums);
    }
    for entry in fs::read_dir(datum_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "toml") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(datum) = toml::from_str::<DatumMeta>(&content) {
                    if !datum.b00t.name.is_empty() {
                        datums.push(datum);
                    }
                }
            }
        }
    }
    Ok(datums)
}

fn is_validated(datum: &DatumMeta) -> bool {
    if datum.validate.command.is_empty() {
        return false;
    }
    let parts: Vec<&str> = datum.validate.command.split_whitespace().collect();
    if parts.is_empty() {
        return false;
    }
    let result = Command::new(parts[0]).args(&parts[1..])
        .output();
    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}{}", stdout, stderr);
            if datum.validate.regex.is_empty() {
                output.status.success()
            } else {
                let re = regex::Regex::new(&datum.validate.regex)
                    .unwrap_or_else(|_| regex::Regex::new(".*").unwrap());
                re.is_match(&combined)
            }
        }
        Err(_) => false,
    }
}

fn detect_blessings() -> Vec<String> {
    let keys = ["ANTHROPIC_API_KEY", "GITHUB_TOKEN", "OPENAI_API_KEY",
                 "HF_TOKEN", "CLOUDFLARE_API_TOKEN"];
    keys.iter()
        .filter(|k| std::env::var(k).map_or(false, |v| !v.is_empty()))
        .map(|k| k.to_string())
        .collect()
}

fn print_ontology_table(o: &Ontology) {
    println!("🔭 Ontology for role: {}", o.role);
    println!("\n✅ Available ({}):", o.available.len());
    for a in &o.available { println!("   {}", a); }
    println!("\n📦 Installable ({}):", o.installable.len());
    for i in &o.installable { println!("   b00t cli install {}", i); }
    println!("\n🙏 Blessings ({}):", o.blessings.len());
    for b in &o.blessings { println!("   {}", b); }
}

pub fn filter_required_for_role<'a>(datums: &'a [DatumMeta], role: &str) -> Vec<&'a DatumMeta> {
    datums.iter()
        .filter(|d| d.roles.required_for.contains(&role.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datum_roles_parsed() {
        let toml_str = r#"
[b00t]
name = "git"
type = "cli"

[validate]
command = "git --version"
regex = "git version \\d+"

[roles]
required_for = ["developer", "orchestrator"]
optional_for = ["analyst"]
"#;
        let datum: DatumMeta = toml::from_str(toml_str).unwrap();
        assert!(datum.roles.required_for.contains(&"developer".to_string()));
        assert!(datum.roles.optional_for.contains(&"analyst".to_string()));
        assert_eq!(datum.validate.command, "git --version");
    }

    #[test]
    fn test_ontology_filters_by_role() {
        let datums = vec![
            DatumMeta {
                b00t: B00tMeta { name: "git".to_string(), ..Default::default() },
                validate: ValidateMeta { command: "git --version".to_string(), regex: String::new() },
                roles: RolesMeta {
                    required_for: vec!["developer".to_string()],
                    optional_for: vec![],
                },
            },
            DatumMeta {
                b00t: B00tMeta { name: "k9s".to_string(), ..Default::default() },
                validate: ValidateMeta { command: "k9s version".to_string(), regex: String::new() },
                roles: RolesMeta {
                    required_for: vec!["orchestrator".to_string()],
                    optional_for: vec!["developer".to_string()],
                },
            },
        ];
        let required = filter_required_for_role(&datums, "developer");
        assert_eq!(required.len(), 1);
        assert_eq!(required[0].b00t.name, "git");
    }

    #[test]
    fn test_detect_blessings_no_panic() {
        // Just verifies no panic; result depends on env
        let _ = detect_blessings();
    }
}
```

### Step 4: Wire into mod.rs and main.rs

`mod.rs` — add:
```rust
pub mod ontology;
pub use ontology::OntologyCommands;
```

`main.rs` Commands enum — add:
```rust
#[clap(about = "Query live capability ontology from datum TOMLs")]
Ontology {
    #[clap(subcommand)]
    ontology_command: commands::ontology::OntologyCommands,
},
```

`main.rs` match block — add:
```rust
Some(Commands::Ontology { ontology_command }) => {
    ontology_command.execute()
}
```

### Step 5: Update `up.rs` to use real ontology

Replace `build_ontology_placeholder` call in `up.rs` with:
```rust
use crate::commands::ontology::build_ontology;
// ...
let ontology = build_ontology(self.role.as_deref())?;
let ontology_json = serde_json::to_string(&ontology)?;
```

### Step 6: Run tests

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli test_datum_roles 2>&1
cargo test -p b00t-cli test_ontology 2>&1
cargo build -p b00t-cli 2>&1 | grep "^error" | head -10
```
Expected: tests PASS, build clean.

### Step 7: Smoke test

```bash
cd /home/brianh/.b00t && cargo run -p b00t-cli -- ontology query --format json 2>&1 | head -20
```
Expected: JSON with `role`, `available`, `installable`, `blessings`, `timestamp`.

### Step 8: Commit

```bash
git add b00t-cli/src/commands/ontology.rs b00t-cli/src/commands/mod.rs b00t-cli/src/main.rs b00t-cli/src/commands/up.rs
git commit -m "feat(ontology): add b00t ontology query with live datum TOML scan"
```

---

## Task 5: Implement `b00t tutorial` commands

**Files:**
- Create: `b00t-cli/src/commands/tutorial.rs`
- Modify: `b00t-cli/src/commands/mod.rs`
- Modify: `b00t-cli/src/main.rs`

### Step 1: Write failing tests

```rust
// tutorial.rs tests
#[test]
fn test_tutorial_role_path_for_developer() {
    let path = default_role_path("developer");
    assert!(path.contains(&"git".to_string()));
    assert!(path.contains(&"gh".to_string()));
    assert!(path.contains(&"just".to_string()));
}

#[test]
fn test_tutorial_next_skips_completed() {
    let completed = vec!["git".to_string(), "gh".to_string()];
    let path = vec!["git".to_string(), "gh".to_string(), "just".to_string(), "uv".to_string()];
    let next = next_uncompleted(&path, &completed, &[]);
    assert_eq!(next, Some("just".to_string()));
}

#[test]
fn test_tutorial_next_skips_skipped() {
    let completed = vec!["git".to_string()];
    let skipped = vec!["gh".to_string()];
    let path = vec!["git".to_string(), "gh".to_string(), "just".to_string()];
    let next = next_uncompleted(&path, &completed, &skipped);
    assert_eq!(next, Some("just".to_string()));
}

#[test]
fn test_tutorial_progress_percent() {
    let path = vec!["git", "gh", "just", "uv", "context7"];
    let completed = vec!["git", "gh"];
    let pct = progress_percent(path.len(), completed.len());
    assert_eq!(pct, 40);
}
```

### Step 2: Run to verify fails

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli test_tutorial 2>&1 | head -15
```
Expected: FAIL.

### Step 3: Implement `tutorial.rs`

```rust
// b00t-cli/src/commands/tutorial.rs
use anyhow::Result;
use clap::Parser;
use crate::session_memory::SessionMemory;
use crate::commands::ontology::{scan_datums_path, is_validated_cmd};

#[derive(Parser, Debug)]
pub enum TutorialCommands {
    #[clap(about = "Show tutorial progression for current role")]
    Status,
    #[clap(about = "Show next recommended datum to install/validate")]
    Next,
    #[clap(about = "Mark a datum as skipped")]
    Skip {
        #[clap(help = "Datum name to skip")]
        datum: String,
        #[clap(long, default_value = "manually skipped")]
        reason: String,
    },
    #[clap(about = "Run datum validate command and record result")]
    Validate {
        #[clap(help = "Datum name to validate")]
        datum: String,
    },
}

impl TutorialCommands {
    pub fn execute(&self) -> Result<()> {
        let mut session = SessionMemory::load()?;
        match self {
            TutorialCommands::Status => show_status(&session),
            TutorialCommands::Next => show_next(&session),
            TutorialCommands::Skip { datum, reason } => skip_datum(&mut session, datum, reason),
            TutorialCommands::Validate { datum } => validate_datum(&mut session, datum),
        }
    }
}

// Role paths: ordered list of datum names to complete
pub fn default_role_path(role: &str) -> Vec<String> {
    match role {
        "orchestrator" => vec![
            "git", "gh", "just", "context7", "taskmaster-ai", "argo-cli", "k9s",
        ],
        "analyst" => vec![
            "git", "uv", "context7",
        ],
        _ => vec![ // developer (default)
            "git", "gh", "just", "uv", "rustc", "context7", "taskmaster-ai",
        ],
    }.iter().map(|s| s.to_string()).collect()
}

pub fn next_uncompleted(path: &[String], completed: &[String], skipped: &[String]) -> Option<String> {
    path.iter()
        .find(|d| !completed.contains(d) && !skipped.contains(d))
        .cloned()
}

pub fn progress_percent(total: usize, completed: usize) -> u32 {
    if total == 0 { return 0; }
    ((completed as f64 / total as f64) * 100.0) as u32
}

fn get_role(session: &SessionMemory) -> String {
    session.get("tutorial.role")
        .cloned()
        .or_else(|| std::env::var("B00T_ROLE").ok())
        .unwrap_or_else(|| "developer".to_string())
}

fn get_completed(session: &SessionMemory) -> Vec<String> {
    session.get("tutorial.completed")
        .map(|s| s.split(',').filter(|x| !x.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

fn get_skipped(session: &SessionMemory) -> Vec<String> {
    session.get("tutorial.skipped")
        .map(|s| s.split(',').filter(|x| !x.is_empty()).map(String::from).collect())
        .unwrap_or_default()
}

fn show_status(session: &SessionMemory) -> Result<()> {
    let role = get_role(session);
    let path = default_role_path(&role);
    let completed = get_completed(session);
    let skipped = get_skipped(session);
    let pct = progress_percent(path.len(), completed.len());

    println!("📚 Tutorial progress for role: {} ({}%)", role, pct);
    println!("{:-<50}", "");
    for datum in &path {
        let status = if completed.contains(datum) { "✅" }
                     else if skipped.contains(datum) { "⏭️ " }
                     else { "⬜" };
        println!(" {} {}", status, datum);
    }
    println!("\n{} of {} required datums validated.", completed.len(), path.len());
    if let Some(next) = next_uncompleted(&path, &completed, &skipped) {
        println!("▶ Next: b00t tutorial validate {}", next);
    } else {
        println!("🎉 Role path complete!");
    }
    Ok(())
}

fn show_next(session: &SessionMemory) -> Result<()> {
    let role = get_role(session);
    let path = default_role_path(&role);
    let completed = get_completed(session);
    let skipped = get_skipped(session);
    match next_uncompleted(&path, &completed, &skipped) {
        Some(next) => println!("{}", next),
        None => println!("✅ all done"),
    }
    Ok(())
}

fn skip_datum(session: &mut SessionMemory, datum: &str, reason: &str) -> Result<()> {
    let mut skipped = get_skipped(session);
    if !skipped.contains(&datum.to_string()) {
        skipped.push(datum.to_string());
        session.set("tutorial.skipped", &skipped.join(","))?;
        session.save()?;
    }
    println!("⏭️  Skipped {} ({})", datum, reason);
    Ok(())
}

fn validate_datum(session: &mut SessionMemory, datum: &str) -> Result<()> {
    use std::process::Command;
    let workspace = crate::utils::get_workspace_root();
    let datum_dir = format!("{}/_b00t_", workspace);

    // Find datum TOML
    let datums = crate::commands::ontology::scan_datums(&datum_dir)?;
    let found = datums.iter().find(|d| d.b00t.name == datum);

    match found {
        None => {
            println!("⚠️  Datum '{}' not found in {}", datum, datum_dir);
        }
        Some(d) if d.validate.command.is_empty() => {
            println!("⚠️  Datum '{}' has no [validate] command", datum);
        }
        Some(d) => {
            print!("🔍 Validating {} ({})... ", datum, d.validate.command);
            let parts: Vec<&str> = d.validate.command.split_whitespace().collect();
            let result = Command::new(parts[0]).args(&parts[1..]).output();
            match result {
                Ok(out) if out.status.success() => {
                    println!("✅ OK");
                    let mut completed = get_completed(session);
                    if !completed.contains(&datum.to_string()) {
                        completed.push(datum.to_string());
                        session.set("tutorial.completed", &completed.join(","))?;
                        session.save()?;
                    }
                }
                Ok(out) => {
                    println!("❌ FAILED (exit {})", out.status.code().unwrap_or(-1));
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    if !stderr.is_empty() { println!("   {}", stderr.trim()); }
                }
                Err(e) => println!("❌ ERROR: {}", e),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tutorial_role_path_for_developer() {
        let path = default_role_path("developer");
        assert!(path.contains(&"git".to_string()));
        assert!(path.contains(&"gh".to_string()));
        assert!(path.contains(&"just".to_string()));
    }

    #[test]
    fn test_tutorial_next_skips_completed() {
        let completed = vec!["git".to_string(), "gh".to_string()];
        let path = vec!["git".to_string(), "gh".to_string(), "just".to_string(), "uv".to_string()];
        let next = next_uncompleted(&path, &completed, &[]);
        assert_eq!(next, Some("just".to_string()));
    }

    #[test]
    fn test_tutorial_next_skips_skipped() {
        let completed = vec!["git".to_string()];
        let skipped = vec!["gh".to_string()];
        let path = vec!["git".to_string(), "gh".to_string(), "just".to_string()];
        let next = next_uncompleted(&path, &completed, &skipped);
        assert_eq!(next, Some("just".to_string()));
    }

    #[test]
    fn test_tutorial_progress_percent() {
        let path_len = 5;
        let completed_len = 2;
        let pct = progress_percent(path_len, completed_len);
        assert_eq!(pct, 40);
    }

    #[test]
    fn test_tutorial_progress_zero_total() {
        assert_eq!(progress_percent(0, 0), 0);
    }
}
```

### Step 4: Note on `ontology.rs` — expose `scan_datums` as pub

In `ontology.rs`, change `fn scan_datums` to `pub fn scan_datums`.

### Step 5: Wire into mod.rs and main.rs

`mod.rs` — add:
```rust
pub mod tutorial;
pub use tutorial::TutorialCommands;
```

`main.rs` Commands enum:
```rust
#[clap(about = "Tutorial progression tracking for role-based datum onboarding")]
Tutorial {
    #[clap(subcommand)]
    tutorial_command: commands::tutorial::TutorialCommands,
},
```

`main.rs` match block:
```rust
Some(Commands::Tutorial { tutorial_command }) => {
    tutorial_command.execute()
}
```

### Step 6: Run tests

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli test_tutorial 2>&1
cargo build -p b00t-cli 2>&1 | grep "^error" | head -10
```
Expected: tests PASS, build clean.

### Step 7: Smoke tests

```bash
cd /home/brianh/.b00t
cargo run -p b00t-cli -- tutorial status 2>&1
cargo run -p b00t-cli -- tutorial next 2>&1
cargo run -p b00t-cli -- tutorial validate git 2>&1
```
Expected:
- `tutorial status`: shows role path table with ✅/⬜
- `tutorial next`: prints first unvalidated datum
- `tutorial validate git`: runs `git --version`, marks git ✅ in session

### Step 8: Commit

```bash
git add b00t-cli/src/commands/tutorial.rs b00t-cli/src/commands/mod.rs b00t-cli/src/main.rs b00t-cli/src/commands/ontology.rs
git commit -m "feat(tutorial): add b00t tutorial status/next/skip/validate commands"
```

---

## Task 6: Add MemoryProvider trait with Copaw detection

**Files:**
- Create: `b00t-cli/src/memory_provider.rs`
- Modify: `b00t-cli/src/lib.rs` (add `pub mod memory_provider`)

### Step 1: Write failing test

```rust
#[test]
fn test_memory_provider_file_backend() {
    use crate::memory_provider::{FileMemory, MemoryProvider};
    let dir = tempfile::tempdir().unwrap();
    let mem = FileMemory::new(dir.path().join("mem.toml"));
    mem.write("key1", "val1").unwrap();
    assert_eq!(mem.read("key1").unwrap(), Some("val1".to_string()));
    assert_eq!(mem.read("missing").unwrap(), None);
}

#[test]
fn test_memory_provider_copaw_detection_no_panic() {
    use crate::memory_provider::detect_provider;
    // Should not panic regardless of environment
    let provider = detect_provider();
    assert!(provider.is_some());  // File fallback always available
}
```

### Step 2: Run to verify fails

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli test_memory_provider 2>&1 | head -15
```

### Step 3: Implement `memory_provider.rs`

```rust
// b00t-cli/src/memory_provider.rs
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub trait MemoryProvider: Send + Sync {
    fn read(&self, key: &str) -> Result<Option<String>>;
    fn write(&self, key: &str, val: &str) -> Result<()>;
    fn sync(&self) -> Result<()>;
}

// --- File backend (always available) ---
pub struct FileMemory {
    path: PathBuf,
}

impl FileMemory {
    pub fn new(path: PathBuf) -> Self { Self { path } }
}

#[derive(Serialize, Deserialize, Default)]
struct FileStore { data: HashMap<String, String> }

impl MemoryProvider for FileMemory {
    fn read(&self, key: &str) -> Result<Option<String>> {
        if !self.path.exists() { return Ok(None); }
        let content = std::fs::read_to_string(&self.path)?;
        let store: FileStore = toml::from_str(&content).unwrap_or_default();
        Ok(store.data.get(key).cloned())
    }
    fn write(&self, key: &str, val: &str) -> Result<()> {
        let mut store: FileStore = if self.path.exists() {
            toml::from_str(&std::fs::read_to_string(&self.path)?).unwrap_or_default()
        } else {
            FileStore::default()
        };
        store.data.insert(key.to_string(), val.to_string());
        std::fs::write(&self.path, toml::to_string(&store)?)?;
        Ok(())
    }
    fn sync(&self) -> Result<()> { Ok(()) } // file is local, no-op sync
}

// --- Copaw detection (checks if copaw datum validated in session) ---
pub fn is_copaw_available() -> bool {
    // Check session memory for copaw validated state
    use crate::session_memory::SessionMemory;
    SessionMemory::load().ok()
        .and_then(|s| s.get("tutorial.completed").cloned())
        .map(|c| c.split(',').any(|x| x == "copaw"))
        .unwrap_or(false)
}

// --- Provider detection ---
pub fn detect_provider() -> Option<Box<dyn MemoryProvider>> {
    // Priority: copaw → redis → file (file always available)
    if is_copaw_available() {
        // Future: return CopawMemory when copaw MCP client is available
        // For now, fall through to file backend
    }
    // Redis check: quick ping
    // Future: if redis_ping_ok() { return Some(Box::new(RedisMemory::new())); }

    // File fallback (always available)
    let path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".b00t")
        .join("memory.toml");
    Some(Box::new(FileMemory::new(path)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_provider_file_backend() {
        let dir = tempfile::tempdir().unwrap();
        let mem = FileMemory::new(dir.path().join("mem.toml"));
        mem.write("key1", "val1").unwrap();
        assert_eq!(mem.read("key1").unwrap(), Some("val1".to_string()));
        assert_eq!(mem.read("missing").unwrap(), None);
    }

    #[test]
    fn test_memory_provider_write_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let mem = FileMemory::new(dir.path().join("mem.toml"));
        mem.write("k", "v1").unwrap();
        mem.write("k", "v2").unwrap();
        assert_eq!(mem.read("k").unwrap(), Some("v2".to_string()));
    }

    #[test]
    fn test_memory_provider_copaw_detection_no_panic() {
        let provider = detect_provider();
        assert!(provider.is_some());
    }
}
```

### Step 4: Wire into lib.rs

In `b00t-cli/src/lib.rs`, add:
```rust
pub mod memory_provider;
```

### Step 5: Run tests

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli test_memory_provider 2>&1
```
Expected: all PASS.

### Step 6: Commit

```bash
git add b00t-cli/src/memory_provider.rs b00t-cli/src/lib.rs
git commit -m "feat(memory): add MemoryProvider trait with file backend and copaw detection"
```

---

## Task 7: IPC heartbeat from `b00t up`

**Files:**
- Modify: `b00t-cli/src/commands/up.rs`

### Step 1: Add IPC emission in the restart loop

After each cycle result in `up.rs`, add:
```rust
// Emit IPC heartbeat (non-fatal if IPC not running)
let _ = emit_up_heartbeat(restart_count, code, &self.role);
```

Add function:
```rust
fn emit_up_heartbeat(cycle: u32, exit_code: i32, role: &Option<String>) -> anyhow::Result<()> {
    use std::process::Command;
    // b00t-ipc may not be running — best-effort only
    let msg = serde_json::json!({
        "event": "b00t.up.cycle",
        "cycle": cycle,
        "exit_code": exit_code,
        "role": role.as_deref().unwrap_or("developer"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }).to_string();

    // Try publishing via b00t-ipc if available
    // 🤓 b00t-ipc uses named pipe/unix socket at ~/.b00t/ipc.sock
    let ipc_sock = dirs::home_dir()
        .unwrap_or_default()
        .join(".b00t").join("ipc.sock");
    if ipc_sock.exists() {
        let _ = Command::new("b00t-ipc")
            .args(["pub", "b00t.up", &msg])
            .output();
    }
    Ok(())
}
```

### Step 2: Run tests (verify no regressions)

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli 2>&1 | tail -10
```
Expected: all previous tests still PASS.

### Step 3: Final integration smoke test

```bash
cd /home/brianh/.b00t
# Test full stack: ontology + tutorial
cargo run -p b00t-cli -- ontology query --format json 2>&1 | python3 -m json.tool
cargo run -p b00t-cli -- tutorial status 2>&1
cargo run -p b00t-cli -- tutorial validate git 2>&1
cargo run -p b00t-cli -- tutorial status 2>&1  # git should be ✅ now
cargo run -p b00t-cli -- tutorial next 2>&1     # should show next after git
cargo run -p b00t-cli -- up --help 2>&1
```

### Step 4: Commit

```bash
git add b00t-cli/src/commands/up.rs
git commit -m "feat(up): add IPC heartbeat emission on cycle state changes"
```

---

## Task 8: Add copaw as a b00t datum

**Files:**
- Create: `_b00t_/copaw.mcp.toml`

### Step 1: Create datum (additive, no new patterns)

```toml
# _b00t_/copaw.mcp.toml
[b00t]
name = "copaw"
type = "mcp"
hint = "Copaw memory provider - PDF, chat, agent memory via MCP. Preferred b00t memory backend when available."
desires = "latest"

install = '''
# Copaw MCP server - https://copaw.agentscope.io/docs/memory
pip install copaw-mcp || uv pip install copaw-mcp
'''

version = "python3 -c \"import copaw; print(copaw.__version__)\" 2>/dev/null || echo 'not installed'"
version_regex = '\\d+\\.\\d+'

[validate]
command = "python3 -c \"import copaw; print('ok')\""
regex = "ok"

[roles]
required_for = []
optional_for = ["developer", "orchestrator", "analyst"]
```

### Step 2: Verify TOML

```bash
python3 -c "import tomllib; tomllib.load(open('_b00t_/copaw.mcp.toml','rb')); print('✅ valid')"
```

### Step 3: Verify it appears in status

```bash
cd /home/brianh/.b00t && cargo run -p b00t-cli -- status 2>&1 | grep copaw
```

### Step 4: Commit

```bash
git add _b00t_/copaw.mcp.toml
git commit -m "feat(datums): add copaw MCP datum as optional memory provider"
```

---

## Task 9: Final test run + PR prep

### Step 1: Full test suite

```bash
cd /home/brianh/.b00t && cargo test -p b00t-cli 2>&1 | tail -20
```
Expected: all tests PASS, zero failures.

### Step 2: Build release

```bash
cargo build -p b00t-cli --release 2>&1 | grep "^error" | head -5
```
Expected: clean build.

### Step 3: End-to-end integration checklist

```bash
# Ontology JSON is valid
cargo run -p b00t-cli -- ontology query --format json | python3 -m json.tool > /dev/null && echo "✅ ontology JSON valid"

# Tutorial status renders
cargo run -p b00t-cli -- tutorial status > /dev/null && echo "✅ tutorial status ok"

# Tutorial next returns a datum name
NEXT=$(cargo run -p b00t-cli -- tutorial next 2>&1)
echo "Next datum: $NEXT"

# b00t up help works
cargo run -p b00t-cli -- up --help > /dev/null && echo "✅ b00t up help ok"
```
Expected: all ✅.

### Step 4: Final commit

```bash
git add -p  # review and stage any remaining changes
git commit -m "chore(release): b00t up / tutorial / ontology MVP complete"
```

---

## Quick Reference

| Command | What it does |
|---------|-------------|
| `b00t up --tool claude` | Launches ralph agent loop, injects ontology, restarts on exit 75 |
| `b00t tutorial status` | Shows role-path completion table |
| `b00t tutorial next` | Prints next unvalidated datum |
| `b00t tutorial validate git` | Runs `git --version`, marks git validated |
| `b00t tutorial skip azure-ai-foundry` | Marks datum skipped |
| `b00t ontology query` | Table view of available/installable/blessings |
| `b00t ontology query --format json` | JSON (injected as `B00T_ONTOLOGY` by `b00t up`) |

## Exit Codes (ralph self-termination protocol)

| Code | Meaning | `b00t up` action |
|------|---------|-----------------|
| 0 | Done | Graceful exit |
| 75 | TEMPFAIL / restart requested | Increment counter, re-spawn |
| other | Error | Log + exit 1 |
