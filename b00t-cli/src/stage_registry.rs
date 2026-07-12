// 🤓 Stage registry — in-memory index of known pipeline stages
//    discovered from _b00t_/*.stage.tomllm files.
//
//    CapsuleProfile docs: https://github.com/elasticdotventures/dotfiles/issues/739
//    Each .stage.tomllm file is a serialized CapsuleProfile (name, ports, resources, etc.)

use crate::pipeline_types::CapsuleProfile;
use anyhow::Result;
use std::path::Path;

/// In-memory registry of known pipeline stages, discovered from
/// `_b00t_/*.stage.tomllm` files on the filesystem.
#[derive(Debug, Clone)]
pub struct StageRegistry {
    pub stages: Vec<CapsuleProfile>,
}

impl StageRegistry {
    /// Create an empty registry (no stages loaded).
    pub fn empty() -> Self {
        Self { stages: Vec::new() }
    }

    /// Scan `_b00t_/*.stage.tomllm` files and parse each as a
    /// `CapsuleProfile`.  Non-existent directories or parse errors
    /// are silently skipped — the registry will simply be empty.
    pub fn discover(b00t_path: &str) -> Self {
        let expanded = shellexpand::tilde(b00t_path).to_string();
        let dir = Path::new(&expanded);
        if !dir.is_dir() {
            return Self::empty();
        }

        let mut stages: Vec<CapsuleProfile> = Vec::new();
        let pattern = dir.join("*.stage.tomllm");

        // glob is re-exported via b00t-cli's glob crate dependency
        if let Ok(entries) = glob::glob(pattern.to_str().unwrap_or("")) {
            for entry in entries.flatten() {
                match std::fs::read_to_string(&entry) {
                    Ok(content) => {
                        match toml::from_str::<CapsuleProfile>(&content) {
                            Ok(profile) => stages.push(profile),
                            Err(e) => {
                                eprintln!(
                                    "  ⚠️  stage_registry: skipping {} — {}",
                                    entry.display(),
                                    e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "  ⚠️  stage_registry: cannot read {} — {}",
                            entry.display(),
                            e
                        );
                    }
                }
            }
        }

        // Stable sort by name for deterministic listing
        stages.sort_by(|a, b| a.name.cmp(&b.name));
        Self { stages }
    }

    /// Case-insensitive substring search on stage name and port
    /// media types (`Debug` representation).
    pub fn search(&self, query: &str) -> Vec<&CapsuleProfile> {
        let q = query.to_ascii_lowercase();
        self.stages
            .iter()
            .filter(|s| {
                if s.name.to_ascii_lowercase().contains(&q) {
                    return true;
                }
                for port in &s.ports {
                    let media = format!("{:?}", port.media_type).to_ascii_lowercase();
                    if media.contains(&q) {
                        return true;
                    }
                }
                false
            })
            .collect()
    }

    /// List all stages, optionally filtering by port media type
    /// (case-insensitive substring match on the `Debug` name).
    pub fn list(&self, filter: Option<&str>) -> Vec<&CapsuleProfile> {
        match filter {
            None => self.stages.iter().collect(),
            Some(f) => {
                let f = f.to_ascii_lowercase();
                self.stages
                    .iter()
                    .filter(|s| {
                        s.ports
                            .iter()
                            .any(|p| format!("{:?}", p.media_type).to_ascii_lowercase() == f)
                    })
                    .collect()
            }
        }
    }

    /// Exact name match — returns the stage profile if found.
    pub fn get(&self, name: &str) -> Option<&CapsuleProfile> {
        self.stages.iter().find(|s| s.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_types::{PortDirection, PortMediaType, ResourceRequirements};
    use tempfile::TempDir;

    fn sample_profile(name: &str) -> CapsuleProfile {
        CapsuleProfile {
            name: name.to_string(),
            ports: vec![],
            resources: ResourceRequirements {
                min_ram_gb: 1.0,
                min_vram_gb: 0.0,
                requires_gpu: false,
                cpu_cores: None,
                scratch_disk_gb: None,
            },
            image: None,
            timeout_seconds: None,
        }
    }

    fn profile_with_ports(name: &str, ports: Vec<(PortDirection, PortMediaType)>) -> CapsuleProfile {
        CapsuleProfile {
            name: name.to_string(),
            ports: ports
                .into_iter()
                .map(|(direction, media_type)| crate::pipeline_types::StagePort {
                    direction,
                    media_type,
                    description: None,
                })
                .collect(),
            resources: ResourceRequirements {
                min_ram_gb: 1.0,
                min_vram_gb: 0.0,
                requires_gpu: false,
                cpu_cores: None,
                scratch_disk_gb: None,
            },
            image: None,
            timeout_seconds: None,
        }
    }

    fn write_stage_tomllm(dir: &TempDir, profile: &CapsuleProfile) {
        let path = dir.path().join(format!("{}.stage.tomllm", profile.name));
        let toml_str = toml::to_string(profile).unwrap();
        std::fs::write(&path, toml_str).unwrap();
    }

    #[test]
    fn empty_registry_has_no_stages() {
        let reg = StageRegistry::empty();
        assert!(reg.stages.is_empty());
        assert!(reg.list(None).is_empty());
        assert!(reg.search("anything").is_empty());
        assert!(reg.get("anything").is_none());
    }

    #[test]
    fn discover_loads_stage_tomllm_files() {
        let dir = TempDir::new().unwrap();
        write_stage_tomllm(&dir, &sample_profile("encode"));
        write_stage_tomllm(&dir, &sample_profile("transcode"));

        let reg = StageRegistry::discover(dir.path().to_str().unwrap());
        assert_eq!(reg.stages.len(), 2);
        assert_eq!(reg.stages[0].name, "encode");
        assert_eq!(reg.stages[1].name, "transcode");
    }

    #[test]
    fn discover_sorts_by_name() {
        let dir = TempDir::new().unwrap();
        write_stage_tomllm(&dir, &sample_profile("z-final"));
        write_stage_tomllm(&dir, &sample_profile("alpha"));
        write_stage_tomllm(&dir, &sample_profile("beta"));

        let reg = StageRegistry::discover(dir.path().to_str().unwrap());
        assert_eq!(reg.stages.len(), 3);
        assert_eq!(reg.stages[0].name, "alpha");
        assert_eq!(reg.stages[1].name, "beta");
        assert_eq!(reg.stages[2].name, "z-final");
    }

    #[test]
    fn discover_skips_invalid_toml() {
        let dir = TempDir::new().unwrap();
        // Valid stage
        write_stage_tomllm(&dir, &sample_profile("valid-stage"));
        // Invalid TOML
        std::fs::write(
            dir.path().join("invalid.stage.tomllm"),
            "this is not valid toml {{{",
        )
        .unwrap();

        let reg = StageRegistry::discover(dir.path().to_str().unwrap());
        assert_eq!(reg.stages.len(), 1);
        assert_eq!(reg.stages[0].name, "valid-stage");
    }

    #[test]
    fn discover_nonexistent_dir_returns_empty() {
        let reg = StageRegistry::discover("/tmp/__nonexistent_b00t_stage_test__");
        assert!(reg.stages.is_empty());
    }

    #[test]
    fn search_matches_name_substring_case_insensitive() {
        let reg = StageRegistry {
            stages: vec![
                sample_profile("VideoIngest"),
                sample_profile("AudioTranscode"),
                sample_profile("ImageResize"),
            ],
        };

        let results = reg.search("video");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "VideoIngest");

        let results = reg.search("trans");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "AudioTranscode");
    }

    #[test]
    fn search_matches_port_media_type() {
        let reg = StageRegistry {
            stages: vec![
                profile_with_ports(
                    "video-source",
                    vec![(PortDirection::Output, PortMediaType::Video)],
                ),
                profile_with_ports(
                    "audio-source",
                    vec![(PortDirection::Output, PortMediaType::Audio)],
                ),
            ],
        };

        let results = reg.search("audio");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "audio-source");
    }

    #[test]
    fn search_returns_empty_when_no_match() {
        let reg = StageRegistry {
            stages: vec![sample_profile("VideoIngest")],
        };
        assert!(reg.search("nonexistent").is_empty());
    }

    #[test]
    fn list_without_filter_returns_all() {
        let reg = StageRegistry {
            stages: vec![
                sample_profile("a"),
                sample_profile("b"),
                sample_profile("c"),
            ],
        };
        assert_eq!(reg.list(None).len(), 3);
    }

    #[test]
    fn list_filters_by_media_type_exact() {
        let reg = StageRegistry {
            stages: vec![
                profile_with_ports(
                    "video-src",
                    vec![(PortDirection::Output, PortMediaType::Video)],
                ),
                profile_with_ports(
                    "audio-src",
                    vec![(PortDirection::Output, PortMediaType::Audio)],
                ),
                profile_with_ports(
                    "video-proc",
                    vec![
                        (PortDirection::Input, PortMediaType::Video),
                        (PortDirection::Output, PortMediaType::Video),
                    ],
                ),
            ],
        };

        let results = reg.list(Some("Video"));
        assert_eq!(results.len(), 2);
        for r in &results {
            assert!(r.name.starts_with("video"));
        }
    }

    #[test]
    fn list_filter_no_match_returns_empty() {
        let reg = StageRegistry {
            stages: vec![sample_profile("only-text")],
        };
        assert!(reg.list(Some("Video")).is_empty());
    }

    #[test]
    fn get_exact_name_match() {
        let reg = StageRegistry {
            stages: vec![
                sample_profile("encode"),
                sample_profile("transcode"),
            ],
        };
        assert!(reg.get("encode").is_some());
        assert_eq!(reg.get("encode").unwrap().name, "encode");
    }

    #[test]
    fn get_no_match_returns_none() {
        let reg = StageRegistry {
            stages: vec![sample_profile("encode")],
        };
        assert!(reg.get("transcode").is_none());
    }

    #[test]
    fn get_is_case_sensitive() {
        let reg = StageRegistry {
            stages: vec![sample_profile("Encode")],
        };
        assert!(reg.get("encode").is_none());
        assert!(reg.get("Encode").is_some());
    }
}
