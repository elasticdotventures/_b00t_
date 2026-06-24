//! b00t-rpa — Browser-based Robotic Process Automation via CDP + ratatui TUI.
//!
//! Connects to a Chrome instance running on the Windows host (from WSL2)
//! and provides an fzf-like TUI for curating and executing browser automation scripts.
//!
//! # Usage
//! ```bash
//! # Start the TUI menu
//! b00t-rpa
//!
//! # Connect to specific host:port
//! b00t-rpa --host 172.30.96.1 --port 9222
//!
//! # Run a saved script headless
//! b00t-rpa --script my-automation
//! ```

use b00t_cli::rpa_cdp::RpaSession;
use b00t_cli::rpa_tui::{run_curation_menu, print_script, ScriptStep};
use clap::Parser;
use std::io::{BufRead, Write};

#[derive(Parser, Debug)]
#[clap(name = "b00t-rpa", about = "Browser RPA via CDP — TUI menu for curating automation scripts")]
struct Cli {
    #[clap(long, help = "Windows host IP (auto-detected from WSL2 if omitted)")]
    host: Option<String>,
    #[clap(long, default_value = "9222", help = "Chrome DevTools Protocol port")]
    port: u16,
    #[clap(long, help = "Run a saved script headless (bypass TUI)")]
    script: Option<String>,
    #[clap(long, help = "Directly execute a JavaScript expression")]
    eval: Option<String>,
    #[clap(long, help = "Navigate to a URL directly")]
    url: Option<String>,
    #[clap(long, help = "Open the TUI curation menu (default if no flags)")]
    menu: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Connect to Windows Chrome
    let session = RpaSession::connect(cli.host.clone(), cli.port).await?;

    // Mode dispatch
    if let Some(js) = &cli.eval {
        return eval_mode(&session, js).await;
    }
    if let Some(url) = &cli.url {
        return navigate_mode(&session, url).await;
    }
    if cli.menu || !atty::is(atty::Stream::Stdout) {
        return tui_menu(&session).await;
    }

    // Default: show menu
    tui_menu(&session).await
}

async fn eval_mode(session: &RpaSession, js: &str) -> anyhow::Result<()> {
    println!("🔍 Evaluating JavaScript...");
    let pages = session.list_pages().await?;
    if pages.is_empty() {
        anyhow::bail!("No open pages found. Open a page first with --url");
    }
    let (_, url) = &pages[0];
    let page = session.open_page(url).await?;
    let result = session.evaluate(&page, js).await?;
    println!("{}", result);
    Ok(())
}

async fn navigate_mode(session: &RpaSession, url: &str) -> anyhow::Result<()> {
    println!("🌐 Navigating to {} ...", url);
    let _page = session.open_page(url).await?;
    println!("✅ Page loaded");
    Ok(())
}

async fn tui_menu(session: &RpaSession) -> anyhow::Result<()> {
    println!("🔌 Connected to Chrome. Opening TUI...");

    let curated = run_curation_menu()?;

    if curated.is_empty() {
        println!("No commands selected. Exiting.");
        return Ok(());
    }

    println!("\n🧩 Curated RPA Script:");
    print_script(&curated);

    print!("\n▶ Execute this script? [Y/n]: ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().lock().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input == "n" || input == "no" {
        println!("Script saved but not executed.");
        return Ok(());
    }

    execute_script(session, &curated).await
}

async fn execute_script(session: &RpaSession, steps: &[ScriptStep]) -> anyhow::Result<()> {
    let pages = session.list_pages().await?;
    if pages.is_empty() {
        anyhow::bail!("No open pages. Open one with --url first.");
    }
    let (_, first_url) = &pages[0];
    let page = session.open_page(first_url).await?;

    for (i, step) in steps.iter().enumerate() {
        print!("  {}/{} {} ... ", i + 1, steps.len(), step.action);
        std::io::stdout().flush()?;

        match step.action.as_str() {
            "navigate" => {
                let _ = session.open_page(&step.args).await?;
            }
            "click" => {
                session.click(&page, &step.selector).await?;
            }
            "type" => {
                let parts: Vec<&str> = step.args.splitn(2, ' ').collect();
                let text = parts.get(1).unwrap_or(&"");
                session.type_text(&page, &step.selector, text).await?;
            }
            "evaluate" => {
                let result = session.evaluate(&page, &step.args).await?;
                println!("{}", result);
                continue;
            }
            "get_text" => {
                let text = session.get_page_text(&page).await?;
                println!("\n{}", &text[..text.len().min(200)]);
                continue;
            }
            other => {
                println!("⚠️ Unknown action: {}", other);
                continue;
            }
        }
        println!("✅");
    }

    println!("\n✅ Script complete ({} steps)", steps.len());
    Ok(())
}
