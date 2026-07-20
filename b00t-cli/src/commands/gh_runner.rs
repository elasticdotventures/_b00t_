//! `b00t gh-runner` — self-hosted GitHub Actions runner via podman kube play
//!
//! # Usage
//! ```bash
//! b00t gh-runner install   --repo X/Y --labels L --workdir W [--ephemeral] [--socket auto|docker|podman|none]
//! b00t gh-runner status    --repo X/Y [--json]
//! b00t gh-runner logs      --repo X/Y [--follow]
//! b00t gh-runner deregister --repo X/Y [--remove-workdir]
//! b00t gh-runner doctor    [--repo X/Y]
//! ```

use anyhow::{Context, Result, bail};
use clap::Parser;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;

fn sh(cmd: &str) -> (bool, String) {
    Command::new("sh")
        .args(["-c", cmd])
        .output()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let e = String::from_utf8_lossy(&o.stderr).trim().to_string();
            (
                o.status.success(),
                if !s.is_empty() {
                    s
                } else if !e.is_empty() {
                    e
                } else {
                    String::new()
                },
            )
        })
        .unwrap_or((false, "exec failed".into()))
}

fn retry_sh(cmd: &str, max_retries: u32, label: &str) -> Result<String> {
    let mut last_err = String::new();
    for attempt in 0..max_retries {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(2u64.pow(attempt - 1)));
            eprintln!(
                "  Retrying {} (attempt {}/{})...",
                label,
                attempt + 1,
                max_retries
            );
        }
        let (ok, out) = sh(cmd);
        if ok {
            return Ok(out);
        }
        last_err = out;
    }
    bail!(
        "{} failed after {} attempts: {}",
        label,
        max_retries,
        last_err
    )
}

fn detect_socket(socket_opt: &str) -> String {
    match socket_opt {
        "docker" => "/var/run/docker.sock".to_string(),
        "podman" => {
            let uid = sh("id -u").1;
            format!("/run/user/{}/podman/podman.sock", uid.trim())
        }
        "none" => String::new(),
        "auto" | _ => {
            // Prefer podman if docker daemon not running, otherwise docker
            let (docker_ok, _) = sh("docker info --format '{{.ServerVersion}}' 2>/dev/null");
            if docker_ok {
                "/var/run/docker.sock".to_string()
            } else {
                let uid = sh("id -u").1;
                format!("/run/user/{}/podman/podman.sock", uid.trim())
            }
        }
    }
}

pub fn repo_slug(repo: &str) -> String {
    repo.replace('/', "-").to_lowercase()
}

fn hostname() -> String {
    sh("hostname 2>/dev/null").1
}

fn runner_name(repo: &str) -> String {
    format!("{}-{}", repo_slug(repo), hostname())
}

// ─── Commands ─────────────────────────────────────────────────────────────────

#[derive(Parser, Clone)]
pub enum GhRunnerCommands {
    #[clap(
        about = "Install and register a self-hosted runner pod",
        long_about = "Download the official actions/runner image, generate a podman kube YAML, fetch a registration token, and deploy the runner pod.\n\nExample:\n  b00t gh-runner install --repo app4dog/middleware --labels 'self-hosted,linux,x64' --workdir /var/lib/gh-runner/middleware --ephemeral"
    )]
    Install {
        #[clap(long, help = "Target repo (owner/repo)")]
        repo: String,
        #[clap(long, help = "Runner labels (comma-separated)")]
        labels: String,
        #[clap(long, help = "Work directory for runner data and pod YAML")]
        workdir: PathBuf,
        #[clap(long, help = "Runner group (default: default)")]
        group: Option<String>,
        #[clap(long, help = "Use ephemeral runner (one job then exit)")]
        ephemeral: bool,
        #[clap(
            long,
            help = "Container socket for CI builds: docker, podman, or none",
            default_value = "auto"
        )]
        socket: String,
        #[clap(
            long,
            help = "Max seconds to wait for pod to start",
            default_value = "120"
        )]
        timeout: u64,
    },
    #[clap(
        about = "Check runner status — podman pod state + GitHub API",
        long_about = "Dual-source status check: podman pod state and GitHub Actions runner registration.\n\nExample:\n  b00t gh-runner status --repo app4dog/middleware\n  b00t gh-runner status --repo app4dog/middleware --json"
    )]
    Status {
        #[clap(long, help = "Target repo (owner/repo)")]
        repo: String,
        #[clap(long, help = "JSON output")]
        json: bool,
    },
    #[clap(
        about = "View runner pod logs",
        long_about = "Stream podman pod logs for the runner container.\n\nExample:\n  b00t gh-runner logs --repo app4dog/middleware\n  b00t gh-runner logs --repo app4dog/middleware --follow"
    )]
    Logs {
        #[clap(long, help = "Target repo (owner/repo)")]
        repo: String,
        #[clap(long, help = "Follow log output")]
        follow: bool,
    },
    #[clap(
        about = "Deregister runner from GitHub and remove pod",
        long_about = "Fetch removal token, deregister from GitHub, tear down podman pod, and optionally remove work directory.\n\nExample:\n  b00t gh-runner deregister --repo app4dog/middleware\n  b00t gh-runner deregister --repo app4dog/middleware --remove-workdir"
    )]
    Deregister {
        #[clap(long, help = "Target repo (owner/repo)")]
        repo: String,
        #[clap(long, help = "Remove work directory after deregistration")]
        remove_workdir: bool,
    },
    #[clap(
        about = "Diagnose runner health",
        long_about = "Run a diagnostic checklist: gh auth, podman daemon, network connectivity, pod state, disk space.\n\nExample:\n  b00t gh-runner doctor\n  b00t gh-runner doctor --repo app4dog/middleware"
    )]
    Doctor {
        #[clap(
            long,
            help = "Target repo (owner/repo) — optional, checks all if omitted"
        )]
        repo: Option<String>,
    },
}

// ─── Podman Kube YAML Template ────────────────────────────────────────────────

const KUBE_YAML_TEMPLATE: &str = r#"apiVersion: v1
kind: Pod
metadata:
  name: gh-runner-{repo_slug}
  labels:
    app: gh-runner
    repo: {repo_slug}
spec:
  restartPolicy: OnFailure
  containers:
  - name: runner
    image: ghcr.io/actions/actions-runner:latest
    command:
    - /bin/bash
    - -c
    - |
      cd /home/runner
      ./config.sh --url "${REPO_URL}" --token "${RUNNER_TOKEN}" --name "${RUNNER_NAME}" --labels "${RUNNER_LABELS}" {ephemeral_flag} --unattended --replace
      ./run.sh
    workingDir: /home/runner
    env:
    - name: REPO_URL
      value: "https://github.com/{repo}"
    - name: RUNNER_NAME
      value: "{runner_name}"
    - name: RUNNER_LABELS
      value: "{labels}"
    - name: RUNNER_TOKEN
      value: "{token}"
    - name: RUNNER_EPHEMERAL
      value: "{ephemeral}"
    - name: RUNNER_WORKDIR
      value: "/runner/_work"
    volumeMounts:
    - name: work
      mountPath: /runner/_work
{docker_sock_volume_mount}
    resources:
      requests:
        memory: "2Gi"
        cpu: "2"
      limits:
        memory: "4Gi"
        cpu: "4"
    securityContext:
      runAsNonRoot: true
      runAsUser: 1001
      allowPrivilegeEscalation: false
  volumes:
  - name: work
    hostPath:
      path: {workdir}/_work
      type: DirectoryOrCreate
{docker_sock_volume}
"#;

pub fn generate_kube_yaml(
    repo: &str,
    labels: &str,
    workdir: &str,
    token: &str,
    ephemeral: bool,
    socket_path: &str,
) -> String {
    let slug = repo_slug(repo);
    let name = runner_name(repo);

    let ephemeral_flag = if ephemeral { "--ephemeral" } else { "" };

    let (docker_sock_volume_mount, docker_sock_volume) = if socket_path.is_empty() {
        (String::new(), String::new())
    } else {
        (
            format!("    - name: docker-sock\n      mountPath: /var/run/docker.sock\n"),
            format!(
                "  - name: docker-sock\n    hostPath:\n      path: {}\n      type: Socket\n",
                socket_path
            ),
        )
    };

    KUBE_YAML_TEMPLATE
        .replace("{repo_slug}", &slug)
        .replace("{repo}", repo)
        .replace("{runner_name}", &name)
        .replace("{labels}", labels)
        .replace("{token}", token)
        .replace("{ephemeral}", if ephemeral { "true" } else { "false" })
        .replace("{ephemeral_flag}", ephemeral_flag)
        .replace("{workdir}", workdir)
        .replace("{docker_sock_volume_mount}", &docker_sock_volume_mount)
        .replace("{docker_sock_volume}", &docker_sock_volume)
}

// ─── Install ─────────────────────────────────────────────────────────────────

fn cmd_install(
    repo: &str,
    labels: &str,
    workdir: &PathBuf,
    _group: &Option<String>,
    ephemeral: bool,
    socket: &str,
    timeout: u64,
) -> Result<()> {
    let slug = repo_slug(repo);
    let workdir_str = workdir.to_string_lossy().to_string();
    let socket_path = detect_socket(socket);

    // 1. Validate prereqs
    let (gh_ok, gh_out) = sh("gh auth status 2>&1");
    if !gh_ok {
        bail!("gh CLI not authenticated: {}", gh_out);
    }
    let (podman_ok, podman_out) = sh("podman info --format '{{.Version.Version}}' 2>&1");
    if !podman_ok {
        bail!("podman not available: {}", podman_out);
    }
    println!(
        "[1/7] Prerequisites OK — gh auth + podman {}",
        podman_out.trim()
    );

    // 2. Validate repo access
    let _ = retry_sh(
        &format!("gh api repos/{} --jq .full_name 2>&1", repo),
        2,
        "repo access check",
    )?;
    println!("[2/7] Repository {} accessible", repo);

    // 3. Create workdir (escalate via sudo if parent not writable)
    if let Err(e) = std::fs::create_dir_all(workdir.join("_work")) {
        let (sudo_ok, _) = sh(&format!(
            "sudo mkdir -p {} && sudo chown -R $(whoami):$(whoami) {} 2>&1",
            workdir.join("_work").display(),
            workdir_str
        ));
        if !sudo_ok {
            bail!(
                "Cannot create workdir: {}. Try:\n  sudo mkdir -p {}/_work && sudo chown -R $USER:$USER {}",
                e,
                workdir_str,
                workdir_str
            );
        }
    }
    println!("[3/7] Workdir created: {}", workdir_str);

    // 4. Fetch registration token (1h TTL)
    let token = retry_sh(
        &format!(
            "gh api -X POST repos/{}/actions/runners/registration-token --jq .token 2>&1",
            repo
        ),
        3,
        "registration token fetch",
    )?;
    if token.is_empty() {
        bail!("Empty registration token returned");
    }
    println!("[4/7] Registration token fetched (expires in 1h)");

    // 5. Generate and write kube YAML
    let yaml_content =
        generate_kube_yaml(repo, labels, &workdir_str, &token, ephemeral, &socket_path);
    let yaml_path = workdir.join("gh-runner.yaml");
    std::fs::write(&yaml_path, &yaml_content)
        .with_context(|| format!("Failed to write YAML: {}", yaml_path.display()))?;
    println!("[5/7] Pod spec generated: {}", yaml_path.display());

    // 6. Deploy pod
    let (deploy_ok, deploy_out) = sh(&format!("podman kube play {} 2>&1", yaml_path.display()));

    if !deploy_ok {
        bail!("Failed to deploy pod: {}", deploy_out);
    }
    println!("[6/7] Pod deployed: {}", deploy_out.trim());

    // Poll pod startup status with timeout
    let start = std::time::Instant::now();
    loop {
        let (ok, state) = sh(&format!(
            "podman pod ps --filter name=gh-runner-{} --format '{{{{.Status}}}}'",
            slug
        ));
        if ok && state.contains("Running") {
            break;
        }
        if start.elapsed().as_secs() > timeout {
            bail!(
                "Pod failed to start within {}s — check podman pod logs gh-runner-{}",
                timeout,
                slug
            );
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    // Verify registration
    let (verify_ok, verify_out) = sh(&format!(
        "gh api repos/{}/actions/runners --jq '.runners[] | select(.name==\"{}\") | {{name, status, labels}}' 2>&1",
        repo,
        runner_name(repo)
    ));
    if verify_ok && !verify_out.is_empty() {
        println!("  Runner registered:\n{}", verify_out);
    } else {
        println!("  Runner pod started — waiting for registration...");
    }

    println!(
        "\nRunner ready: labels=[{}]\n  Pod:    podman pod ps --filter name=gh-runner-{}\n  Status: b00t gh-runner status --repo {}\n  YAML:   {}/gh-runner.yaml",
        labels, slug, repo, workdir_str
    );

    Ok(())
}

// ─── Status ──────────────────────────────────────────────────────────────────

fn cmd_status(repo: &str, json_output: bool) -> Result<()> {
    let slug = repo_slug(repo);
    let name = runner_name(repo);

    // Podman pod status
    let (pod_ok, pod_state) = sh(&format!(
        "podman pod ps --filter name=gh-runner-{} --format '{{{{.Status}}}}' 2>&1",
        slug
    ));
    let pod_running = pod_ok && !pod_state.trim().is_empty();

    // Container status
    let (container_ok, container_state) = sh(&format!(
        "podman ps --filter pod=gh-runner-{} --format '{{{{.Status}}}}' 2>&1",
        slug
    ));

    // GitHub runner status
    let (gh_ok, gh_out) = sh(&format!(
        "gh api repos/{}/actions/runners --jq '.runners[] | select(.name==\"{}\") | {{name,status,busy,labels}}' 2>&1",
        repo, name
    ));

    if json_output {
        let report = json!({
            "repo": repo,
            "slug": slug,
            "runner_name": name,
            "pod": {
                "running": pod_running,
                "state": pod_state.trim(),
            },
            "container": {
                "running": container_ok,
                "state": container_state.trim(),
            },
            "github": if gh_ok && !gh_out.is_empty() {
                serde_json::from_str::<serde_json::Value>(&gh_out).ok()
            } else {
                None
            },
        });
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Pod:       gh-runner-{}  {}", slug, pod_state.trim());
        println!(
            "Container: gh-runner-{}-runner  {}",
            slug,
            container_state.trim()
        );
        if gh_ok && !gh_out.is_empty() {
            println!("Runner:    {}", gh_out.trim());
        } else {
            println!("Runner:    not registered or not found on GitHub");
        }
        if !pod_running {
            let (_, inspect) = sh(&format!(
                "podman pod inspect gh-runner-{} --format '{{{{.State}}}}' 2>/dev/null",
                slug
            ));
            if !inspect.is_empty() {
                println!("Pod state:  {}", inspect);
            }
            let (_, last_log) = sh(&format!(
                "podman pod logs --tail 5 gh-runner-{} 2>/dev/null",
                slug
            ));
            if !last_log.is_empty() {
                println!("Last logs: {}", last_log);
            }
        }
    }

    Ok(())
}

// ─── Logs ─────────────────────────────────────────────────────────────────────

fn cmd_logs(repo: &str, follow: bool) -> Result<()> {
    let slug = repo_slug(repo);

    let cmd_str = if follow {
        format!("podman pod logs --follow gh-runner-{} 2>&1", slug)
    } else {
        format!("podman pod logs --tail 50 gh-runner-{} 2>&1", slug)
    };

    // Use direct exec for logs so follow works with terminal
    let status = std::process::Command::new("sh")
        .args(["-c", &cmd_str])
        .status()
        .with_context(|| format!("Failed to run: {}", cmd_str))?;

    if !status.success() {
        bail!("Log command exited with: {}", status);
    }
    Ok(())
}

// ─── Deregister ───────────────────────────────────────────────────────────────

fn cmd_deregister(repo: &str, remove_workdir: bool) -> Result<()> {
    let slug = repo_slug(repo);
    let name = runner_name(repo);

    // Find workdir from the running pod's YAML or from a common path
    // First try to get workdir from the pod's volume mount
    let (_ok, workdir) = sh(&format!(
        "podman inspect gh-runner-{}-runner --format '{{{{range .Mounts}}}}{{{{if eq .Destination \"/runner/_work\"}}}}{{{{.Source}}}}{{{{end}}}}{{{{end}}}}' 2>&1",
        slug
    ));
    let workdir_path = if !workdir.is_empty() {
        // Source is {workdir}/_work, so parent is workdir
        let p = PathBuf::from(&workdir);
        p.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from(format!("/var/lib/gh-runner/{}", slug)))
    } else {
        PathBuf::from(format!("/var/lib/gh-runner/{}", slug))
    };
    let yaml_path = workdir_path.join("gh-runner.yaml");

    // 1. Fetch removal token and deregister from GitHub
    match retry_sh(
        &format!(
            "gh api -X POST repos/{}/actions/runners/removal-token --jq .token 2>&1",
            repo
        ),
        3,
        "removal token fetch",
    ) {
        Ok(token) if !token.is_empty() => {
            let remove_sh = format!(
                "gh api -X DELETE repos/{}/actions/runners/$(gh api repos/{}/actions/runners --jq '.runners[] | select(.name==\"{}\") | .id') 2>&1",
                repo, repo, name
            );
            let (rm_ok, rm_out) = sh(&remove_sh);
            if rm_ok {
                println!("[1/3] Runner deregistered from GitHub");
            } else {
                println!("[1/3] GitHub deregister: {}", rm_out);
            }
        }
        _ => {
            // Fallback: try direct runner ID lookup
            let lookup_sh = format!(
                "RUNNER_ID=$(gh api repos/{}/actions/runners --jq '.runners[] | select(.name==\"{}\") | .id' 2>/dev/null); \
             if [ -n \"$RUNNER_ID\" ]; then gh api -X DELETE repos/{}/actions/runners/$RUNNER_ID && echo ok; else echo 'runner not found on GitHub'; fi",
                repo, name, repo
            );
            let (_, rm_out) = sh(&lookup_sh);
            println!("[1/3] GitHub: {}", rm_out.trim());
        }
    }

    // 2. Tear down pod
    if yaml_path.exists() {
        let (down_ok, down_out) = sh(&format!("podman kube down {} 2>&1", yaml_path.display()));
        if down_ok {
            println!("[2/3] Pod torn down: gh-runner-{}", slug);
        } else {
            eprintln!("  Warning: podman kube down: {}", down_out.trim());
            // Try direct pod removal
            let _ = sh(&format!("podman pod rm -f gh-runner-{} 2>/dev/null", slug));
        }
    } else {
        let _ = sh(&format!("podman pod rm -f gh-runner-{} 2>/dev/null", slug));
        println!("[2/3] Pod removed (no YAML found, direct pod rm)");
    }

    // 3. Remove workdir if requested
    if remove_workdir && workdir_path.exists() {
        std::fs::remove_dir_all(&workdir_path)
            .with_context(|| format!("Failed to remove workdir: {}", workdir_path.display()))?;
        println!("[3/3] Workdir removed: {}", workdir_path.display());
    } else if remove_workdir {
        println!("[3/3] Workdir not found: {}", workdir_path.display());
    } else {
        println!("[3/3] Workdir preserved: {}", workdir_path.display());
    }

    Ok(())
}

// ─── Doctor ───────────────────────────────────────────────────────────────────

fn cmd_doctor(repo: Option<&str>) -> Result<()> {
    println!("gh-runner doctor\n");

    // 1. gh auth
    let (gh_ok, gh_out) = sh("gh auth status 2>&1");
    println!(
        "  {}  gh auth: {}",
        if gh_ok { "ok" } else { "FAIL" },
        gh_out.lines().next().unwrap_or("unknown")
    );

    // 2. podman
    let (podman_ok, podman_out) = sh("podman version --format '{{.Version}}' 2>&1");
    println!(
        "  {}  podman: {}",
        if podman_ok { "ok" } else { "FAIL" },
        podman_out.trim()
    );

    // 3. Network
    let (net_ok, _) =
        sh("curl -sf --max-time 5 -o /dev/null -w '%{http_code}' https://api.github.com/zen 2>&1");
    println!(
        "  {}  github.com reachable",
        if net_ok { "ok" } else { "FAIL" }
    );

    // 4. Container socket
    let socket_path = detect_socket("auto");
    let (sock_ok, _) = sh(&format!("test -S {} && echo exists 2>&1", socket_path));
    let socket_label = if socket_path.contains("docker.sock") {
        format!("docker socket ({})", socket_path)
    } else if socket_path.contains("podman") {
        format!("podman socket ({})", socket_path)
    } else {
        "none (no container builds)".to_string()
    };
    println!(
        "  {}  {}",
        if sock_ok { "ok" } else { "warn (not found)" },
        socket_label
    );

    // 5. Actions runner image
    let (img_ok, img_out) = sh(
        "podman image exists ghcr.io/actions/actions-runner:latest 2>&1 || echo 'not pulled yet (will pull on install)' 2>&1",
    );
    println!(
        "  {}  actions-runner image: {}",
        if img_ok { "ok" } else { "info" },
        img_out.lines().next().unwrap_or("checking...")
    );

    // 6. Per-repo checks if specified
    if let Some(repo) = repo {
        let slug = repo_slug(repo);
        println!("\n  Repo: {}", repo);

        // Pod status
        let (_pod_ok, pod_state) = sh(&format!(
            "podman pod ps --filter name=gh-runner-{} --format '{{{{.Status}}}}' 2>&1",
            slug
        ));
        println!(
            "  {}  pod: {}",
            if !pod_state.trim().is_empty() && pod_state.trim() != "{{.Status}}" {
                "ok"
            } else {
                "info"
            },
            pod_state.trim()
        );

        // Registration
        let name = runner_name(repo);
        let (reg_ok, reg_out) = sh(&format!(
            "gh api repos/{}/actions/runners --jq '[.runners[] | select(.name==\"{}\")] | length' 2>&1",
            repo, name
        ));
        println!(
            "  {}  registered on GitHub: {}",
            if reg_ok && reg_out.trim() == "1" {
                "ok"
            } else {
                "info"
            },
            reg_out.trim()
        );

        // Disk
        let workdir = format!("/var/lib/gh-runner/{}", slug);
        let (disk_ok, disk_out) = sh(&format!("df -h {} 2>&1 | tail -1", workdir));
        println!(
            "  {}  disk: {}",
            if disk_ok { "ok" } else { "info" },
            disk_out.trim()
        );
    }

    Ok(())
}

// ─── Handler ──────────────────────────────────────────────────────────────────

pub fn handle_gh_runner_command(args: &GhRunnerCommands) -> Result<()> {
    match args {
        GhRunnerCommands::Install {
            repo,
            labels,
            workdir,
            group,
            ephemeral,
            socket,
            timeout,
        } => cmd_install(repo, labels, workdir, group, *ephemeral, socket, *timeout),
        GhRunnerCommands::Status { repo, json } => cmd_status(repo, *json),
        GhRunnerCommands::Logs { repo, follow } => cmd_logs(repo, *follow),
        GhRunnerCommands::Deregister {
            repo,
            remove_workdir,
        } => cmd_deregister(repo, *remove_workdir),
        GhRunnerCommands::Doctor { repo } => cmd_doctor(repo.as_deref()),
    }
}
