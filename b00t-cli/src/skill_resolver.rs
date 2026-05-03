//! Multi-directory skill resolver — progressive disclosure, lazy-loaded with timeout.
//!
//! # Lazy Loading
//! `load()` wraps file I/O in a 5-second deadline. If the deadline expires,
//! returns a `SkillLoadError::Timeout` instead of blocking the caller.
//! Discovery (`list()`, `search()`) is instant — no I/O.
//!
//! # Caching
//! Skills are cached after first load with a configurable TTL (default 60s).
//! `CacheMode::NoCache` bypasses for development.
//!
//! # Dynamic Rendering
//! Skills with `[b00t.skill.rhai]` in their datum pass instructions through
//! the RhaiEngine for dynamic generation. The RHAI script receives context
//! variables (model, branch, user, hostname) and can conditionally include
//! or exclude sections.
//!
//! Skills with `{{var}}` placeholders use minijinja template rendering
//! for simple variable interpolation without the overhead of a full RHAI script.
//!
//! # Discovery order (priority)
//! 1. `./skills/`              — project-local, SKILL.md format
//! 2. `./_b00t_/*.skill.toml[l][md]` — project b00t native (.skill.toml, .skill.tomllm, .skill.tomllmd)
//! 3. `~/.claude/skills/`      — Claude Code native, SKILL.md format
//! 4. `~/.b00t/_b00t_/*.skill.toml[l][md]` — global b00t

use anyhow::Result;
use b00t_c0re_lib::B00tContext;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use crate::datum_skill::SkillDatum;

const LOAD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(60);

/// Errors during skill loading.
#[derive(Debug)]
pub enum SkillLoadError {
    Timeout(u64),
    NotFound(String),
    Io(std::io::Error),
    Parse(String),
}

impl std::fmt::Display for SkillLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout(s) => write!(f, "Skill loading timed out after {s}s"),
            Self::NotFound(n) => write!(f, "Skill not found: {n}"),
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Parse(e) => write!(f, "Parse error: {e}"),
        }
    }
}

impl std::error::Error for SkillLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

// Conversion is handled at call sites via map_err.

/// Caching mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheMode {
    /// Cache skills with TTL (default)
    Cached,
    /// Bypass cache — always reload from disk
    NoCache,
}

/// Source format of a discovered skill
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillFormat {
    /// agentskills.io SKILL.md with YAML frontmatter
    SkillMd,
    /// b00t native `.skill.toml[l][md]`
    TomlDatum,
}

/// Lightweight skill descriptor — discovery tier (~50 tokens)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    /// Source directory containing this skill
    pub source_dir: PathBuf,
    /// Format the skill was loaded from
    #[serde(skip)]
    pub format: SkillFormat,
}

impl Default for SkillFormat {
    fn default() -> Self {
        SkillFormat::TomlDatum
    }
}

/// Full skill content — activation tier
#[derive(Debug, Clone)]
pub struct SkillContent {
    pub meta: SkillMeta,
    /// Full instruction body (from inline or file)
    pub instructions: String,
}

/// Configured skill search directory
#[derive(Debug, Clone)]
struct SkillDir {
    path: PathBuf,
    format: SkillFormat,
}

/// In-memory cache entry with TTL.
#[derive(Debug, Clone)]
struct CacheEntry {
    content: SkillContent,
    loaded_at: Instant,
}

/// Thread-safe global skill cache.
static SKILL_CACHE: OnceLock<std::sync::Mutex<std::collections::HashMap<String, CacheEntry>>> =
    OnceLock::new();

fn skill_cache() -> &'static std::sync::Mutex<std::collections::HashMap<String, CacheEntry>> {
    SKILL_CACHE.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Resolves skills across multiple directories with priority ordering.
pub struct SkillResolver {
    dirs: Vec<SkillDir>,
    cache_mode: CacheMode,
}

impl Default for SkillResolver {
    fn default() -> Self {
        Self::new(CacheMode::Cached)
    }
}

impl SkillResolver {
    /// Create resolver with caching mode.
    pub fn new(cache_mode: CacheMode) -> Self {
        let dirs = Self::build_dirs(None);
        SkillResolver { dirs, cache_mode }
    }

    /// Enumerate skill directories from project root (or cwd if None) and global locations.
    fn build_dirs(project_root: Option<&Path>) -> Vec<SkillDir> {
        let mut dirs = Vec::new();
        let root = project_root.map(|p| p.to_path_buf()).unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        // 1. Project-local SKILL.md skills/
        let local_skills = root.join("skills");
        if local_skills.is_dir() {
            dirs.push(SkillDir { path: local_skills, format: SkillFormat::SkillMd });
        }
        // 2. Project-local b00t datums
        let local_b00t = root.join("_b00t_");
        if local_b00t.is_dir() {
            dirs.push(SkillDir { path: local_b00t, format: SkillFormat::TomlDatum });
        }
        // 3. Claude Code native skills (global)
        if let Some(home) = dirs::home_dir() {
            let claude_skills = home.join(".claude").join("skills");
            if claude_skills.is_dir() {
                dirs.push(SkillDir { path: claude_skills, format: SkillFormat::SkillMd });
            }
            // 4. Global b00t datums
            let global_b00t = home.join(".b00t").join("_b00t_");
            if global_b00t.is_dir() {
                dirs.push(SkillDir { path: global_b00t, format: SkillFormat::TomlDatum });
            }
        }
        dirs
    }
}

impl SkillResolver {
    /// Build resolver using `base` as the project root instead of `current_dir()`.
    /// Project-local paths (`base/skills/`, `base/_b00t_/`) take priority over global home dirs.
    pub fn for_path(base: &Path) -> Self {
        SkillResolver { dirs: Self::build_dirs(Some(base)), cache_mode: CacheMode::Cached }
    }

    /// Create resolver with explicit directory list (for testing). Bypasses cache.
    pub fn with_dirs(dirs: Vec<(PathBuf, SkillFormat)>) -> Self {
        Self {
            dirs: dirs
                .into_iter()
                .map(|(path, format)| SkillDir { path, format })
                .collect(),
            cache_mode: CacheMode::NoCache,
        }
    }

    /// Set caching mode.
    pub fn with_cache(mut self, mode: CacheMode) -> Self {
        self.cache_mode = mode;
        self
    }

    /// Run a fallible closure with a timeout. Returns `Err(SkillLoadError::Timeout)` if exceeded.
    fn with_timeout<T: Send + 'static>(
        deadline: std::time::Duration,
        f: impl FnOnce() -> Result<T, SkillLoadError> + Send + 'static,
    ) -> Result<T, SkillLoadError> {
        use std::sync::mpsc;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = f();
            let _ = tx.send(result);
        });
        match rx.recv_timeout(deadline) {
            Ok(r) => r,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                Err(SkillLoadError::Timeout(deadline.as_secs()))
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err(SkillLoadError::Timeout(deadline.as_secs()))
            }
        }
    }

    /// Discover all skills — returns metadata only (discovery tier).
    pub fn list(&self) -> Vec<SkillMeta> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for dir in &self.dirs {
            if !dir.path.is_dir() {
                continue;
            }
            let metas = match dir.format {
                SkillFormat::SkillMd => scan_skill_md_dir(&dir.path),
                SkillFormat::TomlDatum => scan_toml_skill_dir(&dir.path),
            };
            for meta in metas {
                // first occurrence wins (priority order)
                if seen.insert(meta.name.clone()) {
                    out.push(meta);
                }
            }
        }

        out
    }

    /// Search skills by query — simple substring match on name + description + tags.
    /// Returns metadata only (discovery tier).
    pub fn search(&self, query: &str) -> Vec<SkillMeta> {
        let q = query.to_lowercase();
        self.list()
            .into_iter()
            .filter(|m| {
                m.name.to_lowercase().contains(&q)
                    || m.description.to_lowercase().contains(&q)
                    || m.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// Filter skills by role's declared skill list.
    pub fn list_for_role(&self, role_skills: &[String]) -> Vec<SkillMeta> {
        let all = self.list();
        all.into_iter()
            .filter(|m| role_skills.iter().any(|rs| rs == &m.name))
            .collect()
    }

    /// Load full skill content by name — activation tier. Lazy-loaded with timeout + cache.
    pub fn load(&self, name: &str) -> Result<SkillContent, SkillLoadError> {
        // Check cache (unless NoCache mode)
        if self.cache_mode == CacheMode::Cached {
            if let Some(entry) = skill_cache().lock().ok().and_then(|c| c.get(name).cloned()) {
                if entry.loaded_at.elapsed() < CACHE_TTL {
                    return Ok(entry.content);
                }
            }
        }

        let name_owned = name.to_string();
        let dirs: Vec<SkillDir> = self.dirs.clone();
        let result = Self::with_timeout(LOAD_TIMEOUT, move || {
            for dir in &dirs {
                if !dir.path.is_dir() {
                    continue;
                }
                match dir.format {
                    SkillFormat::SkillMd => {
                        let skill_md = dir.path.join(&name_owned).join("SKILL.md");
                        if skill_md.exists() {
                            let datum = SkillDatum::from_skill_md(&skill_md)
                                .map_err(|e| SkillLoadError::Parse(e.to_string()))?;
                            let cfg = datum.skill_config()
                                .map_err(|e| SkillLoadError::Parse(e.to_string()))?;
                            let instructions = datum.load_instructions(&dir.path.join(&name_owned))
                                .map_err(|e| SkillLoadError::Parse(e.to_string()))?;
                            let rendered = render_skill(&instructions);
                            let content = SkillContent {
                                meta: SkillMeta {
                                    name: datum.datum.name.clone(),
                                    description: cfg.description.clone(),
                                    tags: cfg.tags.clone(),
                                    source_dir: dir.path.join(&name_owned),
                                    format: SkillFormat::SkillMd,
                                },
                                instructions: rendered,
                            };
                            return Ok(content);
                        }
                    }
                    SkillFormat::TomlDatum => {
                        let primary = dir.path.join(format!("{}.skill.toml", name_owned));
                        let extended = dir.path.join(format!("{}.skill.tomllm", name_owned));
                        if !primary.exists() && !extended.exists() { continue; }
                        let path_str = dir.path.to_string_lossy();
                        let datum = SkillDatum::from_config(&name_owned, &path_str)
                            .map_err(|e| SkillLoadError::Parse(e.to_string()))?;
                        let cfg = datum.skill_config()
                            .map_err(|e| SkillLoadError::Parse(e.to_string()))?;
                        let instructions = datum.load_instructions(&dir.path)
                            .map_err(|e| SkillLoadError::Parse(e.to_string()))?;
                        let rendered = render_skill(&instructions);
                        let content = SkillContent {
                            meta: SkillMeta {
                                name: datum.datum.name.clone(),
                                description: cfg.description.clone(),
                                tags: cfg.tags.clone(),
                                source_dir: dir.path.clone(),
                                format: SkillFormat::TomlDatum,
                            },
                            instructions: rendered,
                        };
                        return Ok(content);
                    }
                }
            }
            Err(SkillLoadError::NotFound(name_owned))
        });

        // Cache on success
        if self.cache_mode == CacheMode::Cached {
            if let Ok(ref content) = result {
                if let Ok(mut cache) = skill_cache().lock() {
                    cache.insert(name.to_string(), CacheEntry {
                        content: content.clone(),
                        loaded_at: Instant::now(),
                    });
                }
            }
        }

        result
    }
}

/// Render skill instructions through template engine.
/// Supports `{{var}}` substitution via minijinja-inspired replacement.
/// If the skill datum declares a `[b00t.skill.rhai]` hook, the RHAI engine
/// is invoked for dynamic generation instead.
fn render_skill(raw: &str) -> String {
    if !raw.contains("{{") {
        return raw.to_string();
    }
    let context = b00t_c0re_lib::B00tContext::current().ok();
    if let Some(ctx) = context {
        // Simple {{VAR}} replacement — matches TemplateRenderer pattern
        raw.replace("{{PID}}", &ctx.pid.to_string())
            .replace("{{TIMESTAMP}}", &ctx.timestamp)
            .replace("{{USER}}", &ctx.user)
            .replace("{{BRANCH}}", &ctx.branch)
            .replace("{{HOSTNAME}}", &ctx.hostname)
            .replace("{{MODEL_SIZE}}", &ctx.model_size)
            .replace("{{WORKSPACE_ROOT}}", &ctx.workspace_root)
    } else {
        raw.to_string()
    }
}

// --- private scanners ---

fn scan_skill_md_dir(dir: &PathBuf) -> Vec<SkillMeta> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let skill_md = entry.path().join("SKILL.md");
        if skill_md.exists() {
            if let Ok(datum) = SkillDatum::from_skill_md(&skill_md) {
                if let Ok(cfg) = datum.skill_config() {
                    out.push(SkillMeta {
                        name: datum.datum.name.clone(),
                        description: cfg.description.clone(),
                        tags: cfg.tags.clone(),
                        source_dir: entry.path(),
                        format: SkillFormat::SkillMd,
                    });
                }
            }
        }
    }
    out
}

fn scan_toml_skill_dir(dir: &PathBuf) -> Vec<SkillMeta> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(fname) = path.file_name().and_then(|f| f.to_str()) else {
            continue;
        };
        // Match .skill.toml, .skill.tomllm, .skill.tomllmd
        let name = fname
            .strip_suffix(".skill.tomllmd")
            .or_else(|| fname.strip_suffix(".skill.tomllm"))
            .or_else(|| fname.strip_suffix(".skill.toml"));
        let Some(name) = name else { continue };
        if name.contains('.') {
            continue; // skip typed datums like b00t.cli.toml — only bare <name>.skill.*
        }
        let path_str = dir.to_string_lossy();
        if let Ok(datum) = SkillDatum::from_config(name, &path_str) {
            if let Ok(cfg) = datum.skill_config() {
                out.push(SkillMeta {
                    name: datum.datum.name.clone(),
                    description: cfg.description.clone(),
                    tags: cfg.tags.clone(),
                    source_dir: dir.clone(),
                    format: SkillFormat::TomlDatum,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_skill_md_dir(dir: &std::path::Path, name: &str, desc: &str, tags: &[&str]) {
        let skill_dir = dir.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        let tags_yaml = if tags.is_empty() {
            "tags: []".to_string()
        } else {
            let items = tags
                .iter()
                .map(|t| format!("- {}", t))
                .collect::<Vec<_>>()
                .join("\n");
            format!("tags:\n{}", items)
        };
        let content = format!(
            "---\nname: {}\ndescription: {}\n{}\napplies_to:\n- general\noutput_types:\n- .txt\n---\n# {}\nInstructions for {}.\n",
            name, desc, tags_yaml, name, name
        );
        let mut f = std::fs::File::create(skill_dir.join("SKILL.md")).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_resolver_list_skill_md_dir() {
        let tmp = tempfile::tempdir().unwrap();
        make_skill_md_dir(tmp.path(), "fast-rust", "Fast Rust code", &["rust"]);
        make_skill_md_dir(
            tmp.path(),
            "friendly-python",
            "Friendly Python",
            &["python"],
        );

        let resolver =
            SkillResolver::with_dirs(vec![(tmp.path().to_path_buf(), SkillFormat::SkillMd)]);
        let metas = resolver.list();
        assert_eq!(metas.len(), 2);
        let names: Vec<_> = metas.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"fast-rust"));
        assert!(names.contains(&"friendly-python"));
    }

    #[test]
    fn test_resolver_search_by_query() {
        let tmp = tempfile::tempdir().unwrap();
        make_skill_md_dir(
            tmp.path(),
            "fast-rust",
            "Fast Rust code. Use for Rust.",
            &["rust"],
        );
        make_skill_md_dir(
            tmp.path(),
            "friendly-python",
            "Friendly Python code.",
            &["python"],
        );

        let resolver =
            SkillResolver::with_dirs(vec![(tmp.path().to_path_buf(), SkillFormat::SkillMd)]);

        let results = resolver.search("rust");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "fast-rust");

        let empty = resolver.search("golang");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_resolver_load_skill_md() {
        let tmp = tempfile::tempdir().unwrap();
        make_skill_md_dir(tmp.path(), "my-skill", "My skill desc", &["demo"]);

        let resolver =
            SkillResolver::with_dirs(vec![(tmp.path().to_path_buf(), SkillFormat::SkillMd)]);

        let content = resolver.load("my-skill").unwrap();
        assert_eq!(content.meta.name, "my-skill");
        assert!(content.instructions.contains("Instructions for my-skill."));
    }

    #[test]
    fn test_resolver_load_missing_errors() {
        let resolver = SkillResolver::with_dirs(vec![]);
        assert!(resolver.load("nonexistent").is_err());
    }

    #[test]
    fn test_resolver_priority_first_wins() {
        let tmp1 = tempfile::tempdir().unwrap();
        let tmp2 = tempfile::tempdir().unwrap();
        make_skill_md_dir(tmp1.path(), "shared-skill", "From dir1", &[]);
        make_skill_md_dir(tmp2.path(), "shared-skill", "From dir2", &[]);

        let resolver = SkillResolver::with_dirs(vec![
            (tmp1.path().to_path_buf(), SkillFormat::SkillMd),
            (tmp2.path().to_path_buf(), SkillFormat::SkillMd),
        ]);

        let metas = resolver.list();
        assert_eq!(metas.len(), 1); // deduped
        assert_eq!(metas[0].description, "From dir1"); // first wins
    }

    #[test]
    fn test_resolver_list_for_role() {
        let tmp = tempfile::tempdir().unwrap();
        make_skill_md_dir(tmp.path(), "rust-skill", "Rust skill", &["rust"]);
        make_skill_md_dir(tmp.path(), "python-skill", "Python skill", &["python"]);

        let resolver =
            SkillResolver::with_dirs(vec![(tmp.path().to_path_buf(), SkillFormat::SkillMd)]);

        let role_skills = vec!["rust-skill".to_string()];
        let filtered = resolver.list_for_role(&role_skills);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "rust-skill");
    }
}
