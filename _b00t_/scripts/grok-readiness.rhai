// grok-readiness.rhai — b00t primitive guard/repair for grok RAG backends.
//
// Idempotent checks:
// - resolve the b00t venv decision tree
// - align HOME/.venv for current RagLightConfig compatibility
// - ensure raglite + llama-cpp-python imports work for GGUF embeddings
// - ensure ~/.local/share/raglite exists

fn shell_quote(s) {
    let out = s;
    out = out.replace("'", "'\\''");
    "'" + out + "'"
}

fn first_existing(paths) {
    for p in paths {
        if p != "" && file_exists(p) {
            return p;
        }
    }
    ""
}

fn ensure_cmd(name, hint) {
    if !command_exists(name) {
        log_error(name + " not found. " + hint);
        throw name + " missing";
    }
}

fn run_ok(cmd) {
    let probe = "if " + cmd + " >/dev/null 2>&1; then echo OK; else echo FAIL; fi";
    let out = run_cmd(probe);
    if out.contains("OK") {
        true
    } else {
        log_warn("check failed");
        false
    }
}

fn ensure_python_import(py, module_name, package_name) {
    let check = shell_quote(py) + " -c " + shell_quote("import " + module_name);
    if run_ok(check) {
        log_success(module_name + " import ok");
        return true;
    }

    ensure_cmd("uv", "Install uv via b00t install uv.");
    log_warn(module_name + " missing; installing " + package_name + " into " + py);
    run_cmd("uv pip install --python " + shell_quote(py) + " " + package_name);

    if !run_ok(check) {
        log_error(module_name + " still does not import after install");
        throw module_name + " import failed";
    }
    log_success(module_name + " installed");
    true
}

fn ensure_home_venv(home_venv, b00t_venv) {
    if file_exists(home_venv + "/bin/python") {
        return home_venv;
    }
    if file_exists(b00t_venv + "/bin/python") {
        log_warn("HOME/.venv missing; linking to b00t workspace venv");
        run_cmd("ln -s " + shell_quote(b00t_venv) + " " + shell_quote(home_venv));
        return home_venv;
    }

    ensure_cmd("uv", "Install uv via b00t install uv.");
    log_warn("No grok venv found; creating " + home_venv);
    run_cmd("uv venv " + shell_quote(home_venv));
    return home_venv;
}

let home = get_env("HOME");
let workspace = if WORKSPACE_ROOT != "" { WORKSPACE_ROOT } else { home + "/.b00t" };
let configured = get_env("B00T_GROK_VENV");
let home_venv = home + "/.venv";
let workspace_venv = workspace + "/.venv";

let selected = if configured != "" {
    configured
} else {
    first_existing([home_venv, workspace_venv])
};

if selected == "" {
    selected = ensure_home_venv(home_venv, workspace_venv);
} else if selected != home_venv {
    ensure_home_venv(home_venv, selected);
}

let py = selected + "/bin/python";
if !file_exists(py) {
    log_error("Selected grok venv has no python: " + selected);
    throw "grok python missing";
}

let raglite_dir = home + "/.local/share/raglite";
create_dir(raglite_dir);

log_info("grok python: " + py);
ensure_python_import(py, "raglite", "raglite");

// Current RagLightConfig uses llama-cpp GGUF embeddings by default.
// If a node overrides B00T_GROK_EMBEDDER away from llama-cpp, this gate can skip it.
let embedder = get_env("B00T_GROK_EMBEDDER");
if embedder == "" || embedder.contains("llama-cpp") || embedder.contains("gguf") {
    ensure_python_import(py, "llama_cpp", "llama-cpp-python");
}

session_track("grok_readiness", "ok:" + py);
log_success("grok readiness ok");
