//! b00t-rpa — Browser-based Robotic Process Automation via CDP + ratatui TUI.
//!
//! Auto-detects WSL environment, discovers Windows host, launches Chrome if needed.
//!
//! # Usage
//! ```bash
//! b00t-rpa                              # Auto-detect + TUI menu
//! b00t-rpa start                        # Start Chrome + open TUI
//! b00t-rpa --eval "document.title"      # Execute JS on current page
//! b00t-rpa --url https://example.com    # Navigate
//! ```

use b00t_cli::rpa_cdp::RpaSession;
use b00t_cli::rpa_tui::{run_curation_menu, print_script, ScriptStep};
use clap::Parser;
use std::io::{BufRead, Write};
use std::time::Duration;

#[derive(Parser, Debug)]
#[clap(name = "b00t-rpa", about = "Browser RPA via CDP — WSL-aware Chrome automation")]
struct Cli {
    #[clap(long, help = "Windows host IP (auto-detected from WSL if omitted)")]
    host: Option<String>,
    #[clap(long, default_value = "9222", help = "Chrome DevTools Protocol port")]
    port: u16,
    #[clap(long, help = "Start Chrome on Windows host automatically")]
    start: bool,
    #[clap(long, help = "Run a saved script headless (bypass TUI)")]
    script: Option<String>,
    #[clap(long, help = "Execute JavaScript on current page")]
    eval: Option<String>,
    #[clap(long, help = "Navigate to a URL")]
    url: Option<String>,
    #[clap(long, help = "Force TUI menu")]
    menu: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Detect WSL environment
    let is_wsl = detect_wsl();
    let win_host = cli.host.clone().or_else(|| {
        if is_wsl { Some(windows_host_ip()) } else { None }
    });

    // Auto-start Chrome on Windows if needed
    if cli.start || is_wsl {
        ensure_chrome(win_host.as_deref(), cli.port).await?;
    }

    // Connect
    let session = RpaSession::connect(win_host, cli.port).await?;

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

    tui_menu(&session).await
}

/// Detect if running inside WSL2 by checking for WSL-specific markers.
fn detect_wsl() -> bool {
    // WSL2 has /proc/sys/fs/binfmt_misc/WSLInterop
    // Also /proc/version contains "Microsoft" or "WSL"
    if std::path::Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists() {
        return true;
    }
    if let Ok(version) = std::fs::read_to_string("/proc/version") {
        if version.contains("Microsoft") || version.contains("WSL") || version.contains("microsoft") {
            return true;
        }
    }
    false
}

/// Resolve Windows host IP from WSL2 routing table.
fn windows_host_ip() -> String {
    // Try nameserver from resolv.conf first (most reliable)
    if let Ok(resolv) = std::fs::read_to_string("/etc/resolv.conf") {
        for line in resolv.lines() {
            if line.starts_with("nameserver ") {
                if let Some(ip) = line.split_whitespace().nth(1) {
                    return ip.to_string();
                }
            }
        }
    }
    // Fallback: default gateway
    if let Ok(output) = std::process::Command::new("ip")
        .args(["route", "show", "default"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(ip) = stdout.lines().next()
            .and_then(|l| l.split_whitespace().nth(2))
        {
            return ip.to_string();
        }
    }
    "127.0.0.1".to_string()
}

/// Check if Chrome CDP endpoint is reachable. If not and we're in WSL,
/// launch Chrome on Windows via PowerShell over the network.
async fn ensure_chrome(host: Option<&str>, port: u16) -> anyhow::Result<()> {
    let host = host.unwrap_or_else(|| "127.0.0.1");
    let probe_url = format!("http://{}:{}/json/version", host, port);

    // Probe CDP endpoint
    match reqwest::get(&probe_url).await {
        Ok(resp) if resp.status().is_success() => {
            eprintln!("✅ Chrome already running with CDP on {}:{}", host, port);
            return Ok(());
        }
        _ => {
            eprintln!("🔍 Chrome CDP not reachable at {}:{}", host, port);
        }
    }

    // Launch Chrome on Windows from WSL via PowerShell over the network
    eprintln!("🚀 Launching Chrome on Windows host ({})...", host);
    let chrome_path = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
    let user_data = r"C:\temp\chrome-debug";
    let ps_cmd = format!(
        "Start-Process -FilePath '{}' -ArgumentList '--remote-debugging-port={}', '--user-data-dir={}' -WindowStyle Hidden",
        chrome_path, port, user_data
    );

    // Try launching via PowerShell on the Windows host
    // WSL can execute Windows binaries from /mnt/c/Windows/System32/
    let result = std::process::Command::new("powershell.exe")
        .args(["-Command", &ps_cmd])
        .output();

    match result {
        Ok(out) if out.status.success() => {
            eprintln!("  Chrome launch command sent. Waiting for CDP...");
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!("  ⚠️  PowerShell launch returned: {}", stderr);
            eprintln!("  Trying alternate launch method...");
            // Fallback: try via cmd.exe
            let _ = std::process::Command::new("cmd.exe")
                .args(["/C", "start", &format!("\"\" \"{}\" --remote-debugging-port={} --user-data-dir={}", chrome_path, port, user_data)])
                .output();
        }
        Err(e) => {
            eprintln!("  ⚠️  Could not launch from WSL: {}", e);
            eprintln!("  Please start Chrome manually on Windows:");
            eprintln!("    chrome.exe --remote-debugging-port={} --user-data-dir={}", port, user_data);
            anyhow::bail!("Chrome CDP not available");
        }
    }

    // Wait for CDP to become available (poll up to 30s)
    for i in 0..30 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        match reqwest::get(&probe_url).await {
            Ok(resp) if resp.status().is_success() => {
                eprintln!("✅ Chrome CDP ready after {}s", i + 1);
                return Ok(());
            }
            _ => {
                if i % 5 == 4 {
                    eprintln!("  Waiting for Chrome... ({}s)", i + 1);
                }
            }
        }
    }

    eprintln!("⚠️  Chrome did not start within 30s. Check Windows host.");
    eprintln!("  Run manually: chrome.exe --remote-debugging-port={} --user-data-dir={}", port, user_data);
    anyhow::bail!("Chrome CDP timeout");
}

async fn eval_mode(session: &RpaSession, js: &str) -> anyhow::Result<()> {
    eprintln!("🔍 Evaluating JavaScript...");
    let pages = session.list_pages().await?;
    if pages.is_empty() {
        anyhow::bail!("No open pages found. Use --url <url> first.");
    }
    let page = session.open_page(&pages[0].1).await?;
    let result = session.evaluate(&page, js).await?;
    println!("{}", result);
    Ok(())
}

async fn navigate_mode(session: &RpaSession, url: &str) -> anyhow::Result<()> {
    eprintln!("🌐 Navigating to {} ...", url);
    let _page = session.open_page(url).await?;
    println!("✅ Page loaded");
    Ok(())
}

async fn tui_menu(session: &RpaSession) -> anyhow::Result<()> {
    eprintln!("\n🔌 Connected. Opening command menu...\n");

    let curated = run_curation_menu()?;

    if curated.is_empty() {
        println!("No commands selected.");
        return Ok(());
    }

    println!("\n🧩 RPA Script ({} steps):", curated.len());
    print_script(&curated);

    print!("\n▶ Execute? [Y/n]: ");
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().lock().read_line(&mut input)?;
    if input.trim().to_lowercase() == "n" {
        println!("Saved.");
        return Ok(());
    }

    execute_script(session, &curated).await
}

async fn execute_script(session: &RpaSession, steps: &[ScriptStep]) -> anyhow::Result<()> {
    let pages = session.list_pages().await?;
    let target_url = pages.first().map(|(_, u)| u.clone()).unwrap_or_default();
    let page = session.open_page(&target_url).await?;

    for (i, step) in steps.iter().enumerate() {
        print!("  {}/{} {} ... ", i + 1, steps.len(), step.action);
        std::io::stdout().flush()?;

        match step.action.as_str() {
            "navigate" => { let _ = session.open_page(&step.args).await?; }
            "click" => { session.click(&page, &step.selector).await?; }
            "type" => {
                let parts: Vec<&str> = step.args.splitn(2, ' ').collect();
                session.type_text(&page, &step.selector, parts.get(1).unwrap_or(&"")).await?;
            }
            "evaluate" => {
                let r = session.evaluate(&page, &step.args).await?;
                println!("{}", r);
                continue;
            }
            "get_text" => {
                let t = session.get_page_text(&page).await?;
                println!("\n{}", &t[..t.len().min(200)]);
                continue;
            }
            other => { println!("⚠️ Unknown: {}", other); continue; }
        }
        println!("✅");
    }

    println!("\n✅ Done ({} steps)", steps.len());
    Ok(())
}
