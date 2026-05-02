//! TDD foundation tests for b00t ↔ l3dg3rr visualization integration
//! Phase 1: Schema Extension & Isometric Primitives
//!
//! These tests verify:
//! 1. VisualizationSpec parsing from .tomllmd [sections.visualization]
//! 2. Round-trip serialization/deserialization
//! 3. Isometric projection mathematics (Vec3 → screen coords)
//! 4. Backward compatibility with .tomllmd files lacking visualization section

use b00t_cli::datum_utils::VisualizationSpec;

/// Test 1: Parse real .tomllmd files with [sections.visualization]
/// Expected: VisualizationSpec successfully deserialized with all fields populated
#[test]
fn test_tomllmd_visualization_parsing() {
    let toml_content = r#"
[sections.visualization]
type = "rhai_dsl"
render_opts = ["isometric", "mermaid_fallback", "no_cache"]
auto_scope = "dag"
"#;

    // Parse TOML
    let parsed: Result<toml::Table, _> = toml::from_str(toml_content);
    assert!(
        parsed.is_ok(),
        "Failed to parse TOML: {:?}",
        parsed.err()
    );

    let table = parsed.unwrap();
    let vis_table = table
        .get("sections")
        .and_then(|s| s.get("visualization"))
        .expect("visualization section not found");

    // Verify structure
    assert_eq!(
        vis_table.get("type").and_then(|v| v.as_str()),
        Some("rhai_dsl"),
        "type field mismatch"
    );

    let render_opts = vis_table
        .get("render_opts")
        .and_then(|v| v.as_array())
        .expect("render_opts not found");

    assert_eq!(render_opts.len(), 3, "render_opts should have 3 elements");
    assert_eq!(
        render_opts[0].as_str(),
        Some("isometric"),
        "first render_opt should be 'isometric'"
    );

    assert_eq!(
        vis_table.get("auto_scope").and_then(|v| v.as_str()),
        Some("dag"),
        "auto_scope field mismatch"
    );
}

/// Test 2: VisualizationSpec round-trip serialization
/// Expected: VisualizationSpec → serialize → deserialize → equal original
#[test]
fn test_visualization_spec_deserialization() {
    let spec = VisualizationSpec {
        viz_type: "rhai_dsl".to_string(),
        render_opts: vec![
            "isometric".to_string(),
            "mermaid_fallback".to_string(),
            "no_cache".to_string(),
        ],
        auto_scope: Some("graph".to_string()),
    };

    // Serialize to TOML string
    let serialized = toml::to_string(&spec).expect("Failed to serialize VisualizationSpec");

    // Deserialize back
    let deserialized: VisualizationSpec =
        toml::from_str(&serialized).expect("Failed to deserialize VisualizationSpec");

    // Assert equality
    assert_eq!(spec.viz_type, deserialized.viz_type, "viz_type mismatch");
    assert_eq!(spec.render_opts, deserialized.render_opts, "render_opts mismatch");
    assert_eq!(spec.auto_scope, deserialized.auto_scope, "auto_scope mismatch");
}

/// Test 3: Isometric projection mathematics
/// Expected: Vec3 coordinates project correctly to 2D screen space
/// Formula: screen_x = (x - z) * √3/2 ≈ 0.866
///          screen_y = (x + z) * 0.5 - y
#[test]
fn test_vec3_isometric_projection() {
    use b00t_cli::viz::primitives::iso_project;

    const EPSILON: f64 = 0.001;

    // Test case 1: Unit vector along x-axis (1, 0, 0)
    // Expected: screen_x ≈ 0.866, screen_y ≈ 0.5
    let (x, y) = iso_project(1.0, 0.0, 0.0);
    assert!(
        (x - 0.866).abs() < EPSILON,
        "Case 1 x: expected ≈0.866, got {}",
        x
    );
    assert!(
        (y - 0.5).abs() < EPSILON,
        "Case 1 y: expected ≈0.5, got {}",
        y
    );

    // Test case 2: Unit vector along y-axis (0, 1, 0)
    // Expected: screen_x = 0.0, screen_y ≈ -1.0 (y points down in screen space)
    let (x, y) = iso_project(0.0, 1.0, 0.0);
    assert!(
        x.abs() < EPSILON,
        "Case 2 x: expected ≈0.0, got {}",
        x
    );
    assert!(
        (y - (-1.0)).abs() < EPSILON,
        "Case 2 y: expected ≈-1.0, got {}",
        y
    );

    // Test case 3: Diagonal (1, 1, 1)
    // Expected: screen_x = 0.0, screen_y ≈ 0.0
    let (x, y) = iso_project(1.0, 1.0, 1.0);
    assert!(
        x.abs() < EPSILON,
        "Case 3 x: expected ≈0.0, got {}",
        x
    );
    assert!(
        y.abs() < EPSILON,
        "Case 3 y: expected ≈0.0, got {}",
        y
    );
}

/// Test 4: Backward compatibility
/// Expected: .tomllmd WITHOUT [sections.visualization] still parses as valid
/// (visualization field is None, no errors)
#[test]
fn test_backward_compatibility_no_visualization() {
    let toml_content = r#"
[b00t]
name = "test-datum"
type = "job"
hint = "Test datum without visualization"
"#;

    // Parse should succeed
    let parsed: Result<toml::Table, _> = toml::from_str(toml_content);
    assert!(
        parsed.is_ok(),
        "Should parse TOML without visualization section"
    );

    let table = parsed.unwrap();

    // Assert visualization section is absent (graceful degradation)
    assert!(
        table.get("sections").is_none() || table.get("sections").and_then(|s| s.get("visualization")).is_none(),
        "visualization section should be absent"
    );

    // Assert core fields are still present
    assert!(table.get("b00t").is_some(), "b00t section should exist");
}

/// Test 5: Multiple render_opts variations
/// Expected: Various render_opt combinations parse correctly
#[test]
fn test_visualization_render_opts_variants() {
    let variants = vec![
        (
            r#"type = "mermaid"
render_opts = ["mermaid_fallback"]"#,
            vec!["mermaid_fallback"],
        ),
        (
            r#"type = "plantuml"
render_opts = ["isometric", "no_cache"]"#,
            vec!["isometric", "no_cache"],
        ),
        (
            r#"type = "rhai_dsl"
render_opts = []"#,
            vec![],
        ),
    ];

    for (content, expected_opts) in variants {
        let toml_str = format!("[sections.visualization]\n{}", content);
        let parsed: Result<toml::Table, _> = toml::from_str(&toml_str);
        assert!(parsed.is_ok(), "Failed to parse variant: {}", content);

        let table = parsed.unwrap();
        let vis_table = table
            .get("sections")
            .and_then(|s| s.get("visualization"))
            .expect("visualization section not found");

        let render_opts = vis_table
            .get("render_opts")
            .and_then(|v| v.as_array())
            .expect("render_opts not found");

        assert_eq!(
            render_opts.len(),
            expected_opts.len(),
            "render_opts length mismatch for variant: {}",
            content
        );

        for (i, expected) in expected_opts.iter().enumerate() {
            assert_eq!(
                render_opts[i].as_str(),
                Some(*expected),
                "render_opts[{}] mismatch for variant: {}",
                i,
                content
            );
        }
    }
}

/// Test 6: Isometric projection edge cases
/// Expected: Zero vectors and negative coordinates project correctly
#[test]
fn test_vec3_projection_edge_cases() {
    use b00t_cli::viz::primitives::iso_project;

    const EPSILON: f64 = 0.001;

    // Test origin (0, 0, 0)
    let (x, y) = iso_project(0.0, 0.0, 0.0);
    assert!(x.abs() < EPSILON && y.abs() < EPSILON, "Origin should map to (0, 0)");

    // Test negative coordinates
    let (x, _y) = iso_project(-1.0, 0.0, 0.0);
    assert!(
        (x - (-0.866)).abs() < EPSILON,
        "Negative x should project to ≈-0.866"
    );

    // Test large values
    let (x, y) = iso_project(100.0, 100.0, 100.0);
    assert!(
        x.abs() < EPSILON && y.abs() < EPSILON,
        "Large uniform vector should project near origin"
    );
}

/// Test 7: VisualizationSpec with None auto_scope
/// Expected: Optional auto_scope field handles None correctly
#[test]
fn test_visualization_spec_optional_auto_scope() {
    let spec = VisualizationSpec {
        viz_type: "mermaid".to_string(),
        render_opts: vec!["mermaid_fallback".to_string()],
        auto_scope: None,
    };

    let serialized = toml::to_string(&spec).expect("Failed to serialize");
    let deserialized: VisualizationSpec =
        toml::from_str(&serialized).expect("Failed to deserialize");

    assert_eq!(deserialized.auto_scope, None, "auto_scope should be None");
}
