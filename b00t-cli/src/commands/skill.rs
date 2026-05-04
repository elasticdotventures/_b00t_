//! `b00t skill` — multi-directory skill discovery and activation
//!
//! Progressive disclosure pattern:
//! - `list` / `search` → metadata only (~50 tokens per skill)
//! - `load`            → metadata + applies_to summary
//! - `activate`        → full instruction body → stdout for LLM injection
//! - `serve`           → HTTP server for remote skill loading by opencode

use anyhow::Result;
use clap::Parser;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;

use crate::get_expanded_path;
use crate::skill_resolver::SkillFormat;
use crate::skill_resolver::SkillResolver;

#[derive(Parser)]
pub enum SkillCommands {
    #[clap(about = "List all available skills (metadata only)")]
    List {
        #[clap(long, help = "Filter to skills declared by a role datum")]
        role: Option<String>,
        #[clap(long, help = "Output JSON")]
        json: bool,
    },

    #[clap(about = "Search skills by query (name, description, tags)")]
    Search {
        #[clap(help = "Search query")]
        query: String,
        #[clap(long, help = "Output JSON")]
        json: bool,
    },

    #[clap(about = "Show skill metadata (discovery tier — does not load instructions)")]
    Load {
        #[clap(help = "Skill name")]
        name: String,
        #[clap(long, help = "Output JSON")]
        json: bool,
    },

    #[clap(
        about = "Activate skill — emit full instruction body to stdout for LLM context injection"
    )]
    Activate {
        #[clap(help = "Skill name")]
        name: String,
        #[clap(long, help = "Prefix with role context from named role datum")]
        role: Option<String>,
    },

    #[clap(about = "Serve skills via HTTP for remote loading by opencode")]
    Serve {
        #[clap(long, default_value = "4097", help = "Port to listen on")]
        port: u16,
        #[clap(long, default_value = "127.0.0.1", help = "Host to bind to")]
        host: String,
    },

    #[clap(about = "Pull skills from a remote opencode-compatible URL")]
    Sync {
        #[clap(long, help = "Remote URL serving /index.json (e.g. http://localhost:4097)")]
        url: String,
        #[clap(long, help = "Output directory for downloaded skills", default_value = ".opencode/skills")]
        output: PathBuf,
    },
}

pub fn handle_skill_command(cmd: &SkillCommands, path: &str) -> Result<()> {
    // Resolve skills relative to the provided path (honours `b00t --path <repo> skill ...`)
    let resolver = build_resolver(path);
    match cmd {
        SkillCommands::List { role, json } => {
            let metas = match role {
                Some(role_name) => {
                    let role_skills = load_role_skill_list(role_name, path)?;
                    resolver.list_for_role(&role_skills)
                }
                None => resolver.list(),
            };

            if *json {
                let out: Vec<_> = metas
                    .iter()
                    .map(|m| {
                        json!({
                            "name": &m.name,
                            "description": &m.description,
                            "tags": &m.tags,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                if metas.is_empty() {
                    println!("No skills found. Add skills to ./skills/ or _b00t_/*.skill.toml");
                } else {
                    for m in &metas {
                        let tags = if m.tags.is_empty() {
                            String::new()
                        } else {
                            format!(" [{}]", m.tags.join(", "))
                        };
                        println!("• {} — {}{}", m.name, m.description, tags);
                    }
                    println!("\n{} skill(s) found", metas.len());
                }
            }
            Ok(())
        }

        SkillCommands::Search { query, json } => {
            let metas = resolver.search(query);

            if *json {
                let out: Vec<_> = metas
                    .iter()
                    .map(|m| {
                        json!({
                            "name": &m.name,
                            "description": &m.description,
                            "tags": &m.tags,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else if metas.is_empty() {
                println!("No skills match '{}'", query);
            } else {
                for m in &metas {
                    println!("• {} — {}", m.name, m.description);
                }
            }
            Ok(())
        }

        SkillCommands::Load { name, json } => {
            // list() is cheap — find the meta without loading instructions
            let metas = resolver.list();
            let meta = metas
                .iter()
                .find(|m| m.name == name.as_str())
                .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found", name))?;

            if *json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "name": &meta.name,
                        "description": &meta.description,
                        "tags": &meta.tags,
                        "source_dir": &meta.source_dir,
                    }))?
                );
            } else {
                println!("🎯 {}", meta.name);
                println!("   {}", meta.description);
                if !meta.tags.is_empty() {
                    println!("   tags: {}", meta.tags.join(", "));
                }
                println!("   source: {}", meta.source_dir.display());
                println!(
                    "\n💡 Use `b00t skill activate {}` to load full instructions",
                    name
                );
            }
            Ok(())
        }

        SkillCommands::Activate { name, role } => {
            let content = resolver.load(name)?;

            // Optional role context prefix
            if let Some(role_name) = role {
                if let Ok(role_context) = load_role_context_summary(role_name) {
                    println!("## Role Context: {}\n{}\n---\n", role_name, role_context);
                }
            }

            // Emit full instruction body — LLM reads this as skill activation
            println!("## Skill: {}", content.meta.name);
            println!("<!-- description: {} -->\n", content.meta.description);
            println!("{}", content.instructions);
            Ok(())
        }

        SkillCommands::Serve { port, host } => handle_serve(*port, host, path),

        SkillCommands::Sync { url, output } => handle_sync(url, output),
    }
}

/// Build a `SkillResolver` relative to the provided CLI `path` argument.
fn build_resolver(path: &str) -> SkillResolver {
    match get_expanded_path(path) {
        Ok(expanded) => SkillResolver::for_path(&expanded),
        Err(_) => {
            eprintln!(
                "⚠️  Could not expand path '{}', resolving skills from current directory",
                path
            );
            SkillResolver::default()
        }
    }
}

/// Load skill names declared by a role datum from _b00t_/*.role.toml(l)
fn load_role_skill_list(role_name: &str, base_path: &str) -> Result<Vec<String>> {
    let b00t_dir = find_b00t_dir_for(base_path)?;
    // Try <role>.role.tomllmd, <role>.role.tomllm, <role>.role.toml
    for ext in &["role.tomllmd", "role.tomllm", "role.toml"] {
        let path = b00t_dir.join(format!("{}.{}", role_name, ext));
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            // Extract skills = [...] array from TOML
            if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                if let Some(skills) = value.get("skills").and_then(|v| v.as_array()) {
                    return Ok(skills
                        .iter()
                        .filter_map(|s| s.as_str().map(String::from))
                        .collect());
                }
                // Also check [b00t].skills
                if let Some(skills) = value
                    .get("b00t")
                    .and_then(|b| b.get("skills"))
                    .and_then(|v| v.as_array())
                {
                    return Ok(skills
                        .iter()
                        .filter_map(|s| s.as_str().map(String::from))
                        .collect());
                }
            }
        }
    }

    // Not found — return empty (graceful degradation)
    Ok(vec![])
}

/// Emit a short role context summary for skill activation preamble
fn load_role_context_summary(role_name: &str) -> Result<String> {
    let b00t_dir = find_b00t_dir()?;

    for ext in &["role.tomllmd", "role.tomllm", "role.toml"] {
        let path = b00t_dir.join(format!("{}.{}", role_name, ext));
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            if let Ok(value) = toml::from_str::<toml::Value>(&content) {
                let hint = value
                    .get("b00t")
                    .and_then(|b| b.get("hint"))
                    .or_else(|| value.get("hint"))
                    .and_then(|h| h.as_str())
                    .unwrap_or(role_name);
                return Ok(format!("Role `{}`: {}", role_name, hint));
            }
        }
    }

    Ok(format!("Role: {}", role_name))
}

/// Find the nearest _b00t_ directory relative to `base_path` (or global fallback)
fn find_b00t_dir_for(base_path: &str) -> Result<std::path::PathBuf> {
    // Try expanded path first
    if let Ok(expanded) = get_expanded_path(base_path) {
        let local = expanded.join("_b00t_");
        if local.is_dir() {
            return Ok(local);
        }
    }
    // Fall back to cwd-based search
    find_b00t_dir()
}

/// Find the nearest _b00t_ directory (project-local or global)
fn find_b00t_dir() -> Result<std::path::PathBuf> {
    // Project-local first
    if let Ok(cwd) = std::env::current_dir() {
        let local = cwd.join("_b00t_");
        if local.is_dir() {
            return Ok(local);
        }
    }
    // Global fallback
    if let Some(home) = dirs::home_dir() {
        let global = home.join(".b00t").join("_b00t_");
        if global.is_dir() {
            return Ok(global);
        }
    }
    anyhow::bail!("No _b00t_ directory found (tried project-local and ~/.b00t/_b00t_/)")
}

// ── HTTP serve (b00t skill serve) ──────────────────────────────────────────

/// Serve skills via HTTP for opencode's remote skill URL mechanism.
fn handle_serve(port: u16, host: &str, base_path: &str) -> Result<()> {
    // Build standard resolver
    let resolver = build_resolver(base_path);
    let mut skills = resolver.list();

    // Also include .opencode/skills/ — the primary b00t skill directory
    // (not in the resolver's standard search paths by default)
    if let Ok(base) = get_expanded_path(base_path) {
        let opencode_dir = base.join(".opencode").join("skills");
        if opencode_dir.is_dir() {
            let extra =
                SkillResolver::with_dirs(vec![(opencode_dir, SkillFormat::SkillMd)]);
            let seen: std::collections::HashSet<String> =
                skills.iter().map(|s| s.name.clone()).collect();
            for s in extra.list() {
                if !seen.contains(&s.name) {
                    skills.push(s);
                }
            }
        }
    }

    let skills = Arc::new(skills);
    let addr = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&addr)
        .map_err(|e| anyhow::anyhow!("Cannot bind to {}: {}", addr, e))?;

    println!("🥾 b00t skill server — http://{}", addr);
    println!("   {} skills available", skills.len());
    println!("   GET /index.json                              → skill manifest");
    println!("   GET /skills/<name>/<file>                     → skill file");
    println!("   Press Ctrl+C to stop");

    for stream in listener.incoming() {
        let skills = Arc::clone(&skills);
        match stream {
            Ok(mut stream) => {
                std::thread::spawn(move || {
                    if let Err(e) = serve_connection(&mut stream, &skills) {
                        eprintln!("⚠️  Serve error: {}", e);
                    }
                });
            }
            Err(e) => eprintln!("⚠️  Accept error: {}", e),
        }
    }

    Ok(())
}

/// Handle a single HTTP connection.
fn serve_connection(
    stream: &mut std::net::TcpStream,
    skills: &[crate::skill_resolver::SkillMeta],
) -> Result<()> {
    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf)?;
    if n == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buf[..n]);
    let response = route_request(&request, skills);

    stream.write_all(response.as_bytes())?;
    Ok(())
}

/// Parse the HTTP request and dispatch to the appropriate handler.
fn route_request(request: &str, skills: &[crate::skill_resolver::SkillMeta]) -> String {
    // Parse request line: "GET /path HTTP/1.1"
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    match path {
        "/" | "/index.json" => serve_index_json(skills),
        _ => {
            // Support two URL schemes:
            //   1. /skills/<name>/<file>  (task-specified format)
            //   2. /<name>/<file>          (opencode native format)
            let trimmed = path.trim_start_matches('/');
            if let Some(rest) = trimmed.strip_prefix("skills/") {
                serve_skill_file(rest, skills)
            } else if trimmed.contains('/') {
                // Direct /<name>/<file> — but only if first component is not "index"
                let first = trimmed.split('/').next().unwrap_or("");
                if first == "index" || first == "index.json" {
                    serve_index_json(skills)
                } else {
                    serve_skill_file(trimmed, skills)
                }
            } else {
                http_response(
                    "400 Bad Request",
                    "text/plain",
                    "Invalid path. Use /index.json or /<name>/<file>",
                )
            }
        }
    }
}

/// Build the manifest JSON in opencode's remote skill format.
fn serve_index_json(skills: &[crate::skill_resolver::SkillMeta]) -> String {
    let entries: Vec<serde_json::Value> = skills
        .iter()
        .map(|skill| {
            let files = list_skill_files(skill);
            json!({
                "name": skill.name,
                "files": files,
                "description": skill.description,
            })
        })
        .collect();

    let body = serde_json::to_string_pretty(&json!({ "skills": entries }))
        .unwrap_or_else(|_| "{}".to_string());
    http_response("200 OK", "application/json", &body)
}

/// Serve a skill file: path is "<name>/<filename>".
fn serve_skill_file(
    path: &str,
    skills: &[crate::skill_resolver::SkillMeta],
) -> String {
    let parts: Vec<&str> = path.splitn(2, '/').collect();
    if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
        return http_response(
            "400 Bad Request",
            "text/plain",
            "Expected path format: <skill_name>/<file_name>",
        );
    }

    let skill_name = parts[0];
    let file_name = parts[1];

    // Security: prevent path traversal
    if file_name.contains("..") || file_name.contains('/') || file_name.contains('\\') {
        return http_response("400 Bad Request", "text/plain", "Invalid file name");
    }

    // Find the skill
    let skill = match skills.iter().find(|s| s.name == skill_name) {
        Some(s) => s,
        None => {
            return http_response(
                "404 Not Found",
                "text/plain",
                &format!("Skill '{}' not found", skill_name),
            )
        }
    };

    let file_path = skill.source_dir.join(file_name);
    if !file_path.exists() || !file_path.is_file() {
        return http_response(
            "404 Not Found",
            "text/plain",
            &format!("File '{}' not found in skill '{}'", file_name, skill_name),
        );
    }

    match std::fs::read_to_string(&file_path) {
        Ok(content) => {
            let mime = mime_type(file_name);
            http_response("200 OK", mime, &content)
        }
        Err(e) => http_response(
            "500 Internal Server Error",
            "text/plain",
            &format!("Error reading file: {}", e),
        ),
    }
}

/// List non-hidden files in a skill's source directory.
fn list_skill_files(skill: &crate::skill_resolver::SkillMeta) -> Vec<String> {
    match skill.format {
        SkillFormat::SkillMd => {
            let mut files = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&skill.source_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            if !name.starts_with('.') {
                                files.push(name.to_string());
                            }
                        }
                    }
                }
            }
            files.sort();
            files
        }
        SkillFormat::TomlDatum => {
            // Source dir is the shared _b00t_ dir; find the specific skill datum file
            for ext in &[".skill.tomllmd", ".skill.tomllm", ".skill.toml"] {
                let path = skill.source_dir.join(format!("{}{}", skill.name, ext));
                if path.exists() {
                    return vec![format!("{}{}", skill.name, ext)];
                }
            }
            vec![]
        }
    }
}

/// Build a minimal HTTP response string.
fn http_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {}\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        status,
        content_type,
        body.len(),
        body
    )
}

/// Guess MIME type from file extension.
fn mime_type(filename: &str) -> &'static str {
    if filename.ends_with(".md") {
        "text/markdown"
    } else if filename.ends_with(".json") {
        "application/json"
    } else if filename.ends_with(".sh") || filename.ends_with(".bash") {
        "text/plain"
    } else if filename.ends_with(".toml") || filename.ends_with(".tomllm")
        || filename.ends_with(".tomllmd")
    {
        "text/plain"
    } else if filename.ends_with(".html") || filename.ends_with(".htm") {
        "text/html"
    } else if filename.ends_with(".css") {
        "text/css"
    } else if filename.ends_with(".js") || filename.ends_with(".mjs") {
        "application/javascript"
    } else if filename.ends_with(".ts") || filename.ends_with(".tsx") {
        "text/plain"
    } else if filename.ends_with(".yaml") || filename.ends_with(".yml") {
        "text/plain"
    } else {
        "application/octet-stream"
    }
}

// ── Sync (b00t skill sync) ────────────────────────────────────────────

/// Pull skills from a remote opencode-compatible URL.
/// Fetches `{url}/index.json` manifest, then downloads each skill file.
fn handle_sync(url: &str, output: &std::path::PathBuf) -> Result<()> {
    let base_url = url.trim_end_matches('/');

    // ── Fetch index.json ──────────────────────────────────────────────
    let manifest_url = format!("{}/index.json", base_url);
    let index_output = std::process::Command::new("curl")
        .args(["-s", "-f", &manifest_url])
        .output()
        .map_err(|e| anyhow::anyhow!("curl failed to fetch index.json: {}", e))?;

    if !index_output.status.success() {
        anyhow::bail!(
            "curl exited {:?} fetching {manifest_url} — is the remote server running?",
            index_output.status.code()
        );
    }

    let index: serde_json::Value = serde_json::from_slice(&index_output.stdout)
        .map_err(|e| anyhow::anyhow!("index.json parse error: {}", e))?;

    let skills = index["skills"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("index.json missing 'skills' array"))?;

    // ── Download each skill ───────────────────────────────────────────
    let mut total_files = 0usize;

    for skill_val in skills {
        let name = skill_val["name"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("skill entry missing 'name'"))?;

        let files = skill_val["files"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("skill '{name}' missing 'files' array"))?;

        let skill_dir = output.join(name);
        std::fs::create_dir_all(&skill_dir)
            .map_err(|e| anyhow::anyhow!("cannot create {skill_dir:?}: {e}"))?;

        for file_val in files {
            let file = file_val
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("file entry in '{name}' is not a string"))?;

            let file_url = format!("{base_url}/skills/{name}/{file}");
            let dest = skill_dir.join(file);

            let dl = std::process::Command::new("curl")
                .args(["-s", "-f", "-o", &dest.to_string_lossy(), &file_url])
                .output()
                .map_err(|e| anyhow::anyhow!("curl failed downloading {name}/{file}: {e}"))?;

            if !dl.status.success() {
                anyhow::bail!(
                    "curl exited {:?} downloading {name}/{file} from {file_url}",
                    dl.status.code()
                );
            }

            total_files += 1;
            println!("  ✓ {name}/{file}");
        }

        println!("  ✓ skill '{name}' ({} files)", files.len());
    }

    println!(
        "\nSynced {} skill(s) with {total_files} file(s) to {}",
        skills.len(),
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_skill_md(dir: &std::path::Path, name: &str, desc: &str) {
        let skill_dir = dir.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        let content = format!(
            "---\nname: {}\ndescription: {}\ntags:\n- test\napplies_to:\n- testing\noutput_types:\n- .txt\n---\n# {}\nDo the thing.\n",
            name, desc, name
        );
        let mut f = std::fs::File::create(skill_dir.join("SKILL.md")).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_skill_list_empty() {
        // Empty resolver — should not panic
        let resolver = SkillResolver::with_dirs(vec![]);
        let metas = resolver.list();
        assert!(metas.is_empty());
    }

    #[test]
    fn test_skill_search_no_match() {
        let resolver = SkillResolver::with_dirs(vec![]);
        let results = resolver.search("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_find_b00t_dir_project_local() {
        // Use a temporary project-local _b00t_ directory so the test is hermetic
        let original_cwd = std::env::current_dir().expect("failed to get current dir");

        // Create a unique temp project directory
        let temp_root =
            std::env::temp_dir().join(format!("b00t_test_find_b00t_dir_{}", std::process::id()));
        std::fs::create_dir_all(&temp_root).expect("failed to create temp project dir");

        // Create the project-local _b00t_ directory that find_b00t_dir() should discover
        let local_b00t = temp_root.join("_b00t_");
        std::fs::create_dir_all(&local_b00t).expect("failed to create _b00t_ dir");

        // Point current_dir at the temp project root so project-local lookup wins
        std::env::set_current_dir(&temp_root).expect("failed to set current dir to temp root");

        let result = find_b00t_dir();
        assert!(
            result.as_ref().is_ok_and(|p| p == &local_b00t),
            "expected project-local _b00t_ dir at {:?}, got: {:?}",
            local_b00t,
            result
        );

        // Restore original current directory
        std::env::set_current_dir(original_cwd).expect("failed to restore original dir");
    }
}
