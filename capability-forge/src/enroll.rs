use crate::identity::AgentKeyPair;
use crate::tiers::{add_to_allowlist, get_agent_allowlist, set_agent_pubkey, set_agent_suspended};
use anyhow::Result;
use b00t_c0re_gov::scope_store::ScopeStore;

pub fn enroll_agent(
    store: &mut dyn ScopeStore,
    agent_id: &str,
    base_skills: &[String],
) -> Result<AgentKeyPair> {
    let kp = AgentKeyPair::generate();
    set_agent_pubkey(store, agent_id, &kp.public_key())?;
    add_to_allowlist(store, agent_id, base_skills)?;
    Ok(kp)
}

pub fn suspend_agent(store: &mut dyn ScopeStore, agent_id: &str) -> Result<()> {
    set_agent_suspended(store, agent_id, true)
}

pub fn grant_base_skill(store: &mut dyn ScopeStore, agent_id: &str, skill: &str) -> Result<()> {
    add_to_allowlist(store, agent_id, std::slice::from_ref(&skill.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity;
    use crate::tiers::is_agent_suspended;
    use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
    use b00t_c0re_gov::scope_store::ScopeId;

    fn store() -> RedbScopeStore {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.redb");
        std::mem::forget(dir);
        RedbScopeStore::open(path, ScopeId::Global, None).unwrap()
    }

    #[test]
    fn enroll_registers_pubkey_and_allowlist_and_key_is_usable() {
        let mut s = store();
        let kp = enroll_agent(&mut s, "agent-1", &["skill.read".into()]).unwrap();
        assert_eq!(get_agent_allowlist(&s, "agent-1").unwrap(), vec!["skill.read".to_string()]);
        let sig = kp.sign(b"proof").unwrap();
        identity::verify(&kp.public_key(), b"proof", &sig).unwrap();
    }

    #[test]
    fn suspend_sets_flag() {
        let mut s = store();
        enroll_agent(&mut s, "agent-1", &[]).unwrap();
        assert!(!is_agent_suspended(&s, "agent-1").unwrap());
        suspend_agent(&mut s, "agent-1").unwrap();
        assert!(is_agent_suspended(&s, "agent-1").unwrap());
    }

    #[test]
    fn grant_base_skill_appends_to_existing_allowlist() {
        let mut s = store();
        enroll_agent(&mut s, "agent-1", &["skill.read".into()]).unwrap();
        grant_base_skill(&mut s, "agent-1", "skill.delete-infra").unwrap();
        let list = get_agent_allowlist(&s, "agent-1").unwrap();
        assert!(list.contains(&"skill.read".to_string()));
        assert!(list.contains(&"skill.delete-infra".to_string()));
    }
}
