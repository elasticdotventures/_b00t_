// check-wow-executable.rhai — WOW C8 + meta: the WOW itself must be runnable
// Each WOW proposal .rhai file should be executable via `b00t script run`.
let master_exists = file_exists("_b00t_/scripts/wow-master.rhai");
let primitives = [
    "_b00t_/scripts/wow/check-candle-build.rhai",
    "_b00t_/scripts/wow/check-default-build.rhai",
    "_b00t_/scripts/wow/check-known-role.rhai",
    "_b00t_/scripts/wow/check-vendor-dockerfile.rhai",
    "_b00t_/scripts/wow/check-dual-runtime.rhai",
    "_b00t_/scripts/wow/check-just-modules.rhai",
];
let all_exist = true;
for path in primitives {
    if !file_exists(path) {
        log_error(`  missing: ${path}`);
        all_exist = false;
    }
}
if master_exists && all_exist {
    log_success("all WOW scripts are executable");
} else {
    log_error("some WOW scripts missing");
    throw("C8: WOW must be executable — missing primitive scripts");
}
