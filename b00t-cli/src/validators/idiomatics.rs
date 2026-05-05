use crate::traits::{
    ConstraintEvaluator, DatumChecker, DatumProvider, DatumValidator, FilterLogic, StatusProvider,
    VersionStatus,
};
use crate::BootDatum;

pub struct IdiomaticsValidator {
    datum: BootDatum,
}

impl IdiomaticsValidator {
    pub fn new(datum: &BootDatum) -> Self {
        Self {
            datum: datum.clone(),
        }
    }
}

impl DatumChecker for IdiomaticsValidator {
    fn is_installed(&self) -> bool {
        // Idiomatic patterns are always "installed" — they are structural declarations
        true
    }

    fn current_version(&self) -> Option<String> {
        None
    }

    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }

    fn version_status(&self) -> VersionStatus {
        VersionStatus::Unknown
    }
}

impl StatusProvider for IdiomaticsValidator {
    fn name(&self) -> &str {
        &self.datum.name
    }

    fn subsystem(&self) -> &str {
        "idiomatics"
    }

    fn hint(&self) -> &str {
        &self.datum.hint
    }

    fn is_disabled(&self) -> bool {
        false
    }
}

impl FilterLogic for IdiomaticsValidator {
    fn is_available(&self) -> bool {
        true
    }

    fn prerequisites_satisfied(&self) -> bool {
        true
    }

    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
}

impl ConstraintEvaluator for IdiomaticsValidator {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for IdiomaticsValidator {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumValidator for IdiomaticsValidator {
    fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();
        // Placeholder — full structural validation comes in Phase 9
        if self.datum.name.is_empty() {
            errors.push("Idiomatic datum has no name".to_string());
        }
        errors
    }

    fn handler_name(&self) -> &'static str {
        "idiomatics"
    }
}
