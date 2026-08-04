pub mod continuation;
pub mod epoch3;
pub mod errors;
pub mod gates;
pub mod ring;
pub mod scheduler;
pub mod scope_store;
pub mod scoring;
pub mod store;
pub mod traits;
pub mod types;

// Phase 3: Zellij gate integration
pub mod eisenhower;
pub mod gate_audit;
pub mod zellij_gate;

pub use eisenhower::EisenhowerRouter;
pub use gate_audit::{AuditLog, GateAudit};
pub use zellij_gate::ZellijGate;
