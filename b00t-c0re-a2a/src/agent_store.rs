use std::path::PathBuf;

use crate::agent_card::AgentCard;
use crate::error::{A2AError, A2AResult};

/// File-backed store for A2A agent cards.
///
/// Each agent is persisted as an individual JSON file in the store directory.
/// This provides low-overhead, human-readable persistent storage for agent
/// discovery without requiring a database.
#[derive(Debug, Clone)]
pub struct AgentStore {
    dir: PathBuf,
}

impl AgentStore {
    /// Create a new `AgentStore` rooted at the default location:
    /// `~/.local/share/b00t/agents/`
    pub fn new() -> A2AResult<Self> {
        let dir = dirs_data_dir()?.join("b00t").join("agents");
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Create a new `AgentStore` rooted at an explicit path.
    pub fn with_path(path: PathBuf) -> Self {
        Self { dir: path }
    }

    /// Return a reference to the store directory path.
    pub fn dir(&self) -> &PathBuf {
        &self.dir
    }

    /// Save (create or update) an agent card.
    ///
    /// The card is serialized to JSON and written to
    /// `{store_dir}/{agent_name}.json`.
    pub fn save(&self, card: &AgentCard) -> A2AResult<()> {
        std::fs::create_dir_all(&self.dir)?;
        let path = self.card_path(&card.name);
        let json = serde_json::to_string_pretty(card)?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    /// Load an agent card by name.
    ///
    /// Returns `Ok(None)` if no card with that name exists.
    pub fn load(&self, name: &str) -> A2AResult<Option<AgentCard>> {
        let path = self.card_path(name);
        if !path.exists() {
            return Ok(None);
        }
        let json = std::fs::read_to_string(&path)?;
        let card = serde_json::from_str(&json)?;
        Ok(Some(card))
    }

    /// List all agent cards in the store.
    pub fn list(&self) -> A2AResult<Vec<AgentCard>> {
        let mut cards = Vec::new();
        if !self.dir.exists() {
            return Ok(cards);
        }
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                let json = std::fs::read_to_string(&path)?;
                match serde_json::from_str::<AgentCard>(&json) {
                    Ok(card) => cards.push(card),
                    Err(e) => {
                        // Skip malformed files but surface the error
                        eprintln!("Warning: skipping malformed agent card at {}: {}", path.display(), e);
                    }
                }
            }
        }
        Ok(cards)
    }

    /// Delete an agent card by name.
    ///
    /// Returns `A2AError::AgentNotFound` if it doesn't exist.
    pub fn delete(&self, name: &str) -> A2AResult<()> {
        let path = self.card_path(name);
        if path.exists() {
            std::fs::remove_file(&path)?;
            Ok(())
        } else {
            Err(A2AError::AgentNotFound(name.to_string()))
        }
    }

    /// Search agents whose skills match the given skill name (case-insensitive).
    pub fn search_by_skill(&self, skill_name: &str) -> A2AResult<Vec<AgentCard>> {
        let skill_lower = skill_name.to_lowercase();
        let cards = self.list()?;
        Ok(cards
            .into_iter()
            .filter(|card| {
                card.skills
                    .iter()
                    .any(|s| s.name.to_lowercase().contains(&skill_lower) || s.id.to_lowercase() == skill_lower)
            })
            .collect())
    }

    /// Count the number of registered agents.
    pub fn count(&self) -> A2AResult<usize> {
        Ok(self.list()?.len())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn card_path(&self, name: &str) -> PathBuf {
        // Sanitize name for filesystem safety: replace non-alphanumeric chars
        let safe_name: String = name
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        self.dir.join(format!("{}.json", safe_name))
    }
}

impl Default for AgentStore {
    fn default() -> Self {
        // Best-effort default — panics if no home dir is available.
        // Use `new()` for a fallible version.
        let dir = dirs_data_dir().unwrap_or_else(|_| PathBuf::from("/tmp/b00t/agents"));
        Self { dir: dir.join("b00t").join("agents") }
    }
}

/// Returns the platform-appropriate data directory.
fn dirs_data_dir() -> A2AResult<PathBuf> {
    if let Some(dir) = dirs::data_dir() {
        Ok(dir)
    } else {
        // Fallback: use XDG_DATA_HOME or temp
        std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
            .map_err(|_| A2AError::RuntimeError("Cannot determine data directory. Set HOME or XDG_DATA_HOME.".to_string()))
    }
}

// The `dirs` crate provides `data_dir()` on all platforms.
// We use it here via a simple inline check rather than adding a dep.
// If `dirs` isn't available, we fall back to env vars.
mod dirs {
    use std::path::PathBuf;

    #[cfg(target_os = "linux")]
    pub fn data_dir() -> Option<PathBuf> {
        std::env::var("XDG_DATA_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".local").join("share")))
    }

    #[cfg(not(target_os = "linux"))]
    pub fn data_dir() -> Option<PathBuf> {
        // Fallback: use HOME
        std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(".local").join("share"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_card::{AuthenticationScheme, Skill};
    use url::Url;

    /// Create a temporary store for testing.
    fn setup_store() -> (AgentStore, PathBuf) {
        let tmpdir = std::env::temp_dir().join(format!(
            "b00t_a2a_test_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&tmpdir).unwrap();
        let store = AgentStore::with_path(tmpdir.join("agents"));
        (store, tmpdir)
    }

    fn sample_card(name: &str) -> AgentCard {
        let url = Url::parse("stdio://test").unwrap();
        AgentCard::new(name, &format!("Agent {}", name), url)
            .with_skill(Skill::new("s1", "Skill 1", "Does stuff", serde_json::json!({}), serde_json::json!({})))
            .with_auth(AuthenticationScheme::none())
    }

    #[test]
    fn test_save_and_load() {
        let (store, _tmp) = setup_store();
        let card = sample_card("test-save");
        store.save(&card).unwrap();

        let loaded = store.load("test-save").unwrap().expect("card should exist");
        assert_eq!(loaded.name, "test-save");
        assert_eq!(loaded.skills.len(), 1);
        assert_eq!(loaded.authentication.len(), 1);
    }

    #[test]
    fn test_load_missing() {
        let (store, _tmp) = setup_store();
        let result = store.load("does-not-exist").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_list() {
        let (store, _tmp) = setup_store();
        store.save(&sample_card("alpha")).unwrap();
        store.save(&sample_card("beta")).unwrap();
        let cards = store.list().unwrap();
        assert_eq!(cards.len(), 2);
    }

    #[test]
    fn test_delete() {
        let (store, _tmp) = setup_store();
        store.save(&sample_card("to-delete")).unwrap();
        assert!(store.load("to-delete").unwrap().is_some());
        store.delete("to-delete").unwrap();
        assert!(store.load("to-delete").unwrap().is_none());
    }

    #[test]
    fn test_delete_missing() {
        let (store, _tmp) = setup_store();
        let err = store.delete("ghost").unwrap_err();
        assert!(matches!(err, A2AError::AgentNotFound(_)));
    }

    #[test]
    fn test_count() {
        let (store, _tmp) = setup_store();
        assert_eq!(store.count().unwrap(), 0);
        store.save(&sample_card("a")).unwrap();
        assert_eq!(store.count().unwrap(), 1);
        store.save(&sample_card("b")).unwrap();
        assert_eq!(store.count().unwrap(), 2);
    }

    #[test]
    fn test_search_by_skill() {
        let (store, _tmp) = setup_store();
        store.save(&sample_card("coder")).unwrap();
        let results = store.search_by_skill("Skill 1").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "coder");
    }

    #[test]
    fn test_sanitized_filename() {
        let (store, tmpdir) = setup_store();
        let card = sample_card("my cool agent!@#");
        store.save(&card).unwrap();
        // The filename should be sanitized
        let dir_path = tmpdir.join("agents");
        let entries: Vec<_> = std::fs::read_dir(&dir_path).unwrap().collect();
        assert_eq!(entries.len(), 1);
        let fname = entries[0].as_ref().unwrap().file_name().into_string().unwrap();
        assert!(!fname.contains('!'));
        assert!(fname.ends_with(".json"));
    }
}
