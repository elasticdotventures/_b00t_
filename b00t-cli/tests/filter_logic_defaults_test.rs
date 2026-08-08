//! Tests for the shared `AvailabilityEvaluator`/`impl_boot_datum_accessors!`
//! extraction (#865) — proves the Docker/K8s/Podman datum types still behave
//! identically after deduplicating their `FilterLogic::is_available` bodies
//! and `TryFrom`/`ConstraintEvaluator`/`DatumProvider` boilerplate into
//! shared trait defaults + a macro. Pure refactor: no behavior change.

use b00t_cli::datum_docker::DockerDatum;
use b00t_cli::datum_k8s::K8sDatum;
use b00t_cli::datum_podman::PodmanDatum;
use b00t_cli::traits::{AvailabilityEvaluator, DatumChecker, FilterLogic};
use b00t_cli::BootDatum;

/// Compile-time proof: all three container datum types implement
/// `AvailabilityEvaluator` via the blanket impl (`DatumChecker + FilterLogic`).
#[test]
fn all_three_types_implement_availability_evaluator() {
    fn assert_impl<T: AvailabilityEvaluator>() {}
    assert_impl::<DockerDatum>();
    assert_impl::<K8sDatum>();
    assert_impl::<PodmanDatum>();
}

fn fixture_datum(name: &str) -> BootDatum {
    BootDatum {
        name: name.into(),
        hint: "test".into(),
        ..Default::default()
    }
}

/// Behavioral proof: `FilterLogic::is_available()` for each type still equals
/// the independently-computed `!is_installed() && prerequisites_satisfied()`
/// — i.e. delegating to `AvailabilityEvaluator::is_available_default()`
/// changed nothing observable. Builds datum structs directly (no
/// `from_config`/CLI shell-out), so no real docker/kubectl/helm/podman
/// binaries are required and no `#[ignore]` is needed — mirrors the direct
/// struct-construction pattern already used by
/// `datum_podman.rs`'s `manifest_path_none_without_resource_path` test.
#[test]
fn is_available_matches_independent_computation_docker() {
    let datum = DockerDatum {
        datum: fixture_datum("docker-fixture"),
    };
    let expected = !DatumChecker::is_installed(&datum) && datum.prerequisites_satisfied();
    assert_eq!(FilterLogic::is_available(&datum), expected);
}

#[test]
fn is_available_matches_independent_computation_k8s() {
    let datum = K8sDatum {
        datum: fixture_datum("k8s-fixture"),
    };
    let expected = !DatumChecker::is_installed(&datum) && datum.prerequisites_satisfied();
    assert_eq!(FilterLogic::is_available(&datum), expected);
}

#[test]
fn is_available_matches_independent_computation_podman() {
    let datum = PodmanDatum {
        datum: fixture_datum("podman-fixture"),
    };
    let expected = !DatumChecker::is_installed(&datum) && datum.prerequisites_satisfied();
    assert_eq!(FilterLogic::is_available(&datum), expected);
}
