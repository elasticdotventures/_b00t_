use crate::just_ast::{AstDiff, JustfileAst};
use crate::traits::*;
use crate::{BootDatum, JustfileConfig, check_command_available, get_config};
use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

/// A registered justfile — the b00t-native executable unit.
///
/// Carries a live JustfileAst loaded from `just --dump --dump-format json`.
/// Call `reload()` to detect changes and get a structural diff.
/// The sandbox kind vector declares which isolation contexts this justfile
/// is compatible with; the runtime picks the first one it supports.
pub struct JustfileDatum {
    pub datum: BootDatum,
    pub justfile_path: PathBuf,
    /// Live AST — lazily loaded; Mutex enables &self access (DatumProvider: Send+Sync).
    ast: Mutex<Option<JustfileAst>>,
}

impl JustfileDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow!("{}", e))?;
        let datum = config.b00t;
        let justfile_path = Self::resolve_justfile_path(&datum, path)?;
        Ok(JustfileDatum {
            datum,
            justfile_path,
            ast: Mutex::new(None),
        })
    }

    pub fn from_datum(datum: BootDatum, base_dir: &Path) -> Result<Self> {
        let justfile_path = Self::resolve_justfile_path(&datum, &base_dir.display().to_string())?;
        Ok(JustfileDatum {
            datum,
            justfile_path,
            ast: Mutex::new(None),
        })
    }

    fn ensure_ast(&self) -> Result<()> {
        let mut g = self.ast.lock().unwrap();
        if g.is_none() {
            *g = Some(JustfileAst::load(&self.justfile_path)?);
        }
        Ok(())
    }

    pub fn with_ast<T, F: FnOnce(&JustfileAst) -> T>(&self, f: F) -> Result<T> {
        self.ensure_ast()?;
        Ok(f(self.ast.lock().unwrap().as_ref().unwrap()))
    }

    pub fn reload(&self) -> Result<AstDiff> {
        self.ensure_ast()?;
        self.ast.lock().unwrap().as_mut().unwrap().reload()
    }

    fn resolve_justfile_path(datum: &BootDatum, base_dir: &str) -> Result<PathBuf> {
        let relative = datum
            .justfile
            .as_ref()
            .and_then(|jf| jf.path.as_deref())
            .unwrap_or("justfile");

        let base = Path::new(base_dir);
        let path = if Path::new(relative).is_absolute() {
            PathBuf::from(relative)
        } else {
            base.join(relative)
        };

        // Try the declared path, then common fallbacks
        if path.exists() {
            return Ok(path);
        }
        for fallback in &["justfile", "Justfile", ".justfile"] {
            let fb = base.join(fallback);
            if fb.exists() {
                return Ok(fb);
            }
        }
        Ok(path) // Return declared path even if missing — is_installed() handles it
    }

    fn justfile_config(&self) -> JustfileConfig {
        self.datum.justfile.clone().unwrap_or_default()
    }
}

impl TryFrom<(&str, &str)> for JustfileDatum {
    type Error = anyhow::Error;
    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
    }
}

impl DatumChecker for JustfileDatum {
    fn is_installed(&self) -> bool {
        self.justfile_path.exists() && check_command_available("just")
    }

    fn current_version(&self) -> Option<String> {
        // Use mtime as the "version" — changes are detected via AstDiff
        std::fs::metadata(&self.justfile_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs().to_string())
                    .unwrap_or_else(|_| "unknown".to_string())
            })
    }

    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }

    fn version_status(&self) -> VersionStatus {
        if !self.justfile_path.exists() {
            VersionStatus::Missing
        } else if !check_command_available("just") {
            VersionStatus::Missing
        } else {
            VersionStatus::Unknown // justfiles don't carry semver
        }
    }
}

impl StatusProvider for JustfileDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }
    fn subsystem(&self) -> &str {
        "justfile"
    }
    fn hint(&self) -> &str {
        &self.datum.hint
    }
    fn is_disabled(&self) -> bool {
        false
    }
}

impl FilterLogic for JustfileDatum {
    fn is_available(&self) -> bool {
        self.justfile_path.exists()
    }
    fn prerequisites_satisfied(&self) -> bool {
        check_command_available("just")
    }
    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
}

impl ConstraintEvaluator for JustfileDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for JustfileDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

// ── CliExecutor impl ──────────────────────────────────────────────────────────

impl CliExecutor for JustfileDatum {
    fn execute(&self, args: &[String]) -> Result<ExecOutput<String>> {
        if !self.justfile_path.exists() {
            return Err(anyhow!(
                "justfile not found: {}",
                self.justfile_path.display()
            ));
        }

        let working_dir = self
            .justfile_path
            .parent()
            .unwrap_or_else(|| Path::new("."));

        let mut just_args = vec![
            "--justfile".to_string(),
            self.justfile_path.display().to_string(),
        ];
        just_args.extend_from_slice(args);

        let start = Instant::now();
        let output = std::process::Command::new("just")
            .args(&just_args)
            .current_dir(working_dir)
            .output()
            .context("just execution failed")?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok(ExecOutput {
            value: stdout,
            exit_code,
            duration_ms: start.elapsed().as_millis() as u64,
            sandbox: self.sandbox_requirements(),
            sandbox_kind: self
                .allowed_sandboxes()
                .into_iter()
                .next()
                .unwrap_or(SandboxKind::None),
            io_method: self.io_method(),
            declared_effects: if self.justfile_config().allow_side_effects.unwrap_or(true) {
                vec!["filesystem-write".to_string()]
            } else {
                vec![]
            },
        })
    }

    fn dry_run(&self, args: &[String]) -> Result<ExecPlan> {
        let mut just_args = vec![
            "--justfile".to_string(),
            self.justfile_path.display().to_string(),
            "--dry-run".to_string(),
        ];
        just_args.extend_from_slice(args);

        let working_dir = self
            .justfile_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        let sandbox_kind = self
            .allowed_sandboxes()
            .into_iter()
            .next()
            .unwrap_or(SandboxKind::None);
        let io_method = IoMethod::for_sandbox(&sandbox_kind);

        Ok(ExecPlan {
            command_line: format!("just {}", just_args.join(" ")),
            working_dir,
            env: vec![],
            declared_effects: if self.justfile_config().allow_side_effects.unwrap_or(true) {
                vec!["filesystem-write".to_string()]
            } else {
                vec![]
            },
            sandbox_kind,
            io_method,
        })
    }

    fn list_commands(&self) -> Result<Vec<CommandSignature>> {
        self.with_ast(|ast| {
            let mut recipes = ast.recipes_sorted();
            recipes.retain(|r| !r.private);
            recipes
                .iter()
                .map(|r| CommandSignature {
                    name: r.name.clone(),
                    description: r.doc.clone(),
                    parameters: r
                        .parameters
                        .iter()
                        .map(|p| ParameterSignature {
                            name: p.name.clone(),
                            default_value: p.default.clone(),
                            required: p.default.is_none() && p.kind == "singular",
                            kind: p.kind.clone(),
                        })
                        .collect(),
                    dependencies: r.dependencies.clone(),
                    private: r.private,
                })
                .collect()
        })
    }

    fn sandbox_requirements(&self) -> SandboxRequirements {
        let caps = self.justfile_config().capabilities.unwrap_or_default();
        let justfile_dir = self
            .justfile_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        let mut filesystem = vec![justfile_dir];
        if let Some(paths) = caps.filesystem {
            for p in paths {
                let path = if Path::new(&p).is_absolute() {
                    PathBuf::from(&p)
                } else {
                    self.justfile_path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(&p)
                };
                if !filesystem.contains(&path) {
                    filesystem.push(path);
                }
            }
        }

        SandboxRequirements {
            network: caps.network.unwrap_or(false),
            filesystem,
            env_vars: caps.env_vars.unwrap_or_default(),
            secrets: caps.secrets.unwrap_or_default(),
            max_duration: None,
        }
    }

    fn allowed_sandboxes(&self) -> Vec<SandboxKind> {
        let config = self.justfile_config();
        if let Some(kinds) = config.allowed_sandboxes {
            kinds.iter().map(|s| SandboxKind::from_str(s)).collect()
        } else if let Some(single) = config.sandbox {
            vec![SandboxKind::from_str(&single)]
        } else {
            // Default: None (direct execution) is always permitted
            vec![SandboxKind::None]
        }
    }

    fn io_method(&self) -> IoMethod {
        let preferred_sandbox = self
            .allowed_sandboxes()
            .into_iter()
            .next()
            .unwrap_or(SandboxKind::None);
        IoMethod::for_sandbox(&preferred_sandbox)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    fn tmp(c: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", c).unwrap();
        f
    }
    fn mkd(path: &std::path::Path) -> JustfileDatum {
        JustfileDatum {
            datum: BootDatum::default(),
            justfile_path: path.to_path_buf(),
            ast: Mutex::new(None),
        }
    }
    #[test]
    fn list_commands_uses_ast_cache() {
        let f = tmp("# Build\nbuild:\n    echo build\n\n# Test\ntest:\n    echo test\n");
        let d = mkd(f.path());
        let first = d.list_commands().unwrap();
        let second = d.list_commands().unwrap();
        assert!(d.ast.lock().unwrap().is_some());
        assert_eq!(first.len(), second.len());
        let names: Vec<&str> = first.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"build") && names.contains(&"test"));
    }
    #[test]
    fn ensure_ast_caches_on_first_call() {
        let f = tmp("hello:\n    echo hello\n");
        let d = mkd(f.path());
        assert!(d.ast.lock().unwrap().is_none());
        d.ensure_ast().unwrap();
        assert!(d.ast.lock().unwrap().is_some());
        d.ensure_ast().unwrap(); // no double-lock panic
    }
    #[test]
    fn list_commands_excludes_private_recipes() {
        let f = tmp("pub-recipe:\n    echo public\n\n[private]\n_priv:\n    echo private\n");
        let d = mkd(f.path());
        let cmds = d.list_commands().unwrap();
        let names: Vec<&str> = cmds.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"pub-recipe"));
        assert!(!names.contains(&"_priv"));
    }
}
