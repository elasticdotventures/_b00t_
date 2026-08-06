//! Unit tests for pure functions in `b00t_cli::commands::gh_runner`.
//!
//! Tests cover: `repo_slug` (string transformation) and `generate_kube_yaml`
//! (YAML template rendering). All test data uses placeholder/fake values —
//! no real GitHub repos, tokens, or hostnames are involved.
//!
//! ## Coverage targets
//! - `repo_slug`: basic transformation, mixed case, single segment
//! - `generate_kube_yaml`: pod name, repo URL, labels, ephemeral flag,
//!   docker socket mount/omission, security context, resource limits,
//!   valid YAML structure

use b00t_cli::commands::gh_runner::{generate_kube_yaml, repo_slug};

// ─── repo_slug tests ──────────────────────────────────────────────────────────

#[test]
fn test_repo_slug_basic() {
    // ✅ Positive: standard owner/repo → lowercase slug
    assert_eq!(repo_slug("app4dog/middleware"), "app4dog-middleware");
}

#[test]
fn test_repo_slug_mixed_case() {
    // ✅ Positive: mixed-case input → fully lowercased
    assert_eq!(repo_slug("Foo/Bar"), "foo-bar");
}

#[test]
fn test_repo_slug_single_slash() {
    // ✅ Positive: minimal two-segment input
    assert_eq!(repo_slug("owner/repo"), "owner-repo");
}

#[test]
fn test_repo_slug_no_slash() {
    // ❌ Negative / edge case: input without slash → unchanged but lowercased
    assert_eq!(repo_slug("SingleRepo"), "singlerepo");
}

#[test]
fn test_repo_slug_multiple_slashes() {
    // ❌ Negative / edge case: multiple slashes → all replaced with hyphens
    assert_eq!(repo_slug("org/team/repo"), "org-team-repo");
}

#[test]
fn test_repo_slug_empty() {
    // ❌ Negative / edge case: empty string → empty string
    assert_eq!(repo_slug(""), "");
}

#[test]
fn test_repo_slug_already_lowercase() {
    // ✅ Positive: already-lowercase input is unchanged
    assert_eq!(repo_slug("already/lower"), "already-lower");
}

// ─── generate_kube_yaml tests ─────────────────────────────────────────────────

#[test]
fn test_generate_kube_yaml_contains_pod_name() {
    // ✅ Positive: pod name uses slugified repo
    let yaml = generate_kube_yaml(
        "myorg/myrepo",
        "linux,x64",
        "/tmp/work",
        "fake-token",
        false,
        "/var/run/docker.sock",
    );
    assert!(yaml.contains("name: gh-runner-myorg-myrepo"));
}

#[test]
fn test_generate_kube_yaml_contains_repo_url() {
    // ✅ Positive: REPO_URL env var references correct GitHub URL
    let yaml = generate_kube_yaml(
        "myorg/myrepo",
        "linux,x64",
        "/tmp/work",
        "fake-token",
        false,
        "/var/run/docker.sock",
    );
    assert!(yaml.contains("https://github.com/myorg/myrepo"));
}

#[test]
fn test_generate_kube_yaml_contains_labels() {
    // ✅ Positive: labels are injected correctly
    let yaml = generate_kube_yaml(
        "a/b",
        "linux,x64,self-hosted",
        "/tmp/w",
        "fake-token",
        false,
        "/var/run/docker.sock",
    );
    assert!(yaml.contains("linux,x64,self-hosted"));
}

#[test]
fn test_generate_kube_yaml_ephemeral_true() {
    // ✅ Positive: ephemeral=true → value is "true"
    let yaml = generate_kube_yaml(
        "a/b",
        "l",
        "/tmp/w",
        "fake-token",
        true,
        "/var/run/docker.sock",
    );
    assert!(yaml.contains("value: \"true\""));
}

#[test]
fn test_generate_kube_yaml_ephemeral_false() {
    // ✅ Positive: ephemeral=false → value is "false"
    let yaml = generate_kube_yaml(
        "a/b",
        "l",
        "/tmp/w",
        "fake-token",
        false,
        "/var/run/docker.sock",
    );
    assert!(yaml.contains("value: \"false\""));
}

#[test]
fn test_generate_kube_yaml_docker_socket_present() {
    // ✅ Positive: socket path → volume mount and volume definition present
    let yaml = generate_kube_yaml(
        "a/b",
        "l",
        "/tmp/w",
        "fake-token",
        false,
        "/var/run/docker.sock",
    );
    assert!(yaml.contains("docker-sock"));
    assert!(yaml.contains("/var/run/docker.sock"));
}

#[test]
fn test_generate_kube_yaml_socket_none() {
    // ❌ Negative / edge case: empty socket path → no docker-sock references
    let yaml = generate_kube_yaml("a/b", "l", "/tmp/w", "fake-token", false, "");
    assert!(!yaml.contains("docker-sock"));
}

#[test]
fn test_generate_kube_yaml_is_valid_yaml() {
    // ✅ Positive: output must parse as valid YAML
    let yaml = generate_kube_yaml(
        "a/b",
        "l",
        "/tmp/w",
        "fake-token",
        false,
        "/var/run/docker.sock",
    );
    let _doc: serde_yaml::Value = serde_yaml::from_str(&yaml).expect("YAML should be valid");
}

#[test]
fn test_generate_kube_yaml_has_security_context() {
    // ✅ Positive: non-root + no privilege escalation
    let yaml = generate_kube_yaml(
        "a/b",
        "l",
        "/tmp/w",
        "fake-token",
        false,
        "/var/run/docker.sock",
    );
    assert!(yaml.contains("runAsNonRoot: true"));
    assert!(yaml.contains("allowPrivilegeEscalation: false"));
}

#[test]
fn test_generate_kube_yaml_has_resource_limits() {
    // ✅ Positive: resource requests and limits present
    let yaml = generate_kube_yaml(
        "a/b",
        "l",
        "/tmp/w",
        "fake-token",
        false,
        "/var/run/docker.sock",
    );
    assert!(yaml.contains("memory: \"4Gi\""));
    assert!(yaml.contains("cpu: \"4\""));
}

#[test]
fn test_generate_kube_yaml_has_resource_requests() {
    // ✅ Positive: resource requests (2Gi/2CPU) also present
    let yaml = generate_kube_yaml(
        "a/b",
        "l",
        "/tmp/w",
        "fake-token",
        false,
        "/var/run/docker.sock",
    );
    assert!(yaml.contains("memory: \"2Gi\""));
    assert!(yaml.contains("cpu: \"2\""));
}

#[test]
fn test_generate_kube_yaml_has_workdir_volume() {
    // ✅ Positive: workdir hostPath volume is defined
    let yaml = generate_kube_yaml("x/y", "l", "/tmp/mywork", "fake-token", false, "");
    assert!(yaml.contains("path: /tmp/mywork/_work"));
}

#[test]
fn test_generate_kube_yaml_custom_socket_path() {
    // ✅ Positive: custom podman socket path is embedded correctly
    let yaml = generate_kube_yaml(
        "a/b",
        "l",
        "/tmp/w",
        "fake-token",
        false,
        "/run/user/1000/podman/podman.sock",
    );
    assert!(yaml.contains("docker-sock"));
    assert!(yaml.contains("/run/user/1000/podman/podman.sock"));
}

#[test]
fn test_generate_kube_yaml_contains_restart_policy() {
    // ✅ Positive: restart policy OnFailure
    let yaml = generate_kube_yaml("x/y", "l", "/tmp/w", "fake-token", false, "");
    assert!(yaml.contains("restartPolicy: OnFailure"));
}

#[test]
fn test_generate_kube_yaml_contains_token_value() {
    // ✅ Positive: RUNNER_TOKEN is embedded in YAML (podman 5.x limitation — no --secret flag)
    let yaml = generate_kube_yaml("x/y", "l", "/tmp/w", "fake-token", false, "");
    assert!(yaml.contains("value: \"fake-token\""));
}

#[test]
fn test_generate_kube_yaml_runner_name_includes_hostname() {
    // ❌ Negative / edge case: runner_name injects hostname — verify the pattern exists.
    //    Since hostname is runtime-dependent, we verify the format not the exact value.
    let yaml = generate_kube_yaml("myorg/myrepo", "l", "/tmp/w", "fake-token", false, "");
    // RUNNER_NAME should start with the slug
    assert!(yaml.contains("value: \"myorg-myrepo-"));
}
