use crate::traits::*;
use crate::{BootDatum, check_command_available, get_expanded_path};
use anyhow::{Context, Result};
use b00t_c0re_lib::datum_ai_model::{AiModelDatum, ModelProvider};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct AiModelConfig {
    pub b00t: BootDatum,
    pub ai_model: AiModelDatum,
}

/// Datum wrapper that binds BootDatum metadata with AI model configuration.
pub struct AiModelDatumEntry {
    pub datum: BootDatum,
    pub model: AiModelDatum,
    pub path: PathBuf,
}

impl AiModelDatumEntry {
    pub fn from_config(name: &str, base_path: &str) -> Result<Self> {
        let mut resolved = get_expanded_path(base_path)?;
        resolved.push(format!("{}.ai_model.toml", name));
        if !resolved.exists() {
            anyhow::bail!("AI model '{}' not found at {}", name, resolved.display());
        }
        Self::from_file(&resolved)
    }

    pub fn from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read AI model datum {}", path.display()))?;
        let config: AiModelConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse AI model datum {}", path.display()))?;
        Ok(Self {
            datum: config.b00t,
            model: config.ai_model,
            path: path.to_path_buf(),
        })
    }

    pub fn cache_dir(&self) -> Option<PathBuf> {
        self.lookup_env("VLLM_MODEL_DIR")
            .or_else(|| self.metadata_value("local_cache"))
            .or_else(|| self.metadata_value("cache_dir"))
    }

    pub fn container_path(&self) -> Option<String> {
        self.env_value("VLLM_MODEL_PATH")
            .or_else(|| self.model.metadata.get("container_mount").cloned())
    }

    pub fn dtype(&self) -> Option<String> {
        self.env_value("VLLM_DTYPE")
            .or_else(|| self.model.metadata.get("dtype").cloned())
    }

    pub fn huggingface_repo(&self) -> Option<String> {
        if let Some(repo) = self.model.metadata.get("hf_repo") {
            return Some(repo.clone());
        }
        if let Some(repo) = self.model.metadata.get("repo") {
            return Some(repo.clone());
        }
        if let ModelProvider::HuggingFace = self.model.provider {
            let id = self.model.litellm_model.trim();
            if let Some(stripped) = id.strip_prefix("huggingface/") {
                return Some(stripped.to_string());
            }
            return Some(id.to_string());
        }
        None
    }

    fn env_value(&self, key: &str) -> Option<String> {
        self.datum.env.as_ref()?.get(key).cloned()
    }

    fn lookup_env(&self, key: &str) -> Option<PathBuf> {
        self.env_value(key)
            .map(|raw| shellexpand::tilde(&raw).to_string())
            .map(PathBuf::from)
    }

    fn metadata_value(&self, key: &str) -> Option<PathBuf> {
        self.model
            .metadata
            .get(key)
            .cloned()
            .map(|raw| shellexpand::tilde(&raw).to_string())
            .map(PathBuf::from)
    }

    fn directory_has_content(path: &Path) -> bool {
        fs::read_dir(path)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false)
    }
}

impl TryFrom<(&str, &str)> for AiModelDatumEntry {
    type Error = anyhow::Error;

    fn try_from((name, path): (&str, &str)) -> Result<Self, Self::Error> {
        Self::from_config(name, path)
    }
}

impl DatumChecker for AiModelDatumEntry {
    fn is_installed(&self) -> bool {
        self.cache_dir()
            .filter(|dir| dir.exists() && Self::directory_has_content(dir))
            .is_some()
    }

    fn current_version(&self) -> Option<String> {
        if self.is_installed() {
            self.dtype().or_else(|| Some("cached".to_string()))
        } else {
            None
        }
    }

    fn desired_version(&self) -> Option<String> {
        self.datum
            .desires
            .clone()
            .or_else(|| self.model.metadata.get("revision").cloned())
    }

    fn version_status(&self) -> VersionStatus {
        if self.is_installed() {
            VersionStatus::Match
        } else {
            VersionStatus::Missing
        }
    }
}

impl StatusProvider for AiModelDatumEntry {
    fn name(&self) -> &str {
        &self.datum.name
    }

    fn subsystem(&self) -> &str {
        "ai-model"
    }

    fn hint(&self) -> &str {
        &self.datum.hint
    }

    fn is_disabled(&self) -> bool {
        self.huggingface_repo().is_none()
    }
}

impl FilterLogic for AiModelDatumEntry {
    fn is_available(&self) -> bool {
        !DatumChecker::is_installed(self) && self.prerequisites_satisfied()
    }

    fn prerequisites_satisfied(&self) -> bool {
        if self.huggingface_repo().is_none() {
            return false;
        }
        if matches!(self.model.provider, ModelProvider::HuggingFace) {
            return check_command_available("huggingface-cli");
        }
        true
    }

    fn evaluate_constraints(&self, require: &[String]) -> bool {
        self.evaluate_constraints_default(require)
    }
}

impl ConstraintEvaluator for AiModelDatumEntry {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

impl DatumProvider for AiModelDatumEntry {
    fn datum(&self) -> &BootDatum {
        &self.datum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> AiModelDatumEntry {
        let toml = r#"
            [b00t]
            name = "vision-test"
            type = "aimodel"
            hint = "Vision test model"

            [b00t.env]
            VLLM_MODEL_DIR = "~/tmp/vision-test"
            VLLM_MODEL_PATH = "/models/test"
            VLLM_DTYPE = "bfloat16"

            [ai_model]
            provider = "huggingface"
            size = "small"
            capabilities = ["vision"]
            litellm_model = "huggingface/example/model"

            [ai_model.metadata]
            hf_repo = "example/model"
            revision = "main"
        "#;
        let config: AiModelConfig = toml::from_str(toml).unwrap();
        AiModelDatumEntry {
            datum: config.b00t,
            model: config.ai_model,
            path: PathBuf::from("/tmp/vision-test.ai_model.toml"),
        }
    }

    #[test]
    fn repo_resolution_prefers_metadata() {
        let entry = sample_config();
        assert_eq!(entry.huggingface_repo().as_deref(), Some("example/model"));
    }

    #[test]
    fn cache_dir_expands_tilde() {
        let entry = sample_config();
        let dir = entry.cache_dir().unwrap();
        let home = dirs::home_dir().expect("HOME not set");
        assert!(dir.starts_with(&home));
        assert!(dir.ends_with("tmp/vision-test"));
    }

    #[test]
    fn installed_flag_checks_directory() {
        let mut entry = sample_config();
        let tempdir = tempfile::tempdir().unwrap();
        let file_path = tempdir.path().join("weights.bin");
        fs::write(&file_path, b"test").unwrap();
        entry.datum.env.get_or_insert_with(Default::default).insert(
            "VLLM_MODEL_DIR".to_string(),
            tempdir.path().to_string_lossy().to_string(),
        );
        assert!(entry.is_installed());
    }
}
