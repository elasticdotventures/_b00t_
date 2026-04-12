//! b00t task — native task management (replaces taskmaster-ai)
//!
//! Storage: `.b00t/tasks.json` at the git workspace root (via `get_workspace_root()`).
//! Schema: minimal CRUD — no AI expansion, no LLM deps, no cloud APIs.
//! Compat: `b00t task import` migrates from `.taskmaster/tasks/tasks.json`.
//!
//! 🤓 This is the extracted core of taskmaster-ai v0.x (before enshittification).
//!    Keep it lean: list/add/next/done/update/show/import — nothing else.

use anyhow::{ensure, Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

// ── Task schema ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    #[serde(rename = "in-progress")]
    InProgress,
    Done,
    Blocked,
    Deferred,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::InProgress => write!(f, "in-progress"),
            TaskStatus::Done => write!(f, "done"),
            TaskStatus::Blocked => write!(f, "blocked"),
            TaskStatus::Deferred => write!(f, "deferred"),
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "pending" | "p" => Ok(TaskStatus::Pending),
            "in-progress" | "in_progress" | "inprogress" | "wip" | "i" => Ok(TaskStatus::InProgress),
            "done" | "complete" | "completed" | "d" => Ok(TaskStatus::Done),
            "blocked" | "b" => Ok(TaskStatus::Blocked),
            "deferred" | "defer" | "skip" => Ok(TaskStatus::Deferred),
            _ => anyhow::bail!("unknown status '{}'; valid: pending|in-progress|done|blocked|deferred", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: u32,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: TaskStatus,
    /// 1=critical 2=high 3=medium 4=low (lower = more urgent, matches taskmaster convention)
    #[serde(default = "default_priority")]
    pub priority: u8,
    #[serde(default)]
    pub tags: Vec<String>,
    /// IDs of tasks that must be done before this one
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// Verifiable exit conditions (R5: GOAL.md fitness contract)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_criteria: Vec<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

fn default_priority() -> u8 { 3 }

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct TaskStore {
    pub tasks: Vec<Task>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

// ── Storage helpers ────────────────────────────────────────────────────────────

fn tasks_path() -> PathBuf {
    // B00T_TASKS_PATH env override — used in tests to avoid CWD conflicts
    if let Ok(p) = std::env::var("B00T_TASKS_PATH") {
        return PathBuf::from(p);
    }
    let local = PathBuf::from(".b00t/tasks.json");
    if local.parent().map(|p| p.exists()).unwrap_or(false) {
        return local;
    }
    local // will be created on first write
}

fn load_store() -> Result<TaskStore> {
    let path = tasks_path();
    if !path.exists() {
        // Transparent migration from .taskmaster — only when not overridden by env
        // 🤓 Skip legacy check in test mode (B00T_TASKS_PATH set) to avoid test pollution
        if std::env::var("B00T_TASKS_PATH").is_err() {
            let legacy = PathBuf::from(".taskmaster/tasks/tasks.json");
            if legacy.exists() {
                return import_from_legacy(&legacy);
            }
        }
        return Ok(TaskStore { tasks: vec![], version: Some("1".into()) });
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw)
        .with_context(|| format!("parse {}", path.display()))
}

fn save_store(store: &TaskStore) -> Result<()> {
    let path = tasks_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(store)?;
    fs::write(&path, json + "\n")
        .with_context(|| format!("write {}", path.display()))
}

fn now_iso() -> String {
    // Generate ISO-8601/RFC3339 UTC in-process to avoid shelling out to `date`.
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn next_id(store: &TaskStore) -> u32 {
    store.tasks.iter().map(|t| t.id).max().unwrap_or(0) + 1
}

// ── taskmaster compat import ───────────────────────────────────────────────────

fn import_from_legacy(path: &Path) -> Result<TaskStore> {
    let raw = fs::read_to_string(path)?;
    // taskmaster schema: { "tasks": [ { "id", "title", "description", "status", "priority", ... } ] }
    let v: serde_json::Value = serde_json::from_str(&raw)?;
    let arr = v["tasks"].as_array().cloned().unwrap_or_default();
    let ts = now_iso();
    let tasks: Vec<Task> = arr.iter().enumerate().map(|(i, t)| {
        let status_str = t["status"].as_str().unwrap_or("pending");
        let status = status_str.parse::<TaskStatus>().unwrap_or(TaskStatus::Pending);
        let priority_raw = t["priority"].as_str().unwrap_or("medium");
        let priority = match priority_raw.to_lowercase().as_str() {
            "critical" | "1" => 1,
            "high" | "2"     => 2,
            "medium" | "3"   => 3,
            "low" | "4"      => 4,
            _                => 3,
        };
        let deps: Vec<u32> = t["dependencies"].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_u64().map(|n| n as u32)).collect())
            .unwrap_or_default();
        Task {
            id: t["id"].as_u64().unwrap_or((i + 1) as u64) as u32,
            title: t["title"].as_str().unwrap_or("(untitled)").to_string(),
            description: t["description"].as_str().map(str::to_string),
            status,
            priority,
            tags: vec![],
            dependencies: deps,
            acceptance_criteria: vec![],
            notes: t["details"].as_str().map(str::to_string),
            created_at: ts.clone(),
            updated_at: None,
        }
    }).collect();
    Ok(TaskStore { tasks, version: Some("1".into()) })
}

// ── CLI definition ────────────────────────────────────────────────────────────

#[derive(Parser, Clone)]
pub enum TaskCommands {
    #[clap(about = "List tasks (default: pending + in-progress)")]
    List {
        #[clap(long, help = "Filter by status: pending|in-progress|done|blocked|all")]
        status: Option<String>,
        #[clap(long, help = "Filter by tag")]
        tag: Option<String>,
        #[clap(long, help = "Output as JSON")]
        json: bool,
    },
    #[clap(about = "Add a new task")]
    Add {
        #[clap(help = "Task title")]
        title: String,
        #[clap(long, short, help = "Description")]
        description: Option<String>,
        #[clap(long, short, help = "Priority 1-4 (1=critical, 4=low)", default_value = "3",
               value_parser = clap::value_parser!(u8).range(1..=4))]
        priority: u8,
        #[clap(long, short, help = "Tags (comma-separated)")]
        tags: Option<String>,
        #[clap(long, short = 'c', help = "Acceptance criteria (repeatable: -c 'tests pass' -c 'lint clean')")]
        criteria: Vec<String>,
    },
    #[clap(about = "Show next pending task (highest priority)")]
    Next {
        #[clap(long, help = "Output as JSON")]
        json: bool,
    },
    #[clap(about = "Mark task done")]
    Done {
        #[clap(help = "Task ID")]
        id: u32,
    },
    #[clap(about = "Update task status or fields")]
    Update {
        #[clap(help = "Task ID")]
        id: u32,
        #[clap(long, help = "New status: pending|in-progress|done|blocked|deferred")]
        status: Option<String>,
        #[clap(long, help = "New title")]
        title: Option<String>,
        #[clap(long, help = "Append to notes")]
        note: Option<String>,
        #[clap(long, help = "Priority 1-4",
               value_parser = clap::value_parser!(u8).range(1..=4))]
        priority: Option<u8>,
    },
    #[clap(about = "Show task details")]
    Show {
        #[clap(help = "Task ID")]
        id: u32,
    },
    #[clap(about = "Import tasks from .taskmaster/tasks/tasks.json (migration)")]
    Import {
        #[clap(help = "Source path (default: .taskmaster/tasks/tasks.json)")]
        path: Option<PathBuf>,
        #[clap(long, help = "Overwrite existing .b00t/tasks.json")]
        force: bool,
    },
    #[clap(about = "Remove a task")]
    Rm {
        #[clap(help = "Task ID")]
        id: u32,
    },
    #[clap(about = "Add or remove a dependency between tasks")]
    Dep {
        #[clap(help = "Task ID to modify")]
        id: u32,
        #[clap(subcommand)]
        op: DepOp,
    },
    #[clap(about = "Show task storage path")]
    Path,
}

#[derive(Parser, Clone)]
pub enum DepOp {
    #[clap(about = "Add dependency: task <id> will block until <dep> is done")]
    Add { dep: u32 },
    #[clap(about = "Remove dependency")]
    Rm { dep: u32 },
}

// ── Handlers ──────────────────────────────────────────────────────────────────

pub fn handle_task_command(cmd: TaskCommands) -> Result<()> {
    match cmd {
        TaskCommands::List { status, tag, json } => cmd_list(status.as_deref(), tag.as_deref(), json),
        TaskCommands::Add { title, description, priority, tags, criteria } => cmd_add(title, description, priority, tags, criteria),
        TaskCommands::Next { json } => cmd_next(json),
        TaskCommands::Done { id } => cmd_done(id),
        TaskCommands::Update { id, status, title, note, priority } => cmd_update(id, status, title, note, priority),
        TaskCommands::Show { id } => cmd_show(id),
        TaskCommands::Rm { id } => cmd_rm(id),
        TaskCommands::Dep { id, op } => cmd_dep(id, op),
        TaskCommands::Import { path, force } => cmd_import(path, force),
        TaskCommands::Path => { println!("{}", tasks_path().display()); Ok(()) },
    }
}

fn cmd_list(status_filter: Option<&str>, tag_filter: Option<&str>, json: bool) -> Result<()> {
    let store = load_store()?;
    let tasks: Vec<&Task> = store.tasks.iter().filter(|t| {
        let status_ok = match status_filter {
            None | Some("active") => matches!(t.status, TaskStatus::Pending | TaskStatus::InProgress),
            Some("all")      => true,
            Some("done")     => matches!(t.status, TaskStatus::Done),
            Some("pending")  => matches!(t.status, TaskStatus::Pending),
            Some("blocked")  => matches!(t.status, TaskStatus::Blocked),
            Some(s)          => t.status.to_string() == s,
        };
        let tag_ok = tag_filter.map_or(true, |f| t.tags.iter().any(|tg| tg == f));
        status_ok && tag_ok
    }).collect();

    if json {
        println!("{}", serde_json::to_string_pretty(&tasks)?);
        return Ok(());
    }

    if tasks.is_empty() {
        println!("no tasks (filter: {})", status_filter.unwrap_or("active"));
        return Ok(());
    }
    for t in &tasks {
        let tags = if t.tags.is_empty() { String::new() } else { format!(" [{}]", t.tags.join(",")) };
        println!("[{}] {:>2} P{} {}{}", t.status, t.id, t.priority, t.title, tags);
    }
    println!("  {} task(s)", tasks.len());
    Ok(())
}

fn cmd_add(title: String, description: Option<String>, priority: u8, tags_raw: Option<String>, criteria: Vec<String>) -> Result<()> {
    anyhow::ensure!(priority >= 1 && priority <= 4, "priority must be 1–4 (got {priority})");
    let mut store = load_store()?;
    let id = next_id(&store);
    let tags = tags_raw.map(|t| t.split(',').map(str::trim).map(str::to_string).collect()).unwrap_or_default();
    let task = Task {
        id, title: title.clone(), description, status: TaskStatus::Pending,
        priority, tags, dependencies: vec![], acceptance_criteria: criteria, notes: None, created_at: now_iso(), updated_at: None,
    };
    store.tasks.push(task);
    save_store(&store)?;
    println!("added #{id}: {title}");
    Ok(())
}

fn cmd_next(json: bool) -> Result<()> {
    let store = load_store()?;
    // Build set of done/deferred task IDs for dep checking
    let done_ids: std::collections::HashSet<u32> = store.tasks.iter()
        .filter(|t| matches!(t.status, TaskStatus::Done | TaskStatus::Deferred))
        .map(|t| t.id)
        .collect();
    // next = pending/in-progress with all deps satisfied, sorted by (priority, dep-count, id)
    let next = store.tasks.iter()
        .filter(|t| matches!(t.status, TaskStatus::Pending | TaskStatus::InProgress))
        .filter(|t| t.dependencies.iter().all(|dep| done_ids.contains(dep)))
        .min_by_key(|t| (t.priority, t.dependencies.len(), t.id));
    match next {
        None => { println!("no actionable tasks"); Ok(()) }
        Some(t) if json => { println!("{}", serde_json::to_string_pretty(t)?); Ok(()) }
        Some(t) => {
            println!("[{}] #{} P{} {}", t.status, t.id, t.priority, t.title);
            if let Some(d) = &t.description { println!("  {d}"); }
            if !t.dependencies.is_empty() {
                println!("  deps: {:?} (all satisfied)", t.dependencies);
            }
            Ok(())
        }
    }
}

fn cmd_rm(id: u32) -> Result<()> {
    let mut store = load_store()?;
    let pos = store.tasks.iter().position(|t| t.id == id)
        .with_context(|| format!("task #{id} not found"))?;
    let title = store.tasks[pos].title.clone();
    store.tasks.remove(pos);
    // Remove references from other tasks' dependency lists
    for t in &mut store.tasks {
        t.dependencies.retain(|&dep| dep != id);
    }
    save_store(&store)?;
    println!("removed #{id}: {title}");
    Ok(())
}

fn cmd_dep(id: u32, op: DepOp) -> Result<()> {
    let mut store = load_store()?;
    let task_exists = store.tasks.iter().any(|t| t.id == id);
    ensure!(task_exists, "task #{id} not found");

    match op {
        DepOp::Add { dep } => {
            ensure!(id != dep, "task #{id} cannot depend on itself");
            let dep_exists = store.tasks.iter().any(|t| t.id == dep);
            ensure!(dep_exists, "dependency task #{dep} not found");

            let task = store.tasks.iter_mut().find(|t| t.id == id)
                .with_context(|| format!("task #{id} not found"))?;
            if !task.dependencies.contains(&dep) {
                task.dependencies.push(dep);
                task.dependencies.sort();
            }
            task.updated_at = Some(now_iso());
            println!("#{id} now depends on #{dep}");
        }
        DepOp::Rm { dep } => {
            let task = store.tasks.iter_mut().find(|t| t.id == id)
                .with_context(|| format!("task #{id} not found"))?;
            task.dependencies.retain(|&d| d != dep);
            task.updated_at = Some(now_iso());
            println!("#{id}: removed dep #{dep}");
        }
    }
    save_store(&store)
}

fn cmd_done(id: u32) -> Result<()> {
    let mut store = load_store()?;
    let task = store.tasks.iter_mut().find(|t| t.id == id)
        .with_context(|| format!("task #{id} not found"))?;
    task.status = TaskStatus::Done;
    task.updated_at = Some(now_iso());
    let title = task.title.clone();
    save_store(&store)?;
    println!("done #{id}: {title}");
    Ok(())
}

fn cmd_update(id: u32, status: Option<String>, title: Option<String>, note: Option<String>, priority: Option<u8>) -> Result<()> {
    if let Some(p) = priority {
        anyhow::ensure!(p >= 1 && p <= 4, "priority must be 1–4 (got {p})");
    }
    let mut store = load_store()?;
    let task = store.tasks.iter_mut().find(|t| t.id == id)
        .with_context(|| format!("task #{id} not found"))?;
    if let Some(s) = status { task.status = s.parse()?; }
    if let Some(t) = title  { task.title = t; }
    if let Some(p) = priority { task.priority = p; }
    if let Some(n) = note {
        task.notes = Some(match &task.notes {
            None    => n,
            Some(e) => format!("{e}\n{n}"),
        });
    }
    task.updated_at = Some(now_iso());
    let title = task.title.clone();
    save_store(&store)?;
    println!("updated #{id}: {title}");
    Ok(())
}

fn cmd_show(id: u32) -> Result<()> {
    let store = load_store()?;
    let task = store.tasks.iter().find(|t| t.id == id)
        .with_context(|| format!("task #{id} not found"))?;
    println!("{}", serde_json::to_string_pretty(task)?);
    Ok(())
}

fn cmd_import(path: Option<PathBuf>, force: bool) -> Result<()> {
    let src = path.unwrap_or_else(|| PathBuf::from(".taskmaster/tasks/tasks.json"));
    let dest = tasks_path();
    if dest.exists() && !force {
        anyhow::bail!("{} already exists — use --force to overwrite", dest.display());
    }
    let imported = import_from_legacy(&src)?;
    let n = imported.tasks.len();
    save_store(&imported)?;
    println!("imported {n} tasks from {} → {}", src.display(), dest.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    // Serialize tests that touch B00T_TASKS_PATH env var — env vars are process-global
    static TASK_ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_tmp_store<F: FnOnce(&TempDir)>(f: F) {
        let _guard = TASK_ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let tasks_file = tmp.path().join("tasks.json");
        unsafe { std::env::set_var("B00T_TASKS_PATH", &tasks_file); }
        f(&tmp);
        unsafe { std::env::remove_var("B00T_TASKS_PATH"); }
    }

    #[test]
    fn test_add_and_list() {
        with_tmp_store(|_| {
            cmd_add("test task".into(), None, 3, None, vec![]).unwrap();
            let store = load_store().unwrap();
            assert_eq!(store.tasks.len(), 1);
            assert_eq!(store.tasks[0].title, "test task");
            assert!(matches!(store.tasks[0].status, TaskStatus::Pending));
        });
    }

    #[test]
    fn test_done() {
        with_tmp_store(|_| {
            cmd_add("finish me".into(), None, 2, None, vec![]).unwrap();
            cmd_done(1).unwrap();
            let store = load_store().unwrap();
            assert!(matches!(store.tasks[0].status, TaskStatus::Done));
        });
    }

    #[test]
    fn test_next_priority_order() {
        with_tmp_store(|_| {
            cmd_add("low prio".into(), None, 4, None, vec![]).unwrap();   // id=1
            cmd_add("high prio".into(), None, 1, None, vec![]).unwrap();  // id=2
            let store = load_store().unwrap();
            let next = store.tasks.iter()
                .filter(|t| matches!(t.status, TaskStatus::Pending))
                .min_by_key(|t| (t.priority, t.id))
                .unwrap();
            assert_eq!(next.id, 2);
        });
    }

    #[test]
    fn test_import_from_legacy() {
        with_tmp_store(|tmp| {
            let legacy = tmp.path().join("tasks.json");
            fs::write(&legacy, r#"{
                "tasks": [
                    {"id": 1, "title": "old task", "status": "pending", "priority": "high"},
                    {"id": 2, "title": "done task", "status": "done",   "priority": "low"}
                ]
            }"#).unwrap();
            let imported = import_from_legacy(&legacy).unwrap();
            assert_eq!(imported.tasks.len(), 2);
            assert!(matches!(imported.tasks[1].status, TaskStatus::Done));
        });
    }

    #[test]
    fn test_status_parse() {
        assert!("pending".parse::<TaskStatus>().is_ok());
        assert!("wip".parse::<TaskStatus>().is_ok());
        assert!("done".parse::<TaskStatus>().is_ok());
        assert!("bogus".parse::<TaskStatus>().is_err());
    }
}
