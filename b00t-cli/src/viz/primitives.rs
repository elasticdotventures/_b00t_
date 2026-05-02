//! Isometric projection primitives
//! Implements 3D → 2D isometric projection for visualization rendering
//! Based on l3dg3rr's proven isometric model

use serde::{Deserialize, Serialize};

/// 3D vector in world space
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    /// Create a new 3D vector
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }

    /// Project this vector to 2D screen space using isometric projection
    pub fn to_screen(self) -> (f64, f64) {
        iso_project(self.x, self.y, self.z)
    }
}

/// Isometric projection from 3D world coordinates to 2D screen coordinates
///
/// Mathematical basis:
/// ```
/// screen_x = (x - z) * √3/2 ≈ 0.866025...
/// screen_y = (x + z) * 0.5 - y
/// ```
///
/// This projection preserves 45° angles on cardinal axes and is commonly used
/// for isometric games and data visualization.
///
/// # Arguments
/// * `x` - World X coordinate (typically horizontal, right)
/// * `y` - World Y coordinate (typically vertical, up)
/// * `z` - World Z coordinate (typically depth, forward)
///
/// # Returns
/// Tuple of (screen_x, screen_y) coordinates
pub fn iso_project(x: f64, y: f64, z: f64) -> (f64, f64) {
    let sqrt3_over_2 = 0.8660254037844386; // √3/2
    let screen_x = (x - z) * sqrt3_over_2;
    let screen_y = (x + z) * 0.5 - y;
    (screen_x, screen_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 0.001;

    #[test]
    fn test_iso_project_unit_x() {
        // Unit vector along x-axis
        let (x, y) = iso_project(1.0, 0.0, 0.0);
        assert!(
            (x - 0.866).abs() < EPSILON,
            "x should be ≈0.866, got {}",
            x
        );
        assert!((y - 0.5).abs() < EPSILON, "y should be ≈0.5, got {}", y);
    }

    #[test]
    fn test_iso_project_unit_y() {
        // Unit vector along y-axis (down in screen space)
        let (x, y) = iso_project(0.0, 1.0, 0.0);
        assert!(x.abs() < EPSILON, "x should be ≈0.0, got {}", x);
        assert!(
            (y - (-0.5)).abs() < EPSILON,
            "y should be ≈-0.5, got {}",
            y
        );
    }

    #[test]
    fn test_iso_project_unit_z() {
        // Unit vector along z-axis (depth)
        let (x, y) = iso_project(0.0, 0.0, 1.0);
        assert!(
            (x - (-0.866)).abs() < EPSILON,
            "x should be ≈-0.866, got {}",
            x
        );
        assert!((y - 0.5).abs() < EPSILON, "y should be ≈0.5, got {}", y);
    }

    #[test]
    fn test_iso_project_diagonal() {
        // Diagonal vector (1, 1, 1)
        let (x, y) = iso_project(1.0, 1.0, 1.0);
        assert!(x.abs() < EPSILON, "x should be ≈0.0, got {}", x);
        assert!(y.abs() < EPSILON, "y should be ≈0.0, got {}", y);
    }

    #[test]
    fn test_iso_project_origin() {
        // Origin should map to origin
        let (x, y) = iso_project(0.0, 0.0, 0.0);
        assert!(x.abs() < EPSILON && y.abs() < EPSILON, "Origin mismatch");
    }

    #[test]
    fn test_iso_project_negative_coords() {
        // Negative x should reverse x projection
        let (x, y) = iso_project(-1.0, 0.0, 0.0);
        assert!(
            (x - (-0.866)).abs() < EPSILON,
            "negative x: expected ≈-0.866, got {}",
            x
        );
        assert!((y - (-0.5)).abs() < EPSILON, "negative x: y mismatch");
    }

    #[test]
    fn test_iso_project_large_values() {
        // Large uniform vector should still satisfy diagonal property
        let (x, y) = iso_project(100.0, 100.0, 100.0);
        assert!(
            x.abs() < EPSILON && y.abs() < EPSILON,
            "large uniform vector should project near origin"
        );
    }

    #[test]
    fn test_vec3_new_and_to_screen() {
        let v = Vec3::new(1.0, 0.0, 0.0);
        let (x, y) = v.to_screen();
        assert!(
            (x - 0.866).abs() < EPSILON,
            "Vec3.to_screen() mismatch on x"
        );
        assert!((y - 0.5).abs() < EPSILON, "Vec3.to_screen() mismatch on y");
    }

    #[test]
    fn test_vec3_equality() {
        let v1 = Vec3::new(1.0, 2.0, 3.0);
        let v2 = Vec3::new(1.0, 2.0, 3.0);
        let v3 = Vec3::new(1.0, 2.0, 4.0);
        assert_eq!(v1, v2, "Equal vectors should match");
        assert_ne!(v1, v3, "Different vectors should not match");
    }
}
