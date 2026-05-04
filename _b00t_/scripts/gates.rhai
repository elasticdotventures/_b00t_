// gates.rhai — reusable gate evaluation module for datum hooks
//
// Provides: strip(), unquote(), is_installed(), derive_gates(), evaluate_gates()
// Can be imported into any hook script via b00t's hook engine.
//
// Usage as hook_detect:
//   hook_detect = "gates"  (runner prepends this module, then evaluates datum gates)
//
// Auto-derived gates:
//   requires = ["docker","node"] → gate_check("command", "docker")
//   [b00t.env] KEY = "VAL"       → gate_check("env", "KEY")
//   [[b00t.mcp.stdio]] env       → gate_check("env", key) for each env entry
//   Explicit [[b00t.gate]]       → command/file/env/rhai from datum metadata

// ── String utilities ─────────────────────────────────────────────────────────

fn strip(s) {
    if s == "" { return s; }
    let n = s.len();
    let start = 0;
    while start < n { let c = s.sub_string(start, 1); if c != " " && c != "\t" { break; } start = start + 1; }
    let end = n;
    while end > start { let c = s.sub_string(end-1, 1); if c != " " && c != "\t" { break; } end = end - 1; }
    if start >= end { "" } else { s.sub_string(start, end - start) }
}

fn unquote(s) {
    let t = strip(s);
    if t.starts_with("\"") && t.ends_with("\"") { t.sub_string(1, t.len()-2) } else { t }
}

// ── Install state check ──────────────────────────────────────────────────────

fn is_installed(name) {
    let cfg = get_env("HOME") + "/.claude/settings.json";
    if file_exists(cfg) && read_file(cfg).contains(`"${name}"`) { return true; }
    // also check b00t's own MCP registry
    let b00t_path = get_env("_B00T_Path");
    if b00t_path == "" { b00t_path = "~/.b00t/_b00t_"; }
    let datum = b00t_path + "/" + name + ".mcp.toml";
    file_exists(datum)  // true if datum exists (install state is for the hook to decide)
}

// ── Gate derivation from datum TOML content ──────────────────────────────────

fn derive_gates(content) {
    let gates = [];
    let lines = content.split("\n");
    let in_env_block = false;
    let in_stdio_env = false;
    let in_explicit_gate = false;
    let mut ckind = "";
    let mut cspec = "";
    let mut chint = "";

    for line in lines {
        let t = strip(line);

        if t == "[b00t.env]" { in_env_block = true; continue; }
        if t == "[b00t.mcp.stdio.env]" { in_stdio_env = true; continue; }
        if t.starts_with("[[b00t.gate]]") {
            in_explicit_gate = true; ckind = ""; cspec = ""; chint = "";
            continue;
        }
        if t.starts_with("[") && (in_env_block || in_stdio_env || in_explicit_gate) {
            if in_explicit_gate && ckind != "" && cspec != "" {
                gates = gates + #{ kind: ckind, spec: cspec, hint: chint };
            }
            in_env_block = false; in_stdio_env = false; in_explicit_gate = false;
        }

        // requires → command gates
        if t.starts_with("requires = ") {
            let raw = strip(t.sub_string(11));
            if raw != "" {
                let inner = raw.sub_string(1, raw.len()-2);
                for dep in inner.split(",") {
                    let d = strip(dep);
                    if d == "" { continue; }
                    let d2 = if d.starts_with("\"") { d.sub_string(1, d.len()-2) } else { d };
                    if d2 != "" && d2 != "internet" {
                        let exists = false;
                        for g in gates { if g.kind == "command" && g.spec == d2 { exists = true; } }
                        if !exists { gates = gates + #{ kind: "command", spec: d2, hint: "" }; }
                    }
                }
            }
        }

        // [b00t.env] KEY = "VAL" → env gates (skip non-credential defaults)
        if in_env_block && t.contains("=") && !t.starts_with("#") {
            let ek = strip(t.split("=")[0]);
            if ek != "" && !ek.starts_with("LOG_") && !ek.starts_with("FAST") {
                let exists = false;
                for g in gates { if g.kind == "env" && g.spec == ek { exists = true; } }
                if !exists { gates = gates + #{ kind: "env", spec: ek, hint: "" }; }
            }
        }

        // [[b00t.mcp.stdio]] env field → env gates
        if (t.starts_with("env = ") || t.starts_with("env={")) && !in_env_block {
            let raw = if t.starts_with("env = ") { t.sub_string(6) } else { t.sub_string(5) };
            let inner = strip(raw).sub_string(1);
            for pair in inner.split(",") {
                let p = strip(pair);
                if p.contains("=") {
                    let ek = strip(p.split("=")[0]);
                    if ek != "" && !ek.starts_with("LOG_") && !ek.starts_with("FAST") {
                        let exists = false;
                        for g in gates { if g.kind == "env" && g.spec == ek { exists = true; } }
                        if !exists { gates = gates + #{ kind: "env", spec: ek, hint: "" }; }
                    }
                }
            }
        }

        // explicit [[b00t.gate]] fields
        if in_explicit_gate {
            if t.starts_with("command = ") { ckind = "command"; cspec = unquote(t.sub_string(10)); }
            if t.starts_with("file = ") { ckind = "file"; cspec = unquote(t.sub_string(7)); }
            if t.starts_with("env = ") { ckind = "env"; cspec = unquote(t.sub_string(5)); }
            if t.starts_with("rhai = ") { ckind = "rhai"; cspec = unquote(t.sub_string(7)); }
            if t.starts_with("hint = ") { chint = unquote(t.sub_string(7)); }
        }
    }

    if in_explicit_gate && ckind != "" && cspec != "" {
        gates = gates + #{ kind: ckind, spec: cspec, hint: chint };
    }

    gates
}

// ── Gate evaluation ──────────────────────────────────────────────────────────

fn evaluate_gates(name, datum_content) {
    let gates = derive_gates(datum_content);
    let fail = "";
    for gate in gates {
        let ok = false;
        try { ok = gate_check(gate.kind, gate.spec); } catch(e) { ok = false; }
        if !ok {
            let hint = gate.hint;
            fail = if hint != "" { hint } else { gate.kind + ": " + gate.spec };
            break;
        }
    }
    if fail != "" {
        log_info("⏭️  " + name + " blocked: " + fail);
        try { session_track("gate_block", name + ": " + fail); } catch(e) {}
        false
    } else {
        true
    }
}
