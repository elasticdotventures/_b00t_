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
use std::io::Write;

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

/// Warning printed to stderr before `export-zone`'s JSON goes to stdout.
///
/// `export-zone` prints raw, unredacted secret values as JSON on stdout —
/// that's the contract Terraform's `data external` needs (Terraform reads
/// real values from stdout, no way around it). But nothing else guards
/// against those values being accidentally captured by `set -x` shell
/// tracing, `TF_LOG=DEBUG`, or CI log collectors. This gives a human running
/// the command interactively a visible heads-up. It is written to
/// **stderr only** — see [`write_export_zone_output`] — so it never mixes
/// into the JSON stream Terraform parses from stdout.
const EXPORT_ZONE_STDOUT_WARNING: &str =
    "⚠️  secret values follow on stdout — ensure this isn't captured by shell tracing (set -x), TF_LOG=DEBUG, or CI log capture";

/// Write `export-zone`'s output: the human warning to `warn_writer`
/// (production: stderr) and the JSON secret map to `json_writer`
/// (production: stdout). Split into its own function, parameterized over
/// writers, so tests can assert the warning never lands in the same stream
/// as the JSON Terraform consumes.
fn write_export_zone_output<W1: Write, W2: Write>(
    out: &BTreeMap<String, String>,
    warn_writer: &mut W1,
    json_writer: &mut W2,
) -> Result<()> {
    writeln!(warn_writer, "{EXPORT_ZONE_STDOUT_WARNING}")?;
    writeln!(json_writer, "{}", serde_json::to_string(out)?)?;
    Ok(())
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
            write_export_zone_output(&out, &mut std::io::stderr(), &mut std::io::stdout())?;
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

    // ── stdout/stderr separation for export-zone output ──────────────────

    #[test]
    fn warning_goes_to_stderr_stream_only() {
        let mut out = BTreeMap::new();
        out.insert("VULTR_API_KEY".to_string(), "s3cr3t".to_string());

        let mut warn_buf: Vec<u8> = Vec::new();
        let mut json_buf: Vec<u8> = Vec::new();
        write_export_zone_output(&out, &mut warn_buf, &mut json_buf).unwrap();

        let warn_str = String::from_utf8(warn_buf).unwrap();
        assert!(
            warn_str.contains("stdout"),
            "warning should mention stdout: {warn_str}"
        );
        assert!(
            warn_str.contains(EXPORT_ZONE_STDOUT_WARNING),
            "warn stream should contain the exact warning text"
        );
        // The warning must never contain the secret value itself.
        assert!(!warn_str.contains("s3cr3t"));
    }

    #[test]
    fn json_stream_is_uncorrupted_by_warning() {
        let mut out = BTreeMap::new();
        out.insert("VULTR_API_KEY".to_string(), "s3cr3t".to_string());
        out.insert("CLOUDFLARE_API_TOKEN".to_string(), "another-value".to_string());

        let mut warn_buf: Vec<u8> = Vec::new();
        let mut json_buf: Vec<u8> = Vec::new();
        write_export_zone_output(&out, &mut warn_buf, &mut json_buf).unwrap();

        let json_str = String::from_utf8(json_buf).unwrap();
        // The JSON stream must contain *only* valid JSON — no warning text,
        // no stray characters — since Terraform's `data external` parses
        // stdout directly.
        assert!(
            !json_str.contains("⚠️") && !json_str.contains("stdout"),
            "warning text must not leak into the JSON stream: {json_str}"
        );
        let parsed: BTreeMap<String, String> = serde_json::from_str(json_str.trim()).unwrap();
        assert_eq!(parsed, out);
    }

    #[test]
    fn warning_text_is_human_visible_and_actionable() {
        assert!(EXPORT_ZONE_STDOUT_WARNING.contains("stdout"));
        assert!(
            EXPORT_ZONE_STDOUT_WARNING.to_lowercase().contains("shell tracing")
                || EXPORT_ZONE_STDOUT_WARNING.contains("set -x")
        );
        assert!(EXPORT_ZONE_STDOUT_WARNING.contains("TF_LOG"));
    }
}
