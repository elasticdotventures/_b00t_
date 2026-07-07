// b00t-self-upgrade.rhai — zero-shot self-upgrade detection + execution
//
// Runs before the ralph loop. Checks if workspace has a newer b00t version
// than the installed binary. Auto-upgrades if needed.
//
// Returns: "upgraded" | "current" | "error:<msg>"
//
// Used by: b00t up (default flow, no --self flag)

fn shell_quote(s) {
    let out = s;
    out = out.replace("'", "'\\''");
    "'" + out + "'"
}

let result = "current";
let workspace_root = WORKSPACE_ROOT;

if workspace_root == "" {
    log_warn("b00t-self-upgrade: no workspace root — skipping");
    return result;
}

// 1. Detect installed version vs workspace version
let cargo_toml = workspace_root + "/b00t-cli/Cargo.toml";

if !file_exists(cargo_toml) {
    log_warn("b00t-self-upgrade: no b00t-cli/Cargo.toml found at " + workspace_root);
    return result;
}

// Read workspace version from Cargo.toml
let version_line = run_cmd("grep '^version' " + shell_quote(cargo_toml) + " | head -1");
let workspace_version = version_line.trim();

if workspace_version == "" {
    log_warn("b00t-self-upgrade: could not parse workspace version");
    return result;
}

// Strip prefix
let parts = workspace_version.split("=");
if parts.len() < 2 {
    log_warn("b00t-self-upgrade: unexpected version format: " + workspace_version);
    return result;
}
let raw_ver = parts[1].trim();
let ws_ver = raw_ver.trim_matches('"').trim_matches('\'');

// 2. Get installed version
let installed_out = run_cmd("b00t --version 2>/dev/null | head -1");
let installed_parts = installed_out.split("\n");
let installed_line = installed_parts[0].trim();
// Format: "b00t-cli 0.9.1" — extract the version part
let ver_parts = installed_line.split(" ");
let installed_ver = if ver_parts.len() >= 2 { ver_parts[ver_parts.len()-1].trim() } else { "" };

log_info("b00t version: installed=" + installed_ver + " workspace=" + ws_ver);

if installed_ver == ws_ver || installed_ver == "" || ws_ver == "" {
    log_success("b00t is current (v" + installed_ver + ")");
    return result;
}

// 3. Upgrade needed — run workspace build
log_warn("b00t version mismatch: installed v" + installed_ver + " < workspace v" + ws_ver);
log_info("Running self-upgrade via workspace build...");

let upgrade_out = run_cmd("cargo install --path " + shell_quote(workspace_root + "/b00t-cli") + " --force 2>&1");

log_info("upgrade output: " + upgrade_out);

// 4. Verify
let verify_out = run_cmd("b00t --version 2>/dev/null | head -1");
let verify_parts = verify_out.split("\n");
let verify_line = verify_parts[0].trim();
let new_parts = verify_line.split(" ");
let new_ver = if new_parts.len() >= 2 { new_parts[new_parts.len()-1].trim() } else { "" };

if new_ver == ws_ver {
    log_success("b00t upgraded to v" + new_ver);
    result = "upgraded";
} else {
    log_error("b00t upgrade may have failed: got v" + new_ver + " expected v" + ws_ver);
    result = "error:version_mismatch";
}

result
