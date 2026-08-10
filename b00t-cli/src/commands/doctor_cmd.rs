//! `b00t doctor` — dependency verification, IDE wiring, env documentation
//!
//! # Usage
//! ```bash
//! b00t doctor check             # verify deps
//! b00t doctor check --json      # JSON report
//! b00t doctor check --probe gh  # single dep
//! b00t doctor setup --role=executive,operator  # verify + wire MCP
//! b00t doctor env               # env docs for model
//! b00t doctor ide list          # MCP servers in IDEs
//! ```

use crate::datum_store::{DatumStore, HashMapStore, ReferenceError};
use anyhow::{Context, Result};
use b00t_c0re_lib::redis::{RedisComms, RedisConfig};
use clap::Parser;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/home/brianh"))
}

fn sh(cmd: &str) -> (bool, String) {
    Command::new("sh")
        .args(["-c", cmd])
        .output()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let e = String::from_utf8_lossy(&o.stderr).trim().to_string();
            (
                o.status.success() && !s.is_empty(),
                if !s.is_empty() {
                    s
                } else if !e.is_empty() {
                    e
                } else {
                    "not found".into()
                },
            )
        })
        .unwrap_or((false, "exec failed".into()))
}

fn check_version(name: &str) -> Value {
    let which = sh(&format!("which {} 2>/dev/null", name));
    let ver = if which.0 {
        sh(&format!("{} --version 2>/dev/null | head -1", name))
    } else {
        (false, String::new())
    };
    json!({"id": name, "pass": which.0 || ver.0, "detail": if which.0 || ver.0 {
        format!("{} {}", which.1.trim(), ver.1.trim())
    } else { "not found".into() }})
}

/// Probe the optional Redis-backed agent registry (issue #83). Reuses
/// `RedisComms::is_available()` rather than shelling out to `redis-cli` so
/// this exercises the same connection path `agent discover`/`agent
/// capability` use. Always `pass: true` — Redis is an optional accelerant
/// for live multi-host discovery, not a hard dependency; both commands fall
/// back to local `_b00t_/*.agent.toml` when it's unreachable.
fn check_redis_agent_registry() -> Value {
    let start = Instant::now();
    let reachable = RedisComms::new(RedisConfig::default(), "doctor-probe".into())
        .map(|c| c.is_available())
        .unwrap_or(false);
    json!({
        "id": "redis-agent-registry",
        "pass": true,
        "detail": if reachable {
            "reachable — live multi-host agent discovery available"
        } else {
            "unreachable — optional, accelerates live multi-host agent discovery; falls back to local `_b00t_/*.agent.toml` when unavailable"
        },
        "latency_ms": start.elapsed().as_millis(),
    })
}

/// Submodule pin drift check (#923): shells out to the standalone bash
/// script that is the single source of truth for this check (both
/// `just doctor` and this call it). Kept as a script rather than a Rust
/// implementation because a pre-cargo gate that itself requires compiling
/// b00t-cli is a chicken-and-egg risk — see the justfile's `viz-entangle`
/// comment ("cargo run fails on b00t repo due to git worktree structure").
///
/// Distinguishes drifted+clean (safe, auto-fixable via `fix: true`) from
/// drifted+dirty (report only — the script itself never touches dirty
/// submodules regardless of the `--fix` flag it's given).
fn check_submodule_drift(b00t_path: &str, fix: bool) -> Value {
    let script = PathBuf::from(b00t_path).join("scripts/check-submodule-drift.sh");
    let mut cmd = Command::new("bash");
    cmd.arg(&script).arg("--json");
    if fix {
        cmd.arg("--fix");
    }

    let start = Instant::now();
    let output = cmd.output();
    let ms = start.elapsed().as_millis();

    match output {
        Ok(o) => match serde_json::from_slice::<Value>(&o.stdout) {
            Ok(submodules) => {
                let unresolved = submodules
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter(|s| {
                                matches!(
                                    s["status"].as_str(),
                                    Some("drifted_dirty") | Some("drifted_clean")
                                )
                            })
                            .count()
                    })
                    .unwrap_or(0);
                // branch_status: stale (2026-08-09, see check-submodule-drift.sh
                // doc comment) — recorded pin unreachable from the .gitmodules
                // branch=; a separate axis from checked-out-vs-recorded drift,
                // so counted separately here even though it's the same
                // `pass`/exit-code from the script.
                let stale_branch_pins: Vec<&str> = submodules
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter(|s| s["branch_status"].as_str() == Some("stale"))
                            .filter_map(|s| s["path"].as_str())
                            .collect()
                    })
                    .unwrap_or_default();
                let pass = o.status.success();
                json!({
                    "id": "submodule-drift",
                    "pass": pass,
                    "detail": if pass {
                        "0 drifted submodules".to_string()
                    } else {
                        format!(
                            "{unresolved} unresolved submodule drift, {} stale branch pin(s){} (see submodules[])",
                            stale_branch_pins.len(),
                            if stale_branch_pins.is_empty() {
                                String::new()
                            } else {
                                format!(" [{}]", stale_branch_pins.join(", "))
                            }
                        )
                    },
                    "submodules": submodules,
                    "latency_ms": ms,
                })
            }
            Err(e) => json!({
                "id": "submodule-drift",
                "pass": false,
                "detail": format!("failed to parse {} --json output: {e}", script.display()),
                "latency_ms": ms,
            }),
        },
        Err(e) => json!({
            "id": "submodule-drift",
            "pass": false,
            "detail": format!("failed to run {}: {e}", script.display()),
            "latency_ms": ms,
        }),
    }
}

/// A vendor crate whose built release binary something in b00t's own config
/// actually spawns/expects (#814) — sourced from the `[b00t.vendor]` table's
/// `health_check` field in `_b00t_/datums/VENDOR-*.tomllmd` registry
/// entries. Scope is deliberately narrow: only registry entries whose
/// `health_check` asserts `test -x <path>` are considered (Rust/Go binaries
/// built via `cargo`/`make` into a concrete path) — python/bun-installed
/// vendors (VENDOR-AGENT-FRAMEWORK, VENDOR-HERMES-AGENT-B00T,
/// VENDOR-OPENCODE-B00T, VENDOR-PI-MONO, VENDOR-PINGAP-DEVPROXY-B00T) and the
/// ~20 other vendored submodules with no VENDOR-*.tomllmd entry at all are
/// out of scope, per #814's "don't flag every vendored crate" requirement.
struct VendorBinaryExpectation {
    /// Datum key, e.g. "VENDOR-IRONTOLOGY-MCP".
    name: String,
    /// Binary path relative to the b00t repo root, e.g.
    /// "vendor/irontology-mcp/target/release/mcp-server".
    binary_path: String,
    /// Exact build command from the registry entry, verbatim for the operator.
    build_command: String,
}

/// Line-oriented scrape of the `[b00t.vendor]` table's `key = "value"`
/// entries. `_b00t_/datums/*.tomllmd` files mix TOML front matter with a
/// markdown prose body and are NOT valid whole-file TOML — verified:
/// `toml::from_str` fails on every `VENDOR-*.tomllmd` in this repo, on its
/// prose lines. The crate's own generic datum loader
/// (`get_all_datums`/`scan_datums_recursive` in datum_utils.rs, which does
/// parse the whole file) therefore silently skips these files; they never
/// appear in `HashMapStore`. Scanning just the one table this check needs
/// sidesteps that pre-existing parser gap rather than widening #814's scope
/// to fix it.
fn parse_vendor_table(content: &str) -> std::collections::HashMap<String, String> {
    let mut fields = std::collections::HashMap::new();
    let mut in_vendor = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(header) = trimmed.strip_prefix('[') {
            in_vendor = header.trim_end_matches(']') == "b00t.vendor";
            continue;
        }
        if !in_vendor {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            fields.insert(
                k.trim().to_string(),
                v.trim().trim_matches('"').to_string(),
            );
        }
    }
    fields
}

/// Enumerate vendor binary expectations from `<repo_root>/_b00t_/datums/VENDOR-*.tomllmd`.
fn find_vendor_binary_expectations(repo_root: &Path) -> Vec<VendorBinaryExpectation> {
    let datums_dir = repo_root.join("_b00t_/datums");
    let Ok(entries) = std::fs::read_dir(&datums_dir) else {
        return vec![];
    };
    let mut out: Vec<VendorBinaryExpectation> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let path = e.path();
            let fname = path.file_name()?.to_str()?;
            if !(fname.starts_with("VENDOR-") && fname.ends_with(".tomllmd")) {
                return None;
            }
            let content = std::fs::read_to_string(&path).ok()?;
            let fields = parse_vendor_table(&content);
            let health_check = fields.get("health_check")?;
            let idx = health_check.find("test -x ")?;
            let binary_path = health_check[idx + "test -x ".len()..]
                .split_whitespace()
                .next()?
                .to_string();
            if binary_path.is_empty() {
                return None;
            }
            Some(VendorBinaryExpectation {
                name: fname.trim_end_matches(".tomllmd").to_string(),
                binary_path,
                build_command: fields.get("build_command").cloned().unwrap_or_default(),
            })
        })
        .collect();
    // De-dupe by binary path: VENDOR-LEDGRRR / VENDOR-L3DG3RR both point at
    // vendor/ledgrrr's ledgerr-mcp (two checkouts of the same upstream repo)
    // — report the shared binary once.
    out.sort_by(|a, b| a.binary_path.cmp(&b.binary_path).then(a.name.cmp(&b.name)));
    out.dedup_by(|a, b| a.binary_path == b.binary_path);
    out
}

/// Missing vendor binary detection (#814): `b00t lfmf` and friends spawn
/// vendor MCP/CLI binaries (e.g. irontology-mcp) that are never auto-built —
/// the submodule can be checked out with no `target/release/<bin>` present,
/// producing an opaque "No such file or directory (os error 2)" at call
/// time. This surfaces that as a doctor check instead: PASS/FAIL per
/// registered vendor binary, with the exact build command to run.
///
/// Deliberately never auto-builds, even under `--fix`: these are `cargo
/// build --release` (or `make`/`bun`) invocations against multi-crate
/// workspaces (irontology-mcp alone has 20+ crates) with unbounded wall
/// time — unlike #923's submodule-drift `--fix` (a bounded `git` pin sync)
/// or #924's gutted-gitdir repair (a bounded re-clone), "safe and fast"
/// can't be guaranteed for an arbitrary vendor release build. `--fix`
/// therefore only annotates the report; the operator always runs the build
/// command manually.
fn check_vendor_binaries(repo_root: &Path, fix: bool) -> Value {
    let start = Instant::now();
    let expectations = find_vendor_binary_expectations(repo_root);
    let results: Vec<Value> = expectations
        .iter()
        .map(|v| {
            let full_path = repo_root.join(&v.binary_path);
            // A present-but-non-executable file (e.g. a stray regular file)
            // would still fail the same "os error 2"-shaped spawn the way
            // `irontology-mcp` was actually reported (os error 13,
            // Permission denied, if present-but-not-+x) — check the
            // executable bit, not just existence.
            #[cfg(unix)]
            let built = {
                use std::os::unix::fs::PermissionsExt;
                full_path
                    .metadata()
                    .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                    .unwrap_or(false)
            };
            #[cfg(not(unix))]
            let built = full_path.is_file();
            json!({
                "name": v.name,
                "binary_path": v.binary_path,
                "status": if built { "built" } else { "missing" },
                "build_command": v.build_command,
            })
        })
        .collect();
    let missing: Vec<&Value> = results
        .iter()
        .filter(|r| r["status"] == "missing")
        .collect();
    let pass = missing.is_empty();
    let mut detail = if pass {
        format!("{}/{} vendor binaries built", results.len(), results.len())
    } else {
        let commands: Vec<String> = missing
            .iter()
            .map(|r| {
                format!(
                    "{}: {}",
                    r["binary_path"].as_str().unwrap_or("?"),
                    r["build_command"].as_str().unwrap_or("?")
                )
            })
            .collect();
        format!(
            "{} missing vendor binar{} — {}",
            missing.len(),
            if missing.len() == 1 { "y" } else { "ies" },
            commands.join(" | ")
        )
    };
    if fix && !pass {
        detail.push_str(
            " (--fix not applied: vendor release builds are slow/unbounded — run the build command(s) above manually)",
        );
    }
    json!({
        "id": "vendor-binaries",
        "pass": pass,
        "detail": detail,
        "vendor_binaries": results,
        "latency_ms": start.elapsed().as_millis(),
    })
}

/// Detect WSL2 the same way `b00t-cli/src/bin/rpa.rs`'s `detect_wsl()` does.
fn is_wsl() -> bool {
    Path::new("/proc/sys/fs/binfmt_misc/WSLInterop").exists()
        || std::fs::read_to_string("/proc/version")
            .map(|v| v.contains("Microsoft") || v.contains("WSL"))
            .unwrap_or(false)
}

/// Run a PowerShell command on the Windows host via `powershell.exe`
/// interop, invoked directly (no intermediate `sh -c`) so `$`-prefixed
/// PowerShell syntax (`$_.Name`, etc.) never gets shell-expanded first.
fn powershell(script: &str) -> (bool, String) {
    Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .map(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            (o.status.success() && !s.is_empty(), s)
        })
        .unwrap_or((false, String::new()))
}

/// Parse a single integer (MTU value) out of a PowerShell one-liner's
/// trimmed stdout — tolerant of trailing blank lines.
fn parse_mtu(output: &str) -> Option<u32> {
    output.trim().lines().next()?.trim().parse().ok()
}

/// True once the local interface's MTU exceeds the tunnel's advertised
/// MTU — the exact shape of the WSL2 + Cloudflare WARP path-MTU blackhole
/// diagnosed 2026-08-09 (see `check_wsl_warp_mtu` doc comment).
fn mtu_at_risk(local_mtu: u32, tunnel_mtu: u32) -> bool {
    local_mtu > tunnel_mtu
}

/// WSL2 + Cloudflare WARP path-MTU blackhole (documented 2026-08-09, this
/// node). When Cloudflare WARP is active on the Windows host, its
/// WireGuard tunnel advertises a lower MTU (commonly ~1300) than WSL's
/// `eth0` vEthernet, which stays at 1500 regardless of what the host
/// actually routes through. Windows doesn't relay an ICMP "fragmentation
/// needed" back into the WSL guest, so PMTUD never corrects it (and
/// `net.ipv4.tcp_mtu_probing=0` by default means Linux won't self-heal
/// either): TCP connections complete their handshake fine (small packets)
/// and then hang forever the instant a larger payload has to cross —
/// TLS ServerHello, SSH channel data, `git-upload-pack` responses. Symptom
/// is `git fetch`/`curl`/`ssh` hanging indefinitely with no error, not a
/// clean failure.
///
/// Only meaningful under WSL2 (no-op pass everywhere else) and only when
/// WARP is actually up (pass if absent/disconnected). `eth0` is assumed —
/// WSL2's default NAT networking mode always names it that; a host running
/// "mirrored" networking mode (a different interface topology) is out of
/// scope here.
///
/// `--fix` attempts `ip link set dev eth0 mtu <tunnel_mtu>` in-process,
/// which requires `CAP_NET_ADMIN` (root); most agent/CI contexts have no
/// TTY for a `sudo` password prompt, so on failure the detail string
/// carries the exact command to run manually instead of blocking on one.
fn check_wsl_warp_mtu(fix: bool) -> Value {
    let start = Instant::now();
    let id = "wsl-warp-mtu";
    let elapsed = |start: Instant| start.elapsed().as_millis();

    if !is_wsl() {
        return json!({"id": id, "pass": true, "detail": "not WSL, skipped", "latency_ms": elapsed(start)});
    }

    let warp = powershell(
        "(Get-NetAdapter -ErrorAction SilentlyContinue | Where-Object { $_.InterfaceDescription -like '*Cloudflare WARP*' -and $_.Status -eq 'Up' }).Name",
    );
    if !warp.0 || warp.1.is_empty() {
        return json!({"id": id, "pass": true, "detail": "Cloudflare WARP not active on Windows host", "latency_ms": elapsed(start)});
    }
    let warp_adapter = warp.1.lines().next().unwrap_or("").trim().to_string();

    let warp_mtu_out = powershell(&format!(
        "(Get-NetIPInterface -InterfaceAlias '{warp_adapter}' -AddressFamily IPv4 -ErrorAction SilentlyContinue).NlMtu"
    ));
    let local_mtu_str = std::fs::read_to_string("/sys/class/net/eth0/mtu").unwrap_or_default();

    let (Some(warp_mtu), Some(local_mtu)) =
        (parse_mtu(&warp_mtu_out.1), parse_mtu(&local_mtu_str))
    else {
        return json!({
            "id": id, "pass": true,
            "detail": format!("WARP active ({warp_adapter}) but couldn't read MTU values to compare — inconclusive, not failing"),
            "latency_ms": elapsed(start),
        });
    };

    if !mtu_at_risk(local_mtu, warp_mtu) {
        return json!({
            "id": id, "pass": true,
            "detail": format!("WARP active ({warp_adapter}, tunnel MTU {warp_mtu}), eth0 MTU {local_mtu} already <= tunnel MTU"),
            "latency_ms": elapsed(start),
        });
    }

    let mut detail = format!(
        "WARP active ({warp_adapter}, tunnel MTU {warp_mtu}) but eth0 MTU is {local_mtu} — \
         packets above {warp_mtu} bytes are silently dropped (path-MTU blackhole, no ICMP \
         frag-needed feedback). git fetch/curl/ssh will hang forever with no error. \
         Fix: sudo ip link set dev eth0 mtu {warp_mtu}"
    );
    let mut pass = false;
    if fix {
        let applied = Command::new("ip")
            .args(["link", "set", "dev", "eth0", "mtu", &warp_mtu.to_string()])
            .output();
        match applied {
            Ok(o) if o.status.success() => {
                pass = true;
                detail = format!("eth0 MTU lowered to {warp_mtu} to match WARP tunnel");
            }
            _ => detail.push_str(" (--fix attempted without root; run the sudo command above manually)"),
        }
    }

    json!({"id": id, "pass": pass, "detail": detail, "latency_ms": elapsed(start)})
}

fn all_deps(fix: bool) -> Vec<Value> {
    let mut results: Vec<Value> = vec![
        check_version("b00t-cli"),
        check_version("b00t-mcp"),
        check_version("b00t-task"),
        check_version("git"),
        check_version("gh"),
        check_version("just"),
        check_version("cargo"),
        check_version("rustc"),
        check_version("node"),
        check_version("npm"),
        check_version("python3"),
        check_version("docker"),
        check_version("jq"),
        check_version("curl"),
        // Special checks with auth/daemon info
        json!({"id": "gh-auth", "check": "gh auth status 2>&1 | grep -q 'Logged in' && echo yes || echo no"}),
        json!({"id": "docker-daemon", "check": "docker info --format '{{.ServerVersion}}' 2>/dev/null"}),
        check_redis_agent_registry(),
        // Filesystem
        json!({"id": "b00t-repo", "check": "test -d $HOME/.b00t/.git && cd $HOME/.b00t && git log --oneline -1 2>/dev/null"}),
        json!({"id": "soul-db", "check": "test -f $HOME/._b00t_/soul.db && ls -la $HOME/._b00t_/soul.db || echo missing"}),
        json!({"id": "task-queue", "check": "test -d $HOME/.local/share/b00t/task-queue/pending && ls $HOME/.local/share/b00t/task-queue/pending/*.json 2>/dev/null | wc -l || echo 0"}),
        json!({"id": "epoch-state", "check": "cat $HOME/.local/share/b00t/epoch-state.json 2>/dev/null | jq -e '.epoch' >/dev/null 2>&1 && echo valid || echo invalid"}),
        // Network
        json!({"id": "dns", "check": "host github.com 2>/dev/null | head -1 | grep -q 'has address' && echo ok || echo fail"}),
        json!({"id": "gh-api", "check": "curl -sf --max-time 5 -o /dev/null -w '%{http_code}' https://api.github.com/zen 2>/dev/null"}),
        // Skill datum integrity: no loose .skill.* files in _b00t_/
        json!({"id": "skill-symlinks", "check": "cd $HOME/.dotfiles 2>/dev/null && find _b00t_/ -name '*.skill.*' ! -type l 2>/dev/null | wc -l | tr -d ' ' || echo 0"}),
        // Stray root-level test/ dir (#935): bats-core/bats-assert/bats-support
        // are registered as submodules under _b00t_/test/ only. A root-level
        // test/ dir is an untracked, unregistered footgun — a duplicate
        // checkout that could silently diverge from the real submodule.
        json!({"id": "no-stray-root-test-dir", "check": "test -d $HOME/.b00t/test && echo FAIL || echo PASS"}),
        // Gutted submodule gitdir (#924): .git/modules/<path> exists (config present)
        // but HEAD is missing — the low-level gitdir was partially destroyed,
        // making bare `git status` fatal for the whole superproject.
        json!({"id": "no-gutted-submodule-gitdir", "check":
            "cd $HOME/.b00t 2>/dev/null && bad=$(git config -f .gitmodules --get-regexp '\\.path$' 2>/dev/null | awk '{print $2}' | while read -r p; do d=\".git/modules/$p\"; [ -f \"$d/config\" ] && [ ! -f \"$d/HEAD\" ] && echo \"$p\"; done); [ -z \"$bad\" ] && echo PASS || echo \"FAIL: $bad\""
        }),
    ].into_iter().map(|mut v| {
        let check = v.get("check").and_then(|c| c.as_str()).unwrap_or("");
        if !check.is_empty() {
            let start = Instant::now();
            let (ok, detail) = sh(check);
            // For skill-symlinks, pass is only when count is 0
            if v["id"] == "skill-symlinks" {
                let count: usize = detail.trim().parse().unwrap_or(1);
                v["pass"] = json!(count == 0);
                v["detail"] = if count == 0 {
                    json!("all .skill.* files are symlinks")
                } else {
                    json!(format!("{} loose .skill.* files in _b00t_/ (must be symlinks to skills/*/SKILL.md)", count))
                };
            } else if v["id"] == "no-stray-root-test-dir" {
                let pass = detail.trim() == "PASS";
                v["pass"] = json!(pass);
                v["detail"] = if pass {
                    json!("no stray root-level test/ dir")
                } else {
                    json!("stray $HOME/.b00t/test/ dir present — use _b00t_/test/* submodules instead (#935)")
                };
            } else if v["id"] == "no-gutted-submodule-gitdir" {
                let pass = detail.trim() == "PASS";
                v["pass"] = json!(pass);
                v["detail"] = if pass {
                    json!("no gutted submodule gitdirs")
                } else {
                    let bad = detail.trim().trim_start_matches("FAIL: ");
                    json!(format!(
                        "gutted gitdir(s): {} — repair: git -C $HOME/.b00t submodule deinit -f <path>; rm -rf $HOME/.b00t/.git/modules/<path>; git -C $HOME/.b00t submodule update --init <path> (#924), or run `b00t doctor fix-submodule <path>`",
                        bad
                    ))
                };
            } else {
                v["pass"] = json!(ok);
                v["detail"] = json!(detail);
            }
            v["latency_ms"] = json!(start.elapsed().as_millis());
        }
        v
    }).collect();

    // Submodule pin drift (#923): recorded gitlink vs checked-out HEAD.
    // check_submodule_drift() joins "scripts/check-submodule-drift.sh" onto
    // whatever it's given, so it needs the `_b00t_/` datum dir itself, not
    // the repo root — mirrors the "b00t-repo" check above's $HOME/.b00t,
    // just one level deeper. Distinct from the `b00t_path` fn parameter
    // threaded through this module (defaults to ~/.dotfiles/_b00t_).
    let repo_root = home().join(".b00t");
    let repo_b00t_dir = repo_root.join("_b00t_");
    results.push(check_submodule_drift(&repo_b00t_dir.to_string_lossy(), fix));

    // Missing vendor binary detection (#814): see check_vendor_binaries() doc
    // comment. Shares the $HOME/.b00t repo-root convention with the
    // submodule-drift check above rather than the `b00t_path` fn parameter.
    results.push(check_vendor_binaries(&repo_root, fix));

    // WSL2 + Cloudflare WARP path-MTU blackhole: see check_wsl_warp_mtu()
    // doc comment. No-op pass on non-WSL hosts.
    results.push(check_wsl_warp_mtu(fix));

    results
}

/// Enumerate submodules whose gitdir was "gutted": `.git/modules/<path>/config`
/// exists but `HEAD` does not (objects/refs are typically gone too). See #924.
fn find_gutted_submodules(repo_root: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(repo_root.join(".gitmodules")) else {
        return vec![];
    };
    content
        .lines()
        .filter_map(|l| l.trim().strip_prefix("path = ").map(str::trim))
        .filter(|p| {
            let gitdir = repo_root.join(".git/modules").join(p);
            gitdir.join("config").is_file() && !gitdir.join("HEAD").is_file()
        })
        .map(String::from)
        .collect()
}

/// Repair a gutted submodule gitdir (#924). A gutted gitdir has no HEAD,
/// objects, or refs — there is nothing recoverable to lose — so the repair
/// is a straight re-clone: deinit the submodule (clears its checked-out
/// working-tree content, which normally survives the gitdir being gutted),
/// remove the corrupt gitdir, then re-init from the remote registered in
/// .gitmodules. Safe to auto-apply for exactly this reason (unlike #923's
/// drift checks, which distinguish safe/unsafe because they may discard
/// *uncommitted* local state — a gutted gitdir has none, and `deinit -f`
/// only ever removes files that came from the (unrecoverable) checkout).
fn repair_gutted_submodule(repo_root: &Path, submodule_path: &str) -> Result<String> {
    let gitdir = repo_root.join(".git/modules").join(submodule_path);
    anyhow::ensure!(
        gitdir.join("config").is_file() && !gitdir.join("HEAD").is_file(),
        "{} does not match the gutted-gitdir shape (config present, HEAD missing) — refusing to touch it",
        submodule_path
    );

    // `git submodule update --init` refuses to clone into a non-empty
    // directory, and the submodule's working-tree content typically
    // survives the gitdir being gutted (only .git/modules/<path> was
    // destroyed). `deinit -f` clears that working-tree content and the
    // stale `.git` pointer file together, and must run before the gitdir
    // is removed — it still resolves config through the (gutted but
    // present) gitdir.
    let deinit = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["submodule", "deinit", "-f", submodule_path])
        .output()
        .context("running git submodule deinit")?;
    anyhow::ensure!(
        deinit.status.success(),
        "git submodule deinit -f {} failed: {}",
        submodule_path,
        String::from_utf8_lossy(&deinit.stderr)
    );

    std::fs::remove_dir_all(&gitdir)
        .with_context(|| format!("removing gutted gitdir {}", gitdir.display()))?;

    // Belt-and-suspenders: deinit already removes the `.git` pointer file,
    // but clean it up if anything unexpected left it behind.
    let dotgit = repo_root.join(submodule_path).join(".git");
    if dotgit.is_file() {
        std::fs::remove_file(&dotgit)
            .with_context(|| format!("removing stale .git pointer at {}", dotgit.display()))?;
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["submodule", "update", "--init", submodule_path])
        .output()
        .context("running git submodule update --init")?;
    anyhow::ensure!(
        output.status.success(),
        "git submodule update --init {} failed: {}",
        submodule_path,
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(format!("repaired {}", submodule_path))
}

#[derive(Default)]
struct RoleComposite {
    agents: Vec<String>,
    cli: Vec<String>,
    mcps: Vec<String>,
    skills: Vec<String>,
    compliance: Vec<String>,
}

fn compose_roles(roles: &[String], b00t_path: &str) -> Result<RoleComposite> {
    let mut merged = RoleComposite::default();
    for role_name in roles {
        let ext = if *role_name == "executive" {
            "role.tomllm"
        } else {
            "role.toml"
        };
        let path = PathBuf::from(b00t_path).join(format!("{}.{}", role_name, ext));
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("role datum not found: {}", path.display()))?;
        let v: Value = toml::from_str(&content)?;
        let b = &v["b00t"];
        if let Some(arr) = b["entangled_agents"].as_array() {
            merged
                .agents
                .extend(arr.iter().filter_map(|s| s.as_str().map(String::from)));
        }
        if let Some(arr) = b["entangled_cli"].as_array() {
            merged
                .cli
                .extend(arr.iter().filter_map(|s| s.as_str().map(String::from)));
        }
        if let Some(arr) = b["entangled_mcp"].as_array() {
            merged
                .mcps
                .extend(arr.iter().filter_map(|s| s.as_str().map(String::from)));
        }
        if let Some(arr) = b["skills"].as_array() {
            merged
                .skills
                .extend(arr.iter().filter_map(|s| s.as_str().map(String::from)));
        }
        if let Some(arr) = b["compliance"].as_array() {
            merged
                .compliance
                .extend(arr.iter().filter_map(|s| s.as_str().map(String::from)));
        }
    }
    Ok(merged)
}

fn list_ide_mcp(name: &str) -> Value {
    match name {
        "vscode" => {
            let exts = sh("code --list-extensions 2>/dev/null | grep -i mcp || true");
            let mcp = home().join(".config/Code/User/globalStorage/ms-vscode.mcp-server/mcp.json");
            json!({"ide":"vscode", "mcp_json": mcp.exists(), "extensions": exts.1.trim()})
        }
        "claudecode" => {
            let out = sh("claude mcp list 2>/dev/null | head -20 || echo 'not configured'");
            json!({"ide":"claudecode", "mcp_list": out.1.trim()})
        }
        "geminicli" => {
            let out = sh("geminicli mcp list 2>/dev/null | head -10 || echo 'not configured'");
            json!({"ide":"geminicli", "mcp_list": out.1.trim()})
        }
        "copilot" => {
            let mcp = home().join(".vscode/mcp.json");
            json!({"ide":"copilot", "mcp_json": mcp.exists(), "path": mcp.display().to_string()})
        }
        _ => json!({"ide": name, "error": "unknown IDE"}),
    }
}

fn install_role_mcps(composite: &RoleComposite, target: &str) -> Vec<String> {
    composite
        .mcps
        .iter()
        .map(|mcp| {
            let name = mcp.trim_end_matches(".mcp");
            let out = sh(&format!("b00t-cli mcp install {} {} 2>&1", name, target));
            format!("{} {}: {}", name, target, out.1.trim())
        })
        .collect()
}

fn generate_env_doc(b00t_path: &str) -> Value {
    // Docs generator — never auto-fix, only an explicit `doctor check --fix` may.
    let deps = all_deps(false);
    json!({
        "hostname": sh("hostname 2>/dev/null").1.trim(),
        "os": sh("cat /etc/os-release 2>/dev/null | grep PRETTY_NAME | cut -d= -f2 | tr -d '\"'").1.trim(),
        "memory": sh("free -h 2>/dev/null | grep Mem | awk '{print $2}'").1.trim(),
        "disk": sh("df -h / 2>/dev/null | tail -1 | awk '{print $3\"/\"$2\" (\"$5\")\"}'").1.trim(),
        "b00t_path": b00t_path,
        "home": home().display().to_string(),
        "whoami": sh("whoami 2>/dev/null").1.trim(),
        "deps": deps,
        "ide_mcp": vec![list_ide_mcp("vscode"), list_ide_mcp("claudecode"), list_ide_mcp("geminicli"), list_ide_mcp("copilot")],
        "epoch": sh("cat ~/.local/share/b00t/epoch-state.json 2>/dev/null | jq -c '{epoch,cycle,phase}' 2>/dev/null || echo none").1.trim(),
    })
}

// ─── Commands ─────────────────────────────────────────────────────────────────

#[derive(Parser, Clone)]
pub enum DoctorCommands {
    #[clap(about = "Verify b00t system dependencies")]
    Check {
        #[clap(long, help = "JSON output")]
        json: bool,
        #[clap(long, help = "Check a single dependency by id")]
        probe: Option<String>,
        #[clap(
            long,
            help = "Attempt safe auto-fixes (currently: sync drifted+clean submodules to their recorded pin; never touches drifted+dirty ones)"
        )]
        fix: bool,
    },
    #[clap(about = "Verify role deps + wire MCP into IDEs")]
    Setup {
        #[clap(long, help = "Roles (comma-separated, e.g. executive,operator)")]
        role: Option<String>,
        #[clap(
            long,
            help = "Target IDE: vscode, claudecode, geminicli, copilot (default: all)"
        )]
        target: Option<String>,
        #[clap(long, help = "JSON output")]
        json: bool,
        #[clap(long, help = "Skip MCP install, verify only")]
        dry_run: bool,
    },
    #[clap(about = "Environment documentation for the AI model")]
    Env {
        #[clap(long)]
        json: bool,
    },
    #[clap(about = "List MCP servers registered in IDEs")]
    Ide {
        #[clap(subcommand)]
        cmd: Option<IdeAction>,
    },
    #[clap(hide = true)]
    HealthJson,
    #[clap(about = "Repair gutted submodule gitdir(s) (#924) — safe: gutted state has no recoverable data")]
    FixSubmodule {
        #[clap(help = "Submodule path from .gitmodules; omit to repair all detected")]
        path: Option<String>,
        #[clap(long, help = "List gutted submodules without repairing")]
        dry_run: bool,
    },
}

#[derive(Parser, Clone)]
pub enum IdeAction {
    #[clap(about = "List all")]
    List,
    #[clap(about = "Show one")]
    Show { name: String },
}

// ─── Handler ──────────────────────────────────────────────────────────────────

pub fn handle_doctor_command(args: &DoctorCommands, b00t_path: &str) -> Result<()> {
    match args {
        DoctorCommands::Check { json, probe, fix } => {
            let results: Vec<Value> = all_deps(*fix)
                .into_iter()
                .filter(|d| {
                    probe.as_ref().map_or(true, |p| {
                        d["id"].as_str().map_or(false, |id| id.contains(p))
                    })
                })
                .collect();

            // Phase 1: well-formedness — count datums that pass prove_by_type()
            let store = HashMapStore::from_path(b00t_path).unwrap_or_default();
            let total_datums = store.len();
            let (provable, broken): (Vec<_>, Vec<_>) =
                store.iter().partition(|d| d.datum.prove_by_type().is_ok());
            // Phase 2: coherence — cross-datum reference validation
            let ref_errors = store.validate_references();
            let self_deps: Vec<_> = ref_errors
                .iter()
                .filter(|e| matches!(e, ReferenceError::SelfDependency { .. }))
                .collect();
            let empty_deps: Vec<_> = ref_errors
                .iter()
                .filter(|e| matches!(e, ReferenceError::EmptyDependency { .. }))
                .collect();

            if *json {
                let datum_report = json!({
                    "total": total_datums,
                    "provable": provable.len(),
                    "broken": broken.iter().map(|d| {
                        let err = d.datum.prove_by_type().unwrap_err();
                        json!({"key": d.key.as_ref(), "error": err.to_string()})
                    }).collect::<Vec<_>>(),
                    "reference_errors": ref_errors.iter().map(|e| e.to_string()).collect::<Vec<_>>(),
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "system_deps": results,
                        "datum_store": datum_report
                    }))?
                );
            } else {
                println!("🥾 b00t doctor — dependency check\n");
                for r in &results {
                    let ok = r["pass"].as_bool().unwrap_or(false);
                    let ms = r["latency_ms"].as_u64().unwrap_or(0);
                    println!(
                        "  {}  {}  {}ms  {}",
                        if ok { "✅" } else { "❌" },
                        r["id"].as_str().unwrap_or("?"),
                        ms,
                        r["detail"].as_str().unwrap_or("")
                    );
                    if r["id"] == "submodule-drift" {
                        if let Some(subs) = r["submodules"].as_array() {
                            for s in subs {
                                let status = s["status"].as_str().unwrap_or("");
                                if matches!(
                                    status,
                                    "broken" | "drifted_clean" | "drifted_dirty" | "drifted_fixed"
                                ) {
                                    println!(
                                        "      {}  {}  {}",
                                        status,
                                        s["path"].as_str().unwrap_or("?"),
                                        s["action"].as_str().unwrap_or("")
                                    );
                                }
                                // branch_status: stale can co-occur with an
                                // otherwise-clean "ok" status — surfaced
                                // separately since it's a different failure
                                // mode (see check-submodule-drift.sh).
                                if s["branch_status"] == "stale" {
                                    println!(
                                        "      ⚠️  stale-branch-pin  {}  {}",
                                        s["path"].as_str().unwrap_or("?"),
                                        s["branch_detail"].as_str().unwrap_or("")
                                    );
                                }
                            }
                        }
                    }
                    if r["id"] == "vendor-binaries" {
                        if let Some(vb) = r["vendor_binaries"].as_array() {
                            for b in vb {
                                if b["status"] == "missing" {
                                    println!(
                                        "      MISSING  {}  build: {}",
                                        b["binary_path"].as_str().unwrap_or("?"),
                                        b["build_command"].as_str().unwrap_or("?")
                                    );
                                }
                            }
                        }
                    }
                }
                let ok = results
                    .iter()
                    .filter(|r| r["pass"].as_bool().unwrap_or(false))
                    .count();
                println!("\n  {}/{} satisfied", ok, results.len());

                println!("\n🗄️  datum store — {b00t_path}");
                println!("  {}/{} datums provable", provable.len(), total_datums);
                for d in &broken {
                    let err = d.datum.prove_by_type().unwrap_err();
                    println!("  ❌ {}: {}", d.key.as_ref(), err);
                }
                if !self_deps.is_empty() || !empty_deps.is_empty() {
                    println!(
                        "  ⚠️  reference errors: {} self-deps, {} empty deps",
                        self_deps.len(),
                        empty_deps.len()
                    );
                    for e in self_deps.iter().chain(empty_deps.iter()) {
                        println!("     {e}");
                    }
                } else {
                    println!("  ✅ store coherence: no self-deps or empty deps");
                }
            }
            Ok(())
        }
        DoctorCommands::Setup {
            role,
            target,
            json,
            dry_run,
        } => {
            let roles: Vec<String> = role
                .as_deref()
                .unwrap_or("worker")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let composite = compose_roles(&roles, b00t_path)?;
            if *json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "roles": roles, "composite": {
                            "agents": composite.agents, "cli": composite.cli, "mcps": composite.mcps,
                            "skills": composite.skills, "compliance": composite.compliance
                        }, "dry_run": dry_run
                    }))?
                );
                return Ok(());
            }
            println!("🥾 b00t doctor setup — roles: {}", roles.join(", "));
            println!("  Skills: {}", composite.skills.join(", "));
            println!("  CLI tools: {}", composite.cli.join(", "));
            println!("  MCP servers: {}", composite.mcps.join(", "));
            println!("\n  🔧 CLI check:");
            for cli in &composite.cli {
                let name = cli.trim_end_matches(".cli");
                let (ok, d) = sh(&format!(
                    "which {} 2>/dev/null && {} --version 2>/dev/null | head -1 || echo MISSING",
                    name, name
                ));
                println!(
                    "    {} {}: {}",
                    if ok { "✅" } else { "❌" },
                    name,
                    d.trim()
                );
            }
            println!("\n  🔌 MCP datums:");
            for mcp in &composite.mcps {
                let name = mcp.trim_end_matches(".mcp");
                let p = PathBuf::from(b00t_path).join(format!("{}.mcp.toml", name));
                println!(
                    "    {} {}: {}",
                    if p.exists() { "✅" } else { "❌" },
                    name,
                    p.display()
                );
            }
            if !*dry_run {
                let idelist: Vec<&str> = if target.as_deref().unwrap_or("all") == "all" {
                    vec!["vscode", "claudecode", "geminicli", "copilot"]
                } else {
                    vec![target.as_deref().unwrap_or("all")]
                };
                for ide in &idelist {
                    println!("\n  📡 Installing into {}:", ide);
                    for r in &install_role_mcps(&composite, ide) {
                        println!("    {}", r);
                    }
                }
            }
            Ok(())
        }
        DoctorCommands::Env { json } => {
            let doc = generate_env_doc(b00t_path);
            if *json {
                println!("{}", serde_json::to_string_pretty(&doc)?);
            } else {
                println!("🥾 b00t doctor env — local environment\n");
                println!("  Host: {}", doc["hostname"].as_str().unwrap_or("?"));
                println!("  OS: {}", doc["os"].as_str().unwrap_or("?"));
                println!(
                    "  RAM: {} | Disk: {}",
                    doc["memory"].as_str().unwrap_or("?"),
                    doc["disk"].as_str().unwrap_or("?")
                );
                println!("  Epoch: {}", doc["epoch"].as_str().unwrap_or("?"));
                println!("\n  Deps:");
                for d in doc["deps"].as_array().unwrap_or(&vec![]) {
                    let ok = d["pass"].as_bool().unwrap_or(false);
                    println!(
                        "    {}  {}: {}",
                        if ok { "●" } else { "○" },
                        d["id"].as_str().unwrap_or("?"),
                        d["detail"].as_str().unwrap_or("")
                    );
                }
            }
            Ok(())
        }
        DoctorCommands::Ide { cmd } => {
            let ides = vec![
                list_ide_mcp("vscode"),
                list_ide_mcp("claudecode"),
                list_ide_mcp("geminicli"),
                list_ide_mcp("copilot"),
            ];
            match cmd.as_ref().unwrap_or(&IdeAction::List) {
                IdeAction::List => println!("{}", serde_json::to_string_pretty(&ides)?),
                IdeAction::Show { name } => {
                    println!("{}", serde_json::to_string_pretty(&list_ide_mcp(name))?)
                }
            }
            Ok(())
        }
        DoctorCommands::HealthJson => {
            // JSON health endpoint — never auto-fix, only an explicit
            // `doctor check --fix` may.
            let results = all_deps(false);
            let ok = results
                .iter()
                .filter(|r| r["pass"].as_bool().unwrap_or(false))
                .count();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                    "total": results.len(), "passed": ok, "failed": results.len() - ok, "probes": results
                }))?
            );
            Ok(())
        }
        DoctorCommands::FixSubmodule { path, dry_run } => {
            let root = home().join(".b00t");
            let targets = match path {
                Some(p) => vec![p.clone()],
                None => find_gutted_submodules(&root),
            };
            if targets.is_empty() {
                println!("no gutted submodules found");
                return Ok(());
            }
            for t in &targets {
                if *dry_run {
                    println!("would repair: {t}");
                } else {
                    match repair_gutted_submodule(&root, t) {
                        Ok(msg) => println!("✅ {msg}"),
                        Err(e) => println!("❌ {t}: {e}"),
                    }
                }
            }
            Ok(())
        }
    }
}
pub fn health_json() -> serde_json::Value {
    // Never auto-fix, only an explicit `doctor check --fix` may.
    let results = all_deps(false);
    let ok = results
        .iter()
        .filter(|r| r["pass"].as_bool().unwrap_or(false))
        .count();
    serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "total": results.len(), "passed": ok, "failed": results.len() - ok, "probes": results
    })
}

/// Check 1: b00t-cli binary exists and reports version
fn check_b00t_cli() -> Value {
    let exists = Command::new("which")
        .arg("b00t-cli")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let version = if exists {
        Command::new("b00t-cli")
            .arg("--version")
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout)
                        .or_else(|_| String::from_utf8(o.stderr))
                        .ok()
                } else {
                    None
                }
            })
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    } else {
        String::new()
    };

    json!({
        "check": "b00t-cli binary",
        "status": if exists { "ok" } else { "fail" },
        "detail": if exists { format!("v{}", version) } else { "not found in PATH".to_string() }
    })
}

/// Check 2: _b00t_ directory exists with datums
fn check_b00t_dir(b00t_path: &str, fix: bool) -> Value {
    let expanded = shellexpand::tilde(b00t_path).to_string();
    let path = PathBuf::from(&expanded);

    let exists = path.exists();
    if !exists && fix {
        let _ = std::fs::create_dir_all(&path);
    }

    // Count datum files (.toml)
    let datum_count = if path.exists() {
        match std::fs::read_dir(&path) {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext == "toml" || ext == "tomllm" || ext == "tomllmd")
                        .unwrap_or(false)
                })
                .count(),
            Err(_) => 0,
        }
    } else {
        0
    };

    let exists_now = path.exists();
    json!({
        "check": "_b00t_/ directory",
        "status": if exists_now && datum_count > 0 { "ok" } else if exists_now { "warn" } else { "fail" },
        "detail": format!("{} datums at {}", datum_count, path.display())
    })
}

/// Check 3: .opencode/ directory exists with skills
fn check_opencode_dir(fix: bool) -> Value {
    let path = PathBuf::from(".opencode");
    let exists = path.exists();
    if !exists && fix {
        let _ = std::fs::create_dir_all(path.join("skills"));
        let _ = std::fs::create_dir_all(path.join("context"));
    }

    let skills_count = if path.join("skills").exists() {
        match std::fs::read_dir(path.join("skills")) {
            Ok(entries) => entries.filter_map(|e| e.ok()).count(),
            Err(_) => 0,
        }
    } else {
        0
    };

    let exists_now = path.exists();
    json!({
        "check": ".opencode/ directory",
        "status": if exists_now { "ok" } else { "fail" },
        "detail": format!("{} skills", skills_count)
    })
}

/// Check 4: Focus schema datum exists
fn check_focus_schema(b00t_path: &str) -> Value {
    let expanded = shellexpand::tilde(b00t_path).to_string();
    let path = PathBuf::from(&expanded).join("focus.schema.tomllmd");

    let exists = path.exists();
    json!({
        "check": "focus schema datum",
        "status": if exists { "ok" } else { "fail" },
        "detail": if exists {
            format!("found at {}", path.display())
        } else {
            format!("not found at {}", path.display())
        }
    })
}

/// Check 5: ledgrrr-mcp / ledgerr-mcp service status
fn check_ledgrrr_service() -> Value {
    let output = Command::new("systemctl")
        .args(["--user", "is-active", "ledgrrr-mcp"])
        .output();

    match output {
        Ok(o) => {
            let status = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let active = status == "active" || o.status.success();
            json!({
                "check": "ledgrrr-mcp service", // alt: ledgerr-mcp
                "status": if active { "ok" } else { "fail" },
                "detail": if active { "active".to_string() } else { status }
            })
        }
        Err(e) => {
            json!({
                "check": "ledgrrr-mcp service", // alt: ledgerr-mcp
                "status": "fail",
                "detail": format!("systemctl not available: {}", e)
            })
        }
    }
}

/// Check 6: Local model endpoint reachable
fn check_model_endpoint() -> Value {
    // Use curl (preferred) to avoid tokio runtime panic (#[tokio::main]); reqwest fallback commented below
    let reachable = std::process::Command::new("curl")
        .args([
            "-s",
            "--max-time",
            "3",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "http://localhost:8001/v1/models",
        ])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "200")
        // alt: reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(5)).build()...
        .unwrap_or(false);

    json!({
        "check": "model endpoint (localhost:8001)",
        "status": if reachable { "ok" } else { "fail" },
        "detail": if reachable {
            "reachable".to_string()
        } else {
            "not reachable (is vllm/ch0nky running?)".to_string()
        }
    })
}

/// Check 7: .b00t/fsl/ directory exists for FSL examples
fn check_fsl_dir(fix: bool) -> Value {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
    let path = home.join(".b00t").join("fsl");
    let expanded = shellexpand::tilde(&path.to_string_lossy()).to_string();
    let path = PathBuf::from(&expanded);

    let exists = path.exists();
    if !exists && fix {
        let _ = std::fs::create_dir_all(&path);
    }

    let exists_now = path.exists();
    json!({
        "check": ".b00t/fsl/ directory",
        "status": if exists_now { "ok" } else { "fail" },
        "detail": if exists_now {
            format!("exists at {}", path.display())
        } else {
            "not found".to_string()
        }
    })
}

#[cfg(test)]
mod gutted_submodule_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_gutted_fixture() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join(".gitmodules"),
            "[submodule \"x\"]\n\tpath = sub/x\n\turl = https://example.com/x.git\n",
        )
        .unwrap();
        let gitdir = dir.path().join(".git/modules/sub/x");
        fs::create_dir_all(&gitdir).unwrap();
        fs::write(gitdir.join("config"), "[core]\n\tbare = true\n").unwrap();
        dir
    }

    #[test]
    fn detects_gutted_gitdir() {
        let dir = make_gutted_fixture();
        assert_eq!(
            find_gutted_submodules(dir.path()),
            vec!["sub/x".to_string()]
        );
    }

    #[test]
    fn does_not_flag_healthy_gitdir() {
        let dir = make_gutted_fixture();
        fs::write(
            dir.path().join(".git/modules/sub/x/HEAD"),
            "ref: refs/heads/main\n",
        )
        .unwrap();
        assert!(find_gutted_submodules(dir.path()).is_empty());
    }

    #[test]
    fn repair_refuses_non_gutted_path() {
        let dir = make_gutted_fixture();
        fs::write(
            dir.path().join(".git/modules/sub/x/HEAD"),
            "ref: refs/heads/main\n",
        )
        .unwrap();
        assert!(repair_gutted_submodule(dir.path(), "sub/x").is_err());
    }
}

#[cfg(test)]
mod vendor_binary_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Mirrors the real `_b00t_/datums/VENDOR-*.tomllmd` shape: a
    /// `[b00t.vendor]` table followed by a markdown prose body that is NOT
    /// valid TOML on its own — regression coverage for the reason
    /// `parse_vendor_table` scans just the one table instead of doing a
    /// whole-file `toml::from_str`.
    fn write_vendor_datum(repo_root: &std::path::Path, key: &str, binary_rel: &str, build_cmd: &str) {
        let content = format!(
            "# b00t Vendor Datum — {key} (fixture)\n\n\
             [b00t]\n\
             name = \"VENDOR-{key}\"\n\
             type = \"vendor\"\n\
             hint = \"test fixture\"\n\n\
             [b00t.vendor]\n\
             path = \"vendor/{key}\"\n\
             upstream = \"https://example.com/{key}.git\"\n\
             branch = \"main\"\n\
             build_command = \"{build_cmd}\"\n\
             install_command = \"cp {binary_rel} ~/.local/bin/\"\n\
             health_check = \"{key} --version || test -x {binary_rel}\"\n\
             required_tools = [\"cargo\"]\n\n\
             ## What is it?\n\n\
             A prose body line that is not valid TOML on its own — mirrors\n\
             the real VENDOR-*.tomllmd files.\n"
        );
        let datums_dir = repo_root.join("_b00t_/datums");
        fs::create_dir_all(&datums_dir).unwrap();
        fs::write(datums_dir.join(format!("VENDOR-{key}.tomllmd")), content).unwrap();
    }

    #[test]
    fn detects_missing_vendor_binary() {
        let dir = TempDir::new().unwrap();
        write_vendor_datum(
            dir.path(),
            "FOO",
            "vendor/foo/target/release/foo",
            "cargo build --release --manifest-path vendor/foo/Cargo.toml",
        );

        let result = check_vendor_binaries(dir.path(), false);

        assert_eq!(result["pass"], json!(false));
        let vb = result["vendor_binaries"].as_array().unwrap();
        assert_eq!(vb.len(), 1);
        assert_eq!(vb[0]["status"], json!("missing"));
        assert_eq!(vb[0]["binary_path"], json!("vendor/foo/target/release/foo"));
        assert_eq!(
            vb[0]["build_command"],
            json!("cargo build --release --manifest-path vendor/foo/Cargo.toml")
        );
    }

    #[test]
    fn passes_when_binary_built_and_executable() {
        let dir = TempDir::new().unwrap();
        write_vendor_datum(dir.path(), "FOO", "vendor/foo/target/release/foo", "cargo build --release");
        let bin_path = dir.path().join("vendor/foo/target/release/foo");
        fs::create_dir_all(bin_path.parent().unwrap()).unwrap();
        fs::write(&bin_path, "#!/bin/sh\necho hi\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let result = check_vendor_binaries(dir.path(), false);

        assert_eq!(result["pass"], json!(true));
    }

    #[test]
    fn present_but_non_executable_file_still_fails() {
        let dir = TempDir::new().unwrap();
        write_vendor_datum(dir.path(), "FOO", "vendor/foo/target/release/foo", "cargo build --release");
        let bin_path = dir.path().join("vendor/foo/target/release/foo");
        fs::create_dir_all(bin_path.parent().unwrap()).unwrap();
        fs::write(&bin_path, "not a binary").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o644)).unwrap();
        }

        let result = check_vendor_binaries(dir.path(), false);

        assert_eq!(result["pass"], json!(false));
    }

    #[test]
    fn skips_registry_entries_without_test_dash_x_health_check() {
        // e.g. real VENDOR-AGENT-FRAMEWORK.tomllmd's python health_check —
        // no `test -x <path>` means there is no binary to check for, and it
        // must not be flagged "missing" (#814 scoping requirement: only
        // vendor crates that actually produce a binary b00t expects).
        let dir = TempDir::new().unwrap();
        let datums_dir = dir.path().join("_b00t_/datums");
        fs::create_dir_all(&datums_dir).unwrap();
        fs::write(
            datums_dir.join("VENDOR-PY.tomllmd"),
            "[b00t]\nname = \"VENDOR-PY\"\ntype = \"vendor\"\nhint = \"test\"\n\n\
             [b00t.vendor]\npath = \"vendor/py\"\nbuild_command = \"uv pip install -e vendor/py\"\n\
             health_check = \"python -c 'import py' 2>/dev/null || echo 'not installed'\"\n",
        )
        .unwrap();

        let result = check_vendor_binaries(dir.path(), false);

        assert_eq!(result["pass"], json!(true));
        assert_eq!(result["vendor_binaries"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn dedupes_two_registry_entries_pointing_at_the_same_binary() {
        // Mirrors real VENDOR-LEDGRRR.tomllmd / VENDOR-L3DG3RR.tomllmd, both
        // of which point at vendor/ledgrrr/target/release/ledgerr-mcp.
        let dir = TempDir::new().unwrap();
        write_vendor_datum(dir.path(), "A", "vendor/shared/target/release/shared", "cargo build --release");
        write_vendor_datum(dir.path(), "B", "vendor/shared/target/release/shared", "cargo build --release");

        let result = check_vendor_binaries(dir.path(), false);

        assert_eq!(result["vendor_binaries"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn no_vendor_datums_directory_yields_empty_pass() {
        let dir = TempDir::new().unwrap();
        let result = check_vendor_binaries(dir.path(), false);
        assert_eq!(result["pass"], json!(true));
        assert_eq!(result["vendor_binaries"].as_array().unwrap().len(), 0);
    }
}

#[cfg(test)]
mod wsl_warp_mtu_tests {
    use super::*;

    // is_wsl()/powershell() are real host I/O and not mocked here — these
    // cover the pure decision logic the check is built on: parsing a
    // PowerShell one-liner's stdout, and the actual "is this a blackhole"
    // comparison (#confirmed 2026-08-09 against a live WSL2 + WARP host).

    #[test]
    fn parses_single_line_numeric_mtu() {
        assert_eq!(parse_mtu("1300\n"), Some(1300));
        assert_eq!(parse_mtu("1500"), Some(1500));
    }

    #[test]
    fn parse_mtu_rejects_empty_or_non_numeric() {
        assert_eq!(parse_mtu(""), None);
        assert_eq!(parse_mtu("\n\n"), None);
        assert_eq!(parse_mtu("not-a-number"), None);
    }

    #[test]
    fn flags_risk_when_local_mtu_exceeds_tunnel_mtu() {
        // The diagnosed shape: eth0 stuck at 1500, WARP tunnel at 1300.
        assert!(mtu_at_risk(1500, 1300));
    }

    #[test]
    fn no_risk_when_local_mtu_already_at_or_below_tunnel_mtu() {
        assert!(!mtu_at_risk(1280, 1300));
        assert!(!mtu_at_risk(1300, 1300));
    }
}
