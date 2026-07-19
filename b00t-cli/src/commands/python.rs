//! `b00t python` — Python version management
//!
//! minimum-version  — print the canonical minimum Python version from PYTHON-MINIMUM datum
//! current          — show current uv project python version

use anyhow::Result;
use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct PythonMinimum {
    b00t: PythonMinimumB00t,
}

#[derive(Deserialize)]
struct PythonMinimumB00t {
    python: PythonMinimumConfig,
}

#[derive(Deserialize)]
struct PythonMinimumConfig {
    minimum: String,
}

#[derive(Parser)]
pub enum PythonCommands {
    #[clap(
        about = "Print canonical minimum Python version",
        long_about = "Reads the PYTHON-MINIMUM datum and prints the minimum required version.\n\nThe datum is the single source of truth for Python version requirements.\n\nExamples:\n  b00t python minimum-version\n  b00t python minimum-version --datum /custom/path"
    )]
    MinimumVersion {
        #[clap(long, help = "Path to _b00t_ directory (default: auto-detect)")]
        datum: Option<String>,
    },
    #[clap(
        about = "Show current uv project python version (.python-version)",
        long_about = "Prints the current uv-managed Python version from .python-version file.\n\nExamples:\n  b00t python current\n  b00t python current --project /path/to/project"
    )]
    Current {
        #[clap(long, help = "Project root path (default: current directory)")]
        project: Option<String>,
    },
}

impl PythonCommands {
    pub fn execute(&self, _b00t_path: &str) -> Result<()> {
        match self {
            PythonCommands::MinimumVersion { datum } => {
                let path = datum.clone().unwrap_or_else(|| {
                    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                    format!("{home}/.b00t/_b00t_/datums/PYTHON-MINIMUM.tomllmd")
                });
                let raw = std::fs::read_to_string(&path)
                    .map_err(|e| anyhow::anyhow!("Cannot read PYTHON-MINIMUM datum at {path}: {e}"))?;
                // Strip comment lines (#) before TOML parsing — .tomllmd has leading comments.
                let toml_src: String = raw.lines()
                    .filter(|l| !l.trim_start().starts_with('#'))
                    .collect::<Vec<_>>()
                    .join("\n");
                let toml_src = toml_src.trim();
                let cfg: PythonMinimum = toml::from_str(toml_src)
                    .map_err(|e| anyhow::anyhow!("Cannot parse PYTHON-MINIMUM datum: {e}"))?;
                println!("{}", cfg.b00t.python.minimum);
                Ok(())
            }
            PythonCommands::Current { project } => {
                let dir = project.clone().unwrap_or_else(|| ".".to_string());
                let mut path = PathBuf::from(&dir);
                path.push(".python-version");
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| anyhow::anyhow!(
                        "No .python-version found at {dir} (run 'uv python pin 3.14'): {e}"))?;
                print!("{content}");
                Ok(())
            }
        }
    }
}
