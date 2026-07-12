// 🤓 Capability Webring — control loop verification via browser fetch()
//
// Each check: name, type, URL or JS expression
// All HTTP checks use browser-side fetch() — no curl needed.
// Testing is a side effect of the loop running.
//
// Usage: b00t-rpa script scripts/capability-webring.rhai

const LOG = "/tmp/capability-webring.jsonl";

fn pad(s, n) {
    let r = s;
    while r.len() < n { r += " "; }
    return r;
}

fn check(name, type, url_or_js) {
    let name_pad = pad(name, 32);
    print("  🔄 " + name_pad + " ...");

    let t0 = evaluate("performance.now()");
    let result = "";
    let ok = false;

    if type == "http" {
        // HTTP fetch via browser
        let js = `(async () => { try { let r = await fetch('${url_or_js}'); return JSON.stringify({ok: r.ok, status: r.status}); } catch(e) { return JSON.stringify({ok: false, error: e.message}); } })()`;
        result = evaluate(js);
        ok = result.contains("ok") && result.contains("true");
    } else if type == "js" {
        // JS expression evaluation
        result = evaluate(url_or_js);
        ok = result == "true" || result.contains("pong");
    }

    let elapsed = evaluate("performance.now()") - t0;
    let status = if ok { "✅" } else { "❌" };

    print("  " + status + "  " + name_pad + pad(elapsed.to_string(), 8) + " ms  " + result.sub_string(0, 60));
    return ok;
}

// ─── Main ───────────────────────────────────────────────────────────────────

print("╔═══════════════════════════════════════════════════════╗");
print("║  b00t Capability Webring — Control Loop              ║");
print("╚═══════════════════════════════════════════════════════╝");
print("");

// Load dashboard context
print("🌐 Loading dashboard...");
navigate("http://localhost:31337/");
wait_for("body");
print("");

let passed = 0;
let failed = 0;

// ─── Core API nodes ─────────────────────────────────────────────────────────
if check("b00t-admin Health", "http", "http://localhost:31337/api/admin/health") { passed += 1; } else { failed += 1; }
if check("Pipeline API", "http", "http://localhost:31337/api/admin/processes") { passed += 1; } else { failed += 1; }
if check("Viz entangle API", "http", "http://localhost:31337/api/admin/viz/entangle") { passed += 1; } else { failed += 1; }
if check("Viz task API", "http", "http://localhost:31337/api/admin/viz/task") { passed += 1; } else { failed += 1; }

// ─── Viz capability nodes (tested via browser JS) ───────────────────────────
if check("Mermaid.js loaded", "js", "typeof mermaid !== 'undefined'") { passed += 1; } else { failed += 1; }
if check("renderMermaid() defined", "js", "typeof renderMermaid === 'function'") { passed += 1; } else { failed += 1; }
if check("Cytoscape.js loaded", "js", "typeof cytoscape !== 'undefined'") { passed += 1; } else { failed += 1; }
if check("Ping/pong round-trip", "js", "JSON.stringify({pong:true,ts:Date.now()})") { passed += 1; } else { failed += 1; }

// ─── CDP bridge nodes (via browser fetch to Windows host) ───────────────────
if check("CDP Chrome version", "http", "http://172.30.64.1:9223/json/version") { passed += 1; } else { failed += 1; }
if check("CDP tab list", "http", "http://172.30.64.1:9223/json") { passed += 1; } else { failed += 1; }

// ─── Summary ────────────────────────────────────────────────────────────────
print("");
print("╔═══════════════════════════════════════════════════════╗");
print("║  Webring Summary                                      ║");
print("╠═══════════════════════════════════════════════════════╣");
print("║  " + pad("Passed: " + passed, 52) + "║");
print("║  " + pad("Failed: " + failed, 52) + "║");
print("║  " + pad("Total:  " + (passed + failed), 52) + "║");
print("╚═══════════════════════════════════════════════════════╝");
print("");
print("Done.");
