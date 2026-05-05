# Dynamic Steering Gates — Rhai Proof of Concept
# These functions use b00t's datum-based tool detection (not brittle path checks).
# tool_installed("podman") calls b00t-cli detect podman which uses the podman CLI datum.
# This works on Windows, macOS, and Linux — the datum system handles OS abstraction.

# Container runtime steering — datum-based, OS-agnostic
fn best_container_cmd(cmd) {
    let podman_status = tool_installed("podman");
    if podman_status != "" {
        log_info("🦨 docker → podman (preferred, datum-confirmed)")
        cmd.replace("docker", "podman")
    } else {
        let docker_status = tool_installed("docker");
        if docker_status != "" {
            log_warn("⚠️ podman not available — using docker (datum-confirmed fallback)")
            cmd
        } else {
            throw("No container runtime detected by b00t datum system. Install: b00t install podman")
        }
    }
}

# Package manager steering — datum-based, cross-platform
fn best_install_cmd(pkg) {
    if tool_installed("apt") != "" {
        `sudo apt install -y ${pkg}`
    } else if tool_installed("brew") != "" {
        `brew install ${pkg}`
    } else if tool_installed("dnf") != "" {
        `sudo dnf install -y ${pkg}`
    } else {
        throw(`No package manager detected by b00t datum for ${pkg}`)
    }
}

# --- Idiomatic Guard Functions (8 concepts from Plan) ---
# Pipe-compatible function-form alternatives for the expression macros above.
# These enable `|>` pipe chaining in guard patterns.

# GUARD 4: rename-all-references
fn is_rename_op(cmd) {
    cmd.contains("git mv") || cmd.contains("git rename")
}

# GUARD 1: agent-format-errors
fn is_long_err_string(cmd) {
    cmd.contains("bail!") || cmd.contains("anyhow!") || cmd.contains("eprintln!") || (cmd.contains("error") && cmd.len() > 120)
}

# GUARD 2: help-from-impl
fn is_clap_help_drift(cmd) {
    cmd.contains("clap") && (cmd.contains("about") || cmd.contains("long_about"))
}

# GUARD 3: boolean-over-enum
fn has_enum_flag(cmd) {
    cmd.contains("--type") || cmd.contains("--severity") || cmd.contains("--mode") || cmd.contains("--kind")
}

# GUARD 5: asymmetric-naming
fn is_singular_subcommand(cmd) {
    cmd.contains("enum") && (cmd.contains("Command") || cmd.contains("Subcommand"))
}
