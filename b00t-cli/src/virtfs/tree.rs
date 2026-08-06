//! Pure, fuser-independent directory-tree logic for the virtfs skeleton
//! (ROADMAP-virtfs.md Phase 1). Kept separate from `fuser::Filesystem`
//! glue so the tree structure itself is unit-testable without a FUSE
//! kernel session.
//!
//! Phase 1 scope: three empty top-level directories (skills/, agents/,
//! datums/), matching ARCHITECTURE-virtfs.md's Core Concept layout.
//! Dynamic datum enumeration is a later phase, not this one.

pub const ROOT_INO: u64 = 1;
const SKILLS_INO: u64 = 2;
const AGENTS_INO: u64 = 3;
const DATUMS_INO: u64 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Directory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirEntry {
    pub ino: u64,
    pub name: &'static str,
    pub kind: NodeKind,
}

fn root_entries() -> Vec<DirEntry> {
    vec![
        DirEntry {
            ino: SKILLS_INO,
            name: "skills",
            kind: NodeKind::Directory,
        },
        DirEntry {
            ino: AGENTS_INO,
            name: "agents",
            kind: NodeKind::Directory,
        },
        DirEntry {
            ino: DATUMS_INO,
            name: "datums",
            kind: NodeKind::Directory,
        },
    ]
}

/// Entries contained in the directory at `ino`, or `None` if `ino` isn't a
/// known directory in the Phase 1 tree.
pub fn entries_for(ino: u64) -> Option<Vec<DirEntry>> {
    match ino {
        ROOT_INO => Some(root_entries()),
        SKILLS_INO | AGENTS_INO | DATUMS_INO => Some(vec![]),
        _ => None,
    }
}

pub fn is_directory(ino: u64) -> bool {
    matches!(ino, ROOT_INO | SKILLS_INO | AGENTS_INO | DATUMS_INO)
}

/// Resolve `name` within `parent`'s directory to a child inode.
pub fn lookup_child(parent: u64, name: &str) -> Option<u64> {
    entries_for(parent)?
        .into_iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.ino)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_lists_skills_agents_datums() {
        let names: Vec<&str> = entries_for(ROOT_INO)
            .expect("root must be a known directory")
            .iter()
            .map(|entry| entry.name)
            .collect();
        assert_eq!(names, vec!["skills", "agents", "datums"]);
    }

    #[test]
    fn phase1_leaf_directories_are_empty() {
        for ino in [SKILLS_INO, AGENTS_INO, DATUMS_INO] {
            assert_eq!(
                entries_for(ino),
                Some(vec![]),
                "ino {ino} should be an empty directory in the Phase 1 skeleton"
            );
        }
    }

    #[test]
    fn unknown_inode_has_no_entries() {
        assert_eq!(entries_for(999), None);
    }

    #[test]
    fn lookup_child_resolves_root_children_and_rejects_unknown_names() {
        assert_eq!(lookup_child(ROOT_INO, "skills"), Some(SKILLS_INO));
        assert_eq!(lookup_child(ROOT_INO, "agents"), Some(AGENTS_INO));
        assert_eq!(lookup_child(ROOT_INO, "datums"), Some(DATUMS_INO));
        assert_eq!(lookup_child(ROOT_INO, "nonexistent"), None);
    }

    #[test]
    fn lookup_child_of_a_leaf_directory_finds_nothing() {
        assert_eq!(lookup_child(SKILLS_INO, "anything"), None);
    }

    #[test]
    fn is_directory_recognizes_all_phase1_inodes_and_rejects_unknown() {
        for ino in [ROOT_INO, SKILLS_INO, AGENTS_INO, DATUMS_INO] {
            assert!(is_directory(ino));
        }
        assert!(!is_directory(999));
    }
}
