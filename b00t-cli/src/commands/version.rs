use anyhow::{Context, Result};
use b00t_chat::{ChatClient, ChatMessage, ChatTransportConfig, ChatTransportKind};
use clap::{Parser, Subcommand, ValueEnum};
use duct::cmd;
use reqwest::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::future::Future;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const DEFAULT_INSTALLER_REF: &str = "main";

#[derive(Parser, Debug, Clone)]
pub enum VersionCommands {
    #[clap(about = "Check the installed b00t-cli version against the latest GitHub release")]
    Check,
    #[clap(about = "Persist or inspect upgrade channel policy")]
    Channel {
        #[clap(subcommand)]
        channel_command: VersionChannelCommands,
    },
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
        #[clap(long, value_enum, help = "Upgrade strategy to use")]
        strategy: Option<UpgradeStrategy>,
        #[clap(long, help = "Send start/finish notifications to a b00t chat channel")]
        channel: Option<String>,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpgradeStrategy {
    ReleaseInstaller,
    WorkspaceBuild,
    WorkspaceSync,
}

#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum VersionChannelCommands {
    #[clap(about = "Show the persisted upgrade channel policy")]
    Show,
    #[clap(about = "Persist the default upgrade strategy and optional notify channel")]
    Set {
        #[clap(value_enum)]
        strategy: UpgradeStrategy,
        #[clap(long, help = "Default b00t chat channel for upgrade notifications")]
        notify_channel: Option<String>,
    },
    #[clap(about = "Clear the persisted upgrade channel policy")]
    Clear,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
struct VersionChannelConfig {
    strategy: Option<UpgradeStrategy>,
    notify_channel: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseStatus {
    current: String,
    latest: String,
    release_url: String,
    workspace_version: Option<String>,
    upgrade_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UpgradeContext {
    detected_agent: String,
    workspace_root: Option<PathBuf>,
    git_branch: Option<String>,
    git_clean: bool,
}

impl UpgradeContext {
    fn in_workspace(&self) -> bool {
        self.workspace_root.is_some()
    }

    fn is_claude(&self) -> bool {
        self.detected_agent.eq_ignore_ascii_case("claude")
            || self.detected_agent.to_ascii_lowercase().contains("claude")
    }
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
            let context = gather_upgrade_context();
            print_status(&status, &context);
            Ok(())
        }
        VersionCommands::Channel { channel_command } => handle_channel_command(channel_command),
        VersionCommands::Upgrade {
            yes,
            strategy,
            channel,
        } => {
            let status = fetch_release_status()?;
            let context = gather_upgrade_context();
            let config = load_version_channel_config()?;
            print_status(&status, &context);

            let selected_channel = channel.clone().or_else(|| config.notify_channel.clone());
            let recommended = strategy
                .or(config.strategy)
                .unwrap_or_else(|| recommend_strategy(&status, &context));

            if !status.upgrade_available
                && !matches!(
                    recommended,
                    UpgradeStrategy::WorkspaceBuild | UpgradeStrategy::WorkspaceSync
                )
            {
                println!("b00t-cli is already current.");
                return Ok(());
            }

            if !yes {
                let selected = prompt_for_strategy(recommended, &context);
                println!("plan: {}", strategy_description(selected));
                println!(
                    "re-run: {}",
                    upgrade_command_hint(selected, selected_channel.as_deref())
                );

                if io::stdin().is_terminal() && confirm_execute_now()? {
                    execute_upgrade(selected, selected_channel.as_deref(), &status, &context)?;
                    return Ok(());
                }

                return Ok(());
            }

            execute_upgrade(recommended, selected_channel.as_deref(), &status, &context)?;
            Ok(())
        }
    }
}

fn handle_channel_command(command: &VersionChannelCommands) -> Result<()> {
    match command {
        VersionChannelCommands::Show => {
            let config = load_version_channel_config()?;
            println!(
                "strategy: {}",
                config.strategy.map(strategy_name).unwrap_or("unset")
            );
            println!(
                "notify_channel: {}",
                config.notify_channel.as_deref().unwrap_or("unset")
            );
            Ok(())
        }
        VersionChannelCommands::Set {
            strategy,
            notify_channel,
        } => {
            let config = VersionChannelConfig {
                strategy: Some(*strategy),
                notify_channel: notify_channel.clone(),
            };
            save_version_channel_config(&config)?;
            println!("strategy: {}", strategy_name(*strategy));
            println!(
                "notify_channel: {}",
                config.notify_channel.as_deref().unwrap_or("unset")
            );
            Ok(())
        }
        VersionChannelCommands::Clear => {
            save_version_channel_config(&VersionChannelConfig::default())?;
            println!("strategy: unset");
            println!("notify_channel: unset");
            Ok(())
        }
    }
}

fn fetch_release_status() -> Result<ReleaseStatus> {
    let current = b00t_c0re_lib::version::VERSION.to_string();
    let release = fetch_latest_release()?;
    let latest = normalize_tag(&release.tag_name);
    let workspace_version = detect_workspace_version();

    let github_upgrade = is_upgrade_available(&current, &latest);
    let workspace_upgrade = workspace_version
        .as_deref()
        .map(|wv| is_upgrade_available(&current, wv))
        .unwrap_or(false);

    Ok(ReleaseStatus {
        current: current.clone(),
        latest: latest.clone(),
        release_url: release.html_url,
        workspace_version,
        upgrade_available: github_upgrade || workspace_upgrade,
    })
}

fn detect_workspace_version() -> Option<String> {
    let root = find_workspace_root(&std::env::current_dir().ok()?)?;
    let cargo_toml = root.join("b00t-cli").join("Cargo.toml");
    let content = std::fs::read_to_string(cargo_toml).ok()?;
    let table: toml::Value = content.parse().ok()?;
    table.get("package")?.get("version")?.as_str().map(|v| v.trim().to_string())
}

fn fetch_latest_release() -> Result<GitHubRelease> {
    let client = Client::builder().build()?;
    block_on_version_future(async move {
        let response = client
            .get(release_api_url())
            .header(reqwest::header::USER_AGENT, "b00t-cli")
            .send()
            .await
            .context("failed to fetch latest GitHub release")?
            .error_for_status()
            .context("GitHub release API returned an error")?;

        response
            .json::<GitHubRelease>()
            .await
            .context("failed to parse GitHub release response")
    })
}

fn print_status(status: &ReleaseStatus, context: &UpgradeContext) {
    println!("current: {}", status.current);
    println!("commit: {}", env!("GIT_HASH"));
    println!("built: {}", env!("BUILD_TIMESTAMP"));
    println!("latest: {}", status.latest);
    println!("release: {}", status.release_url);
    println!("agent: {}", context.detected_agent);

    if let Some(workspace_root) = &context.workspace_root {
        println!("workspace: {}", workspace_root.display());
        if let Some(wv) = &status.workspace_version {
            println!("workspace version: {}", wv);
        }
        if let Some(branch) = &context.git_branch {
            let cleanliness = if context.git_clean { "clean" } else { "dirty" };
            println!("git: {} ({})", branch, cleanliness);
        }
    } else {
        println!("workspace: none");
    }

    // Only show _B00T_Path when overridden from default
    if let Ok(b00t_path) = std::env::var("_B00T_Path") {
        let expanded_path = shellexpand::tilde(&b00t_path).to_string();
        let default = shellexpand::tilde("~/.b00t/_b00t_").to_string();
        if expanded_path != default {
            println!("path: {}", b00t_path);
        }
    }

    if status.upgrade_available {
        println!("status: upgrade available");
    } else {
        println!("status: up to date");
    }

    println!(
        "recommended: {}",
        recommend_strategy(status, context)
            .to_possible_value()
            .unwrap()
            .get_name()
    );
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

fn strategy_name(strategy: UpgradeStrategy) -> &'static str {
    match strategy {
        UpgradeStrategy::ReleaseInstaller => "release-installer",
        UpgradeStrategy::WorkspaceBuild => "workspace-build",
        UpgradeStrategy::WorkspaceSync => "workspace-sync",
    }
}

fn version_channel_config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir().context("failed to resolve config directory")?;
    Ok(config_dir.join("b00t").join("version-channel.toml"))
}

fn load_version_channel_config() -> Result<VersionChannelConfig> {
    let path = version_channel_config_path()?;
    if !path.exists() {
        return Ok(VersionChannelConfig::default());
    }

    confy::load_path(path).context("failed to load version channel config")
}

fn save_version_channel_config(config: &VersionChannelConfig) -> Result<()> {
    let path = version_channel_config_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("failed to create b00t config directory")?;
    }
    confy::store_path(path, config).context("failed to save version channel config")
}

fn block_on_version_future<F, T>(future: F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        tokio::runtime::Runtime::new()
            .context("failed to create runtime for version command")?
            .block_on(future)
    }
}

fn gather_upgrade_context() -> UpgradeContext {
    let detected_agent = crate::whoami::detect_agent(false);
    let cwd = std::env::current_dir().ok();
    let workspace_root = cwd.as_deref().and_then(find_workspace_root);

    let git_branch = workspace_root
        .as_deref()
        .and_then(|root| git_stdout(root, &["rev-parse", "--abbrev-ref", "HEAD"]));
    let git_clean = workspace_root.as_deref().map(is_git_clean).unwrap_or(false);

    UpgradeContext {
        detected_agent,
        workspace_root,
        git_branch,
        git_clean,
    }
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();

    loop {
        if dir.join("b00t-cli/Cargo.toml").exists() && dir.join("install.sh").exists() {
            return Some(dir);
        }

        if !dir.pop() {
            return None;
        }
    }
}

fn git_stdout(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn is_git_clean(root: &Path) -> bool {
    Command::new("git")
        .args(["status", "--short"])
        .current_dir(root)
        .output()
        .map(|output| output.status.success() && output.stdout.is_empty())
        .unwrap_or(false)
}

fn recommend_strategy(status: &ReleaseStatus, context: &UpgradeContext) -> UpgradeStrategy {
    if context.in_workspace() && context.is_claude() && context.git_clean {
        return UpgradeStrategy::WorkspaceSync;
    }

    if context.in_workspace() && (context.is_claude() || !status.upgrade_available) {
        return UpgradeStrategy::WorkspaceBuild;
    }

    UpgradeStrategy::ReleaseInstaller
}

fn strategy_description(strategy: UpgradeStrategy) -> &'static str {
    match strategy {
        UpgradeStrategy::ReleaseInstaller => "download latest GitHub release installer",
        UpgradeStrategy::WorkspaceBuild => {
            "compile the current workspace and self-install b00t binaries"
        }
        UpgradeStrategy::WorkspaceSync => {
            "git fetch/pull the current workspace, then compile and self-install"
        }
    }
}

fn prompt_for_strategy(default: UpgradeStrategy, context: &UpgradeContext) -> UpgradeStrategy {
    if !io::stdin().is_terminal() {
        return default;
    }

    println!("menu:");
    println!(
        "  1. release-installer{}",
        if default == UpgradeStrategy::ReleaseInstaller {
            " [default]"
        } else {
            ""
        }
    );
    println!(
        "  2. workspace-build{}",
        if default == UpgradeStrategy::WorkspaceBuild {
            " [default]"
        } else {
            ""
        }
    );
    println!(
        "  3. workspace-sync{}",
        if default == UpgradeStrategy::WorkspaceSync {
            " [default]"
        } else {
            ""
        }
    );
    println!(
        "  note: agent={} workspace={}",
        context.detected_agent,
        context.in_workspace()
    );
    print!("choice [1-3, Enter=default]: ");
    let _ = io::stdout().flush();

    let mut choice = String::new();
    if io::stdin().read_line(&mut choice).is_err() {
        return default;
    }

    match choice.trim() {
        "" => default,
        "1" => UpgradeStrategy::ReleaseInstaller,
        "2" => UpgradeStrategy::WorkspaceBuild,
        "3" => UpgradeStrategy::WorkspaceSync,
        _ => default,
    }
}

fn confirm_execute_now() -> Result<bool> {
    print!("execute now? [y/N]: ");
    io::stdout().flush().context("failed to flush stdout")?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .context("failed to read confirmation")?;

    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "YES"))
}

fn upgrade_command_hint(strategy: UpgradeStrategy, channel: Option<&str>) -> String {
    let mut command = format!(
        "b00t-cli version upgrade --yes --strategy {}",
        strategy.to_possible_value().unwrap().get_name()
    );

    if let Some(channel) = channel {
        command.push_str(&format!(" --channel {}", channel));
    }

    command
}

fn execute_upgrade(
    strategy: UpgradeStrategy,
    channel: Option<&str>,
    status: &ReleaseStatus,
    context: &UpgradeContext,
) -> Result<()> {
    emit_upgrade_notification(channel, "upgrade.start", strategy, status, context);

    match strategy {
        UpgradeStrategy::ReleaseInstaller => run_release_installer()?,
        UpgradeStrategy::WorkspaceBuild => run_workspace_build(context)?,
        UpgradeStrategy::WorkspaceSync => run_workspace_sync(context)?,
    }

    emit_upgrade_notification(channel, "upgrade.finish", strategy, status, context);
    println!("Upgrade complete. Verify with `b00t-cli --version`.");
    Ok(())
}

fn run_release_installer() -> Result<()> {
    let installer = installer_url();

    cmd!(
        "bash",
        "-lc",
        "curl -fsSL \"$1\" | sh",
        "b00t-cli-upgrade",
        installer.as_str()
    )
    .run()
    .with_context(|| format!("failed to run installer {}", installer))?;

    Ok(())
}

fn run_workspace_build(context: &UpgradeContext) -> Result<()> {
    let workspace_root = context
        .workspace_root
        .as_deref()
        .context("workspace-build requested outside a b00t workspace")?;

    println!("building from workspace: {}", workspace_root.display());

    cmd!("cargo", "install", "--path", "b00t-mcp", "--force")
        .dir(workspace_root)
        .run()
        .context("failed to install b00t-mcp from workspace")?;
    cmd!("cargo", "install", "--path", "b00t-cli", "--force")
        .dir(workspace_root)
        .run()
        .context("failed to install b00t-cli from workspace")?;

    Ok(())
}

fn run_workspace_sync(context: &UpgradeContext) -> Result<()> {
    let workspace_root = context
        .workspace_root
        .as_deref()
        .context("workspace-sync requested outside a b00t workspace")?;

    if !context.git_clean {
        anyhow::bail!(
            "workspace-sync requires a clean git worktree at {}",
            workspace_root.display()
        );
    }

    let branch = context
        .git_branch
        .as_deref()
        .filter(|branch| !branch.is_empty() && *branch != "HEAD")
        .context("workspace-sync requires a named git branch")?;

    println!(
        "syncing workspace: {} [{}]",
        workspace_root.display(),
        branch
    );

    cmd!("git", "fetch", "origin")
        .dir(workspace_root)
        .run()
        .context("failed to fetch origin")?;
    cmd!("git", "pull", "--rebase", "origin", branch)
        .dir(workspace_root)
        .run()
        .with_context(|| format!("failed to rebase branch {}", branch))?;

    run_workspace_build(context)
}

fn notify_channel(
    channel: Option<&str>,
    phase: &str,
    strategy: UpgradeStrategy,
    status: &ReleaseStatus,
    context: &UpgradeContext,
) -> Result<()> {
    let Some(channel) = channel else {
        return Ok(());
    };

    let client = ChatClient::new(ChatTransportConfig {
        kind: ChatTransportKind::LocalSocket,
        socket_path: None,
        nats_url: None,
    })
    .context("failed to initialize local chat client")?;

    let body = format!(
        "{} {} current={} latest={}",
        phase,
        strategy.to_possible_value().unwrap().get_name(),
        status.current,
        status.latest
    );
    let mut message = ChatMessage::new(channel, format!("version.{}", whoami::username()), body);
    message.metadata = json!({
        "phase": phase,
        "strategy": strategy.to_possible_value().unwrap().get_name(),
        "agent": context.detected_agent,
        "workspace": context.workspace_root.as_ref().map(|path| path.display().to_string()),
        "git_branch": context.git_branch,
        "git_clean": context.git_clean,
        "release_url": status.release_url,
    });

    block_on_version_future(async move {
        client
            .send(&message)
            .await
            .context("failed to deliver chat notification")
    })
}

fn emit_upgrade_notification(
    channel: Option<&str>,
    phase: &str,
    strategy: UpgradeStrategy,
    status: &ReleaseStatus,
    context: &UpgradeContext,
) {
    if let Err(error) = notify_channel(channel, phase, strategy, status, context) {
        eprintln!(
            "warning: skipped {} notification on {}: {}",
            phase,
            channel.unwrap_or("<none>"),
            error
        );
    }
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

    fn context(agent: &str, workspace: bool, git_clean: bool) -> UpgradeContext {
        UpgradeContext {
            detected_agent: agent.to_string(),
            workspace_root: workspace.then(|| PathBuf::from("/tmp/.b00t")),
            git_branch: Some("main".to_string()),
            git_clean,
        }
    }

    fn test_status(current: &str, latest: &str, upgrade: bool) -> ReleaseStatus {
        ReleaseStatus {
            current: current.to_string(),
            latest: latest.to_string(),
            release_url: "https://example.invalid/release".to_string(),
            workspace_version: None,
            upgrade_available: upgrade,
        }
    }

    #[test]
    fn detects_workspace_upgrade() {
        assert!(is_upgrade_available("0.7.35", "0.7.36"));
        assert!(!is_upgrade_available("0.7.35", "0.7.35"));
        assert!(!is_upgrade_available("0.7.36", "0.7.35"));
    }

    #[test]
    fn recommends_workspace_sync_for_clean_claude_workspace() {
        let status = test_status("0.7.35", "0.7.36", true);

        assert_eq!(
            recommend_strategy(&status, &context("claude", true, true)),
            UpgradeStrategy::WorkspaceSync
        );
    }

    #[test]
    fn recommends_workspace_build_for_dirty_claude_workspace() {
        let status = test_status("0.7.35", "0.7.36", true);

        assert_eq!(
            recommend_strategy(&status, &context("claude", true, false)),
            UpgradeStrategy::WorkspaceBuild
        );
    }

    #[test]
    fn recommends_release_installer_outside_workspace() {
        let status = test_status("0.7.35", "0.7.36", true);

        assert_eq!(
            recommend_strategy(&status, &context("codex", false, false)),
            UpgradeStrategy::ReleaseInstaller
        );
    }

    #[test]
    fn upgrade_hint_includes_strategy_and_channel() {
        assert_eq!(
            upgrade_command_hint(
                UpgradeStrategy::WorkspaceBuild,
                Some("mission.exec-upgrade")
            ),
            "b00t-cli version upgrade --yes --strategy workspace-build --channel mission.exec-upgrade"
        );
    }

    #[test]
    fn default_channel_config_is_empty() {
        assert_eq!(
            VersionChannelConfig::default(),
            VersionChannelConfig {
                strategy: None,
                notify_channel: None,
            }
        );
    }

    #[test]
    fn strategy_name_uses_kebab_case() {
        assert_eq!(
            strategy_name(UpgradeStrategy::ReleaseInstaller),
            "release-installer"
        );
        assert_eq!(
            strategy_name(UpgradeStrategy::WorkspaceBuild),
            "workspace-build"
        );
        assert_eq!(
            strategy_name(UpgradeStrategy::WorkspaceSync),
            "workspace-sync"
        );
    }
}
