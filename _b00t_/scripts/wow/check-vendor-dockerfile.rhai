// check-vendor-dockerfile.rhai — WOW rule C9: vendor IS upstream
// Verifies the Dockerfile lives in the vendored source, not in dotfiles root.
if file_exists("vendor/l3dg3rr/Dockerfile.ledgerr-mcp") {
    log_success("vendor Dockerfile exists at vendor/l3dg3rr/Dockerfile.ledgerr-mcp");
} else {
    log_error("Dockerfile not found in vendor tree");
    throw("C9: Dockerfile must live in vendor/l3dg3rr/, not in dotfiles root");
}
