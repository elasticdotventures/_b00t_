// check-default-build.rhai — WOW rule A1: default build compiles
log_info("  checking: cargo check (default features)");
try {
    run_cmd("cargo check 2>&1");
    log_success("default build compiles");
} catch (e) {
    log_error("default build FAILED");
    throw("A1: default build broken — run `cargo check` and fix errors");
}
