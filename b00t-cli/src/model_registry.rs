//! Local model registry — overlay datum store for AI model endpoints.
//!
//! Primary storage: `${_B00T_Path}/models.overlay.toml` (overlay datum, committed to enclave branch).
//! Legacy fallback: `~/._b00t_/models.toml` (global, gitignored) + `.b00t/models.toml` (project-local).
//! Overlay datum is the source of truth; legacy paths provide backward compat on first load.
//!
//! Uses `b00t_c0re_lib::datum_ai_model::ModelRegistry` as the data model.
//! Classification by size (`Small`/`Large`) and cost (via `metadata["cost"]`).

use anyhow::{Context, Result};
use b00t_c0re_lib::datum_ai_model::{
    AiModelDatum, ModelCapability, ModelProvider, ModelRegistry, ModelSize,
};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// ─── Path resolution ─────────────────────────────────────────────────────────

/// Overlay datum path — `${_B00T_Path}/models.overlay.toml` (primary, enclave-committed).
pub fn overlay_registry_path() -> PathBuf {
    let b00t_path = std::env::var("_B00T_Path").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        format!("{home}/.b00t/_b00t_")
    });
    let expanded = shellexpand::tilde(&b00t_path).to_string();
    PathBuf::from(expanded).join("models.overlay.toml")
}

/// Legacy global registry path — `~/._b00t_/models.toml` (gitignored, user-scoped).
pub fn global_registry_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join("._b00t_").join("models.toml")
}

/// Project-local registry path — `.b00t/models.toml` (gitignored, project-scoped).
pub fn project_registry_path() -> PathBuf {
    PathBuf::from(".b00t/models.toml")
}

// ─── Load / Save ─────────────────────────────────────────────────────────────

/// Load effective registry: merge overlay + legacy global + project-local.
/// Priority: overlay > project-local > global (first definition wins).
pub fn load_registry() -> ModelRegistry {
    let overlay = load_from_path(&overlay_registry_path());
    let global = load_from_path(&global_registry_path());
    let local = load_from_path(&project_registry_path());
    // Merge: global is base, local overrides global, overlay overrides all
    let merged_legacy = merge_registries(global, local);
    merge_registries(merged_legacy, overlay)
}

fn load_from_path(path: &Path) -> ModelRegistry {
    match fs::read_to_string(path) {
        Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
            eprintln!("⚠️  model registry parse error in {}: {e}", path.display());
            ModelRegistry::new()
        }),
        Err(_) => ModelRegistry::new(),
    }
}

/// Save registry to the overlay datum path (creates parent dir if needed).
/// Appends a `b00t:map` tail block for datum discoverability.
pub fn save_registry(registry: &ModelRegistry) -> Result<()> {
    let path = overlay_registry_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }
    let toml_str = toml::to_string_pretty(registry)
        .context("serialize model registry to TOML")?;
    let content = format!(
        "{toml_str}\n\
         # ── b00t:map v1 ──────────────────────────────────────────────\n\
         # summary: node-local model registry overlay — AI endpoints for this node\n\
         # tags: model, registry, overlay, ai\n\
         # type: overlay\n\
         # b00t.overlay: true\n"
    );
    fs::write(&path, content)
        .with_context(|| format!("write registry to {}", path.display()))?;
    Ok(())
}

/// Merge `overlay` into `base` — overlay models replace base by key.
fn merge_registries(base: ModelRegistry, overlay: ModelRegistry) -> ModelRegistry {
    let mut merged = base;
    for (name, datum) in overlay.models {
        merged.models.insert(name, datum);
    }
    for (provider, defaults) in overlay.provider_defaults {
        merged.provider_defaults.insert(provider, defaults);
    }
    merged
}

// ─── CRUD operations ─────────────────────────────────────────────────────────

/// Register a model endpoint in the local registry.
///
/// Ergonomic entry point for CLI: `b00t model register <name> --endpoint ... --model ...`
#[allow(clippy::too_many_arguments)]
pub fn register_model(
    name: &str,
    endpoint: &str,
    model: &str,
    provider: ModelProvider,
    size: ModelSize,
    api_key_env: Option<&str>,
    context_window: Option<u32>,
    capabilities: Vec<ModelCapability>,
    cost: Option<&str>,
    tier: Option<&str>,
) -> Result<()> {
    let mut registry = load_registry();

    let mut metadata = HashMap::new();
    if let Some(cost) = cost {
        metadata.insert("cost".to_string(), cost.to_string());
    }
    if let Some(tier) = tier {
        metadata.insert("tier".to_string(), tier.to_string());
    }

    let datum = AiModelDatum {
        provider,
        size,
        capabilities,
        litellm_model: model.to_string(),
        api_base: Some(endpoint.to_string()),
        api_key_env: api_key_env.map(|s| s.to_string()),
        parameters: HashMap::new(),
        metadata,
        rpm_limit: None,
        context_window,
        enabled: true,
        access_groups: vec![],
    };

    let was_update = registry.models.contains_key(name);
    registry.add_model(name.to_string(), datum);
    save_registry(&registry)?;

    if was_update {
        println!("✅ updated model '{name}' in registry");
    } else {
        println!("✅ registered model '{name}' in registry");
    }
    Ok(())
}

/// Enable a model (sets `enabled = true`).
pub fn enable_model(name: &str) -> Result<()> {
    let mut registry = load_registry();
    match registry.models.get_mut(name) {
        Some(datum) => {
            datum.enabled = true;
            save_registry(&registry)?;
            println!("✅ enabled model '{name}'");
            Ok(())
        }
        None => anyhow::bail!("Model '{}' not found in registry", name),
    }
}

/// Disable a model (sets `enabled = false`).
pub fn disable_model(name: &str) -> Result<()> {
    let mut registry = load_registry();
    match registry.models.get_mut(name) {
        Some(datum) => {
            datum.enabled = false;
            save_registry(&registry)?;
            println!("🚫 disabled model '{name}'");
            Ok(())
        }
        None => anyhow::bail!("Model '{}' not found in registry", name),
    }
}

/// Remove a model from the registry entirely.
pub fn remove_model(name: &str) -> Result<()> {
    let mut registry = load_registry();
    match registry.remove_model(name) {
        Some(_) => {
            save_registry(&registry)?;
            println!("🗑️  removed model '{name}' from registry");
            Ok(())
        }
        None => anyhow::bail!("Model '{}' not found in registry", name),
    }
}

// ─── Listing ─────────────────────────────────────────────────────────────────

/// A flattened view of a registry entry for CLI display.
#[derive(Debug, Clone)]
pub struct RegistryEntry {
    pub name: String,
    pub endpoint: String,
    pub model: String,
    pub provider: String,
    pub size: String,
    pub cost: String,
    pub tier: String,
    pub ctx: Option<u32>,
    pub enabled: bool,
    pub source: EntrySource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntrySource {
    /// From `~/._b00t_/models.toml` or `.b00t/models.toml`
    Registry,
    /// From a `_b00t_/*.ai_model.toml` datum file
    Datum,
}

/// List all known models: registry entries + datum entries (deduped by name, registry wins).
pub fn list_all(b00t_path: &str) -> Vec<RegistryEntry> {
    let registry = load_registry();
    let mut entries: Vec<RegistryEntry> = registry
        .models
        .iter()
        .map(|(name, d)| datum_to_entry(name, d, EntrySource::Registry))
        .collect();

    // Merge datum-based models (registry takes precedence)
    if let Ok(datums) = crate::model_manager::list_models(b00t_path) {
        for dm in &datums {
            if !entries.iter().any(|e| e.name == dm.name) {
                entries.push(RegistryEntry {
                    name: dm.name.clone(),
                    endpoint: String::new(),
                    model: String::new(),
                    provider: dm.provider.clone(),
                    size: dm.size.clone(),
                    cost: String::new(),
                    tier: String::new(),
                    ctx: dm.context_window,
                    enabled: dm.installed,
                    source: EntrySource::Datum,
                });
            }
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn datum_to_entry(name: &str, d: &AiModelDatum, source: EntrySource) -> RegistryEntry {
    RegistryEntry {
        name: name.to_string(),
        endpoint: d.api_base.clone().unwrap_or_default(),
        model: d.litellm_model.clone(),
        provider: format!("{:?}", d.provider).to_lowercase(),
        size: format!("{:?}", d.size).to_lowercase(),
        cost: d.metadata.get("cost").cloned().unwrap_or_default(),
        tier: d.metadata.get("tier").cloned().unwrap_or_default(),
        ctx: d.context_window,
        enabled: d.enabled,
        source,
    }
}

/// Filter to enabled models only.
pub fn list_enabled(b00t_path: &str) -> Vec<RegistryEntry> {
    list_all(b00t_path)
        .into_iter()
        .filter(|e| e.enabled)
        .collect()
}

// ─── Env export ──────────────────────────────────────────────────────────────

/// Resolve `B00T_AI_*_BASE` and `B00T_SM0L_MODEL` env vars from the registry.
///
/// Picks the first enabled model matching the given tier (via `metadata["tier"]`).
/// Returns `(base_url, model_id)` or `None` if no match.
pub fn resolve_tier_endpoint(tier: &str) -> Option<(String, String)> {
    let registry = load_registry();
    for (_, d) in &registry.models {
        if d.enabled && d.metadata.get("tier").map(|t| t.as_str()) == Some(tier) {
            if let Some(base) = &d.api_base {
                return Some((base.clone(), d.litellm_model.clone()));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_path_resolves_under_b00t_path() {
        // overlay datum should live in _B00T_Path, not ~/._b00t_
        let path = overlay_registry_path();
        assert!(
            path.ends_with("models.overlay.toml"),
            "expected models.overlay.toml, got {}",
            path.display()
        );
    }

    #[test]
    fn legacy_global_path_still_resolves() {
        let path = global_registry_path();
        assert!(
            path.ends_with("models.toml"),
            "expected models.toml, got {}",
            path.display()
        );
    }

    #[test]
    fn registry_roundtrip() {
    }
}
