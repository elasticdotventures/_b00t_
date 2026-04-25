// LangChain operator helper for safe b00t-native scripted actions.

fn env_bool(name, default_val) {
    let v = get_env(name);
    if v == null { return default_val; }
    if v == "true" || v == "1" || v == "yes" || v == "y" || v == "TRUE" || v == "YES" || v == "Y" { return true; }
    if v == "false" || v == "0" || v == "no" || v == "n" || v == "FALSE" || v == "NO" || v == "N" { return false; }
    return default_val;
}

let action = get_env("OPERATOR_ACTION");
let target = get_env("OPERATOR_TARGET");
let dry_run = env_bool("OPERATOR_DRY_RUN", true);
let force = env_bool("OPERATOR_FORCE", false);
let note = get_env("OPERATOR_NOTE");

if action == null || action == "" {
    throw("OPERATOR_ACTION is required");
}

if note != null && note != "" {
    log_info("operator-agent note=" + note);
}

let cmd = if action == "stack.activate" {
    if target == null || target == "" { throw("stack.activate requires OPERATOR_TARGET"); }
    "b00t stack activate " + target + if dry_run { " --dry-run" } else { "" } + if force { " --force" } else { "" }
} else if action == "stack.deactivate" {
    if target == null || target == "" { throw("stack.deactivate requires OPERATOR_TARGET"); }
    "b00t stack deactivate " + target + if dry_run { " --dry-run" } else { "" }
} else if action == "hive.status" {
    "b00t hive status"
} else if action == "script.list" {
    "b00t script list"
} else {
    throw("Unsupported operator action: " + action);
};

log_info("operator-agent action=" + action + " cmd=" + cmd);
run_cmd(cmd)
