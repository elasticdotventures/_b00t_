//! Calorie tracking for all b00t commands.
//! Every command deducts calories based on the agent's tier.
//! Calories are tracked in the agent store (~/.local/share/b00t/agents/<name>_meta.json).

use b00t_c0re_gov::errors::{GovResult, GovernanceError};
use b00t_c0re_gov::scoring::AgentTier;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalorieRecord {
    pub agent_name: String,
    pub tier: AgentTier,
    pub calories_remaining: f64,
    pub total_burned: f64,
    pub is_alive: bool,
}

pub struct CalorieTracker {
    store_dir: PathBuf,
}

impl CalorieTracker {
    /// Create a new CalorieTracker, storing records under
    /// ~/.local/share/b00t/agents/
    pub fn new() -> Self {
        let base = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("b00t")
            .join("agents");
        Self { store_dir: base }
    }

    /// Set a custom store directory (useful for tests).
    pub fn with_store_dir(dir: PathBuf) -> Self {
        Self { store_dir: dir }
    }

    /// Path to the JSON record file for a given agent.
    fn record_path(&self, agent: &str) -> PathBuf {
        self.store_dir.join(format!("{}_meta.json", agent))
    }

    /// Execute a function with calorie tracking.
    /// Deducts `base_cost * tier.multiplier()` before executing.
    /// If insufficient calories, returns InsufficientCalories error.
    pub fn execute_with_calories<F, T>(
        &self,
        agent: &str,
        tier: AgentTier,
        base_cost: f64,
        op: F,
    ) -> GovResult<T>
    where
        F: FnOnce() -> GovResult<T>,
    {
        let cost = base_cost * tier.calorie_multiplier();
        let mut record = self.load_or_create(agent, tier)?;

        if record.calories_remaining < cost {
            return Err(GovernanceError::InsufficientCalories {
                available: record.calories_remaining,
                required: cost,
            });
        }

        record.calories_remaining -= cost;
        record.total_burned += cost;
        record.is_alive = record.calories_remaining > 0.0;
        self.save(&record)?;

        let result = op()?;

        Ok(result)
    }

    /// Get current calorie record for an agent.
    pub fn get_record(&self, agent: &str) -> GovResult<Option<CalorieRecord>> {
        let path = self.record_path(agent);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let record: CalorieRecord = serde_json::from_str(&content).map_err(|e| {
            GovernanceError::ContextCorrupt(format!(
                "Failed to parse calorie record for '{}': {}",
                agent, e
            ))
        })?;
        Ok(Some(record))
    }

    /// Add calories (from cake conversion or grants).
    pub fn add_calories(&self, agent: &str, amount: f64) -> GovResult<()> {
        let mut record = self.load_or_create(agent, AgentTier::SLM)?;
        record.calories_remaining += amount;
        record.is_alive = record.calories_remaining > 0.0;
        self.save(&record)
    }

    /// Check if agent is alive (has calories remaining).
    pub fn is_alive(&self, agent: &str) -> GovResult<bool> {
        let record = self.load_or_create(agent, AgentTier::SLM)?;
        Ok(record.is_alive)
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn load_or_create(&self, agent: &str, tier: AgentTier) -> GovResult<CalorieRecord> {
        let path = self.record_path(agent);
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let record: CalorieRecord = serde_json::from_str(&content).map_err(|e| {
                GovernanceError::ContextCorrupt(format!(
                    "Failed to parse calorie record for '{}': {}",
                    agent, e
                ))
            })?;
            Ok(record)
        } else {
            // Create a new record with default calorie budget
            let record = CalorieRecord {
                agent_name: agent.to_string(),
                tier,
                calories_remaining: 1000.0,
                total_burned: 0.0,
                is_alive: true,
            };
            self.save(&record)?;
            Ok(record)
        }
    }

    fn save(&self, record: &CalorieRecord) -> GovResult<()> {
        std::fs::create_dir_all(&self.store_dir)?;
        let path = self.record_path(&record.agent_name);
        let content = serde_json::to_string_pretty(record).map_err(|e| {
            GovernanceError::ContextCorrupt(format!(
                "Failed to serialize calorie record for '{}': {}",
                record.agent_name, e
            ))
        })?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_tracker() -> (CalorieTracker, TempDir) {
        let tmp = TempDir::new().unwrap();
        let tracker = CalorieTracker::with_store_dir(tmp.path().to_path_buf());
        (tracker, tmp)
    }

    #[test]
    fn test_new_record_creation() {
        let (tracker, _tmp) = setup_tracker();
        let record = tracker
            .load_or_create("test-agent", AgentTier::SLM)
            .unwrap();
        assert_eq!(record.agent_name, "test-agent");
        assert!((record.calories_remaining - 1000.0).abs() < f64::EPSILON);
        assert!(record.is_alive);
    }

    #[test]
    fn test_execute_with_sufficient_calories() {
        let (tracker, _tmp) = setup_tracker();
        let result = tracker.execute_with_calories("worker-1", AgentTier::SLM, 10.0, || {
            Ok::<_, GovernanceError>(42)
        });
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        let record = tracker.get_record("worker-1").unwrap().unwrap();
        assert!((record.calories_remaining - 990.0).abs() < f64::EPSILON);
        assert!((record.total_burned - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_execute_with_tier_multiplier() {
        let (tracker, _tmp) = setup_tracker();
        // GAI tier: 100x multiplier on base_cost=10 => 1000 calorie cost
        let result = tracker.execute_with_calories("gai-agent", AgentTier::GAI, 10.0, || {
            Ok::<_, GovernanceError>("done")
        });
        assert!(result.is_ok());

        let record = tracker.get_record("gai-agent").unwrap().unwrap();
        // 1000 initial - (10 * 100) = 0
        assert!((record.calories_remaining - 0.0).abs() < f64::EPSILON);
        assert!((record.total_burned - 1000.0).abs() < f64::EPSILON);
        assert!(!record.is_alive); // 0 calories = dead
    }

    #[test]
    fn test_insufficient_calories_error() {
        let (tracker, _tmp) = setup_tracker();
        // Algorithmic tier: 0.01x multiplier on base_cost=200_000 => 2000 cost
        // Starting with 1000 calories, 2000 required → InsufficientCalories
        let result =
            tracker.execute_with_calories("algo-agent", AgentTier::Algorithmic, 200_000.0, || {
                Ok::<_, GovernanceError>("should not run")
            });
        assert!(result.is_err());
        match result.unwrap_err() {
            GovernanceError::InsufficientCalories {
                available,
                required,
            } => {
                assert!((available - 1000.0).abs() < f64::EPSILON);
                assert!((required - 2000.0).abs() < f64::EPSILON); // 200_000 * 0.01 = 2000
            }
            e => panic!("Expected InsufficientCalories, got: {}", e),
        }
    }

    #[test]
    fn test_add_calories() {
        let (tracker, _tmp) = setup_tracker();
        tracker.add_calories("hungry-agent", 50.0).unwrap();
        let record = tracker.get_record("hungry-agent").unwrap().unwrap();
        assert!((record.calories_remaining - 1050.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_is_alive() {
        let (tracker, _tmp) = setup_tracker();
        assert!(tracker.is_alive("fresh-agent").unwrap());

        // Burn all calories
        let _ = tracker.execute_with_calories("dead-agent", AgentTier::GAI, 10.0, || {
            Ok::<_, GovernanceError>("boom")
        });
        assert!(!tracker.is_alive("dead-agent").unwrap());

        // Add calories revives
        tracker.add_calories("dead-agent", 1.0).unwrap();
        assert!(tracker.is_alive("dead-agent").unwrap());
    }

    #[test]
    fn test_execute_preserves_record_on_failure() {
        let (tracker, _tmp) = setup_tracker();
        // Execute once successfully
        let _: GovResult<i32> =
            tracker.execute_with_calories("fragile", AgentTier::SLM, 10.0, || Ok(42));
        let before = tracker.get_record("fragile").unwrap().unwrap();

        // Try again — the op fails but calories are already burned
        let result: GovResult<i32> =
            tracker.execute_with_calories("fragile", AgentTier::SLM, 10.0, || {
                Err(GovernanceError::GateNotFound("oops".into()))
            });
        assert!(result.is_err());

        let after = tracker.get_record("fragile").unwrap().unwrap();
        // Calories were burned even though the op failed
        assert!(after.calories_remaining < before.calories_remaining);
    }
}
