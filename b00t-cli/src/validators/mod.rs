//! Datum validation dispatch — pluggable per-type validators.
//! Each validator implements DatumValidator trait from traits.rs.
pub mod idiomatics;
use crate::traits::DatumValidator;
use crate::BootDatum;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;

type ValidatorFactory = fn(&BootDatum) -> Box<dyn DatumValidator>;

/// Registry of named validators — populated at init time.
static REGISTRY: Lazy<Mutex<HashMap<&'static str, ValidatorFactory>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Register a validator by handler name.
pub fn register(name: &'static str, factory: ValidatorFactory) {
    if let Ok(mut reg) = REGISTRY.lock() {
        reg.insert(name, factory);
    }
}

/// Validate a datum by dispatching to its registered handler.
/// Falls back to shell command + regex validation.
pub fn validate_datum(datum: &BootDatum) -> Vec<String> {
    let mut errors = Vec::new();

    // If datum has a named handler, dispatch
    if let Some(ref spec) = datum.validate {
        if let Some(ref handler) = spec.handler {
            if let Ok(reg) = REGISTRY.lock() {
                if let Some(factory) = reg.get(handler.as_str()) {
                    let validator = factory(datum);
                    let mut errs = validator.validate();
                    errors.append(&mut errs);
                } else {
                    errors.push(format!(
                        "No validator registered for handler '{}'",
                        handler
                    ));
                }
            }
            return errors;
        }
        // Shell command validation
        if let Some(ref cmd) = spec.command {
            match std::process::Command::new("sh").arg("-c").arg(cmd).output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if let Some(ref regex_str) = spec.regex {
                        if let Ok(re) = regex::Regex::new(regex_str) {
                            if !re.is_match(&stdout) {
                                errors.push(format!(
                                    "Shell command output did not match regex: {}",
                                    regex_str
                                ));
                            }
                        }
                    }
                    if !output.status.success() {
                        errors.push(format!(
                            "Shell command failed (exit {}): {}",
                            output.status.code().unwrap_or(-1),
                            cmd
                        ));
                    }
                }
                Err(e) => errors.push(format!("Shell command execution failed: {}", e)),
            }
        }
    }

    errors
}

/// Auto-register all known validators.
pub fn init() {
    // Register known validators here as they're created
    register("idiomatics", |d| Box::new(crate::validators::idiomatics::IdiomaticsValidator::new(d)));
}
