// b00t-self-upgrade.rhai — zero-shot self-upgrade detection + execution

let result = "current";
let workspace_root = WORKSPACE_ROOT;

if workspace_root == "" {
    log_warn("b00t-self-upgrade: no workspace root");
    return result;
}

// Read workspace version from root Cargo.toml
let grep_cmd = "grep '^version' " + workspace_root + "/Cargo.toml | head -1";
let version_line = run_cmd(grep_cmd);
let ws_raw = str_strip(version_line);

if ws_raw == "" {
    log_warn("b00t-self-upgrade: could not parse workspace version");
    return result;
}

// Parse: "version = \"0.9.4\"" -> "0.9.4"
let eq_idx = ws_raw.index_of("=");
if eq_idx < 0 {
    log_warn("b00t-self-upgrade: unexpected version format: " + ws_raw);
    return result;
}
let after_eq = ws_raw.sub_string(eq_idx + 1);
let raw_ver = str_strip(after_eq);
let ws_ver = raw_ver.replace("\"", "");

// Get installed version
let installed_out = run_cmd("b00t --version 2>/dev/null | head -1");
let line_brk = installed_out.index_of("\n");
let installed_line = if line_brk >= 0 { installed_out.sub_string(0, line_brk) } else { installed_out };
let sp_idx = installed_line.index_of(" ");
let installed_ver = if sp_idx >= 0 { installed_line.sub_string(sp_idx + 1) } else { installed_line };

log_info("b00t version: installed=" + installed_ver + " workspace=" + ws_ver);

if installed_ver == ws_ver || installed_ver == "" || ws_ver == "" {
    log_success("b00t is current (v" + installed_ver + ")");
    return result;
}

// Upgrade needed
log_warn("b00t version mismatch: installed v" + installed_ver + " < workspace v" + ws_ver);
log_info("Running self-upgrade via workspace build...");
let upgrade_cmd = "cargo install --path " + workspace_root + "/b00t-cli --force 2>&1";
let upgrade_out = run_cmd(upgrade_cmd);
log_info("upgrade output: " + upgrade_out);

// Verify
let vout = run_cmd("b00t --version 2>/dev/null | head -1");
let vbrk = vout.index_of("\n");
let vline = if vbrk >= 0 { vout.sub_string(0, vbrk) } else { vout };
let vsp = vline.index_of(" ");
let new_ver = if vsp >= 0 { vline.sub_string(vsp + 1) } else { vline };

if new_ver == ws_ver {
    log_success("b00t upgraded to v" + new_ver);
    result = "upgraded";
} else {
    log_error("b00t upgrade may have failed: got v" + new_ver + " expected v" + ws_ver);
    result = "error:version_mismatch";
}

result
