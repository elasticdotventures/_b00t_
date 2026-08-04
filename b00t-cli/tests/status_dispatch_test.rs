// Regression test for #864: K8sDatum (DatumType::K8s) is structurally complete
// (FilterLogic/DatumProvider/StatusProvider all implemented in datum_k8s.rs) but
// was never constructed at runtime — `show_status()`'s dispatch block in main.rs
// called `load_datum_providers::<X>` for Cli/Mcp/Ai/Model/Apt/Bash/Docker/Podman/
// Vscode, but not K8s, so real `.k8s.toml` fixtures (argo-workflows, flux-cd,
// kube-prometheus-stack, kubecost, nvidia-gpu-operator, valkey) were invisible to
// `b00t status` / `--filter k8s`.
//
// This test proves `load_datum_providers::<K8sDatum>(path, ".k8s.toml")` — the
// exact call now wired into main.rs's show_status() dispatch, mirroring how
// DockerDatum/PodmanDatum are already wired — actually constructs and returns a
// provider for a `.k8s.toml` fixture, the same construction path main.rs uses.

use b00t_cli::datum_k8s::K8sDatum;
use b00t_cli::load_datum_providers;
use tempfile::TempDir;

#[test]
fn k8s_datum_is_constructed_by_status_dispatch_loader() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_str().unwrap();

    let config_content = r#"
[b00t]
name = "test-chart"
type = "k8s"
hint = "Test Helm chart for #864 status dispatch regression test"
desires = "1.0.0"
chart_path = "k8s.🚢/test-chart"
namespace = "test-namespace"
"#;

    std::fs::write(temp_dir.path().join("test-chart.k8s.toml"), config_content).unwrap();

    // Same call site pattern as main.rs's show_status() dispatch block.
    let providers = load_datum_providers::<K8sDatum>(path, ".k8s.toml")
        .expect("load_datum_providers::<K8sDatum> should not error on a valid fixture");

    assert_eq!(
        providers.len(),
        1,
        "expected exactly one K8sDatum provider constructed from the fixture, got {}",
        providers.len()
    );

    let names: Vec<&str> = providers.iter().map(|p| p.as_ref().name()).collect();
    assert!(
        names.contains(&"test-chart"),
        "K8sDatum provider for 'test-chart' was not present in dispatch output: {names:?}"
    );

    let subsystems: Vec<&str> = providers.iter().map(|p| p.as_ref().subsystem()).collect();
    assert!(
        subsystems.iter().all(|s| *s == "k8s"),
        "expected subsystem 'k8s' for all providers, got {subsystems:?}"
    );

    // Confirm the DatumType is really K8s, not silently dropped/miscategorized.
    assert_eq!(
        providers[0].as_ref().datum().datum_type,
        Some(b00t_cli::DatumType::K8s)
    );
}
