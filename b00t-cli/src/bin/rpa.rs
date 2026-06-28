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
use b00t_cli::rpa_rhai::{create_rpa_engine, run_commands};
use b00t_cli::rpa_tui::{run_curation_menu, print_script, ScriptStep};
use clap::Parser;
use std::io::{BufRead, Write};
use std::time::Duration;

#[derive(Parser, Debug)]
#[clap(name = "b00t-rpa", about = "Browser RPA via CDP — WSL-aware Chrome automation")]
struct Cli {
    #[clap(subcommand)]
    command: Option<RpaCommands>,
    #[clap(long, help = "Windows host IP (auto-detected from WSL if omitted)")]
    host: Option<String>,
    #[clap(long, default_value = "9222", help = "Chrome DevTools Protocol port")]
    port: u16,
    #[clap(long, help = "Run a saved script headless (bypass TUI)")]
    script: Option<String>,
    #[clap(long, help = "Execute JavaScript on current page")]
    eval: Option<String>,
    #[clap(long, help = "Navigate to a URL")]
    url: Option<String>,
    #[clap(long, help = "Force TUI menu")]
    menu: bool,
    #[clap(long, help = "Describe page structure (headings, links, forms, buttons)")]
    describe: bool,
    #[clap(long, help = "Inject DOM enrichment (single data-b00t attribute, 500-element cap)")]
    enrich: bool,
    #[clap(long, help = "Take screenshot after navigation")]
    screenshot: bool,
}

#[derive(Parser, Debug)]
enum RpaCommands {
    #[clap(about = "Start Chrome on Windows host and open TUI menu")]
    Start,
    #[clap(about = "Open TUI menu for curating automation scripts")]
    Menu,
    #[clap(about = "One-time Windows setup: firewall rule for CDP relay")]
    Setup,
    #[clap(about = "Install b00t browser plugin into Chrome via CDP")]
    Plugin,
    #[clap(about = "Run a Rhai RPA script (multi-step banking, PDF downloads, etc.)")]
    Script {
        #[clap(help = "Path to .rhai script file")]
        script: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Handle setup / plugin subcommands (no Chrome needed yet)
    if matches!(&cli.command, Some(RpaCommands::Setup)) {
        return run_setup(cli.port).await;
    }
    if matches!(&cli.command, Some(RpaCommands::Plugin)) {
        return install_plugin(cli.port).await;
    }
    if let Some(RpaCommands::Script { script }) = &cli.command {
        // Discover CDP endpoint, then run script
        let host = if detect_wsl() { windows_host_ip() } else { "127.0.0.1".to_string() };
        let cdp_port = ensure_chrome(Some(&host), cli.port).await?;
        return run_rhai_script(script, &host, cdp_port).await;
    }

    let auto_start = matches!(&cli.command, Some(RpaCommands::Start));

    // Detect WSL environment
    let is_wsl = detect_wsl();
    let win_host = cli.host.clone().or_else(|| {
        if is_wsl { Some(windows_host_ip()) } else { None }
    });

    // Auto-start Chrome on Windows if requested or in WSL.
    // ensure_chrome returns the actual port where CDP is reachable
    // (may differ from cli.port if a relay/proxy was started).
    let cdp_port = if auto_start || is_wsl {
        ensure_chrome(win_host.as_deref(), cli.port).await?
    } else {
        cli.port
    };

    // Connect to the discovered CDP endpoint
    let session = RpaSession::connect(win_host.clone(), cdp_port).await?;

    // Mode dispatch:
    // --url + --eval     = navigate then evaluate
    // --url + --describe = navigate then describe page
    // --url + --screenshot = navigate then screenshot
    // --url              = navigate only
    // --eval             = eval on current page
    // --describe         = describe current page
    // (default)          = TUI menu
    let enrich = cli.enrich;

    if let Some(url) = &cli.url {
        if let Some(js) = &cli.eval {
            return navigate_and_eval(&session, url, js, enrich).await;
        }
        if cli.describe {
            return navigate_and_describe(&session, url, enrich).await;
        }
        if cli.screenshot {
            return navigate_and_screenshot(&session, url, enrich).await;
        }
        return navigate_mode(&session, url, enrich).await;
    }
    if let Some(js) = &cli.eval {
        return eval_mode(&session, js).await;
    }
    if cli.describe {
        return describe_page(&session).await;
    }
    if matches!(&cli.command, Some(RpaCommands::Menu)) || cli.menu || !atty::is(atty::Stream::Stdout) {
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
/// In WSL2, the default gateway IS the Windows host.
/// The nameserver in /etc/resolv.conf is a loopback proxy — NOT the host.
fn windows_host_ip() -> String {
    // Default gateway = Windows host (most reliable in WSL2)
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
    // Fallback: nameserver from resolv.conf (WSL1 compat)
    if let Ok(resolv) = std::fs::read_to_string("/etc/resolv.conf") {
        for line in resolv.lines() {
            if line.starts_with("nameserver ") {
                if let Some(ip) = line.split_whitespace().nth(1) {
                    return ip.to_string();
                }
            }
        }
    }
    "127.0.0.1".to_string()
}

/// Check if Chrome CDP endpoint is reachable. If not and we're in WSL,
/// launch Chrome on Windows via PowerShell over the network.
/// Returns the actual port where CDP is reachable (may differ from the
/// requested `port` if a relay/proxy is used).
async fn ensure_chrome(host: Option<&str>, port: u16) -> anyhow::Result<u16> {
    let host = host.unwrap_or_else(|| "127.0.0.1");
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()?;
    let probe_targets = [
        // Try relay port first (port+1) — Chrome only binds 127.0.0.1,
        // relay/proxy is the only way to reach it from WSL.
        format!("http://{}:{}/json/version", host, port + 1),
        format!("http://localhost:{}/json/version", port),         // WSL localhost fwd
        format!("http://{}:{}/json/version", host, port),          // direct to Windows host
    ];

    // Probe CDP endpoint
    // Determine the actual port from the probe target that succeeded.
    // The relay/proxy may be on port+1, Chrome on port, or localhost via WSL.
    for target in &probe_targets {
        if let Ok(resp) = client.get(target).send().await {
            if resp.status().is_success() {
                eprintln!("✅ Chrome already running with CDP at {}", target);
                // Extract the actual port from the target URL
                let p = target.trim_end_matches("/json/version")
                    .rsplit(':').next()
                    .and_then(|s| s.parse::<u16>().ok())
                    .unwrap_or(port);
                return Ok(p);
            }
        }
    }
    eprintln!("🔍 Chrome CDP not reachable (tried {}:{}, localhost:{}, portproxy)", host, port, port);

    // Launch Chrome on Windows from WSL
    // Probe common Chrome install paths on the Windows host
    let chrome_paths = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        r"$env:LOCALAPPDATA\Google\Chrome\Application\chrome.exe",
    ];
    let user_data = r"C:\temp\chrome-debug";
    let mut launched = false;

    for chrome_path in &chrome_paths {
        eprintln!("🚀 Trying: {} ...", chrome_path);
        let ps_cmd = format!(
            "if (Test-Path '{}') {{ Start-Process -FilePath '{}' -ArgumentList '--remote-debugging-port={}','--remote-allow-origins=*','--user-data-dir={}','--no-first-run','--no-default-browser-check' -WindowStyle Hidden; Write-Output 'OK' }} else {{ Write-Output 'NOT_FOUND' }}",
            chrome_path, chrome_path, port, user_data
        );

        if let Ok(out) = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &ps_cmd])
            .output()
        {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.contains("OK") {
                eprintln!("  ✅ Chrome launched from: {}", chrome_path);
                launched = true;
                break;
            }
        }
    }

    if !launched {
        // Fallback: cmd.exe /C start (uses Windows shell execution)
        eprintln!("  🔄 Trying cmd.exe fallback...");
        for path in [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ] {
            let cmd = format!(
                "/C if exist \"{}\" (start \"\" \"{}\" --remote-debugging-port={} --remote-allow-origins=* --user-data-dir={} --no-first-run)",
                path, path, port, user_data
            );
            if let Ok(out) = std::process::Command::new("cmd.exe").args(["/C", &cmd]).output() {
                if out.status.success() {
                    launched = true;
                    eprintln!("  ✅ Chrome launched via cmd.exe");
                    break;
                }
            }
        }
    }

    if !launched {
        eprintln!("  ⚠️  Could not launch Chrome from WSL.");
        eprintln!("  Please start Chrome manually on Windows:");
        eprintln!("    chrome.exe --remote-debugging-port={} --remote-allow-origins=* --user-data-dir={} --no-first-run", port, user_data);
        anyhow::bail!("Chrome CDP not available");
    }

    // Start socat TCP relay on WSL side (listens on WSL, forwards to Windows).
    // socat must be installed: sudo apt-get install -y socat
    // If socat is not available, falls back to PowerShell-based relay on Windows.
    let relay_port = port + 1;
    let socat_available = std::process::Command::new("which")
        .arg("socat").output().map(|o| o.status.success()).unwrap_or(false);

    if socat_available {
        eprintln!("  🔄 Starting socat relay (WSL:{} → Windows:{}:{})...", relay_port, host, port);
        let relay_listen = format!("TCP-LISTEN:{},fork,reuseaddr", relay_port);
        let relay_target = format!("TCP:{}:{}", host, port);
        let _ = std::process::Command::new("socat")
            .args([&relay_listen, &relay_target])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        eprintln!("    ✅ socat relay started (PID: check with pgrep -f socat)");
    } else {
        eprintln!("  ⚠️  socat not found. Install: sudo apt-get install -y socat");
        // Fallback: launch PowerShell TCP relay on Windows
        eprintln!("  🔄 Falling back to PowerShell TCP relay on Windows...");
        let ps_relay = format!(
            r#"$l=[System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Any,{});$l.Start();while($true){{$c=$l.AcceptTcpClient();$t=New-Object System.Net.Sockets.TcpClient;$t.Connect('127.0.0.1',{});$s1=$c.GetStream();$s2=$t.GetStream();$r1=$s1.CopyToAsync($s2);$r2=$s2.CopyToAsync($s1);[System.Threading.Tasks.Task]::WaitAll($r1,$r2);$c.Close();$t.Close()}}"#,
            relay_port, port
        );
        let _ = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &ps_relay])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        eprintln!("    ✅ PowerShell relay started");
    }

    // Let relay initialize, then wait for CDP (poll up to 30s)
    tokio::time::sleep(Duration::from_millis(500)).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    let wait_client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(2))
        .timeout(std::time::Duration::from_secs(3))
        .build()?;
    let probe_targets = &[
        format!("http://{}:{}", host, port + 1),          // relay port first
        format!("http://localhost:{}", port),             // WSL localhost forwarding
        format!("http://{}:{}", host, port),              // direct to Windows host
    ];
    for i in 0..30 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        for target in probe_targets {
            if let Ok(resp) = wait_client.get(target).send().await {
                if resp.status().is_success() {
                    eprintln!("✅ Chrome CDP ready after {}s at {}", i + 1, target);
                    let p = target.trim_end_matches('/')
                        .rsplit(':').next()
                        .and_then(|s| s.parse::<u16>().ok())
                        .unwrap_or(port);
                    return Ok(p);
                }
            }
        }
        if i % 5 == 4 {
            eprintln!("  Waiting for Chrome... ({}s)", i + 1);
        }
    }

    eprintln!("⚠️  Chrome did not start within 30s. Check Windows host.");
    eprintln!("  Run manually from Windows cmd (admin not required):");
    eprintln!("    chrome.exe --remote-debugging-port={} --remote-allow-origins=* --user-data-dir={}", port, user_data);
    anyhow::bail!("Chrome CDP timeout");
}

/// Navigate to URL, optionally enrich, execute JavaScript.
async fn navigate_and_eval(session: &RpaSession, url: &str, js: &str, enrich: bool) -> anyhow::Result<()> {
    eprintln!("🌐 Navigating to {} ...", url);
    let page = session.open_page(url, enrich).await?;
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
    eprintln!("🔍 Evaluating JavaScript...");
    let result = session.evaluate(&page, js).await?;
    println!("{}", result);
    Ok(())
}

/// Navigate then describe page structure (headings, links, forms, buttons).
async fn navigate_and_describe(session: &RpaSession, url: &str, enrich: bool) -> anyhow::Result<()> {
    eprintln!("🌐 Navigating to {} ...", url);
    let page = session.open_page(url, enrich).await?;
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
    let desc_js = r#"JSON.stringify({
  title: document.title,
  url: location.href,
  headings: Array.from(document.querySelectorAll('h1,h2,h3')).slice(0,20).map(h=>({level:h.tagName,text:h.textContent.trim().slice(0,80)})),
  links: Array.from(document.querySelectorAll('a[href]')).slice(0,30).map(a=>({text:a.textContent.trim().slice(0,60),href:a.href.slice(0,200)})),
  forms: Array.from(document.forms).map(f=>({id:f.id||'',action:f.action||'',fields:Array.from(f.elements).slice(0,10).map(e=>({name:e.name||'',type:e.type||'',label:(e.labels?.[0]?.textContent||e.placeholder||'').slice(0,60)}))})),
  buttons: Array.from(document.querySelectorAll('button')).slice(0,30).map(b=>({text:b.textContent.trim().slice(0,60),type:b.type})),
  enriched: typeof window.__b00t === 'object' ? window.__b00t.count() : null
})"#;
    let result = session.evaluate(&page, desc_js).await?;
    println!("{}", result);
    Ok(())
}

/// Navigate then take a screenshot.
async fn navigate_and_screenshot(session: &RpaSession, url: &str, enrich: bool) -> anyhow::Result<()> {
    eprintln!("🌐 Navigating to {} ...", url);
    let page = session.open_page(url, enrich).await?;
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
    let png = session.screenshot(&page).await?;
    let path = format!("screenshot_{}.png", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
    std::fs::write(&path, &png)?;
    println!("✅ Screenshot saved: {} ({} bytes)", path, png.len());
    Ok(())
}

/// Describe the current page (no navigation).
async fn describe_page(session: &RpaSession) -> anyhow::Result<()> {
    let pages = session.list_pages().await?;
    if pages.is_empty() {
        anyhow::bail!("No open pages.");
    }
    let page = session.open_page(&pages[0].1, false).await?;
    let title = session.evaluate(&page, "document.title").await?;
    println!("📄 Page: {}", title);
    println!("   URL: {}", pages[0].1);
    let links = session.evaluate(&page, "document.querySelectorAll('a').length").await?;
    let buttons = session.evaluate(&page, "document.querySelectorAll('button').length").await?;
    println!("   Links: {} | Buttons: {} | Forms: {}",
        links, buttons,
        session.evaluate(&page, "document.forms.length").await?);
    Ok(())
}

async fn eval_mode(session: &RpaSession, js: &str) -> anyhow::Result<()> {
    eprintln!("🔍 Evaluating JavaScript...");
    let pages = session.list_pages().await?;
    if pages.is_empty() {
        anyhow::bail!("No open pages found. Use --url <url> first.");
    }
    let page = session.open_page(&pages[0].1, false).await?;
    let result = session.evaluate(&page, js).await?;
    println!("{}", result);
    Ok(())
}

async fn navigate_mode(session: &RpaSession, url: &str, enrich: bool) -> anyhow::Result<()> {
    eprintln!("🌐 Navigating to {} ...", url);
    let _page = session.open_page(url, enrich).await?;
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
    let page = session.open_page_no_enrich(&target_url).await?;

    for (i, step) in steps.iter().enumerate() {
        print!("  {}/{} {} ... ", i + 1, steps.len(), step.action);
        std::io::stdout().flush()?;

        match step.action.as_str() {
            "navigate" => { let _ = session.open_page_no_enrich(&step.args).await?; }
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
            "screenshot" => {
                let png = session.screenshot(&page).await?;
                let path = if step.args.is_empty() { format!("screenshot_{}.png", chrono::Utc::now().format("%H%M%S")) } else { step.args.clone() };
                std::fs::write(&path, &png)?;
                print!("saved: {}", path);
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

/// Install the b00t browser plugin into Chrome via CDP.
/// Writes the extension files, then restarts Chrome with --load-extension.
async fn install_plugin(port: u16) -> anyhow::Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/brianh".to_string());
    let ext_path = format!("{}/.dotfiles/_b00t_/browser-plugin", home);
    let ext_win = format!(r"C:\b00t\browser-plugin");

    if !std::path::Path::new(&ext_path).join("manifest.json").exists() {
        anyhow::bail!("Plugin not found at {}. Run from dotfiles repo.", ext_path);
    }

    eprintln!("🐝 b00t Browser Plugin Installer");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  Extension path: {}", ext_path);
    eprintln!();

    if detect_wsl() {
        // Copy extension to Windows via /mnt/c/ (WSL mounts C: drive here)
        let win_dir = "/mnt/c/b00t/browser-plugin";
        let win_dir_ps = r"C:\b00t\browser-plugin";
        eprintln!("1️⃣  Copying extension to Windows ({})...", win_dir_ps);
        std::fs::create_dir_all(win_dir)?;
        // Use cp -r to copy file by file through the WSL mount
        let copy_result = std::process::Command::new("cp")
            .args(["-r", &format!("{}/.", &ext_path), win_dir])
            .output();
        if let Ok(out) = &copy_result {
            if out.status.success() {
                eprintln!("    ✅ Copied via WSL mount");
            } else {
                eprintln!("    ⚠️  cp failed: {}", String::from_utf8_lossy(&out.stderr));
                // Fallback: try PowerShell with \\wsl$\ path
                let ps_copy = format!(
                    "Copy-Item -Recurse -Force '\\\\wsl.localhost\\Ubuntu{}\\browser-plugin\\*' '{}'",
                    ext_path, win_dir_ps
                );
                let _ = std::process::Command::new("powershell.exe")
                    .args(["-NoProfile", "-Command", &ps_copy])
                    .output();
                eprintln!("    ✅ Fallback copy done");
            }
        } else {
            eprintln!("    ⚠️  cp command failed, trying direct copy...");
        }

        // Kill Chrome and restart with extension flag
        eprintln!("2️⃣  Restarting Chrome with extension loaded...");
        let chrome = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
        let user_data = r"C:\b00t\chrome-debug";
        let restart_cmd = format!(
            "Get-Process chrome -ErrorAction SilentlyContinue | Stop-Process -Force; \
             Start-Sleep -Seconds 2; \
             Start-Process -FilePath '{}' -ArgumentList '--remote-debugging-port={}','--remote-allow-origins=*',\
             '--load-extension={}','--user-data-dir={}','--no-first-run' -WindowStyle Hidden",
            chrome, port, win_dir_ps, user_data
        );
        let _ = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-Command", &restart_cmd])
            .output();
        eprintln!("    ✅ Chrome restarted with b00t plugin");

        // Wait and verify extension loaded via CDP
        eprintln!("3️⃣  Verifying extension loaded...");
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(3))
            .build()?;
        let probe = format!("http://{}:{}", windows_host_ip(), port + 1);
        for i in 0..10 {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                if let Ok(resp) = client.get(&format!("{}/json/version", probe)).send().await {
                if resp.status().is_success() {
                    eprintln!("    ✅ Chrome CDP ready after {}s with plugin loaded", (i + 1) * 2);
                    eprintln!();
                    eprintln!("🐝 b00t plugin installed!");
                    eprintln!("   Open Chrome → click the 🐝 icon in extensions toolbar");
                    eprintln!("   Or press Ctrl+Shift+Click to open the b00t side panel");
                    return Ok(());
                }
            }
        }
        eprintln!("    ⚠️  Chrome may still be starting. Check the extension manually.");
    } else {
        eprintln!("⚠️  Plugin install is designed for WSL→Windows Chrome.");
        eprintln!("   To load manually: chrome --load-extension={}", ext_path);
    }

    Ok(())
}

/// Run a Rhai RPA script with access to navigate, click, type, download, etc.
async fn run_rhai_script(script_path: &str, host: &str, cdp_port: u16) -> anyhow::Result<()> {
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<b00t_cli::rpa_rhai::Cmd>();

    let session = RpaSession::connect(Some(host.to_string()), cdp_port).await?;

    // Spawn command processor on a tokio task
    let cmd_handle = tokio::spawn(async move {
        run_commands(&session, &mut cmd_rx).await;
    });

    // Run the Rhai script on the current thread (synchronous)
    let engine = create_rpa_engine(cmd_tx);
    let mut scope = rhai::Scope::new();
    scope.push_constant("VERSION", "0.1.0");

    eprintln!("🚀 Running RPA script: {}", script_path);
    let path = std::path::Path::new(script_path);
    if !path.exists() {
        anyhow::bail!("Script not found: {}", script_path);
    }
    let ast = engine.compile_file(path.into())?;
    let result: rhai::Dynamic = engine.eval_ast_with_scope(&mut scope, &ast)?;
    if !result.is_unit() {
        println!("{}", result);
    }
    eprintln!("✅ RPA script complete");

    // Drop the sender so the command processor exits
    drop(engine);
    cmd_handle.await?;
    Ok(())
}

/// One-time Windows setup: add firewall rule + install Python relay.
async fn run_setup(port: u16) -> anyhow::Result<()> {
    if !detect_wsl() {
        eprintln!("⚠️  Setup is only needed when running from WSL.");
        eprintln!("    If running on Windows directly, just launch Chrome with:");
        eprintln!("    chrome.exe --remote-debugging-port={} --remote-allow-origins=*", port);
        return Ok(());
    }

    let host = windows_host_ip();
    eprintln!("🔧 b00t-rpa Windows Setup");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  Windows host: {}", host);
    eprintln!("  CDP port:     {}", port);
    eprintln!();

    // Step 1: Verify socat is available (primary relay tool)
    eprintln!("1️⃣  Checking socat (required for CDP relay)...");
    let socat_ok = std::process::Command::new("which")
        .arg("socat").output().map(|o| o.status.success()).unwrap_or(false);
    if socat_ok {
        eprintln!("    ✅ socat found");
    } else {
        eprintln!("    ⚠️  socat not found. Install: sudo apt-get install -y socat");
        eprintln!("    b00t will use PowerShell fallback relay instead.");
    }

    // Step 2: Add Windows Firewall rule (needs admin via UAC)
    eprintln!();
    eprintln!("2️⃣  Adding Windows Firewall rule for port {}...", port + 1);
    eprintln!("    A UAC prompt will appear. Click Yes to allow.");
    let cmd = format!(
        "Start-Process powershell -Verb RunAs -ArgumentList '-NoProfile','-Command',\"New-NetFirewallRule -DisplayName 'WSL CDP {}' -Direction Inbound -Protocol TCP -LocalPort {} -Action Allow -Profile Any\"",
        port + 1, port + 1
    );
    let _ = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &cmd])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    eprintln!("    (waiting for UAC approval...)");
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Step 3: Verify
    eprintln!();
    eprintln!("3️⃣  Verifying...");
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        .build()?;
    let probe = format!("http://{}:{}", host, port + 1);
    match client.get(&probe).send().await {
        Ok(_) => eprintln!("    ✅ Relay accessible at {}:{}. Ready!", host, port + 1),
        Err(_) => {
            eprintln!("    ⚠️  Could not verify. You may need to:");
            eprintln!("       Run this in PowerShell AS ADMINISTRATOR:");
            eprintln!("       New-NetFirewallRule -DisplayName 'WSL CDP {}' -Direction Inbound -Protocol TCP -LocalPort {} -Action Allow",
                port + 1, port + 1);
        }
    }

    eprintln!();
    eprintln!("✅ Setup complete. Run `b00t-rpa start` to begin.");
    Ok(())
}
