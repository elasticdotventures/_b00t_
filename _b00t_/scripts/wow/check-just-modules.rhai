// check-just-modules.rhai — WOW rule E12: modules over flat recipes
// Verifies the justfile uses mod declarations for b00t/ledgrrr/irontology.
let justfile = read_file("justfile");
if !justfile.contains("mod b00t") || !justfile.contains("mod ledgrrr") || !justfile.contains("mod irontology") {
    log_error("justfile missing required module declarations");
    throw("E12: justfile must declare mod b00t, mod ledgrrr, mod irontology");
}
log_success("justfile has all required modules (b00t, ledgrrr, irontology)");
