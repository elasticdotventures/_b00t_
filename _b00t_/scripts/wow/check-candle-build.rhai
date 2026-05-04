// check-candle-build.rhai — WOW rule A2: feature gates must compile
log_info("  checking: cargo check --features candle");
try {
    run_cmd("cargo check --features candle 2>&1");
    log_success("candle feature compiles");
} catch (e) {
    log_error("candle feature build FAILED");
    throw("A2: candle feature gate broken — run `cargo build --features candle`");
}
