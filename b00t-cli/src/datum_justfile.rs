use crate::traits::*;
use crate::{BootDatum, JustfileCapabilities, JustfileConfig, check_command_available, get_config};
use anyhow::{Context, Result, anyhow};
use duct::cmd;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// A registered justfile — the b00t-native executable unit.
///
/// Only registered justfile datums are visible to just-mcp.
/// The sandbox reads `[b00t.justfile.capabilities]` to scope the agent's
/// filesystem view via eBPF before recipe execution begins.
pub struct JustfileDatum {
    pub datum: BootDatum,
    pub justfile_path: PathBuf,
}

impl JustfileDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow!("{}", e))?;
        let datum = config.b00t;
        let justfile_path = Self::resolve_justfile_path(&datum, path)?;
        Ok(JustfileDatum { datum, justfile_path })
    }

    pub fn from_datum(datum: BootDatum, base_dir: &Path) -> Result<Self> {
        let justfile_path = Self::resolve_justfile_path(&datum, &base_dir.display().to_string())?;
        Ok(JustfileDatum { datum, justfile_path })
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

    fn capabilities(&self) -> JustfileCapabilities {
        self.justfile_config().capabilities.unwrap_or_default()
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
        // Justfiles don't carry versions — return the file hash as a change signal
        std::fs::read(&self.justfile_path)
            .ok()
            .map(|bytes| format!("{:x}", md5_simple(&bytes)))
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
        let output = cmd("just", &just_args)
            .dir(working_dir)
            .stderr_to_stdout()
            .read()
            .context("just execution failed")?;

        Ok(ExecOutput {
            value: output,
            exit_code: 0,
            duration_ms: start.elapsed().as_millis() as u64,
            sandbox: self.sandbox_requirements(),
            declared_effects: vec![],
        })
    }

    fn dry_run(&self, args: &[String]) -> Result<ExecPlan> {
        let mut just_args = vec![
            "--justfile".to_string(),
            self.justfile_path.display().to_string(),
            "--dry-run".to_string(),
        ];
        just_args.extend_from_slice(args);

        let command_line = format!("just {}", just_args.join(" "));
        let working_dir = self
            .justfile_path
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        Ok(ExecPlan {
            command_line,
            working_dir,
            env: vec![],
            declared_effects: if self.justfile_config().allow_side_effects.unwrap_or(true) {
                vec!["filesystem-write".to_string()]
            } else {
                vec![]
            },
        })
    }

    fn list_commands(&self) -> Result<Vec<CommandSignature>> {
        let output = cmd!(
            "just",
            "--justfile",
            self.justfile_path.display().to_string(),
            "--list",
            "--list-heading",
            "",
            "--list-prefix",
            ""
        )
        .read()
        .context("just --list failed")?;

        let signatures = output
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                let parts: Vec<&str> = line.splitn(2, ' ').collect();
                CommandSignature {
                    name: parts[0].trim().to_string(),
                    description: parts.get(1).map(|s| s.trim().to_string()),
                    parameters: vec![],
                    dependencies: vec![],
                }
            })
            .collect();

        Ok(signatures)
    }

    fn sandbox_requirements(&self) -> SandboxRequirements {
        let caps = self.capabilities();
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
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Minimal MD5-like hash for change detection — not crypto, just a fingerprint.
fn md5_simple(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf29ce484222325u64, |acc, &b| {
        acc.wrapping_mul(0x100000001b3).wrapping_add(b as u64)
    })
}
