//! Chalk-style structural proofs for all 22 b00t DatumType variants.
//!
//! **Phase 1 — Well-formedness** (this module): is the datum structurally valid?
//! **Phase 2 — Coherence** (DatumStore::validate_references): do foreign keys resolve?
//!
//! # Design
//! Each `DatumType` variant gets a typed newtype (`As*Datum<'_>`) that implements
//! `Provable`. Shared field-check predicates are free functions so prove impls
//! stay ~10 lines each. The `typecheck` + `hint_required` helpers extract the
//! boilerplate that every prove impl shares.
//!
//! # Categories (MECE)
//! - A: Executable (8 types) — Docker, Bash, Apt, Nix, Vscode, K8s, Justfile, Job
//! - B: Compositional (3 types) — Stack, Agent, HiveProfile
//! - C: Connection/identity (4 types) — Database, Api, Repo, Mcp
//! - D: Lightweight identity (3 types) — Ai, AiModel, Config
//! - E: Sentinel (1 type) — Unknown
//! - Existing: Cli, Skill, Role (from original impl, refactored to use helpers)

use crate::{BootDatum, DatumType};
use std::fmt;

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum DatumProofError {
    WrongType { expected: &'static str, actual: String },
    MissingInstall { datum: String },
    MissingLearnContent { datum: String },
    MissingDependsOn { datum: String },
    MissingCommand { datum: String },
    MissingHint { datum: String },
    /// Generic structural gap: at least one of `expected` fields must be present.
    MissingStructuralField { datum: String, expected: &'static str },
}

impl fmt::Display for DatumProofError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongType { expected, actual } =>
                write!(f, "type mismatch: expected {expected}, got {actual}"),
            Self::MissingInstall { datum } =>
                write!(f, "Cli datum '{datum}' requires install or version field"),
            Self::MissingLearnContent { datum } =>
                write!(f, "Skill datum '{datum}' requires [b00t.learn] content or keywords"),
            Self::MissingDependsOn { datum } =>
                write!(f, "Role datum '{datum}' requires at least one depends_on entry"),
            Self::MissingCommand { datum } =>
                write!(f, "Mcp datum '{datum}' requires command field"),
            Self::MissingHint { datum } =>
                write!(f, "datum '{datum}' requires non-empty hint"),
            Self::MissingStructuralField { datum, expected } =>
                write!(f, "datum '{datum}' requires at least one of: {expected}"),
        }
    }
}

// ── Provable trait ────────────────────────────────────────────────────────────

pub trait Provable {
    fn prove(&self) -> Result<(), DatumProofError>;
}

// ── Shared predicates ─────────────────────────────────────────────────────────

// These are free functions so each prove impl stays ~10 lines.

fn has_hint(d: &BootDatum) -> bool { !d.hint.is_empty() }
fn has_install(d: &BootDatum) -> bool { d.install.is_some() }
fn has_version(d: &BootDatum) -> bool { d.version.is_some() }
fn has_update(d: &BootDatum) -> bool { d.update.is_some() }
fn has_image(d: &BootDatum) -> bool { d.image.as_ref().map(|s| !s.is_empty()).unwrap_or(false) }
fn has_oci_uri(d: &BootDatum) -> bool { d.oci_uri.as_ref().map(|s| !s.is_empty()).unwrap_or(false) }
fn has_resource_path(d: &BootDatum) -> bool { d.resource_path.as_ref().map(|s| !s.is_empty()).unwrap_or(false) }
fn has_script(d: &BootDatum) -> bool { d.script.as_ref().map(|s| !s.is_empty()).unwrap_or(false) }
fn has_package_name(d: &BootDatum) -> bool { d.package_name.as_ref().map(|s| !s.is_empty()).unwrap_or(false) }
fn has_vsix_id(d: &BootDatum) -> bool { d.vsix_id.as_ref().map(|s| !s.is_empty()).unwrap_or(false) }
fn has_chart_path(d: &BootDatum) -> bool { d.chart_path.as_ref().map(|s| !s.is_empty()).unwrap_or(false) }
fn has_values_file(d: &BootDatum) -> bool { d.values_file.as_ref().map(|s| !s.is_empty()).unwrap_or(false) }
fn has_justfile_path(d: &BootDatum) -> bool {
    d.justfile.as_ref().and_then(|j| j.path.as_ref()).map(|p| !p.is_empty()).unwrap_or(false)
}
fn has_job(d: &BootDatum) -> bool { d.job.as_ref().map(|v| !v.is_null()).unwrap_or(false) }
fn has_stack(d: &BootDatum) -> bool { d.stack.as_ref().map(|v| !v.is_null()).unwrap_or(false) }
fn has_members(d: &BootDatum) -> bool { d.members.as_ref().map(|v| !v.is_empty()).unwrap_or(false) }
fn has_skills(d: &BootDatum) -> bool { d.skills.as_ref().map(|v| !v.is_empty()).unwrap_or(false) }
fn has_depends_on(d: &BootDatum) -> bool { d.depends_on.as_ref().map(|v| !v.is_empty()).unwrap_or(false) }
fn has_channel_prefix(d: &BootDatum) -> bool { d.channel_prefix.as_ref().map(|s| !s.is_empty()).unwrap_or(false) }
fn has_dsn(d: &BootDatum) -> bool { d.dsn.as_ref().map(|s| !s.is_empty()).unwrap_or(false) }
fn has_protocol(d: &BootDatum) -> bool { d.protocol.as_ref().map(|s| !s.is_empty()).unwrap_or(false) }
fn has_provides(d: &BootDatum) -> bool {
    d.provides.as_ref().and_then(|p| p.capability.as_ref()).map(|s| !s.is_empty()).unwrap_or(false)
}
fn has_url(d: &BootDatum) -> bool { d.url.as_ref().map(|s| !s.is_empty()).unwrap_or(false) }
fn has_clone_path(d: &BootDatum) -> bool { d.clone_path.as_ref().map(|s| !s.is_empty()).unwrap_or(false) }
fn has_keywords(d: &BootDatum) -> bool { d.keywords.as_ref().map(|k| !k.is_empty()).unwrap_or(false) }
fn has_learn_inline(d: &BootDatum) -> bool {
    d.learn.as_ref().and_then(|l| l.inline.as_ref()).map(|c| !c.is_empty()).unwrap_or(false)
}
fn has_command(d: &BootDatum) -> bool { d.command.is_some() }

// ── Shared boilerplate helpers ────────────────────────────────────────────────

fn typecheck(dt: &Option<DatumType>, expected: DatumType, name: &'static str)
    -> Result<(), DatumProofError>
{
    if let Some(actual) = dt {
        if actual != &expected {
            return Err(DatumProofError::WrongType {
                expected: name,
                actual: format!("{actual:?}").to_lowercase(),
            });
        }
    }
    Ok(())
}

fn hint_required(d: &BootDatum) -> Result<(), DatumProofError> {
    if !has_hint(d) { Err(DatumProofError::MissingHint { datum: d.name.clone() }) } else { Ok(()) }
}

fn require_any(d: &BootDatum, checks: &[fn(&BootDatum) -> bool], expected: &'static str)
    -> Result<(), DatumProofError>
{
    if checks.iter().any(|f| f(d)) {
        Ok(())
    } else {
        Err(DatumProofError::MissingStructuralField { datum: d.name.clone(), expected })
    }
}

// ── Category D: Lightweight identity (hint sufficient) ───────────────────────

macro_rules! hint_only_datum {
    ($newtype:ident, $variant:ident, $name:literal) => {
        pub struct $newtype<'a>(pub &'a BootDatum);
        impl<'a> Provable for $newtype<'a> {
            fn prove(&self) -> Result<(), DatumProofError> {
                typecheck(&self.0.datum_type, DatumType::$variant, $name)?;
                hint_required(self.0)
            }
        }
    };
}

hint_only_datum!(AsAiDatum,        Ai,          "ai");
hint_only_datum!(AsAiModelDatum,   AiModel,     "ai_model");
hint_only_datum!(AsConfigDatum,    Config,      "config");
hint_only_datum!(AsHiveProfileDatum, HiveProfile, "hive_profile");

// ── Category E: Sentinel ──────────────────────────────────────────────────────

pub struct AsUnknownDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsUnknownDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> { Ok(()) }
}

// ── Category A: Executable ────────────────────────────────────────────────────

pub struct AsCliDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsCliDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Cli, "cli")?;
        hint_required(d)?;
        require_any(d, &[has_install, has_version, has_update], "install, version, or update")
    }
}

pub struct AsDockerDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsDockerDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Docker, "docker")?;
        hint_required(d)?;
        require_any(d, &[has_image, has_oci_uri, has_install, has_resource_path],
            "image, oci_uri, install, or resource_path")
    }
}

pub struct AsBashDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsBashDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Bash, "bash")?;
        hint_required(d)?;
        require_any(d, &[has_script, has_install], "script or install")
    }
}

pub struct AsAptDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsAptDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Apt, "apt")?;
        hint_required(d)?;
        require_any(d, &[has_install, has_package_name], "install or package_name")
    }
}

pub struct AsNixDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsNixDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Nix, "nix")?;
        hint_required(d)?;
        require_any(d, &[has_install, has_package_name], "install or package_name")
    }
}

pub struct AsVscodeDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsVscodeDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Vscode, "vscode")?;
        hint_required(d)?;
        require_any(d, &[has_vsix_id, has_install], "vsix_id or install")
    }
}

pub struct AsK8sDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsK8sDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::K8s, "k8s")?;
        hint_required(d)?;
        require_any(d, &[has_chart_path, has_values_file, has_install],
            "chart_path, values_file, or install")
    }
}

pub struct AsJustfileDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsJustfileDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Justfile, "justfile")?;
        hint_required(d)?;
        require_any(d, &[has_justfile_path, has_install], "justfile.path or install")
    }
}

pub struct AsJobDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsJobDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Job, "job")?;
        hint_required(d)?;
        require_any(d, &[has_job, has_script], "job metadata or script")
    }
}

// ── Category B: Compositional ─────────────────────────────────────────────────

pub struct AsSkillDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsSkillDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Skill, "skill")?;
        hint_required(d)?;
        require_any(d, &[has_learn_inline, has_keywords], "learn.inline or keywords")
    }
}

pub struct AsRoleDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsRoleDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Role, "role")?;
        hint_required(d)?;
        if !has_depends_on(d) {
            return Err(DatumProofError::MissingDependsOn { datum: d.name.clone() });
        }
        Ok(())
    }
}

pub struct AsStackDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsStackDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Stack, "stack")?;
        hint_required(d)?;
        require_any(d, &[has_stack, has_members], "stack metadata or members")
    }
}

pub struct AsAgentDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsAgentDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Agent, "agent")?;
        hint_required(d)?;
        require_any(d, &[has_skills, has_depends_on, has_channel_prefix],
            "skills, depends_on, or channel_prefix")
    }
}

// ── Category C: Connection/identity ──────────────────────────────────────────

pub struct AsMcpDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsMcpDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Mcp, "mcp")?;
        if !has_command(d) {
            return Err(DatumProofError::MissingCommand { datum: d.name.clone() });
        }
        Ok(())
    }
}

pub struct AsDatabaseDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsDatabaseDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Database, "database")?;
        hint_required(d)?;
        require_any(d, &[has_dsn, has_url], "dsn or url")
    }
}

pub struct AsApiDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsApiDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Api, "api")?;
        hint_required(d)?;
        require_any(d, &[has_url, has_protocol, has_provides], "url, protocol, or provides.capability")
    }
}

pub struct AsRepoDatum<'a>(pub &'a BootDatum);
impl<'a> Provable for AsRepoDatum<'a> {
    fn prove(&self) -> Result<(), DatumProofError> {
        let d = self.0;
        typecheck(&d.datum_type, DatumType::Repo, "repo")?;
        hint_required(d)?;
        require_any(d, &[has_url, has_clone_path], "url or clone_path")
    }
}

// ── Convenience methods on BootDatum ─────────────────────────────────────────

impl BootDatum {
    pub fn prove_cli(&self)         -> Result<(), DatumProofError> { AsCliDatum(self).prove() }
    pub fn prove_skill(&self)       -> Result<(), DatumProofError> { AsSkillDatum(self).prove() }
    pub fn prove_role(&self)        -> Result<(), DatumProofError> { AsRoleDatum(self).prove() }
    pub fn prove_mcp(&self)         -> Result<(), DatumProofError> { AsMcpDatum(self).prove() }
    pub fn prove_docker(&self)      -> Result<(), DatumProofError> { AsDockerDatum(self).prove() }
    pub fn prove_bash(&self)        -> Result<(), DatumProofError> { AsBashDatum(self).prove() }
    pub fn prove_apt(&self)         -> Result<(), DatumProofError> { AsAptDatum(self).prove() }
    pub fn prove_nix(&self)         -> Result<(), DatumProofError> { AsNixDatum(self).prove() }
    pub fn prove_vscode(&self)      -> Result<(), DatumProofError> { AsVscodeDatum(self).prove() }
    pub fn prove_k8s(&self)         -> Result<(), DatumProofError> { AsK8sDatum(self).prove() }
    pub fn prove_justfile(&self)    -> Result<(), DatumProofError> { AsJustfileDatum(self).prove() }
    pub fn prove_job(&self)         -> Result<(), DatumProofError> { AsJobDatum(self).prove() }
    pub fn prove_stack(&self)       -> Result<(), DatumProofError> { AsStackDatum(self).prove() }
    pub fn prove_agent(&self)       -> Result<(), DatumProofError> { AsAgentDatum(self).prove() }
    pub fn prove_hive_profile(&self)-> Result<(), DatumProofError> { AsHiveProfileDatum(self).prove() }
    pub fn prove_database(&self)    -> Result<(), DatumProofError> { AsDatabaseDatum(self).prove() }
    pub fn prove_api(&self)         -> Result<(), DatumProofError> { AsApiDatum(self).prove() }
    pub fn prove_repo(&self)        -> Result<(), DatumProofError> { AsRepoDatum(self).prove() }
    pub fn prove_ai(&self)          -> Result<(), DatumProofError> { AsAiDatum(self).prove() }
    pub fn prove_ai_model(&self)    -> Result<(), DatumProofError> { AsAiModelDatum(self).prove() }
    pub fn prove_config(&self)      -> Result<(), DatumProofError> { AsConfigDatum(self).prove() }

    /// Dispatch prove based on declared datum_type. `Unknown` always passes.
    pub fn prove_by_type(&self) -> Result<(), DatumProofError> {
        match &self.datum_type {
            Some(DatumType::Cli)         => self.prove_cli(),
            Some(DatumType::Skill)       => self.prove_skill(),
            Some(DatumType::Role)        => self.prove_role(),
            Some(DatumType::Mcp)         => self.prove_mcp(),
            Some(DatumType::Docker)      => self.prove_docker(),
            Some(DatumType::Bash)        => self.prove_bash(),
            Some(DatumType::Apt)         => self.prove_apt(),
            Some(DatumType::Nix)         => self.prove_nix(),
            Some(DatumType::Vscode)      => self.prove_vscode(),
            Some(DatumType::K8s)         => self.prove_k8s(),
            Some(DatumType::Justfile)    => self.prove_justfile(),
            Some(DatumType::Job)         => self.prove_job(),
            Some(DatumType::Stack)       => self.prove_stack(),
            Some(DatumType::Agent)       => self.prove_agent(),
            Some(DatumType::HiveProfile) => self.prove_hive_profile(),
            Some(DatumType::Database)    => self.prove_database(),
            Some(DatumType::Api)         => self.prove_api(),
            Some(DatumType::Repo)        => self.prove_repo(),
            Some(DatumType::Ai)          => self.prove_ai(),
            Some(DatumType::AiModel)     => self.prove_ai_model(),
            Some(DatumType::Config)      => self.prove_config(),
            Some(DatumType::Hardware) | Some(DatumType::Overlay) | Some(DatumType::Unknown) | None => Ok(()),
        }
    }

    /// Check if `type_tags` includes a given tag (bridge to trait resolution).
    pub fn has_type_tag(&self, tag: &str) -> bool {
        self.type_tags
            .as_ref()
            .map(|tags| tags.iter().any(|t| t == tag))
            .unwrap_or(false)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BootDatum, DatumType, InstallSpec, JustfileConfig};

    // ── Builder macro: reduces per-type test boilerplate ─────────────────────
    // Usage: datum!(name, TypeVariant, field = value, ...)
    macro_rules! datum {
        ($name:literal, $typ:ident $(, $field:ident = $val:expr)*) => {
            BootDatum {
                name: $name.to_string(),
                datum_type: Some(DatumType::$typ),
                hint: concat!($name, " hint").to_string(),
                $($field: $val,)*
                ..Default::default()
            }
        };
    }

    // helper: same but hint intentionally empty to test hint_required
    fn no_hint(dt: DatumType) -> BootDatum {
        BootDatum { name: "no-hint".to_string(), datum_type: Some(dt), ..Default::default() }
    }

    // ── Category D ────────────────────────────────────────────────────────────

    #[test] fn ai_proves()            { assert!(datum!("gpt4", Ai).prove_ai().is_ok()); }
    #[test] fn ai_no_hint_fails()     { assert!(no_hint(DatumType::Ai).prove_ai().is_err()); }
    #[test] fn aimodel_proves()       { assert!(datum!("gpt4o", AiModel).prove_ai_model().is_ok()); }
    #[test] fn config_proves()        { assert!(datum!("cfg", Config).prove_config().is_ok()); }
    #[test] fn hive_profile_proves()  { assert!(datum!("hive", HiveProfile).prove_hive_profile().is_ok()); }

    // ── Category E ────────────────────────────────────────────────────────────

    #[test] fn unknown_always_proves() {
        let d = BootDatum { name: "x".to_string(), ..Default::default() };
        assert!(AsUnknownDatum(&d).prove().is_ok());
    }

    // ── Category A: Cli ───────────────────────────────────────────────────────

    #[test] fn cli_version_proves() {
        let d = datum!("git", Cli, version = Some("git --version".to_string()));
        assert!(d.prove_cli().is_ok());
    }
    #[test] fn cli_install_proves() {
        let d = datum!("git", Cli, install = Some(InstallSpec::Command("apt install git".to_string())));
        assert!(d.prove_cli().is_ok());
    }
    #[test] fn cli_empty_fails() {
        let d = datum!("bare", Cli);
        assert!(matches!(d.prove_cli(), Err(DatumProofError::MissingStructuralField { .. })));
    }
    #[test] fn cli_wrong_type_fails() {
        let d = datum!("skill", Skill, keywords = Some(vec!["x".to_string()]), version = Some("v".to_string()));
        assert!(matches!(d.prove_cli(), Err(DatumProofError::WrongType { .. })));
    }

    // ── Category A: Docker ────────────────────────────────────────────────────

    #[test] fn docker_image_proves() {
        let d = datum!("nginx", Docker, image = Some("nginx:latest".to_string()));
        assert!(d.prove_docker().is_ok());
    }
    #[test] fn docker_oci_proves() {
        let d = datum!("oci", Docker, oci_uri = Some("oci://registry.io/img".to_string()));
        assert!(d.prove_docker().is_ok());
    }
    #[test] fn docker_bare_fails() {
        let d = datum!("bare", Docker);
        assert!(d.prove_docker().is_err());
    }

    // ── Category A: Bash ──────────────────────────────────────────────────────

    #[test] fn bash_script_proves() {
        let d = datum!("setup", Bash, script = Some("#!/bin/bash\necho ok".to_string()));
        assert!(d.prove_bash().is_ok());
    }
    #[test] fn bash_bare_fails() {
        let d = datum!("bare", Bash);
        assert!(d.prove_bash().is_err());
    }

    // ── Category A: Apt ───────────────────────────────────────────────────────

    #[test] fn apt_package_name_proves() {
        let d = datum!("curl", Apt, package_name = Some("curl".to_string()));
        assert!(d.prove_apt().is_ok());
    }
    #[test] fn apt_install_proves() {
        let d = datum!("curl", Apt, install = Some(InstallSpec::Command("apt install curl".to_string())));
        assert!(d.prove_apt().is_ok());
    }
    #[test] fn apt_bare_fails() {
        let d = datum!("bare", Apt);
        assert!(d.prove_apt().is_err());
    }

    // ── Category A: Nix ───────────────────────────────────────────────────────

    #[test] fn nix_package_proves() {
        let d = datum!("ripgrep", Nix, package_name = Some("ripgrep".to_string()));
        assert!(d.prove_nix().is_ok());
    }
    #[test] fn nix_bare_fails() {
        let d = datum!("bare", Nix);
        assert!(d.prove_nix().is_err());
    }

    // ── Category A: Vscode ────────────────────────────────────────────────────

    #[test] fn vscode_vsix_proves() {
        let d = datum!("rust-analyzer", Vscode, vsix_id = Some("rust-lang.rust-analyzer".to_string()));
        assert!(d.prove_vscode().is_ok());
    }
    #[test] fn vscode_bare_fails() {
        let d = datum!("bare", Vscode);
        assert!(d.prove_vscode().is_err());
    }

    // ── Category A: K8s ───────────────────────────────────────────────────────

    #[test] fn k8s_chart_proves() {
        let d = datum!("ingress", K8s, chart_path = Some("charts/ingress".to_string()));
        assert!(d.prove_k8s().is_ok());
    }
    #[test] fn k8s_values_proves() {
        let d = datum!("ingress", K8s, values_file = Some("values.yaml".to_string()));
        assert!(d.prove_k8s().is_ok());
    }
    #[test] fn k8s_bare_fails() {
        let d = datum!("bare", K8s);
        assert!(d.prove_k8s().is_err());
    }

    // ── Category A: Justfile ──────────────────────────────────────────────────

    #[test] fn justfile_path_proves() {
        let d = datum!("ml", Justfile, justfile = Some(JustfileConfig {
            path: Some("justfile.ml".to_string()),
            ..Default::default()
        }));
        assert!(d.prove_justfile().is_ok());
    }
    #[test] fn justfile_bare_fails() {
        let d = datum!("bare", Justfile);
        assert!(d.prove_justfile().is_err());
    }

    // ── Category A: Job ───────────────────────────────────────────────────────

    #[test] fn job_metadata_proves() {
        let d = datum!("train", Job, job = Some(serde_json::json!({"queue": "gpu"})));
        assert!(d.prove_job().is_ok());
    }
    #[test] fn job_null_fails() {
        let d = datum!("bare", Job, job = Some(serde_json::Value::Null));
        assert!(d.prove_job().is_err());
    }
    #[test] fn job_bare_fails() {
        let d = datum!("bare", Job);
        assert!(d.prove_job().is_err());
    }

    // ── Category B: Skill ─────────────────────────────────────────────────────

    #[test] fn skill_keywords_proves() {
        let d = datum!("kaizen", Skill, keywords = Some(vec!["improve".to_string()]));
        assert!(d.prove_skill().is_ok());
    }
    #[test] fn skill_bare_fails() {
        let d = datum!("bare", Skill);
        assert!(d.prove_skill().is_err());
    }

    // ── Category B: Role ──────────────────────────────────────────────────────

    #[test] fn role_depends_proves() {
        let d = datum!("worker", Role, depends_on = Some(vec!["git.cli".to_string()]));
        assert!(d.prove_role().is_ok());
    }
    #[test] fn role_bare_fails() {
        let d = datum!("bare", Role);
        assert!(matches!(d.prove_role(), Err(DatumProofError::MissingDependsOn { .. })));
    }

    // ── Category B: Stack ─────────────────────────────────────────────────────

    #[test] fn stack_members_proves() {
        let d = datum!("llm", Stack, members = Some(vec!["vllm.cli".to_string()]));
        assert!(d.prove_stack().is_ok());
    }
    #[test] fn stack_meta_proves() {
        let d = datum!("llm", Stack, stack = Some(serde_json::json!({"tier": "ch0nky"})));
        assert!(d.prove_stack().is_ok());
    }
    #[test] fn stack_bare_fails() {
        let d = datum!("bare", Stack);
        assert!(d.prove_stack().is_err());
    }

    // ── Category B: Agent ─────────────────────────────────────────────────────

    #[test] fn agent_skills_proves() {
        let d = datum!("ralph", Agent, skills = Some(vec!["kaizen".to_string()]));
        assert!(d.prove_agent().is_ok());
    }
    #[test] fn agent_channel_proves() {
        let d = datum!("ralph", Agent, channel_prefix = Some("ralph:".to_string()));
        assert!(d.prove_agent().is_ok());
    }
    #[test] fn agent_bare_fails() {
        let d = datum!("bare", Agent);
        assert!(d.prove_agent().is_err());
    }

    // ── Category C: Mcp ───────────────────────────────────────────────────────

    #[test] fn mcp_command_proves() {
        let d = datum!("github", Mcp, command = Some("uvx".to_string()));
        assert!(d.prove_mcp().is_ok());
    }
    #[test] fn mcp_bare_fails() {
        let d = datum!("bare", Mcp);
        assert!(matches!(d.prove_mcp(), Err(DatumProofError::MissingCommand { .. })));
    }

    // ── Category C: Database ──────────────────────────────────────────────────

    #[test] fn database_dsn_proves() {
        let d = datum!("pg", Database, dsn = Some("postgres://localhost/db".to_string()));
        assert!(d.prove_database().is_ok());
    }
    #[test] fn database_url_proves() {
        let d = datum!("pg", Database, url = Some("postgres://localhost/db".to_string()));
        assert!(d.prove_database().is_ok());
    }
    #[test] fn database_bare_fails() {
        let d = datum!("bare", Database);
        assert!(d.prove_database().is_err());
    }

    // ── Category C: Api ───────────────────────────────────────────────────────

    #[test] fn api_url_proves() {
        let d = datum!("openai", Api, url = Some("https://api.openai.com".to_string()));
        assert!(d.prove_api().is_ok());
    }
    #[test] fn api_protocol_proves() {
        let d = datum!("grpc-svc", Api, protocol = Some("grpc".to_string()));
        assert!(d.prove_api().is_ok());
    }
    #[test] fn api_bare_fails() {
        let d = datum!("bare", Api);
        assert!(d.prove_api().is_err());
    }

    // ── Category C: Repo ──────────────────────────────────────────────────────

    #[test] fn repo_url_proves() {
        let d = datum!("b00t", Repo, url = Some("https://github.com/pe/b00t".to_string()));
        assert!(d.prove_repo().is_ok());
    }
    #[test] fn repo_clone_path_proves() {
        let d = datum!("b00t", Repo, clone_path = Some("/src/b00t".to_string()));
        assert!(d.prove_repo().is_ok());
    }
    #[test] fn repo_bare_fails() {
        let d = datum!("bare", Repo);
        assert!(d.prove_repo().is_err());
    }

    // ── prove_by_type dispatch ────────────────────────────────────────────────

    #[test] fn prove_by_type_cli_ok() {
        let d = datum!("git", Cli, version = Some("git --version".to_string()));
        assert!(d.prove_by_type().is_ok());
    }
    #[test] fn prove_by_type_cli_fail() {
        assert!(datum!("bare", Cli).prove_by_type().is_err());
    }
    #[test] fn prove_by_type_unknown_always_ok() {
        let d = BootDatum { name: "x".to_string(), hint: "x".to_string(), datum_type: Some(DatumType::Unknown), ..Default::default() };
        assert!(d.prove_by_type().is_ok());
    }
    #[test] fn prove_by_type_none_always_ok() {
        let d = BootDatum { name: "x".to_string(), hint: "x".to_string(), ..Default::default() };
        assert!(d.prove_by_type().is_ok());
    }

    // ── has_type_tag ──────────────────────────────────────────────────────────

    #[test] fn has_type_tag_works() {
        let d = BootDatum {
            name: "kaizen".to_string(), hint: "kaizen".to_string(),
            type_tags: Some(vec!["transferable".to_string()]),
            ..Default::default()
        };
        assert!(d.has_type_tag("transferable"));
        assert!(!d.has_type_tag("domain:rust"));
    }
}
