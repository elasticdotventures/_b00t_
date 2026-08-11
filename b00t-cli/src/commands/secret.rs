// 🤓 b00t secret — standalone CLI resolution of a single SecretRef, for shell
//    scripts that need a secret value without going through StageSpec/
//    SecretStore's in-process-only path (pipeline_secrets.rs's load_secret()
//    was previously only ever called from Rust). Keyring/Prompt sources are
//    deliberately excluded here: Keyring is a broken placeholder (feature
//    flag exists, no keyring crate dependency), Prompt defeats the purpose
//    of a script-invoked, non-interactive call.
use crate::pipeline_secrets::{SecretRef, SecretSource, load_secret};
use anyhow::{Result, bail};
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum SecretCommands {
    #[clap(about = "Resolve a single secret from a file or environment variable and print it to stdout")]
    Resolve {
        #[clap(long, help = "Resolve from this file path (whitespace-trimmed)")]
        file: Option<String>,
        #[clap(long, help = "Resolve from this environment variable name")]
        env: Option<String>,
    },
}

pub fn handle_secret_command(cmd: &SecretCommands) -> Result<()> {
    match cmd {
        SecretCommands::Resolve { file, env } => {
            let source = match (file, env) {
                (Some(path), None) => SecretSource::File { path: path.clone() },
                (None, Some(name)) => SecretSource::EnvVar { name: name.clone() },
                (Some(_), Some(_)) => bail!("pass exactly one of --file or --env, not both"),
                (None, None) => bail!("pass one of --file <path> or --env <name>"),
            };
            let value = load_secret(&SecretRef {
                key: "cli-resolve".to_string(),
                env_var: String::new(),
                source,
            })?;
            println!("{value}");
            Ok(())
        }
    }
}
