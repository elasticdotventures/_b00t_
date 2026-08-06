//! Pure analysis over b00t datum dialect files (.toml / .tomllm / .tomllmd).
//!
//! Everything here is transport-free: functions take paths + file contents and
//! return plain structs, so tests exercise the analysis directly and the LSP
//! layer in `main.rs` is a thin adapter.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use b00t_datum_core::{TomllmDoc, TomllmdExt};

/// Valid `type` tokens for the `[b00t]` / `[b00t.schema]` stanza.
/// 🤓 mirrors datum_type_table! in b00t-cli/src/lib.rs (~line 1497) plus the
///    "model" alias handled in deserialize_datum_type — keep in sync manually;
///    b00t-lsp MUST NOT depend on b00t-cli (dependency inversion: cli is an app).
pub const VALID_TYPE_TOKENS: &[&str] = &[
    "database",
    "db",
    "hive",
    "hive_profile",
    "agent",
    "config",
    "docker",
    "skill",
    "stack",
    "repo",
    "role",
    "bash",
    "vscode",
    "k8s",
    "apt",
    "nix",
    "mcp",
    "cli",
    "api",
    "job",
    "ai",
    "model",
    "ai_model",
    "justfile",
    "hardware",
    "overlay",
    "runtime",
    "wrap",
    "launcher",
    "polyseme",
    "poly",
    "credential",
    "credentials",
    "gate",
    "hook",
    "mcp_server",
    "plan",
    "schema",
    "training",
    "vendor",
    "verifier",
    "ooda",
];

/// Content tags accepted as `type` values without warning.
/// 🤓 mirrors is_known_content_tag in b00t-cli/src/lib.rs:33.
pub const KNOWN_CONTENT_TAGS: &[&str] = &[
    "okr",
    "prd",
    "pattern",
    "datum",
    "reference",
    "learn",
    "hardware",
    "tomllmd",
    "specification",
    "topic",
    "soul",
    "install",
    "github_org",
    "ai_provider",
    "pyinfra",
    "wow",
    "lfmf",
    "capability",
];

/// Filenames that live in datum directories but are not datums.
/// 🤓 mirrors the skip list in b00t-cli/src/datum_utils.rs scan_datums_recursive.
const NON_DATUM_FILES: &[&str] = &[
    "bootstrap.toml",
    "git-cliff.toml",
    "_b00t_.toml",
    "Cargo.toml",
];

pub fn is_valid_type_token(s: &str) -> bool {
    VALID_TYPE_TOKENS.contains(&s) || KNOWN_CONTENT_TAGS.contains(&s)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// A single diagnostic. Positions are 0-based (LSP convention).
#[derive(Debug, Clone)]
pub struct Diag {
    pub line: u32,
    pub col_start: u32,
    pub col_end: u32,
    pub severity: Severity,
    pub message: String,
}

/// Precedence rank per extension: .tomllmd(3) > .tomllm(2) > .toml(1).
/// 🤓 semantics mirror scan_datums_recursive in b00t-cli/src/datum_utils.rs (~line 300).
pub fn datum_rank(path: &Path) -> Option<u8> {
    match TomllmdExt::from_path(path)? {
        TomllmdExt::Tomllmd => Some(3),
        TomllmdExt::Tomllm => Some(2),
        TomllmdExt::Toml => Some(1),
    }
}

/// Datum key: filename with the outer extension stripped ("foo.cli.tomllm" → "foo.cli").
pub fn datum_stem(path: &Path) -> Option<String> {
    let filename = path.file_name()?.to_str()?;
    for ext in [".tomllmd", ".tomllm", ".toml"] {
        if let Some(stem) = filename.strip_suffix(ext) {
            return Some(stem.to_string());
        }
    }
    None
}

/// True when the path looks like a datum file (right extension, not on the skip list).
pub fn is_datum_file(path: &Path) -> bool {
    if datum_rank(path).is_none() {
        return false;
    }
    match path.file_name().and_then(|f| f.to_str()) {
        Some(name) => !NON_DATUM_FILES.contains(&name) && !name.starts_with('.'),
        None => false,
    }
}

/// A `depends_on` / `composes_with` array entry with its source position.
#[derive(Debug, Clone, PartialEq)]
pub struct DepRef {
    /// "depends_on" or "composes_with"
    pub key: String,
    pub name: String,
    /// 0-based line.
    pub line: u32,
    /// Byte column of the first char of the name (inside the quotes).
    pub col_start: u32,
    /// Byte column one past the last char of the name.
    pub col_end: u32,
}

/// Extract `depends_on = [...]` / `composes_with = [...]` entries with positions.
/// Text-scan (not TOML-parse) so positions survive even in partially broken files.
pub fn extract_dep_refs(content: &str) -> Vec<DepRef> {
    let mut refs = Vec::new();
    // Some((key, bracket_depth)) while inside a multi-line array.
    let mut active: Option<(String, i32)> = None;

    for (lineno, line) in content.lines().enumerate() {
        let scan_from = match &active {
            Some(_) => 0,
            None => {
                let trimmed = line.trim_start();
                let key = if trimmed.starts_with("depends_on") {
                    "depends_on"
                } else if trimmed.starts_with("composes_with") {
                    "composes_with"
                } else {
                    continue;
                };
                let after_key = trimmed[key.len()..].trim_start();
                if !after_key.starts_with('=') {
                    continue;
                }
                active = Some((key.to_string(), 0));
                match line.find('=') {
                    Some(idx) => idx + 1,
                    None => continue,
                }
            }
        };

        let (key, mut depth) = active.clone().expect("active set above");
        let bytes = line.as_bytes();
        let mut i = scan_from;
        let mut in_str = false;
        let mut str_start = 0usize;
        let mut closed = false;
        while i < bytes.len() {
            let c = bytes[i] as char;
            if in_str {
                if c == '"' {
                    let name = line[str_start..i].to_string();
                    if !name.is_empty() {
                        refs.push(DepRef {
                            key: key.clone(),
                            name,
                            line: lineno as u32,
                            col_start: str_start as u32,
                            col_end: i as u32,
                        });
                    }
                    in_str = false;
                }
            } else {
                match c {
                    '"' => {
                        in_str = true;
                        str_start = i + 1;
                    }
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth <= 0 {
                            closed = true;
                        }
                    }
                    '#' => break, // trailing comment
                    _ => {}
                }
                if closed {
                    break;
                }
            }
            i += 1;
        }

        if closed || depth <= 0 {
            // Also deactivates non-array forms like `depends_on = "x"`.
            active = None;
        } else {
            active = Some((key, depth));
        }
    }
    refs
}

/// Lightweight per-file record used by the workspace index.
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    pub stem: String,
    pub rank: u8,
    /// `[b00t].name` when present and parseable.
    pub name: Option<String>,
    pub deps: Vec<DepRef>,
}

/// Workspace index over a datum directory — powers shadowing diagnostics,
/// goto-definition and find-references.
#[derive(Debug, Default)]
pub struct WorkspaceIndex {
    pub files: Vec<FileInfo>,
    by_stem: HashMap<String, Vec<usize>>,
    /// datum name → index of the winning (highest-rank) file registered under it.
    by_name: HashMap<String, usize>,
    /// Extra `type` tokens accepted without warning, from `<root>/incubating.tomllm`.
    /// 🤓 mirrors get_incubating_set in b00t-cli/src/lib.rs:45.
    pub incubating: HashSet<String>,
}

impl WorkspaceIndex {
    /// Recursively scan `root` for datum files and index them.
    pub fn scan(root: &Path) -> Self {
        let mut idx = Self::default();
        idx.incubating = load_incubating(root);
        let mut paths = Vec::new();
        collect_datum_paths(root, &mut paths, 0);
        paths.sort();
        for path in paths {
            if let Ok(content) = std::fs::read_to_string(&path) {
                idx.add_file(&path, &content);
            }
        }
        idx
    }

    /// Index a single file (also used to refresh open editor buffers).
    pub fn add_file(&mut self, path: &Path, content: &str) {
        let (Some(stem), Some(rank)) = (datum_stem(path), datum_rank(path)) else {
            return;
        };
        let name = parse_b00t_name(content);
        let deps = extract_dep_refs(content);
        let file_idx = self.files.len();
        self.files.push(FileInfo {
            path: path.to_path_buf(),
            stem: stem.clone(),
            rank,
            name: name.clone(),
            deps,
        });
        self.by_stem.entry(stem.clone()).or_default().push(file_idx);
        for key in registered_names(&stem, name.as_deref()) {
            match self.by_name.get(&key) {
                Some(&existing) if self.files[existing].rank >= rank => {}
                _ => {
                    self.by_name.insert(key, file_idx);
                }
            }
        }
    }

    /// Another file with the same stem and a strictly higher rank, if any.
    pub fn shadowing(&self, path: &Path) -> Option<&FileInfo> {
        let stem = datum_stem(path)?;
        let rank = datum_rank(path)?;
        self.by_stem
            .get(&stem)?
            .iter()
            .map(|&i| &self.files[i])
            .filter(|f| f.path != path && f.rank > rank)
            .max_by_key(|f| f.rank)
    }

    /// Resolve a `depends_on` name to the datum file that owns it.
    pub fn resolve(&self, name: &str) -> Option<&FileInfo> {
        if let Some(&i) = self.by_name.get(name) {
            return Some(&self.files[i]);
        }
        // "hf-cli" should hit "hf-cli.cli.toml" (stem "hf-cli.cli").
        let prefix = format!("{name}.");
        self.by_stem
            .iter()
            .filter(|(stem, _)| stem.starts_with(&prefix))
            .flat_map(|(_, idxs)| idxs.iter().map(|&i| &self.files[i]))
            .max_by_key(|f| f.rank)
    }

    /// All datums whose depends_on/composes_with mention the datum at `path`.
    pub fn references_to(&self, path: &Path) -> Vec<(&FileInfo, &DepRef)> {
        let Some(target) = self.files.iter().find(|f| f.path == path) else {
            return Vec::new();
        };
        let names = registered_names(&target.stem, target.name.as_deref());
        let mut out = Vec::new();
        for file in &self.files {
            if file.path == path {
                continue;
            }
            for dep in &file.deps {
                if names.contains(&dep.name) {
                    out.push((file, dep));
                }
            }
        }
        out
    }
}

/// Names a datum file answers to: its stem, its stem's first dotted segment,
/// and its explicit `[b00t].name`.
fn registered_names(stem: &str, name: Option<&str>) -> Vec<String> {
    let mut names = vec![stem.to_string()];
    if let Some(first) = stem.split('.').next() {
        if first != stem {
            names.push(first.to_string());
        }
    }
    if let Some(n) = name {
        if !names.iter().any(|x| x == n) {
            names.push(n.to_string());
        }
    }
    names
}

/// Load `incubating = [...]` from `<root>/incubating.tomllm` (empty set if absent).
fn load_incubating(root: &Path) -> HashSet<String> {
    let Ok(content) = std::fs::read_to_string(root.join("incubating.tomllm")) else {
        return HashSet::new();
    };
    let Ok(value) = toml::from_str::<toml::Value>(&content) else {
        return HashSet::new();
    };
    value
        .get("incubating")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_b00t_name(content: &str) -> Option<String> {
    let value: toml::Value = toml::from_str(content).ok()?;
    value.get("b00t")?.get("name")?.as_str().map(String::from)
}

fn collect_datum_paths(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    // 🤓 depth cap + dir skip list keeps --check from crawling target/ or vendor/
    if depth > 6 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skip = path
                .file_name()
                .and_then(|f| f.to_str())
                .map(|n| {
                    n.starts_with('.')
                        || n == "_archive_"
                        || n == "target"
                        || n == "node_modules"
                        || n == "vendor"
                })
                .unwrap_or(true);
            if !skip {
                collect_datum_paths(&path, out, depth + 1);
            }
        } else if is_datum_file(&path) {
            out.push(path);
        }
    }
}

fn offset_to_pos(content: &str, offset: usize) -> (u32, u32) {
    let clamped = offset.min(content.len());
    let mut line = 0u32;
    let mut line_start = 0usize;
    for (i, b) in content.bytes().enumerate() {
        if i >= clamped {
            break;
        }
        if b == b'\n' {
            line += 1;
            line_start = i + 1;
        }
    }
    (line, (clamped - line_start) as u32)
}

/// Full diagnostic pass over one file. `index` enables shadowing checks.
pub fn diagnostics(path: &Path, content: &str, index: Option<&WorkspaceIndex>) -> Vec<Diag> {
    let mut diags = Vec::new();

    // (a) TOML parse errors — hard errors; skip the semantic checks below when broken.
    if let Err(err) = toml::from_str::<toml::Value>(content) {
        let (line, col) = err
            .span()
            .map(|span| offset_to_pos(content, span.start))
            .unwrap_or((0, 0));
        diags.push(Diag {
            line,
            col_start: col,
            col_end: col + 1,
            severity: Severity::Error,
            message: format!("TOML parse error: {}", err.message()),
        });
        return diags;
    }

    // (b) tail-map contract on .tomllm / .tomllmd.
    if matches!(
        TomllmdExt::from_path(path),
        Some(TomllmdExt::Tomllm) | Some(TomllmdExt::Tomllmd)
    ) {
        diags.extend(check_tail_map(content));
    }

    // (c) datum key collision — this file loses by rank.
    if let Some(index) = index {
        if let Some(winner) = index.shadowing(path) {
            diags.push(Diag {
                line: 0,
                col_start: 0,
                col_end: 0,
                severity: Severity::Warning,
                message: format!(
                    "datum key '{}' shadowed by {} (rank .tomllmd > .tomllm > .toml)",
                    datum_stem(path).unwrap_or_default(),
                    winner.path.display()
                ),
            });
        }
    }

    // (d) unknown `type` value (incubating types from the workspace are accepted).
    diags.extend(check_type_token(content, index.map(|i| &i.incubating)));

    diags
}

/// Tail-map contract: the last ≤10 lines must contain `# b00t:map v1`,
/// `# summary:` and `# tags:`.
fn check_tail_map(content: &str) -> Vec<Diag> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return vec![Diag {
            line: 0,
            col_start: 0,
            col_end: 0,
            severity: Severity::Warning,
            message: "missing tail-map: empty file (expected `# b00t:map v1` block)".into(),
        }];
    }
    let tail_start = lines.len().saturating_sub(10);
    let tail = &lines[tail_start..];
    let has = |marker: &str| tail.iter().any(|l| l.trim_start().starts_with(marker));

    let mut missing = Vec::new();
    if !has("# b00t:map v1") {
        missing.push("`# b00t:map v1`");
    }
    if !has("# summary:") {
        missing.push("`# summary:`");
    }
    if !has("# tags:") {
        missing.push("`# tags:`");
    }
    if missing.is_empty() {
        return Vec::new();
    }
    let last_line = (lines.len() - 1) as u32;
    let last_len = lines.last().map(|l| l.len()).unwrap_or(0) as u32;
    vec![Diag {
        line: last_line,
        col_start: 0,
        col_end: last_len,
        severity: Severity::Warning,
        message: format!(
            "tail-map incomplete in last 10 lines: missing {}",
            missing.join(", ")
        ),
    }]
}

/// Warn when `[b00t].type` / `[b00t.schema].type` is not a known token.
fn check_type_token(content: &str, incubating: Option<&HashSet<String>>) -> Vec<Diag> {
    let Ok(value) = toml::from_str::<toml::Value>(content) else {
        return Vec::new();
    };
    let b00t = value.get("b00t");
    let type_val = b00t
        .and_then(|b| b.get("type"))
        .or_else(|| {
            b00t.and_then(|b| b.get("schema"))
                .and_then(|s| s.get("type"))
        })
        .and_then(|t| t.as_str());
    let Some(type_str) = type_val else {
        return Vec::new();
    };
    if is_valid_type_token(type_str)
        || incubating
            .map(|set| set.contains(type_str))
            .unwrap_or(false)
    {
        return Vec::new();
    }
    // Locate `type = "<value>"` textually for a useful position.
    let (line, col_start, col_end) = content
        .lines()
        .enumerate()
        .find_map(|(i, l)| {
            let t = l.trim_start();
            if t.starts_with("type") && t.contains('=') && t.contains(&format!("\"{type_str}\"")) {
                let start = l.find(&format!("\"{type_str}\"")).unwrap_or(0);
                Some((i as u32, start as u32, (start + type_str.len() + 2) as u32))
            } else {
                None
            }
        })
        .unwrap_or((0, 0, 0));
    vec![Diag {
        line,
        col_start,
        col_end,
        severity: Severity::Warning,
        message: format!(
            "unknown datum type '{type_str}' (known: {} type tokens + {} content tags)",
            VALID_TYPE_TOKENS.len(),
            KNOWN_CONTENT_TAGS.len()
        ),
    }]
}

/// Hover text (markdown) for a datum file: name/type/hint + tail-map summary.
pub fn hover(path: &Path, content: &str) -> Option<String> {
    let ext = TomllmdExt::from_path(path)?;
    let value: toml::Value = toml::from_str(content).ok()?;
    let b00t = value.get("b00t");
    let get = |key: &str| b00t.and_then(|b| b.get(key)).and_then(|v| v.as_str());
    let name = get("name").map(String::from).or_else(|| datum_stem(path));
    let datum_type = get("type")
        .map(String::from)
        .or_else(|| b00t?.get("schema")?.get("type")?.as_str().map(String::from));
    let hint = get("hint");

    let mut out = String::new();
    out.push_str(&format!("**{}**", name.unwrap_or_else(|| "datum".into())));
    if let Some(t) = datum_type {
        out.push_str(&format!(" · `{t}`"));
    }
    out.push('\n');
    if let Some(h) = hint {
        out.push_str(&format!("\n{h}\n"));
    }
    if let Ok(doc) = TomllmDoc::from_str(content, ext, path.to_path_buf()) {
        if let Some(summary) = doc.summary() {
            out.push_str(&format!("\n---\n_{summary}_\n"));
        }
    }
    Some(out)
}

/// The dep-ref (if any) under the cursor at `line`/`col` (0-based, byte col).
pub fn dep_ref_at(content: &str, line: u32, col: u32) -> Option<DepRef> {
    extract_dep_refs(content)
        .into_iter()
        .find(|d| d.line == line && d.col_start <= col && col <= d.col_end)
}
