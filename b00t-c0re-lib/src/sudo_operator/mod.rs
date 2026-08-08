// b00t-c0re-lib/src/sudo_operator/mod.rs
// 🤓 Sudo-grant governance — adversarial-model review gate for privileged
//    commands. Sibling of the `reviewer` module; see PRD-SUDO-OPERATOR-GOVERNANCE.
//    Reuses reviewer::evidence's SHA-256 content-addressed mechanism rather
//    than a parallel evidence system.

pub mod checkpoint;
pub mod governance;
pub mod verdict;
pub mod vetted;

pub use checkpoint::{checkpoint_system_state, CheckpointRef};
pub use governance::{SudoDisposition, SudoGrantConstraint, SudoGrantEvidence, SudoReviewEvent};
pub use verdict::adversarial_review;
pub use vetted::{check_vetted, load_vetted_registry, VettedResult, VettedScriptEntry};
