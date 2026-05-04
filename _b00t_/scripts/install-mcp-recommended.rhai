// install-mcp-recommended.rhai
// Map/filter/reduce pipeline: discovers MCP datums, auto-derives gates from
// existing config (requires, env, command), evaluates them, and batch-installs.
//
// Auto-derived gates:
//   requires = ["docker","node"] → gate_check("command", "docker")
//   [b00t.env] KEY = "VAL"       → gate_check("env", "KEY")
//   [[b00t.mcp.stdio]] env       → gate_check("env", key) for each env entry
//   Explicit [[b00t.gate]]       → additional conditions (cred files, rhai, etc.)
//
// Usage:  b00t-cli install --mcp=recommended

let target = "claudecode";
let b00t_path = get_env("_B00T_Path");
if b00t_path == "" { b00t_path = "~/.b00t/_b00t_"; }

log_info("🔍 Discovering MCP datums in: " + b00t_path);

// 1. MAP: discover datum files
let files = run_cmd(`ls ${b00t_path}/*.mcp.toml 2>/dev/null`).split("\n").filter(|f| f != "");

fn strip(s) {
    if s == "" { return s; }
    let n = s.len();
    let start = 0;
    while start < n { let c = s.sub_string(start, 1); if c != " " && c != "\t" { break; } start = start + 1; }
    let end = n;
    while end > start { let c = s.sub_string(end-1, 1); if c != " " && c != "\t" { break; } end = end - 1; }
    if start >= end { "" } else { s.sub_string(start, end - start) }
}

// strip surrounding quotes if present
fn unquote(s) {
    let t = strip(s);
    if t.starts_with("\"") && t.ends_with("\"") { t.sub_string(1, t.len()-2) } else { t }
}

fn is_installed(name) {
    let cfg = get_env("HOME") + "/.claude/settings.json";
    !file_exists(cfg) || read_file(cfg).contains(`"${name}"`)
}

// Auto-derive gates from datum content.
// Returns array of { kind, spec, hint } derived from requires, env blocks, and explicit gates.
fn derive_gates(content) {
    let gates = [];
    let lines = content.split("\n");
    let in_env_block = false;
    let in_stdio_env = false;
    let in_explicit_gate = false;
    let current_gate_kind = "";
    let current_gate_spec = "";
    let current_gate_hint = "";

    for line in lines {
        let t = strip(line);

        // track sections
        if t == "[b00t.env]" { in_env_block = true; continue; }
        if t == "[b00t.mcp.stdio.env]" { in_stdio_env = true; continue; }
        if t.starts_with("[[b00t.gate]]") { in_explicit_gate = true; current_gate_kind = ""; current_gate_spec = ""; current_gate_hint = ""; continue; }
        if t.starts_with("[") && (in_env_block || in_stdio_env || in_explicit_gate) {
            // flush explicit gate before leaving section
            if in_explicit_gate && current_gate_kind != "" && current_gate_spec != "" {
                gates = gates + #{ kind: current_gate_kind, spec: current_gate_spec, hint: current_gate_hint };
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
                        // check if not already added
                        let exists = false;
                        for g in gates { if g.kind == "command" && g.spec == d2 { exists = true; } }
                        if !exists { gates = gates + #{ kind: "command", spec: d2, hint: "" }; }
                    }
                }
            }
        }

        // [b00t.env] KEY = "VAL" → env gates (skip common non-credential defaults)
        if in_env_block && t.contains("=") && !t.starts_with("#") {
            let ek = strip(t.split("=")[0]);
            if ek != "" && !ek.starts_with("LOG_") && !ek.starts_with("FAST") {
                let exists = false;
                for g in gates { if g.kind == "env" && g.spec == ek { exists = true; } }
                if !exists { gates = gates + #{ kind: "env", spec: ek, hint: "" }; }
            }
        }

        // [[b00t.mcp.stdio]] env field → env gates
        // these appear as `env = { KEY = "VAL", ... }` on same line
        if t.starts_with("env = ") || t.starts_with("env={") {
            let raw = if t.starts_with("env = ") { t.sub_string(6) } else { t.sub_string(5) };
            let inner = strip(raw).sub_string(1); // skip {
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
            if t.starts_with("command = ") { current_gate_kind = "command"; current_gate_spec = unquote(t.sub_string(10)); }
            if t.starts_with("file = ") { current_gate_kind = "file"; current_gate_spec = unquote(t.sub_string(7)); }
            if t.starts_with("env = ") { current_gate_kind = "env"; current_gate_spec = unquote(t.sub_string(5)); }
            if t.starts_with("rhai = ") { current_gate_kind = "rhai"; current_gate_spec = unquote(t.sub_string(7)); }
            if t.starts_with("hint = ") { current_gate_hint = unquote(t.sub_string(7)); }
        }
    }

    // flush last explicit gate
    if in_explicit_gate && current_gate_kind != "" && current_gate_spec != "" {
        gates = gates + #{ kind: current_gate_kind, spec: current_gate_spec, hint: current_gate_hint };
    }

    gates
}

// 2. FILTER: auto-derive + evaluate gates
let recommended = [];
for file in files {
    let content = read_file(file);

    // extract name
    let lines = content.split("\n");
    let name = "";
    for line in lines {
        if line.starts_with("name = ") {
            let raw = strip(line.sub_string(7));
            name = if raw.starts_with("\"") { raw.sub_string(1, raw.len()-2) } else { raw };
            break;
        }
    }
    if name == "" { continue; }

    // skip if already installed
    if is_installed(name) { continue; }

    // derive + evaluate gates
    let gates = derive_gates(content);
    let gate_fail = "";
    for gate in gates {
        let ok = false;
        try { ok = gate_check(gate.kind, gate.spec); } catch(e) { ok = false; }
        if !ok {
            let hint = gate.hint;
            gate_fail = if hint != "" { hint } else { gate.kind + ": " + gate.spec };
            break;
        }
    }

    if gate_fail != "" {
        log_info("  ⏭️  " + name + " (gate: " + gate_fail + ")");
        try { session_track("gate_block", name + ": " + gate_fail); } catch(e) {}
        continue;
    }

    // try knowledge graph for runtime state (non-fatal)
    try { let kg = kg_query("subject", `b00t:datum/${name}`); if kg != "{}" { log_info("  📊  " + name + " state: " + kg); } } catch(e) {}

    log_info("  ✅ " + name + " is recommended");
    recommended = recommended + name;
}

// 3. SUMMARY TABLE: group by gate kind, show counts
let pass_count = recommended.len();
let by_kind = #{};
for file in files {
    let content = read_file(file);
    let lines = content.split("\n");
    let n = "";
    for line in lines { if line.starts_with("name = ") { let raw = strip(line.sub_string(7)); n = if raw.starts_with("\"") { raw.sub_string(1, raw.len()-2) } else { raw }; break; } }
    if n == "" || is_installed(n) { continue; }
    let in_rec = false;
    for r in recommended { if r == n { in_rec = true; break; } }
    if !in_rec {
        // find first failing gate kind
        let gkind = "unknown";
        let gates = derive_gates(content);
        for gate in gates {
            let ok = false;
            try { ok = gate_check(gate.kind, gate.spec); } catch(e) {}
            if !ok { gkind = gate.kind; break; }
        }
        let prev = by_kind[gkind];
        if prev == () { by_kind[gkind] = 1; } else { by_kind[gkind] = prev + 1; }
    }
}
log_info("");
log_info("📋  Gate results:  " + pass_count + "  pass");
let kind_order = ["command", "env", "file", "rhai", "unknown"];
for k in kind_order {
    let v = by_kind[k];
    if v != () { log_info("     " + k + ": " + v + "  blocked"); }
}
log_info("");

// 4. MAP-REDUCE: batch install
let installed = [];
for name in recommended {
    log_info("🚀  Installing: " + name);
    let ok = true;
    try {
        let out = run_cmd(`b00t-cli mcp install "${name}" ${target} 2>&1`);
        if out != "" { log_info(out); }
        try { session_track("mcp_install", name + ":ok"); } catch(e) {}
    } catch(e) {
        log_warn("⚠️  Failed: " + e);
        try { session_track("mcp_install", name + ":fail:" + e); } catch(e) {}
        ok = false;
    }
    let status = if ok { "installed" } else { "failed" };
    try { kg_query("facts", `b00t:hasStatus ${status} b00t:datum/${name}`); } catch(e) {}
    installed = installed + name;
}

// 5. REPORT
log_success("✅  Installed " + installed.len() + " / " + pass_count + " recommended MCP servers");
return installed.len()
