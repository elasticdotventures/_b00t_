use anyhow::{Context, Result};
use clap::Parser;
use duct::cmd;
use reqwest::blocking::Client;
use semver::Version;
use serde::Deserialize;

const DEFAULT_REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const DEFAULT_INSTALLER_REF: &str = "main";

#[derive(Parser, Debug, Clone)]
pub enum VersionCommands {
    #[clap(about = "Check the installed b00t-cli version against the latest GitHub release")]
    Check,
    #[clap(
        about = "Upgrade b00t-cli using the repository installer",
        long_about = "Upgrade b00t-cli using the repository installer.\n\nBy default this only prints the exact upgrade command.\nUse --yes to execute it."
    )]
    Upgrade {
        #[clap(
            short = 'y',
            long = "yes",
            help = "Execute the upgrade instead of printing it"
        )]
        yes: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseStatus {
    current: String,
    latest: String,
    release_url: String,
    upgrade_available: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

pub fn handle_version_command(command: &VersionCommands) -> Result<()> {
    match command {
        VersionCommands::Check => {
            let status = fetch_release_status()?;
            print_status(&status);
            Ok(())
        }
        VersionCommands::Upgrade { yes } => {
            let status = fetch_release_status()?;
            print_status(&status);

            if !status.upgrade_available {
                println!("b00t-cli is already current.");
                return Ok(());
            }

            let installer = installer_url();
            let install_cmd = format!("curl -fsSL {} | sh", installer);

            if !yes {
                println!("Upgrade available. Re-run with:");
                println!("  b00t-cli version upgrade --yes");
                println!("Installer:");
                println!("  {}", install_cmd);
                return Ok(());
            }

            cmd!(
                "bash",
                "-lc",
                "curl -fsSL \"$1\" | sh",
                "b00t-cli-upgrade",
                installer.as_str()
            )
            .run()
            .with_context(|| format!("failed to run installer {}", installer))?;

            println!("Upgrade complete. Verify with `b00t-cli --version`.");
            Ok(())
        }
    }
}

fn fetch_release_status() -> Result<ReleaseStatus> {
    let current = b00t_c0re_lib::version::VERSION.to_string();
    let release = fetch_latest_release()?;
    let latest = normalize_tag(&release.tag_name);

    Ok(ReleaseStatus {
        current: current.clone(),
        latest: latest.clone(),
        release_url: release.html_url,
        upgrade_available: is_upgrade_available(&current, &latest),
    })
}

fn fetch_latest_release() -> Result<GitHubRelease> {
    let client = Client::builder().build()?;
    let response = client
        .get(release_api_url())
        .header(reqwest::header::USER_AGENT, "b00t-cli")
        .send()
        .context("failed to fetch latest GitHub release")?
        .error_for_status()
        .context("GitHub release API returned an error")?;

    response
        .json::<GitHubRelease>()
        .context("failed to parse GitHub release response")
}

fn print_status(status: &ReleaseStatus) {
    println!("current: {}", status.current);
    println!("latest: {}", status.latest);
    println!("release: {}", status.release_url);

    if status.upgrade_available {
        println!("status: upgrade available");
    } else {
        println!("status: up to date");
    }
}

fn is_upgrade_available(current: &str, latest: &str) -> bool {
    match (Version::parse(current), Version::parse(latest)) {
        (Ok(current), Ok(latest)) => latest > current,
        _ => current != latest,
    }
}

fn normalize_tag(tag: &str) -> String {
    tag.trim_start_matches('v').to_string()
}

fn release_api_url() -> String {
    std::env::var("B00T_RELEASE_API_URL").unwrap_or_else(|_| {
        format!(
            "https://api.github.com/repos/{}/releases/latest",
            repository_slug(DEFAULT_REPOSITORY)
        )
    })
}

fn installer_url() -> String {
    std::env::var("B00T_INSTALLER_URL").unwrap_or_else(|_| {
        format!(
            "https://raw.githubusercontent.com/{}/{}/install.sh",
            repository_slug(DEFAULT_REPOSITORY),
            DEFAULT_INSTALLER_REF
        )
    })
}

fn repository_slug(repository: &str) -> String {
    repository
        .trim_end_matches('/')
        .strip_prefix("https://github.com/")
        .unwrap_or(repository)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_v_prefix_from_tags() {
        assert_eq!(normalize_tag("v0.7.35"), "0.7.35");
        assert_eq!(normalize_tag("0.7.35"), "0.7.35");
    }

    #[test]
    fn compares_semver_for_upgrades() {
        assert!(is_upgrade_available("0.7.35", "0.7.36"));
        assert!(!is_upgrade_available("0.7.35", "0.7.35"));
        assert!(!is_upgrade_available("0.7.36", "0.7.35"));
    }

    #[test]
    fn extracts_repository_slug_from_url() {
        assert_eq!(
            repository_slug("https://github.com/elasticdotventures/dotfiles"),
            "elasticdotventures/dotfiles"
        );
        assert_eq!(
            repository_slug("elasticdotventures/dotfiles"),
            "elasticdotventures/dotfiles"
        );
    }
}
