use anyhow::Result;
use b00t_c0re_gov::scope_store::ScopeStore;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SkillTier {
    Base,
    #[default]
    Escalatable,
    Restricted,
}

fn skill_tier_key(skill: &str) -> String {
    format!("capforge:skill:{skill}:tier")
}

fn skill_description_key(skill: &str) -> String {
    format!("capforge:skill:{skill}:description")
}

fn agent_pubkey_key(agent_id: &str) -> String {
    format!("capforge:agent:{agent_id}:pubkey")
}

fn agent_allowlist_key(agent_id: &str) -> String {
    format!("capforge:agent:{agent_id}:allowlist")
}

fn agent_suspended_key(agent_id: &str) -> String {
    format!("capforge:agent:{agent_id}:suspended")
}

pub fn get_skill_tier(store: &dyn ScopeStore, skill: &str) -> Result<SkillTier> {
    match store.get_raw(&skill_tier_key(skill))? {
        Some(v) => Ok(serde_json::from_value(v)?),
        None => Ok(SkillTier::default()),
    }
}

pub fn set_skill_tier(store: &mut dyn ScopeStore, skill: &str, tier: SkillTier) -> Result<()> {
    Ok(store.set_raw(&skill_tier_key(skill), serde_json::to_value(tier)?)?)
}

pub fn get_skill_description(store: &dyn ScopeStore, skill: &str) -> Result<String> {
    match store.get_raw(&skill_description_key(skill))? {
        Some(v) => Ok(serde_json::from_value(v)?),
        None => Ok(String::new()),
    }
}

pub fn set_skill_description(store: &mut dyn ScopeStore, skill: &str, description: &str) -> Result<()> {
    Ok(store.set_raw(&skill_description_key(skill), serde_json::to_value(description)?)?)
}

pub fn get_agent_pubkey(store: &dyn ScopeStore, agent_id: &str) -> Result<Option<String>> {
    match store.get_raw(&agent_pubkey_key(agent_id))? {
        Some(v) => Ok(Some(serde_json::from_value(v)?)),
        None => Ok(None),
    }
}

pub fn set_agent_pubkey(store: &mut dyn ScopeStore, agent_id: &str, pubkey: &str) -> Result<()> {
    Ok(store.set_raw(&agent_pubkey_key(agent_id), serde_json::to_value(pubkey)?)?)
}

pub fn get_agent_allowlist(store: &dyn ScopeStore, agent_id: &str) -> Result<Vec<String>> {
    match store.get_raw(&agent_allowlist_key(agent_id))? {
        Some(v) => Ok(serde_json::from_value(v)?),
        None => Ok(Vec::new()),
    }
}

pub fn add_to_allowlist(store: &mut dyn ScopeStore, agent_id: &str, skills: &[String]) -> Result<()> {
    let mut current = get_agent_allowlist(store, agent_id)?;
    for skill in skills {
        if !current.contains(skill) {
            current.push(skill.clone());
        }
    }
    Ok(store.set_raw(&agent_allowlist_key(agent_id), serde_json::to_value(current)?)?)
}

pub fn is_agent_suspended(store: &dyn ScopeStore, agent_id: &str) -> Result<bool> {
    match store.get_raw(&agent_suspended_key(agent_id))? {
        Some(v) => Ok(serde_json::from_value(v)?),
        None => Ok(false),
    }
}

pub fn set_agent_suspended(store: &mut dyn ScopeStore, agent_id: &str, suspended: bool) -> Result<()> {
    Ok(store.set_raw(&agent_suspended_key(agent_id), serde_json::to_value(suspended)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
    use b00t_c0re_gov::scope_store::ScopeId;

    fn store() -> RedbScopeStore {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.redb");
        // Leak the TempDir guard deliberately: RedbScopeStore::open keeps the
        // file open for the store's lifetime, and dropping the guard here
        // would delete the directory out from under it mid-test. Test-only;
        // /tmp cleanup on the CI/dev box handles the leak.
        std::mem::forget(dir);
        RedbScopeStore::open(path, ScopeId::Global, None).unwrap()
    }

    #[test]
    fn unknown_skill_defaults_to_escalatable() {
        let s = store();
        assert_eq!(get_skill_tier(&s, "skill.nonexistent").unwrap(), SkillTier::Escalatable);
    }

    #[test]
    fn skill_tier_round_trips() {
        let mut s = store();
        set_skill_tier(&mut s, "skill.delete-infra", SkillTier::Restricted).unwrap();
        assert_eq!(get_skill_tier(&s, "skill.delete-infra").unwrap(), SkillTier::Restricted);
    }

    #[test]
    fn allowlist_accumulates_without_duplicates() {
        let mut s = store();
        add_to_allowlist(&mut s, "agent-1", &["skill.read".into()]).unwrap();
        add_to_allowlist(&mut s, "agent-1", &["skill.read".into(), "skill.write".into()]).unwrap();
        let list = get_agent_allowlist(&s, "agent-1").unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"skill.read".to_string()));
        assert!(list.contains(&"skill.write".to_string()));
    }

    #[test]
    fn suspended_defaults_false_and_round_trips() {
        let mut s = store();
        assert!(!is_agent_suspended(&s, "agent-1").unwrap());
        set_agent_suspended(&mut s, "agent-1", true).unwrap();
        assert!(is_agent_suspended(&s, "agent-1").unwrap());
    }

    #[test]
    fn agent_pubkey_round_trips() {
        let mut s = store();
        assert!(get_agent_pubkey(&s, "agent-1").unwrap().is_none());
        set_agent_pubkey(&mut s, "agent-1", "UABC123").unwrap();
        assert_eq!(get_agent_pubkey(&s, "agent-1").unwrap().unwrap(), "UABC123");
    }
}
