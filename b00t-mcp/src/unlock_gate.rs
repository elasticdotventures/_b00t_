//! SP3-05 — a tool stays locked until the r0le has *learned* the skill that
//! unlocks it. Built from a role's blessing chain
//! (`b00t_cli::commands::blessing::collect_role_unlocks`): each discovered skill
//! declares `unlocks` globs; a tool matching one of those globs requires that
//! skill to have been learned in the current session.

use std::collections::HashSet;

use b00t_cli::commands::blessing::RoleManifest;

/// tool-name glob → the skill key that unlocks it.
#[derive(Debug, Clone, Default)]
pub struct UnlockGate {
    rules: Vec<(glob::Pattern, String)>,
}

impl UnlockGate {
    /// Build from a role's blessing manifest. `required` skills contribute their
    /// `unlocks` globs; unparseable globs are dropped.
    pub fn from_manifest(manifest: &RoleManifest) -> Self {
        let mut rules = Vec::new();
        for (skill, globs) in &manifest.required {
            for g in globs {
                if let Ok(pat) = glob::Pattern::new(g) {
                    rules.push((pat, skill.clone()));
                }
            }
        }
        Self { rules }
    }

    /// The skill a tool needs, if any (first matching rule wins).
    pub fn required_skill(&self, tool_name: &str) -> Option<&str> {
        self.rules
            .iter()
            .find(|(pat, _)| pat.matches(tool_name))
            .map(|(_, skill)| skill.as_str())
    }

    /// A tool is satisfied when it needs no skill, or the needed skill is in
    /// `learned`.
    pub fn is_satisfied(&self, tool_name: &str, learned: &HashSet<String>) -> bool {
        match self.required_skill(tool_name) {
            None => true,
            Some(skill) => learned.contains(skill),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> RoleManifest {
        RoleManifest {
            required: vec![
                ("rust.skill".to_string(), vec!["cargo.*".to_string(), "rustfmt".to_string()]),
                ("soul.skill".to_string(), vec!["soul_*".to_string()]),
            ],
            optional: vec![],
        }
    }

    #[test]
    fn required_skill_matches_globs() {
        let g = UnlockGate::from_manifest(&manifest());
        assert_eq!(g.required_skill("cargo_build"), Some("rust.skill"));
        assert_eq!(g.required_skill("rustfmt"), Some("rust.skill"));
        assert_eq!(g.required_skill("soul_row_insert"), Some("soul.skill"));
        assert_eq!(g.required_skill("b00t_status"), None); // unlisted → always ok
    }

    #[test]
    fn is_satisfied_tracks_learned() {
        let g = UnlockGate::from_manifest(&manifest());
        let mut learned = HashSet::new();

        assert!(g.is_satisfied("b00t_status", &learned)); // no skill needed
        assert!(!g.is_satisfied("cargo_build", &learned)); // locked

        learned.insert("rust.skill".to_string());
        assert!(g.is_satisfied("cargo_build", &learned)); // unlocked
        assert!(!g.is_satisfied("soul_row_insert", &learned)); // still locked
    }

    #[test]
    fn empty_manifest_gates_nothing() {
        let g = UnlockGate::from_manifest(&RoleManifest::default());
        assert!(g.is_empty());
        assert!(g.is_satisfied("anything", &HashSet::new()));
    }
}
