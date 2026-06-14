use crate::traits::*;
use crate::{BootDatum, check_command_available, get_config};
use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct CliDatum {
    pub datum: BootDatum,
}

impl CliDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(CliDatum { datum: config.b00t })
    }
}

impl TryFrom<(&str, &str)> for CliDatum {
    type Error = anyhow::Error;

    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
    }
}

const VERSION_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

fn read_version_command(version_cmd: &str) -> Option<String> {
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(version_cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().ok()?;
                return status
                    .success()
                    .then(|| String::from_utf8_lossy(&output.stdout).to_string());
            }
            Ok(None) if start.elapsed() >= VERSION_PROBE_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(_) => return None,
        }
    }
}

impl DatumChecker for CliDatum {
    fn is_installed(&self) -> bool {
        if let Some(version_cmd) = &self.datum.version {
            read_version_command(version_cmd).is_some()
        } else {
            check_command_available(&self.datum.name)
        }
    }

    fn current_version(&self) -> Option<String> {
        if let Some(version_cmd) = &self.datum.version {
            if let Some(output) = read_version_command(version_cmd) {
                if let Some(regex) = &self.datum.version_regex {
                    if let Ok(re) = regex::Regex::new(regex) {
                        if let Some(caps) = re.captures(&output) {
                            return caps.get(1).map(|m| m.as_str().to_string());
                        }
                    }
                }
                return Some(output.trim().to_string());
            }
        }
        None
    }

    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }

    fn version_status(&self) -> VersionStatus {
        match (self.current_version(), self.desired_version()) {
            (Some(current), Some(desired)) => {
                use semver::Version;
                if let (Ok(curr_ver), Ok(des_ver)) =
                    (Version::parse(&current), Version::parse(&desired))
                {
                    match curr_ver.cmp(&des_ver) {
                        std::cmp::Ordering::Equal => VersionStatus::Match,
                        std::cmp::Ordering::Greater => VersionStatus::Newer,
                        std::cmp::Ordering::Less => VersionStatus::Older,
                    }
                } else {
                    // Fallback to string comparison if not semver
                    match current.cmp(&desired) {
                        std::cmp::Ordering::Equal => VersionStatus::Match,
                        _ => VersionStatus::Unknown,
                    }
                }
            }
            (Some(_), None) => VersionStatus::Unknown,
            (None, Some(_)) => VersionStatus::Missing,
            (None, None) => {
                if DatumChecker::is_installed(self) {
                    VersionStatus::Unknown
                } else {
                    VersionStatus::Missing
                }
            }
        }
    }
}

impl StatusProvider for CliDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }

    fn subsystem(&self) -> &str {
        "cli"
    }

    fn hint(&self) -> &str {
        &self.datum.hint
    }

    fn is_disabled(&self) -> bool {
        false // CLI tools are never disabled by default
    }
}

impl FilterLogic for CliDatum {
    fn is_available(&self) -> bool {
        !DatumChecker::is_installed(self)
    }

    fn prerequisites_satisfied(&self) -> bool {
        true // CLI tools have no special prerequisites
    }

    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
}

impl ConstraintEvaluator for CliDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for CliDatum {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl CliExecutor for CliDatum {
    fn execute(&self, args: &[String]) -> Result<ExecOutput<String>> {
        let name = &self.datum.name;
        if !check_command_available(name) {
            return Err(anyhow!("CLI tool not found: {}", name));
        }
        let start = Instant::now();
        let output = std::process::Command::new(name)
            .args(args)
            .output()
            .map_err(|e| anyhow!("failed to execute {}: {}", name, e))?;
        Ok(ExecOutput {
            value: String::from_utf8_lossy(&output.stdout).into_owned(),
            exit_code: output.status.code().unwrap_or(-1),
            duration_ms: start.elapsed().as_millis() as u64,
            sandbox: self.sandbox_requirements(),
            sandbox_kind: SandboxKind::None,
            io_method: IoMethod::Stdio,
            declared_effects: vec![],
        })
    }
    fn dry_run(&self, args: &[String]) -> Result<ExecPlan> {
        let cmd = std::iter::once(self.datum.name.as_str())
            .chain(args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ");
        Ok(ExecPlan {
            command_line: cmd,
            working_dir: PathBuf::from("."),
            env: vec![],
            declared_effects: vec![],
            sandbox_kind: SandboxKind::None,
            io_method: IoMethod::Stdio,
        })
    }
    fn list_commands(&self) -> Result<Vec<CommandSignature>> {
        Ok(vec![CommandSignature {
            name: self.datum.name.clone(),
            description: Some(self.datum.hint.clone()).filter(|h| !h.is_empty()),
            parameters: vec![ParameterSignature {
                name: "args".to_string(),
                default_value: None,
                required: false,
                kind: "star".to_string(),
            }],
            dependencies: vec![],
            private: false,
        }])
    }
    fn sandbox_requirements(&self) -> SandboxRequirements {
        SandboxRequirements::default()
    }
    fn allowed_sandboxes(&self) -> Vec<SandboxKind> {
        if let Some(s) = self
            .datum
            .justfile
            .as_ref()
            .and_then(|jf| jf.sandbox.as_deref())
        {
            vec![SandboxKind::from_str(s)]
        } else {
            vec![SandboxKind::None]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mkd(name: &str) -> CliDatum {
        CliDatum {
            datum: BootDatum {
                name: name.to_string(),
                hint: format!("{} CLI", name),
                ..BootDatum::default()
            },
        }
    }
    #[test]
    fn list_commands_returns_single_entry_for_cli() {
        let d = mkd("echo");
        let cmds = d.list_commands().unwrap();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].name, "echo");
        assert_eq!(cmds[0].parameters[0].kind, "star");
    }
    #[test]
    fn allowed_sandboxes_defaults_to_none() {
        assert_eq!(mkd("echo").allowed_sandboxes(), vec![SandboxKind::None]);
    }
    #[test]
    fn dry_run_builds_command_line() {
        let plan = mkd("echo").dry_run(&["hello".to_string()]).unwrap();
        assert!(plan.command_line.starts_with("echo"));
        assert!(plan.command_line.contains("hello"));
    }
    #[test]
    fn version_probe_times_out() {
        let start = Instant::now();
        let output = read_version_command("sleep 5; echo should-not-print");
        assert!(output.is_none());
        assert!(start.elapsed() < Duration::from_secs(4));
    }

    #[test]
    fn execute_captures_stdout() {
        let out = mkd("echo").execute(&["hello-b00t".to_string()]).unwrap();
        assert_eq!(out.exit_code, 0);
        assert_eq!(out.value.trim(), "hello-b00t");
    }
}
