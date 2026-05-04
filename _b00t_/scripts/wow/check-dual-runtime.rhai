// check-dual-runtime.rhai — WOW rule D10: dual runtime datum
let datum = read_file("_b00t_/ledgrrr.cli.toml");
if datum.contains("[b00t.providers.runtime]") && datum.contains("detect_order") {
    log_success("dual runtime datum exists with detect_order");
} else {
    log_error("ledgrrr.cli.toml missing [b00t.providers.runtime] section");
    throw("D10: runtime detection must be declared in datum, not hardcoded");
}
