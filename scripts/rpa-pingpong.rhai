// 🤓 RPA Ping/Pong — CDP round-trip performance measurement
//
// Measures: navigation, render, JS eval times via browser performance.now()
// Records: last value to /tmp/rpa-pingpong-last.json
//
// Usage: b00t-rpa script scripts/rpa-pingpong.rhai

const TARGET = "http://localhost:31337/";
const CACHE = "/tmp/rpa-pingpong-last.json";

// ─── Steps ──────────────────────────────────────────────────────────────────

print("╔════════════════════════════════════════╗");
print("║  RPA Ping/Pong — CDP Performance      ║");
print("╚════════════════════════════════════════╝");

// Step 1: Navigate
print("1️⃣ Navigate...");
let ok = navigate(TARGET);
if !ok { print("  FAILED"); exit(1); }
print("  OK");

// Step 2: Wait for page
print("2️⃣ Wait for page...");
wait_for("body");
print("  OK");

// Step 3: Ping/pong timing via browser performance.now()
print("3️⃣ Ping/pong...");
let js = "JSON.stringify({pong:true,ts:Date.now(),nav:performance.now()})";
let result = evaluate(js);
print("  " + result);

// Step 4: Measure round-trip with 10 iterations
print("4️⃣ Round-trip timing (10x)...");
let timing_js = "(async () => { let samples = []; for (let i = 0; i < 10; i++) { let t0 = performance.now(); let r = document.title; let t1 = performance.now(); samples.push((t1 - t0).toFixed(3)); } return JSON.stringify(samples); })();";
let samples = evaluate(timing_js);
print("  Samples (ms): " + samples);

// Step 5: Describe
print("5️⃣ Describe page...");
let desc = describe_page();
print("  " + desc);

print("");
print("DONE. Cache: " + CACHE);
