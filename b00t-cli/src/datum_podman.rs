//! Podman-kube-play datum — a raw Kubernetes Pod manifest deployed via
//! `podman kube play`, not Helm/kubectl (that's what `K8sDatum` is for) and
//! not `docker-compose` (that's `DatumType::Docker`). This is the third,
//! previously-missing leg: declarative Pod YAML, no cluster, no Helm chart,
//! optionally lifecycle-managed by a Quadlet `.kube` systemd unit.
//!
//! Reuses `BootDatum::resource_path` (already used by `Docker` for its
//! compose file) to point at the Pod manifest, so no new struct fields were
//! needed on the shared `BootDatum` schema — see datum_types.rs's own
//! "add new variants ONLY here" note for why that matters.
//!
//! Convention: the Pod manifest's `metadata.name` MUST match the datum's
//! `name` field — that's how `is_deployed` finds it via `podman pod exists`.

use crate::traits::*;
use crate::{BootDatum, check_command_available, get_config};
use anyhow::Result;
use duct::cmd;
use std::path::Path;

pub struct PodmanDatum {
    pub datum: BootDatum,
}

impl PodmanDatum {
    pub fn from_config(name: &str, path: &str) -> Result<Self> {
        let (config, _filename) = get_config(name, path).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(PodmanDatum { datum: config.b00t })
    }

    fn manifest_path(&self) -> Option<std::path::PathBuf> {
        let resource_path = self.datum.resource_path.as_ref()?;
        Some(if let Ok(repo_root) = std::env::var("REPO_ROOT") {
            Path::new(&repo_root).join(resource_path)
        } else {
            Path::new(resource_path).to_path_buf()
        })
    }

    fn is_manifest_available(&self) -> bool {
        self.manifest_path().map(|p| p.is_file()).unwrap_or(false)
    }

    fn is_deployed(&self) -> bool {
        cmd!("podman", "pod", "exists", &self.datum.name)
            .unchecked()
            .run()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Running image tag for the pod's first container, via `podman inspect`.
    /// Best-effort — used for VersionStatus comparison, not authoritative.
    fn deployed_image_tag(&self) -> Option<String> {
        let output = cmd!(
            "podman", "inspect", &self.datum.name,
            "--format", "{{(index .Containers 0).Image}}"
        )
        .read()
        .ok()?;
        output.trim().rsplit(':').next().map(|s| s.to_string())
    }
}

impl DatumChecker for PodmanDatum {
    fn is_installed(&self) -> bool {
        check_command_available("podman") && (self.is_deployed() || self.is_manifest_available())
    }

    fn current_version(&self) -> Option<String> {
        self.deployed_image_tag().or_else(|| {
            // Fallback: parse the tag out of oci_uri (image:tag)
            self.datum.oci_uri.as_deref().and_then(|uri| uri.rsplit(':').next()).map(String::from)
        })
    }

    fn desired_version(&self) -> Option<String> {
        self.datum.desires.clone()
    }

    fn version_status(&self) -> VersionStatus {
        if !check_command_available("podman") {
            return VersionStatus::Missing;
        }

        if self.is_deployed() {
            match (self.current_version(), self.desired_version()) {
                (Some(cur), Some(des)) if cur == des => VersionStatus::Match,
                // Can't tell older/newer from opaque tag strings; `Older` matches
                // how up_command.rs treats "needs update" (Older | Missing).
                (Some(_), Some(_)) => VersionStatus::Older,
                _ => VersionStatus::Match, // deployed, no version pin to compare against
            }
        } else if self.is_manifest_available() {
            VersionStatus::Unknown // manifest present, not deployed
        } else {
            VersionStatus::Missing
        }
    }
}

impl StatusProvider for PodmanDatum {
    fn name(&self) -> &str {
        &self.datum.name
    }

    fn subsystem(&self) -> &str {
        "podman"
    }

    fn hint(&self) -> &str {
        &self.datum.hint
    }

    fn is_disabled(&self) -> bool {
        !check_command_available("podman")
    }
}

impl FilterLogic for PodmanDatum {
    fn is_available(&self) -> bool {
        self.is_available_default()
    }

    fn prerequisites_satisfied(&self) -> bool {
        if let Some(require) = &self.datum.require {
            self.evaluate_constraints(require)
        } else {
            check_command_available("podman")
        }
    }

    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
}

crate::impl_boot_datum_accessors!(PodmanDatum);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_path_none_without_resource_path() {
        let datum = PodmanDatum {
            datum: BootDatum { name: "test".into(), hint: "test".into(), ..Default::default() },
        };
        assert!(datum.manifest_path().is_none());
        assert!(!datum.is_manifest_available());
    }
}
