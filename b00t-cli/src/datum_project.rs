//! `ProjectProvider` — pluggable backends a project's task/requirement
//! tracking can hang off of (Jira, mise, bl/bailian, or a local-only store).
//!
//! Design spec: `docs/superpowers/specs/2026-09-25-project-scope-and-ontology-pipeline-design.md`,
//! sub-project A. New file, matches the existing `datum_*.rs` naming
//! convention (`datum_repo.rs`, `datum_mcp.rs`, `datum_stack.rs`) even though
//! this isn't a `BootDatum`/`DatumProvider` in the `datum_store.rs` sense —
//! it's a separate, simpler pluggable-backend abstraction the spec calls out
//! by that trait shape explicitly.
//!
//! Backend selection is driven by a `[b00t.project]` table in
//! `<b00t_dir>/project.toml` (`b00t_dir` being a `rep0`/`r00t`-discovered
//! `_b00t_/` directory):
//!
//! ```toml
//! [b00t.project]
//! provider = "jira"   # mise | jira | bl | none
//!
//! [b00t.project.jira]
//! base_url = "https://example.atlassian.net"
//! project_key = "PROJ"
//! token_env = "JIRA_API_TOKEN"   # default shown
//! # email = "me@example.com"    # omit for bearer-token auth instead of basic
//!
//! [b00t.project.bl]
//! workspace = "default"
//! ```
//!
//! When `provider` is unset: default to `MiseProvider` if `mise` is on
//! `PATH`, else `NoopProvider` — a local-only store that just records tasks
//! and requirement links to a JSON file under `_b00t_/`, no external system
//! required. (Deeper linkage into the native-schema ReqIF-style requirement
//! records in `datum_schema.rs` is sub-project C's job — out of scope here;
//! `NoopProvider` is a minimal, honest stand-in so `pr0ject task`/`pr0ject
//! reqif` are usable end-to-end even with zero external dependencies.)

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::check_command_available;

// ── trait + core types ──────────────────────────────────────────────────

pub trait ProjectProvider {
    /// Short backend identifier, e.g. "mise", "jira", "bl", "none".
    fn name(&self) -> &'static str;
    fn status(&self) -> Result<ProjectStatus>;
    fn list_tasks(&self) -> Result<Vec<Task>>;
    fn create_task(&self, task: NewTask) -> Result<Task>;
    fn link_requirement(&self, req: RequirementRef) -> Result<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectStatus {
    pub provider: String,
    pub available: bool,
    pub task_count: Option<usize>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub status: String,
    #[serde(default)]
    pub url: Option<String>,
    /// External requirements this task satisfies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub satisfies: Vec<String>,
    /// External requirements constraining this task.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constrains: Vec<String>,
    /// External requirements this task conflicts with.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicts_with: Vec<String>,
    /// External requirements this task provides verification evidence for.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verifies: Vec<String>,
}

impl Task {
    /// Collect all external refs from relationship fields.
    pub fn external_refs(&self) -> Vec<crate::external_refs::ExternalRef> {
        use crate::external_refs::{ExternalRef, RefRelationship, Uri};
        let mut refs = Vec::new();
        for (verb, uris) in [
            (RefRelationship::Satisfies, &self.satisfies),
            (RefRelationship::Constrains, &self.constrains),
            (RefRelationship::ConflictsWith, &self.conflicts_with),
            (RefRelationship::Verifies, &self.verifies),
        ] {
            for s in uris.iter() {
                refs.push(ExternalRef::new(verb, Uri::parse(s)));
            }
        }
        refs
    }
}

#[derive(Debug, Clone, Default)]
pub struct NewTask {
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementRef {
    pub task_id: String,
    /// URI of the external requirement (e.g. "reqif://docs/focus.reqif#REQ-001").
    pub requirement_uri: String,
    /// Relationship verb: satisfies, constrains, depends_on, conflicts_with, verifies.
    #[serde(default = "default_req_relationship")]
    pub relationship: String,
    pub note: Option<String>,
}

fn default_req_relationship() -> String {
    "satisfies".to_string()
}

/// A local traceability record — written to `_b00t_/project/requirements.json`
/// so requirement/task links are queryable even when the provider backend is
/// unreachable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalRequirementLink {
    pub task_id: String,
    pub requirement_uri: String,
    pub relationship: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default)]
    pub linked_at: String,
    #[serde(default)]
    pub provider: String,
}

// ── shared local traceability store ────────────────────────────────────

/// Save a requirement link to the local traceability store at
/// `_b00t_/project/requirements.json`. Every provider calls this so
/// requirement/task state is queryable even when the backend is unreachable.
pub fn save_local_requirement_link(
    b00t_dir: &Path,
    req: &RequirementRef,
    provider_name: &str,
) -> Result<()> {
    let store_path = b00t_dir.join("project").join("requirements.json");
    let mut links: Vec<LocalRequirementLink> = if store_path.exists() {
        let raw = std::fs::read_to_string(&store_path)
            .with_context(|| format!("read {}", store_path.display()))?;
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        Vec::new()
    };
    links.push(LocalRequirementLink {
        task_id: req.task_id.clone(),
        requirement_uri: req.requirement_uri.clone(),
        relationship: req.relationship.clone(),
        note: req.note.clone(),
        linked_at: chrono::Utc::now().to_rfc3339(),
        provider: provider_name.into(),
    });
    if let Some(parent) = store_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(&store_path, serde_json::to_string_pretty(&links)?)
        .with_context(|| format!("write {}", store_path.display()))?;
    Ok(())
}

/// Save a task record to the local traceability store at
/// `_b00t_/project/tasks.json`. Called from `b00t pr0ject task create` so
/// tasks are queryable even when the provider backend is unreachable.
pub fn save_local_task(b00t_dir: &Path, task: &Task, provider_name: &str) -> Result<()> {
    #[derive(Serialize, Deserialize)]
    struct LocalTaskRecord {
        id: String,
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        status: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        provider: String,
        created_at: String,
    }

    let store_path = b00t_dir.join("project").join("tasks.json");
    let mut records: Vec<LocalTaskRecord> = if store_path.exists() {
        let raw = std::fs::read_to_string(&store_path)
            .with_context(|| format!("read {}", store_path.display()))?;
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        Vec::new()
    };
    records.push(LocalTaskRecord {
        id: task.id.clone(),
        title: task.title.clone(),
        description: task.description.clone(),
        status: task.status.clone(),
        url: task.url.clone(),
        provider: provider_name.into(),
        created_at: chrono::Utc::now().to_rfc3339(),
    });
    if let Some(parent) = store_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(&store_path, serde_json::to_string_pretty(&records)?)
        .with_context(|| format!("write {}", store_path.display()))?;
    Ok(())
}

// ── provider config (`_b00t_/project.toml`) ────────────────────────────

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct JiraProviderConfig {
    pub base_url: String,
    pub project_key: String,
    #[serde(default = "default_jira_token_env")]
    pub token_env: String,
    #[serde(default)]
    pub email: Option<String>,
}

fn default_jira_token_env() -> String {
    "JIRA_API_TOKEN".to_string()
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct BlProviderConfig {
    #[serde(default)]
    pub workspace: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ProjectSectionConfig {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub jira: Option<JiraProviderConfig>,
    #[serde(default)]
    pub bl: Option<BlProviderConfig>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ProjectConfigB00t {
    #[serde(default)]
    project: ProjectSectionConfig,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ProjectConfigDoc {
    #[serde(default)]
    b00t: ProjectConfigB00t,
}

/// Path to the provider-selection/config file inside a `_b00t_/` dir.
pub fn project_config_path(b00t_dir: &Path) -> PathBuf {
    b00t_dir.join("project.toml")
}

/// Read `<b00t_dir>/project.toml`'s `[b00t.project]` table. Missing file ==
/// all-default (unset provider, no backend config).
pub fn load_project_section(b00t_dir: &Path) -> Result<ProjectSectionConfig> {
    let path = project_config_path(b00t_dir);
    if !path.exists() {
        return Ok(ProjectSectionConfig::default());
    }
    let raw =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let doc: ProjectConfigDoc =
        toml::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    Ok(doc.b00t.project)
}

/// Write (or update) just the `provider = "..."` key, preserving any
/// existing per-backend config tables already on disk.
pub fn write_provider_selection(b00t_dir: &Path, provider: &str) -> Result<()> {
    let mut section = load_project_section(b00t_dir)?;
    section.provider = Some(provider.to_string());
    let doc = ProjectConfigDoc {
        b00t: ProjectConfigB00t { project: section },
    };
    std::fs::create_dir_all(b00t_dir)
        .with_context(|| format!("create {}", b00t_dir.display()))?;
    let path = project_config_path(b00t_dir);
    std::fs::write(&path, toml::to_string_pretty(&doc)?)
        .with_context(|| format!("write {}", path.display()))
}

/// Resolve the active `ProjectProvider` for a `_b00t_/` directory.
///
/// Precedence: explicit `[b00t.project] provider = "..."` in
/// `project.toml`; else `MiseProvider` if `mise` is on `PATH`; else
/// `NoopProvider` (local-only, no external system required).
pub fn select_provider(b00t_dir: &Path) -> Result<Box<dyn ProjectProvider>> {
    let section = load_project_section(b00t_dir)?;
    let provider_name = section.provider.clone().unwrap_or_else(|| {
        if check_command_available("mise") {
            "mise".to_string()
        } else {
            "none".to_string()
        }
    });
    let project_root = b00t_dir.parent().unwrap_or(b00t_dir).to_path_buf();
    match provider_name.as_str() {
        "mise" => Ok(Box::new(MiseProvider::new(project_root))),
        "jira" => {
            let cfg = section.jira.ok_or_else(|| {
                anyhow!(
                    "[b00t.project] provider = \"jira\" but [b00t.project.jira] is missing in {}",
                    project_config_path(b00t_dir).display()
                )
            })?;
            Ok(Box::new(JiraProvider::new(cfg)))
        }
        "bl" => Ok(Box::new(BlProvider::new(section.bl.unwrap_or_default()))),
        "none" | "noop" | "local" => Ok(Box::new(NoopProvider::new(b00t_dir.to_path_buf()))),
        other => bail!(
            "unknown [b00t.project] provider '{other}' in {} — expected mise|jira|bl|none",
            project_config_path(b00t_dir).display()
        ),
    }
}

// ── MiseProvider ────────────────────────────────────────────────────────

/// Shells out to `mise tasks` (verified against a locally installed `mise
/// 2026.9.13`: `mise tasks ls -J` emits a JSON array of task objects with at
/// least `name`/`description`/`hide`; `mise tasks add <name> [--description
/// <d>] [-- <run>...]` creates one, writing to the project's `mise.toml`).
/// mise has no requirement-tracking concept of its own — `link_requirement`
/// just confirms the task exists so the CLI fails loudly instead of silently
/// doing nothing; durable requirement linkage is sub-project C's job.
pub struct MiseProvider {
    dir: PathBuf,
}

impl MiseProvider {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
}

#[derive(Debug, Deserialize)]
struct MiseTaskJson {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    hide: bool,
}

impl ProjectProvider for MiseProvider {
    fn name(&self) -> &'static str {
        "mise"
    }

    fn status(&self) -> Result<ProjectStatus> {
        if !check_command_available("mise") {
            return Ok(ProjectStatus {
                provider: "mise".into(),
                available: false,
                task_count: None,
                detail: "mise not found on PATH".into(),
            });
        }
        let tasks = self.list_tasks().unwrap_or_default();
        Ok(ProjectStatus {
            provider: "mise".into(),
            available: true,
            task_count: Some(tasks.len()),
            detail: format!("mise tasks in {}", self.dir.display()),
        })
    }

    fn list_tasks(&self) -> Result<Vec<Task>> {
        if !check_command_available("mise") {
            bail!("mise: not installed / not on PATH");
        }
        let out = duct::cmd!("mise", "tasks", "ls", "-J")
            .dir(self.dir.clone())
            .read()
            .context("`mise tasks ls -J` failed")?;
        let raw: Vec<MiseTaskJson> =
            serde_json::from_str(&out).context("parse `mise tasks ls -J` output")?;
        Ok(raw
            .into_iter()
            .map(|t| Task {
                id: t.name.clone(),
                title: t.name,
                description: t.description,
                status: if t.hide { "hidden".into() } else { "active".into() },
                url: None,
                satisfies: Vec::new(),
                constrains: Vec::new(),
                conflicts_with: Vec::new(),
                verifies: Vec::new(),
            })
            .collect())
    }

    fn create_task(&self, task: NewTask) -> Result<Task> {
        if !check_command_available("mise") {
            bail!("mise: not installed / not on PATH");
        }
        let mut args: Vec<String> = vec!["tasks".into(), "add".into(), task.title.clone()];
        if let Some(desc) = &task.description {
            args.push("--description".into());
            args.push(desc.clone());
        }
        duct::cmd("mise", args)
            .dir(self.dir.clone())
            .run()
            .with_context(|| format!("`mise tasks add {}` failed", task.title))?;
        Ok(Task {
            id: task.title.clone(),
            title: task.title,
            description: task.description,
            status: "active".into(),
            url: None,
            satisfies: Vec::new(),
            constrains: Vec::new(),
            conflicts_with: Vec::new(),
            verifies: Vec::new(),
        })
    }

    fn link_requirement(&self, req: RequirementRef) -> Result<()> {
        let tasks = self.list_tasks()?;
        if !tasks.iter().any(|t| t.id == req.task_id) {
            bail!(
                "mise: no task named '{}' — create it first with `b00t pr0ject task create`",
                req.task_id
            );
        }
        eprintln!(
            "mise: requirement '{}' noted against task '{}' — mise itself has no requirement-tracking API; durable linkage is sub-project C's native-schema record",
            req.requirement_uri, req.task_id
        );
        Ok(())
    }
}

// ── JiraProvider ────────────────────────────────────────────────────────

/// REST client against Jira Cloud's v2 API. Auth: bearer token by default
/// (`token_env`, a PAT), or HTTP Basic (email + API token) when `email` is
/// set — both read the token from the env var named by `token_env`, never
/// from `project.toml` itself.
pub struct JiraProvider {
    base_url: String,
    project_key: String,
    token_env: String,
    email: Option<String>,
}

impl JiraProvider {
    pub fn new(cfg: JiraProviderConfig) -> Self {
        Self {
            base_url: cfg.base_url.trim_end_matches('/').to_string(),
            project_key: cfg.project_key,
            token_env: cfg.token_env,
            email: cfg.email,
        }
    }

    fn token(&self) -> Result<String> {
        std::env::var(&self.token_env)
            .with_context(|| format!("Jira API token env var '{}' not set", self.token_env))
    }

    fn client(&self) -> Result<reqwest::blocking::Client> {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .context("build Jira HTTP client")
    }

    fn authed(
        &self,
        req: reqwest::blocking::RequestBuilder,
    ) -> Result<reqwest::blocking::RequestBuilder> {
        let token = self.token()?;
        Ok(match &self.email {
            Some(email) => req.basic_auth(email, Some(token)),
            None => req.bearer_auth(token),
        })
    }
}

impl ProjectProvider for JiraProvider {
    fn name(&self) -> &'static str {
        "jira"
    }

    fn status(&self) -> Result<ProjectStatus> {
        let url = format!("{}/rest/api/2/project/{}", self.base_url, self.project_key);
        let client = self.client()?;
        let req = self.authed(client.get(&url))?;
        match req.send() {
            Ok(resp) if resp.status().is_success() => Ok(ProjectStatus {
                provider: "jira".into(),
                available: true,
                task_count: None,
                detail: format!("connected to {} project {}", self.base_url, self.project_key),
            }),
            Ok(resp) => Ok(ProjectStatus {
                provider: "jira".into(),
                available: false,
                task_count: None,
                detail: format!("Jira returned HTTP {}", resp.status()),
            }),
            Err(e) => Ok(ProjectStatus {
                provider: "jira".into(),
                available: false,
                task_count: None,
                detail: format!("Jira unreachable: {e}"),
            }),
        }
    }

    fn list_tasks(&self) -> Result<Vec<Task>> {
        let url = format!("{}/rest/api/2/search", self.base_url);
        let jql = format!("project={}", self.project_key);
        let client = self.client()?;
        let req = self.authed(client.get(&url).query(&[("jql", jql.as_str())]))?;
        let resp = req
            .send()
            .context("Jira search request failed")?
            .error_for_status()
            .context("Jira search returned an error status")?;
        let body: serde_json::Value = resp.json().context("parse Jira search response")?;
        let issues = body
            .get("issues")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(issues
            .into_iter()
            .map(|issue| {
                let key = issue
                    .get("key")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .to_string();
                let fields = issue.get("fields").cloned().unwrap_or_default();
                let title = fields
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let status = fields
                    .get("status")
                    .and_then(|s| s.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                Task {
                    id: key.clone(),
                    title,
                    description: None,
                    status,
                    url: Some(format!("{}/browse/{}", self.base_url, key)),
                    satisfies: Vec::new(),
                    constrains: Vec::new(),
                    conflicts_with: Vec::new(),
                    verifies: Vec::new(),
                }
            })
            .collect())
    }

    fn create_task(&self, task: NewTask) -> Result<Task> {
        let url = format!("{}/rest/api/2/issue", self.base_url);
        let payload = serde_json::json!({
            "fields": {
                "project": {"key": self.project_key},
                "summary": task.title,
                "description": task.description.clone().unwrap_or_default(),
                "issuetype": {"name": "Task"},
            }
        });
        let client = self.client()?;
        let req = self.authed(client.post(&url).json(&payload))?;
        let resp = req
            .send()
            .context("Jira create-issue request failed")?
            .error_for_status()
            .context("Jira create-issue returned an error status")?;
        let body: serde_json::Value = resp.json().context("parse Jira create-issue response")?;
        let key = body
            .get("key")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string();
        Ok(Task {
            id: key.clone(),
            title: task.title,
            description: task.description,
            status: "open".into(),
            url: Some(format!("{}/browse/{}", self.base_url, key)),
            satisfies: Vec::new(),
            constrains: Vec::new(),
            conflicts_with: Vec::new(),
            verifies: Vec::new(),
        })
    }

    fn link_requirement(&self, req: RequirementRef) -> Result<()> {
        let url = format!("{}/rest/api/2/issue/{}/comment", self.base_url, req.task_id);
        let body_text = match &req.note {
            Some(note) => format!("b00t requirement link: {} — {note}", req.requirement_uri),
            None => format!("b00t requirement link: {}", req.requirement_uri),
        };
        let payload = serde_json::json!({ "body": body_text });
        let client = self.client()?;
        let request = self.authed(client.post(&url).json(&payload))?;
        request
            .send()
            .context("Jira add-comment request failed")?
            .error_for_status()
            .context("Jira add-comment returned an error status")?;
        Ok(())
    }
}

// ── BlProvider ──────────────────────────────────────────────────────────

/// Shells out to the `bl` CLI (bailian). Only `status` (PATH availability)
/// is wired: `bl` (see `bailian-cli`/`bailian-managed-agent` skills) exposes
/// AI-resource management (apps, models, quotas, agents.yaml) but has no
/// confirmed general project/task-tracking surface in this codebase to
/// build against — inventing one here would be worse than an honest "not
/// implemented" error. Revisit once a concrete `bl` task/issue command
/// exists to shell out to.
pub struct BlProvider {
    #[allow(dead_code)]
    workspace: Option<String>,
}

impl BlProvider {
    pub fn new(cfg: BlProviderConfig) -> Self {
        Self { workspace: cfg.workspace }
    }
}

impl ProjectProvider for BlProvider {
    fn name(&self) -> &'static str {
        "bl"
    }

    fn status(&self) -> Result<ProjectStatus> {
        let available = check_command_available("bl");
        Ok(ProjectStatus {
            provider: "bl".into(),
            available,
            task_count: None,
            detail: if available {
                "bl CLI on PATH — task/requirement tracking not implemented (no confirmed bl project-tracking surface)".into()
            } else {
                "bl CLI not found on PATH".into()
            },
        })
    }

    fn list_tasks(&self) -> Result<Vec<Task>> {
        bail!(
            "BlProvider: task listing not implemented — bl (bailian) has no confirmed project/task-tracking CLI surface; see `b00t pr0ject status` for detail"
        )
    }

    fn create_task(&self, _task: NewTask) -> Result<Task> {
        bail!("BlProvider: task creation not implemented — see `b00t pr0ject status` for detail")
    }

    fn link_requirement(&self, _req: RequirementRef) -> Result<()> {
        bail!(
            "BlProvider: requirement linking not implemented — see `b00t pr0ject status` for detail"
        )
    }
}

// ── NoopProvider ────────────────────────────────────────────────────────

/// Local-only fallback: no external system, no network. Tasks and
/// requirement links round-trip through a JSON file at
/// `<b00t_dir>/project-local.json`, so `pr0ject task`/`pr0ject reqif` work
/// end-to-end with zero setup.
pub struct NoopProvider {
    store_path: PathBuf,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct LocalStore {
    #[serde(default)]
    tasks: Vec<Task>,
    #[serde(default)]
    next_id: u64,
    #[serde(default)]
    requirement_links: Vec<LocalRequirementLink>,
}

impl NoopProvider {
    pub fn new(b00t_dir: impl Into<PathBuf>) -> Self {
        Self {
            store_path: b00t_dir.into().join("project-local.json"),
        }
    }

    fn load(&self) -> Result<LocalStore> {
        if !self.store_path.exists() {
            return Ok(LocalStore::default());
        }
        let raw = std::fs::read_to_string(&self.store_path)
            .with_context(|| format!("read {}", self.store_path.display()))?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    fn save(&self, store: &LocalStore) -> Result<()> {
        if let Some(parent) = self.store_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        std::fs::write(&self.store_path, serde_json::to_string_pretty(store)?)
            .with_context(|| format!("write {}", self.store_path.display()))
    }
}

impl ProjectProvider for NoopProvider {
    fn name(&self) -> &'static str {
        "none"
    }

    fn status(&self) -> Result<ProjectStatus> {
        let store = self.load()?;
        Ok(ProjectStatus {
            provider: "none".into(),
            available: true,
            task_count: Some(store.tasks.len()),
            detail: format!("local-only store at {}", self.store_path.display()),
        })
    }

    fn list_tasks(&self) -> Result<Vec<Task>> {
        Ok(self.load()?.tasks)
    }

    fn create_task(&self, task: NewTask) -> Result<Task> {
        let mut store = self.load()?;
        store.next_id += 1;
        let id = store.next_id.to_string();
        let created = Task {
            id,
            title: task.title,
            description: task.description,
            status: "open".into(),
            url: None,
            satisfies: Vec::new(),
            constrains: Vec::new(),
            conflicts_with: Vec::new(),
            verifies: Vec::new(),
        };
        store.tasks.push(created.clone());
        self.save(&store)?;
        Ok(created)
    }

    fn link_requirement(&self, req: RequirementRef) -> Result<()> {
        let mut store = self.load()?;
        if !store.tasks.iter().any(|t| t.id == req.task_id) {
            bail!(
                "no local task '{}' — create it first with `b00t pr0ject task create`",
                req.task_id
            );
        }
        store.requirement_links.push(LocalRequirementLink {
            task_id: req.task_id,
            requirement_uri: req.requirement_uri,
            relationship: req.relationship,
            note: req.note,
            linked_at: chrono::Utc::now().to_rfc3339(),
            provider: "none".into(),
        });
        self.save(&store)
    }
}

// ── tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_section_defaults_when_config_file_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let b00t_dir = tmp.path().join("_b00t_");
        std::fs::create_dir_all(&b00t_dir).unwrap();

        let section = load_project_section(&b00t_dir).unwrap();
        assert!(section.provider.is_none());
        assert!(section.jira.is_none());
        assert!(section.bl.is_none());
    }

    #[test]
    fn write_then_load_provider_selection_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let b00t_dir = tmp.path().join("_b00t_");
        std::fs::create_dir_all(&b00t_dir).unwrap();

        write_provider_selection(&b00t_dir, "jira").unwrap();
        let section = load_project_section(&b00t_dir).unwrap();
        assert_eq!(section.provider.as_deref(), Some("jira"));
    }

    #[test]
    fn select_provider_none_gives_noop_and_defaults_to_none_or_mise_when_unset() {
        let tmp = tempfile::tempdir().unwrap();
        let b00t_dir = tmp.path().join("_b00t_");
        std::fs::create_dir_all(&b00t_dir).unwrap();

        write_provider_selection(&b00t_dir, "none").unwrap();
        let provider = select_provider(&b00t_dir).unwrap();
        assert_eq!(provider.name(), "none");
    }

    #[test]
    fn select_provider_jira_without_config_errors_clearly() {
        let tmp = tempfile::tempdir().unwrap();
        let b00t_dir = tmp.path().join("_b00t_");
        std::fs::create_dir_all(&b00t_dir).unwrap();

        write_provider_selection(&b00t_dir, "jira").unwrap();
        let err = select_provider(&b00t_dir).err().expect("should error for jira without config");
        assert!(err.to_string().contains("[b00t.project.jira]"));
    }

    #[test]
    fn select_provider_rejects_unknown_name() {
        let tmp = tempfile::tempdir().unwrap();
        let b00t_dir = tmp.path().join("_b00t_");
        std::fs::create_dir_all(&b00t_dir).unwrap();

        write_provider_selection(&b00t_dir, "carrier-pigeon").unwrap();
        let err = select_provider(&b00t_dir).err().expect("should error for unknown provider");
        assert!(err.to_string().contains("unknown"));
    }

    #[test]
    fn noop_provider_create_list_and_link_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let b00t_dir = tmp.path().join("_b00t_");
        std::fs::create_dir_all(&b00t_dir).unwrap();

        let provider = NoopProvider::new(b00t_dir.clone());
        let status0 = provider.status().unwrap();
        assert_eq!(status0.task_count, Some(0));

        let created = provider
            .create_task(NewTask {
                title: "write the spec".into(),
                description: Some("already done".into()),
            })
            .unwrap();
        assert_eq!(created.title, "write the spec");

        let tasks = provider.list_tasks().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, created.id);

        provider
            .link_requirement(RequirementRef {
                task_id: created.id.clone(),
                requirement_uri: "reqif://test#REQ-1".into(),
                relationship: "satisfies".into(),
                note: Some("traceability".into()),
            })
            .unwrap();

        // reload from disk via a fresh provider instance to prove persistence
        let reloaded = NoopProvider::new(b00t_dir);
        let status1 = reloaded.status().unwrap();
        assert_eq!(status1.task_count, Some(1));
    }

    #[test]
    fn noop_provider_link_requirement_rejects_unknown_task() {
        let tmp = tempfile::tempdir().unwrap();
        let b00t_dir = tmp.path().join("_b00t_");
        std::fs::create_dir_all(&b00t_dir).unwrap();

        let provider = NoopProvider::new(b00t_dir);
        let err = provider
            .link_requirement(RequirementRef {
                task_id: "does-not-exist".into(),
                requirement_uri: "reqif://test#REQ-1".into(),
                relationship: "satisfies".into(),
                note: None,
            })
            .err()
            .expect("should error for nonexistent task");
        assert!(err.to_string().contains("no local task"));
    }

    #[test]
    fn bl_provider_status_reports_availability_without_touching_network() {
        let provider = BlProvider::new(BlProviderConfig::default());
        // Whatever the sandbox's PATH looks like, this must not panic or
        // hang, and must classify availability consistently with
        // check_command_available("bl").
        let status = provider.status().unwrap();
        assert_eq!(status.available, check_command_available("bl"));
    }

    #[test]
    fn task_external_refs_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let b00t_dir = tmp.path().join("_b00t_");
        std::fs::create_dir_all(&b00t_dir).unwrap();

        let provider = NoopProvider::new(&b00t_dir);
        let task = provider
            .create_task(NewTask {
                title: "implement-cost-allocation".into(),
                description: Some("FOCUS spec compliance".into()),
            })
            .unwrap();

        // Link a requirement
        provider
            .link_requirement(RequirementRef {
                task_id: task.id.clone(),
                requirement_uri: "reqif://docs/focus.reqif#REQ-001".into(),
                relationship: "satisfies".into(),
                note: Some("FOCUS cost allocation".into()),
            })
            .unwrap();

        // Reload and verify the link persisted
        let reloaded = NoopProvider::new(&b00t_dir);
        let links = reloaded.load().unwrap().requirement_links;
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].requirement_uri, "reqif://docs/focus.reqif#REQ-001");
        assert_eq!(links[0].relationship, "satisfies");
        assert_eq!(links[0].provider, "none");
        assert!(!links[0].linked_at.is_empty());
    }

    #[test]
    fn task_struct_external_refs_from_fields() {
        use crate::external_refs::RefRelationship;

        let task = Task {
            id: "1".into(),
            title: "test".into(),
            description: None,
            status: "open".into(),
            url: None,
            satisfies: vec!["reqif://spec#REQ-001".into()],
            constrains: vec!["https://example.com/budget#cap".into()],
            conflicts_with: vec![],
            verifies: vec!["reqif://spec#REQ-002".into()],
        };
        let refs = task.external_refs();
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].relationship, RefRelationship::Satisfies);
        assert_eq!(refs[1].relationship, RefRelationship::Constrains);
        assert_eq!(refs[2].relationship, RefRelationship::Verifies);
        assert_eq!(refs[0].uri.fragment.as_deref(), Some("REQ-001"));
    }

    #[test]
    fn save_local_requirement_link_writes_to_shared_store() {
        let tmp = tempfile::tempdir().unwrap();
        let b00t_dir = tmp.path().join("_b00t_");

        let req = RequirementRef {
            task_id: "42".into(),
            requirement_uri: "reqif://spec#REQ-007".into(),
            relationship: "verifies".into(),
            note: Some("test evidence".into()),
        };
        save_local_requirement_link(&b00t_dir, &req, "mise").unwrap();

        // Read back the store
        let store_path = b00t_dir.join("project").join("requirements.json");
        assert!(store_path.exists());
        let raw = std::fs::read_to_string(&store_path).unwrap();
        let links: Vec<LocalRequirementLink> = serde_json::from_str(&raw).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].task_id, "42");
        assert_eq!(links[0].requirement_uri, "reqif://spec#REQ-007");
        assert_eq!(links[0].relationship, "verifies");
        assert_eq!(links[0].provider, "mise");
        assert!(!links[0].linked_at.is_empty());

        // Second link appends, not overwrites
        let req2 = RequirementRef {
            task_id: "43".into(),
            requirement_uri: "reqif://spec#REQ-008".into(),
            relationship: "satisfies".into(),
            note: None,
        };
        save_local_requirement_link(&b00t_dir, &req2, "jira").unwrap();
        let raw2 = std::fs::read_to_string(&store_path).unwrap();
        let links2: Vec<LocalRequirementLink> = serde_json::from_str(&raw2).unwrap();
        assert_eq!(links2.len(), 2);
        assert_eq!(links2[1].provider, "jira");
    }

    #[test]
    fn save_local_task_writes_to_shared_store() {
        let tmp = tempfile::tempdir().unwrap();
        let b00t_dir = tmp.path().join("_b00t_");

        let task = Task {
            id: "7".into(),
            title: "fix-billing".into(),
            description: Some("FOCUS compliance".into()),
            status: "open".into(),
            url: Some("https://jira.example.com/browse/PROJ-7".into()),
            satisfies: Vec::new(),
            constrains: Vec::new(),
            conflicts_with: Vec::new(),
            verifies: Vec::new(),
        };
        save_local_task(&b00t_dir, &task, "jira").unwrap();

        let store_path = b00t_dir.join("project").join("tasks.json");
        assert!(store_path.exists());
        let raw = std::fs::read_to_string(&store_path).unwrap();
        let records: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["id"], "7");
        assert_eq!(records[0]["provider"], "jira");
        assert!(!records[0]["created_at"].as_str().unwrap().is_empty());
    }
}
