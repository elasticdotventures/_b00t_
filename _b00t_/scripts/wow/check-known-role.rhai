// check-known-role.rhai — WOW rule B7: sealed sum types are exhaustive
// Verifies the KnownRole enum exists and all variant names are recognized.
// RHAI twin of b00t-cli/src/wow.rs::KnownRoleCheck — keep the two in sync.
let src = read_file("b00t-c0re-role/src/lib.rs");
let has_known_role = src.contains("pub enum KnownRole");
let has_worker = src.contains("Worker(RoleRef<Worker>)");
let has_executive = src.contains("Executive(RoleRef<Executive>)");
let has_operator = src.contains("Operator(RoleRef<Operator>)");
let has_specialist = src.contains("Specialist(RoleRef<Specialist>)");

if has_known_role && has_worker && has_executive && has_operator && has_specialist {
    log_success("KnownRole enum is exhaustive (4 variants)");
} else {
    log_error("KnownRole enum is incomplete or missing");
    throw("B7: KnownRole must have Worker, Executive, Operator, Specialist variants");
}
