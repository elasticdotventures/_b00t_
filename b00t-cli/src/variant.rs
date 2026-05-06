//! Variant detection module.
//! Detects whether this is a 'core' (SFW public) or 'nsfw0r1d' (personal) install.
//! Gates are configured based on variant at startup.

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum B00tVariant {
    Core,
    Nsfw0r1d,
}

impl B00tVariant {
    pub fn name(&self) -> &str {
        match self {
            B00tVariant::Core => "core",
            B00tVariant::Nsfw0r1d => "nsfw0r1d",
        }
    }

    pub fn features(&self) -> &[&str] {
        match self {
            B00tVariant::Core => &["sfw", "no-crypto"],
            B00tVariant::Nsfw0r1d => &["nsfw-inspiration", "crypto-enabled"],
        }
    }

    pub fn is_sfw(&self) -> bool {
        matches!(self, B00tVariant::Core)
    }
}

/// Detect the current variant by checking:
/// 1. B00T_VARIANT env var (override)
/// 2. _b00t_/schema/variant.toml (authoritative)
/// 3. r3src_资源/inspiration.yaml for NSFW keywords (fallback)
/// 4. Default: Core
pub fn detect_variant(b00t_path: &PathBuf) -> B00tVariant {
    // 1. Env var override
    if let Ok(v) = std::env::var("B00T_VARIANT") {
        match v.to_lowercase().as_str() {
            "nsfw0r1d" => return B00tVariant::Nsfw0r1d,
            _ => return B00tVariant::Core,
        }
    }

    // 2. variant.toml
    let variant_path = b00t_path.join("_b00t_/schema/variant.toml");
    if let Ok(content) = std::fs::read_to_string(&variant_path) {
        if content.contains("nsfw0r1d") {
            return B00tVariant::Nsfw0r1d;
        }
        if content.contains("core") {
            return B00tVariant::Core;
        }
    }

    // 3. Inspiration file content check
    let insp_path = b00t_path.join("r3src_资源/inspiration.yaml");
    if let Ok(content) = std::fs::read_to_string(&insp_path) {
        if content.contains("\"anal\"")
            || content.contains("\"buttstuff\"")
            || content.contains("\"penetrate\"")
        {
            return B00tVariant::Nsfw0r1d;
        }
    }

    // 4. Default
    B00tVariant::Core
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_env_override_core() {
        // SAFETY: test-only env var modification in single-threaded test context
        unsafe { std::env::set_var("B00T_VARIANT", "core") };
        assert_eq!(
            detect_variant(&PathBuf::from("/nonexistent")),
            B00tVariant::Core
        );
        unsafe { std::env::remove_var("B00T_VARIANT") };
    }

    #[test]
    fn test_env_override_nsfw() {
        // SAFETY: test-only env var modification in single-threaded test context
        unsafe { std::env::set_var("B00T_VARIANT", "nsfw0r1d") };
        assert_eq!(
            detect_variant(&PathBuf::from("/nonexistent")),
            B00tVariant::Nsfw0r1d
        );
        unsafe { std::env::remove_var("B00T_VARIANT") };
    }

    #[test]
    fn test_variant_toml_core() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("_b00t_/schema/variant.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"[b00t.variant]\nname = \"core\"\n")
            .unwrap();

        assert_eq!(
            detect_variant(&dir.path().to_path_buf()),
            B00tVariant::Core
        );
    }

    #[test]
    fn test_variant_toml_nsfw() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("_b00t_/schema/variant.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"[b00t.variant]\nname = \"nsfw0r1d\"\n")
            .unwrap();

        assert_eq!(
            detect_variant(&dir.path().to_path_buf()),
            B00tVariant::Nsfw0r1d
        );
    }

    #[test]
    fn test_default_is_core() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            detect_variant(&dir.path().to_path_buf()),
            B00tVariant::Core
        );
    }

    #[test]
    fn test_features() {
        assert_eq!(B00tVariant::Core.features(), &["sfw", "no-crypto"]);
        assert_eq!(
            B00tVariant::Nsfw0r1d.features(),
            &["nsfw-inspiration", "crypto-enabled"]
        );
    }
}
