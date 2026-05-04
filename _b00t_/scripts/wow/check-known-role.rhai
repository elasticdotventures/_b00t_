// check-known-role.rhai — WOW rule B7: sealed sum types are exhaustive
// Verifies the KnownRole enum exists and all variant names are recognized.
let src = read_file("b00t-cli/src/agentic_role.rs");
let has_known_role = src.contains("pub enum KnownRole");
let has_worker = src.contains("Worker(RoleRef<Worker>)");
let has_executive = src.contains("Executive(RoleRef<Executive>)");
let has_operator = src.contains("Operator(RoleRef<Operator>)");
let has_provider = src.contains("AppProvider(RoleRef<AppProvider>)");

if has_known_role && has_worker && has_executive && has_operator && has_provider {
    log_success("KnownRole enum is exhaustive (4 variants)");
} else {
    log_error("KnownRole enum is incomplete or missing");
    throw("B7: KnownRole must have Worker, Executive, Operator, AppProvider variants");
}
