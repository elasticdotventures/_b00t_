//! `b00t pr0ject` — onboard a directory as a b00t project.
//!
//! Named `pr0ject` (leetspeak), NOT `project`, deliberately: `b00t project`
//! already exists (`src/commands/project.rs`) and means something unrelated
//! — a node-local git overlay-enclave for datum changesets. `pr0ject` is a
//! new metapattern per explicit user clarification while scoping this
//! feature: "a metapattern that does #1 [soul init]" — `pr0ject init` =
//! `rep0 init` (create `_b00t_/`) + `soul init` (create `._b00t_/`) +
//! `ProjectProvider` selection/registration, as one onboarding command for
//! "this directory is now a project."
//!
//! Deliberately NOT given a `visible_alias` toward `soul` or `project`:
//! neither existing command is a strict superset of what `pr0ject init`
//! does (an alias would misrepresent the composition), so it stays a
//! distinct, legible subcommand per the design spec's own guidance.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Parser;

use crate::datum_project::{self, NewTask, ProjectProvider, RequirementRef};

#[derive(Parser)]
pub enum Pr0jectCommands {
    #[clap(
        about = "Onboard this directory: rep0 init + soul init + ProjectProvider selection",
        long_about = "Creates ./_b00t_/ (rep0 init), ./._b00t_/ (soul init), and registers the\nactive ProjectProvider backend in ./_b00t_/project.toml. Idempotent — safe\nto re-run."
    )]
    Init {
        #[clap(
            long,
            help = "Force a specific provider (mise|jira|bl|none) instead of auto-detecting"
        )]
        provider: Option<String>,
    },
    #[clap(about = "Show the active ProjectProvider's status")]
    Status {
        #[clap(long, help = "Output as JSON")]
        json: bool,
    },
    #[clap(about = "Task operations against the active ProjectProvider")]
    Task {
        #[clap(subcommand)]
        task_command: Pr0jectTaskCommands,
    },
    #[clap(about = "Requirement linkage against the active ProjectProvider")]
    Reqif {
        #[clap(subcommand)]
        reqif_command: Pr0jectReqifCommands,
    },
}

#[derive(Parser)]
pub enum Pr0jectTaskCommands {
    #[clap(about = "List tasks from the active ProjectProvider")]
    List,
    #[clap(about = "Create a task via the active ProjectProvider")]
    Create {
        title: String,
        #[clap(long)]
        description: Option<String>,
    },
}

#[derive(Parser)]
pub enum Pr0jectReqifCommands {
    #[clap(about = "Link a requirement id to a task via the active ProjectProvider")]
    Link {
        #[clap(long)]
        task_id: String,
        #[clap(long)]
        requirement_id: String,
        #[clap(long)]
        note: Option<String>,
    },
}

pub fn handle_pr0ject_command(cmd: &Pr0jectCommands) -> Result<()> {
    match cmd {
        Pr0jectCommands::Init { provider } => pr0ject_init(provider.as_deref()),
        Pr0jectCommands::Status { json } => pr0ject_status(*json),
        Pr0jectCommands::Task { task_command } => match task_command {
            Pr0jectTaskCommands::List => pr0ject_task_list(),
            Pr0jectTaskCommands::Create { title, description } => {
                pr0ject_task_create(title, description.clone())
            }
        },
        Pr0jectCommands::Reqif { reqif_command } => match reqif_command {
            Pr0jectReqifCommands::Link {
                task_id,
                requirement_id,
                note,
            } => pr0ject_reqif_link(task_id, requirement_id, note.clone()),
        },
    }
}

/// `_b00t_/` under `base` specifically — `pr0ject` operates on the project
/// you're standing in, not an ancestor scope (walking up to a shared `r00t`
/// scope is what the `r00t` subcommand is explicitly for).
fn b00t_dir_under(base: &std::path::Path) -> Result<PathBuf> {
    let dir = base.join("_b00t_");
    if !dir.is_dir() {
        bail!("no _b00t_/ in the current directory — run `b00t pr0ject init` first");
    }
    Ok(dir)
}

fn current_b00t_dir() -> Result<PathBuf> {
    b00t_dir_under(&std::env::current_dir()?)
}

fn pr0ject_init(provider: Option<&str>) -> Result<()> {
    pr0ject_init_at(&std::env::current_dir()?, provider)
}

/// Core of `pr0ject init`, parameterized on the target directory so it's
/// testable without mutating the process's real cwd.
fn pr0ject_init_at(base: &std::path::Path, provider: Option<&str>) -> Result<()> {
    // 1. rep0 init — literal filesystem anchor
    super::rep0::rep0_init(base)?;
    let b00t_dir = base.join("_b00t_");

    // 2. soul init — workspace agentic identity
    super::soul::soul_init(base)?;

    // 3. ProjectProvider selection/registration
    let selected = match provider {
        Some(p) => p.to_string(),
        None => {
            if crate::check_command_available("mise") {
                "mise".to_string()
            } else {
                "none".to_string()
            }
        }
    };
    datum_project::write_provider_selection(&b00t_dir, &selected)?;
    println!(
        "pr0ject: provider = {selected} ({})",
        datum_project::project_config_path(&b00t_dir).display()
    );
    println!("pr0ject: this directory is now a b00t project.");
    Ok(())
}

fn pr0ject_status(json: bool) -> Result<()> {
    let b00t_dir = current_b00t_dir()?;
    let provider = datum_project::select_provider(&b00t_dir)?;
    let status = provider.status()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&status)?);
    } else {
        println!("provider:  {}", status.provider);
        println!("available: {}", status.available);
        if let Some(n) = status.task_count {
            println!("tasks:     {n}");
        }
        println!("detail:    {}", status.detail);
    }
    Ok(())
}

fn pr0ject_task_list() -> Result<()> {
    let b00t_dir = current_b00t_dir()?;
    let provider = datum_project::select_provider(&b00t_dir)?;
    let tasks = provider.list_tasks()?;
    if tasks.is_empty() {
        println!("(no tasks)");
        return Ok(());
    }
    for t in &tasks {
        println!("{:<12} [{}] {}", t.id, t.status, t.title);
    }
    Ok(())
}

fn pr0ject_task_create(title: &str, description: Option<String>) -> Result<()> {
    let b00t_dir = current_b00t_dir()?;
    let provider = datum_project::select_provider(&b00t_dir)?;
    let task = provider.create_task(NewTask {
        title: title.to_string(),
        description,
    })?;
    // Also write to local traceability store so tasks are queryable offline
    datum_project::save_local_task(&b00t_dir, &task, provider.name())?;
    println!("created task {} — {}", task.id, task.title);
    Ok(())
}

fn pr0ject_reqif_link(task_id: &str, requirement_id: &str, note: Option<String>) -> Result<()> {
    let b00t_dir = current_b00t_dir()?;
    let req = RequirementRef {
        task_id: task_id.to_string(),
        requirement_uri: requirement_id.to_string(),
        relationship: "satisfies".into(),
        note,
    };
    let provider = datum_project::select_provider(&b00t_dir)?;
    provider.link_requirement(req.clone())?;
    // Also write to local traceability store so state is queryable offline
    datum_project::save_local_requirement_link(&b00t_dir, &req, provider.name())?;
    println!("linked requirement {requirement_id} -> task {task_id}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // These exercise the `_at`/`_under` cores directly rather than the real
    // cwd-based CLI entry points, so they don't need to mutate (and
    // serialize on) the process's actual current directory.

    #[test]
    fn pr0ject_init_creates_both_dirs_and_registers_a_provider() {
        let tmp = tempfile::tempdir().unwrap();

        pr0ject_init_at(tmp.path(), Some("none")).unwrap();

        assert!(tmp.path().join("_b00t_").is_dir());
        assert!(tmp.path().join("._b00t_").is_dir());
        let section =
            datum_project::load_project_section(&tmp.path().join("_b00t_")).unwrap();
        assert_eq!(section.provider.as_deref(), Some("none"));
    }

    #[test]
    fn pr0ject_init_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();

        pr0ject_init_at(tmp.path(), Some("none")).unwrap();
        pr0ject_init_at(tmp.path(), Some("none")).unwrap();

        assert!(tmp.path().join("_b00t_").is_dir());
    }

    #[test]
    fn b00t_dir_under_errors_clearly_without_pr0ject_init() {
        let tmp = tempfile::tempdir().unwrap();

        let err = b00t_dir_under(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("pr0ject init"));
    }
}
