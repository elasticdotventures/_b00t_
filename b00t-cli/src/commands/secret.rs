// 🤓 b00t secret — standalone CLI resolution of a single SecretRef, for shell
//    scripts that need a secret value without going through StageSpec/
//    SecretStore's in-process-only path (pipeline_secrets.rs's load_secret()
//    was previously only ever called from Rust). Keyring/Prompt sources are
//    deliberately excluded here: Keyring is a broken placeholder (feature
//    flag exists, no keyring crate dependency), Prompt defeats the purpose
//    of a script-invoked, non-interactive call.
use crate::pipeline_secrets::{SecretRef, SecretSource, list_azure_secret_names, load_secret};
use anyhow::{Result, bail};
use clap::Subcommand;
use std::collections::BTreeMap;

#[derive(Debug, Subcommand)]
pub enum SecretCommands {
    #[clap(about = "Resolve a single secret from a file, environment variable, or Azure Key Vault, and print it to stdout")]
    Resolve {
        #[clap(long, help = "Resolve from this file path (whitespace-trimmed)")]
        file: Option<String>,
        #[clap(long, help = "Resolve from this environment variable name")]
        env: Option<String>,
        #[clap(long, help = "Resolve from this Azure Key Vault name (requires --azure-secret; uses the active `az login` session)", requires = "azure_secret")]
        azure_vault: Option<String>,
        #[clap(long, help = "Secret name within --azure-vault", requires = "azure_vault")]
        azure_secret: Option<String>,
    },
    #[clap(about = "Bulk-export all secrets for a base/zone/org from a cloud secret store, for consumption by Terraform's `data external` (see infrastructure repo's secretEnvy module)")]
    ExportZone {
        #[clap(long, help = "Secret store provider — only 'azure' is currently supported")]
        provider: String,
        #[clap(long, help = "Azure Key Vault name")]
        vault: String,
        #[clap(long, default_value = "config", help = "Base path segment for the name prefix")]
        base: String,
        #[clap(long, help = "Zone path segment for the name prefix (e.g. 'global', 'test', 'live')")]
        zone: String,
        #[clap(long, help = "Optional org path segment for the name prefix")]
        org: Option<String>,
        #[clap(long, help = "Output as a flat JSON object of ENV_VAR: value, for Terraform's `data external` contract")]
        tf: bool,
    },
}

/// Build the Azure Key Vault secret-name prefix for a base/zone/org triple.
/// Key Vault secret names only allow alphanumerics and hyphens, so unlike
/// AWS (`/`-joined) or GCP (`-`-joined) this is always hyphen-joined.
fn azure_export_prefix(base: &str, zone: &str, org: Option<&str>) -> String {
    match org {
        Some(org) => format!("{base}-{zone}-{org}-"),
        None => format!("{base}-{zone}-"),
    }
}

/// Derive an env-var name from an Azure Key Vault secret name, e.g.
/// `vultr-api-key` -> `VULTR_API_KEY`.
fn azure_secret_name_to_env_var(name: &str) -> String {
    name.to_uppercase().replace('-', "_")
}

pub fn handle_secret_command(cmd: &SecretCommands) -> Result<()> {
    match cmd {
        SecretCommands::Resolve {
            file,
            env,
            azure_vault,
            azure_secret,
        } => {
            let source = match (file, env, azure_vault, azure_secret) {
                (Some(path), None, None, None) => SecretSource::File { path: path.clone() },
                (None, Some(name), None, None) => SecretSource::EnvVar { name: name.clone() },
                (None, None, Some(vault), Some(name)) => SecretSource::AzureKeyVault {
                    vault: vault.clone(),
                    name: name.clone(),
                },
                (None, None, None, None) => {
                    bail!("pass one of --file <path>, --env <name>, or --azure-vault <vault> --azure-secret <name>")
                }
                _ => bail!(
                    "pass exactly one of --file, --env, or --azure-vault/--azure-secret together, not a mix"
                ),
            };
            let value = load_secret(&SecretRef {
                key: "cli-resolve".to_string(),
                env_var: String::new(),
                source,
            })?;
            println!("{value}");
            Ok(())
        }
        SecretCommands::ExportZone {
            provider,
            vault,
            base,
            zone,
            org,
            tf,
        } => {
            if provider != "azure" {
                bail!("unsupported provider '{provider}' — only 'azure' is currently supported");
            }
            if !tf {
                bail!("--tf is the only supported output mode currently");
            }
            let prefix = azure_export_prefix(base, zone, org.as_deref());
            let names = list_azure_secret_names(vault, &prefix)?;
            let mut out = BTreeMap::new();
            for name in names {
                let value = load_secret(&SecretRef {
                    key: name.clone(),
                    env_var: String::new(),
                    source: SecretSource::AzureKeyVault {
                        vault: vault.clone(),
                        name: name.clone(),
                    },
                })?;
                out.insert(azure_secret_name_to_env_var(&name), value);
            }
            println!("{}", serde_json::to_string(&out)?);
            Ok(())
        }
    }
}

#[cfg(test)]
mod export_zone_tests {
    use super::*;

    #[test]
    fn prefix_without_org() {
        assert_eq!(azure_export_prefix("config", "global", None), "config-global-");
    }

    #[test]
    fn prefix_with_org() {
        assert_eq!(
            azure_export_prefix("config", "global", Some("app4dog")),
            "config-global-app4dog-"
        );
    }

    #[test]
    fn env_var_derivation() {
        assert_eq!(azure_secret_name_to_env_var("vultr-api-key"), "VULTR_API_KEY");
        assert_eq!(azure_secret_name_to_env_var("cloudflare-api-token"), "CLOUDFLARE_API_TOKEN");
        assert_eq!(azure_secret_name_to_env_var("already-upper-ISH"), "ALREADY_UPPER_ISH");
    }
}
