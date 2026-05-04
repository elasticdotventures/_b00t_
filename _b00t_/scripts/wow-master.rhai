// WOW master check — Benders decomposition
// Calls primitive check scripts, composes results into a structured report.
// Usage: b00t script run _b00t_/scripts/wow-master.rhai

log_info("🥾 b00t WOW integrity check (Benders decomposition)");

let results = [];
let passed = 0;
let total = 0;

// Each check inlined — RHAI closures not available in this engine version.

total += 1;
log_info("  running: candle feature compiles");
try {
    run_cmd("b00t-cli script run _b00t_/scripts/wow/check-candle-build.rhai 2>&1");
    passed += 1;
    log_success("  PASS: candle feature compiles");
    results.push("  PASS: candle feature compiles");
} catch (e) {
    log_error(`  FAIL: candle feature compiles — ${e}`);
    results.push(`  FAIL: candle feature compiles — ${e}`);
}

total += 1;
log_info("  running: default build compiles");
try {
    run_cmd("b00t-cli script run _b00t_/scripts/wow/check-default-build.rhai 2>&1");
    passed += 1;
    log_success("  PASS: default build compiles");
    results.push("  PASS: default build compiles");
} catch (e) {
    log_error(`  FAIL: default build compiles — ${e}`);
    results.push(`  FAIL: default build compiles — ${e}`);
}

total += 1;
log_info("  running: KnownRole is exhaustive");
try {
    run_cmd("b00t-cli script run _b00t_/scripts/wow/check-known-role.rhai 2>&1");
    passed += 1;
    log_success("  PASS: KnownRole is exhaustive");
    results.push("  PASS: KnownRole is exhaustive");
} catch (e) {
    log_error(`  FAIL: KnownRole — ${e}`);
    results.push(`  FAIL: KnownRole — ${e}`);
}

total += 1;
log_info("  running: vendor dockerfile exists");
try {
    run_cmd("b00t-cli script run _b00t_/scripts/wow/check-vendor-dockerfile.rhai 2>&1");
    passed += 1;
    log_success("  PASS: vendor dockerfile exists");
    results.push("  PASS: vendor dockerfile exists");
} catch (e) {
    log_error(`  FAIL: vendor dockerfile — ${e}`);
    results.push(`  FAIL: vendor dockerfile — ${e}`);
}

total += 1;
log_info("  running: WOW is executable");
try {
    run_cmd("b00t-cli script run _b00t_/scripts/wow/check-wow-executable.rhai 2>&1");
    passed += 1;
    log_success("  PASS: WOW is executable");
    results.push("  PASS: WOW is executable");
} catch (e) {
    log_error(`  FAIL: WOW executable — ${e}`);
    results.push(`  FAIL: WOW executable — ${e}`);
}

total += 1;
log_info("  running: dual runtime datum");
try {
    run_cmd("b00t-cli script run _b00t_/scripts/wow/check-dual-runtime.rhai 2>&1");
    passed += 1;
    log_success("  PASS: dual runtime datum");
    results.push("  PASS: dual runtime datum");
} catch (e) {
    log_error(`  FAIL: dual runtime — ${e}`);
    results.push(`  FAIL: dual runtime — ${e}`);
}

total += 1;
log_info("  running: justfile modules exist");
try {
    run_cmd("b00t-cli script run _b00t_/scripts/wow/check-just-modules.rhai 2>&1");
    passed += 1;
    log_success("  PASS: justfile modules exist");
    results.push("  PASS: justfile modules exist");
} catch (e) {
    log_error(`  FAIL: justfile modules — ${e}`);
    results.push(`  FAIL: justfile modules — ${e}`);
}

// ─── Spline report ──────────────────────────────────────────────────────────
log_info("");
log_info("═══════════════════════════════════════");
log_info(`WOW integrity spline: ${passed}/${total} passed`);
log_info("═══════════════════════════════════════");
for r in results { log_info(r); }
log_info("═══════════════════════════════════════");
if passed == total {
    log_success("All checks passed");
} else {
    log_warn(`${total - passed} check(s) failed — fix before proceeding`);
}
