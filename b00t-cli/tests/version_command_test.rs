use assert_cmd::prelude::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;
use tempfile::tempdir;

fn spawn_release_server(response_body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock release server");
    let address = listener.local_addr().expect("mock release server addr");

    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept mock release connection");
        let mut buffer = [0_u8; 1024];
        let _ = stream.read(&mut buffer);

        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write mock release response");
        stream.flush().expect("flush mock release response");
    });

    format!("http://{}/latest", address)
}

#[test]
fn version_check_reports_release_status_from_mock_api() -> Result<(), Box<dyn std::error::Error>> {
    let release_api_url = spawn_release_server(
        r#"{"tag_name":"v9.9.9","html_url":"https://example.invalid/releases/v9.9.9"}"#,
    );

    let mut cmd = Command::cargo_bin("b00t-cli")?;
    cmd.current_dir("/home/brianh/.b00t")
        .env("B00T_RELEASE_API_URL", release_api_url)
        .arg("version")
        .arg("check");

    let output = cmd.output()?;
    assert!(
        output.status.success(),
        "version check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("current:"));
    assert!(stdout.contains("latest: 9.9.9"));
    assert!(stdout.contains("release: https://example.invalid/releases/v9.9.9"));
    assert!(stdout.contains("recommended:"));

    Ok(())
}

#[test]
fn version_channel_round_trips_via_xdg_config() -> Result<(), Box<dyn std::error::Error>> {
    let config_home = tempdir()?;

    let mut set_cmd = Command::cargo_bin("b00t-cli")?;
    set_cmd
        .env("XDG_CONFIG_HOME", config_home.path())
        .arg("version")
        .arg("channel")
        .arg("set")
        .arg("workspace-build")
        .arg("--notify-channel")
        .arg("mission.exec-upgrade");
    let set_output = set_cmd.output()?;
    assert!(
        set_output.status.success(),
        "version channel set failed: {}",
        String::from_utf8_lossy(&set_output.stderr)
    );

    let mut show_cmd = Command::cargo_bin("b00t-cli")?;
    show_cmd
        .env("XDG_CONFIG_HOME", config_home.path())
        .arg("version")
        .arg("channel")
        .arg("show");
    let show_output = show_cmd.output()?;
    assert!(
        show_output.status.success(),
        "version channel show failed: {}",
        String::from_utf8_lossy(&show_output.stderr)
    );

    let show_stdout = String::from_utf8(show_output.stdout)?;
    assert!(show_stdout.contains("strategy: workspace-build"));
    assert!(show_stdout.contains("notify_channel: mission.exec-upgrade"));

    let mut clear_cmd = Command::cargo_bin("b00t-cli")?;
    clear_cmd
        .env("XDG_CONFIG_HOME", config_home.path())
        .arg("version")
        .arg("channel")
        .arg("clear");
    let clear_output = clear_cmd.output()?;
    assert!(
        clear_output.status.success(),
        "version channel clear failed: {}",
        String::from_utf8_lossy(&clear_output.stderr)
    );

    let mut show_cleared_cmd = Command::cargo_bin("b00t-cli")?;
    show_cleared_cmd
        .env("XDG_CONFIG_HOME", config_home.path())
        .arg("version")
        .arg("channel")
        .arg("show");
    let show_cleared_output = show_cleared_cmd.output()?;
    assert!(
        show_cleared_output.status.success(),
        "version channel show after clear failed: {}",
        String::from_utf8_lossy(&show_cleared_output.stderr)
    );

    let show_cleared_stdout = String::from_utf8(show_cleared_output.stdout)?;
    assert!(show_cleared_stdout.contains("strategy: unset"));
    assert!(show_cleared_stdout.contains("notify_channel: unset"));

    Ok(())
}
