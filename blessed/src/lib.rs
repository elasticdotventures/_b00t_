use serde::Deserialize;
use std::collections::HashMap;

const BLESSED_TOML: &str = include_str!("../rust.toml");

#[derive(Debug, Deserialize)]
struct BlessedManifest {
    #[serde(rename = "crate")]
    crates: Vec<CrateEntry>,
}

#[derive(Debug, Deserialize, Clone)]
struct CrateEntry {
    category: String,
    use_case: String,
    recommended: Vec<String>,
    notes: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Recommendation {
    pub category: String,
    pub use_case: String,
    pub crates: Vec<CrateInfo>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CrateInfo {
    pub name: String,
    pub note: String,
}

pub fn query(needle: &str, limit: usize) -> Vec<Recommendation> {
    let manifest: BlessedManifest =
        toml::from_str(BLESSED_TOML).unwrap_or(BlessedManifest { crates: vec![] });
    let needle_lower = needle.to_lowercase();
    let mut scored: Vec<(usize, &CrateEntry)> = manifest
        .crates
        .iter()
        .filter_map(|e| {
            let mut score = 0usize;
            let ul = e.use_case.to_lowercase();
            let cl = e.category.to_lowercase();
            let rl = e.recommended.join(" ").to_lowercase();
            if ul.contains(&needle_lower) {
                score += 10;
            }
            for word in needle_lower.split_whitespace() {
                if ul.contains(word) {
                    score += 5;
                }
                if cl.contains(word) {
                    score += 2;
                }
                if rl.contains(word) {
                    score += 1;
                }
            }
            if score > 0 {
                Some((score, e))
            } else {
                None
            }
        })
        .collect();
    scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
    scored.truncate(limit);

    scored
        .iter()
        .map(|(_, e)| Recommendation {
            category: e.category.clone(),
            use_case: e.use_case.clone(),
            crates: e
                .recommended
                .iter()
                .map(|name| CrateInfo {
                    name: name.clone(),
                    note: e.notes.get(name).cloned().unwrap_or_default(),
                })
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_http() {
        let results = query("http", 5);
        assert!(!results.is_empty(), "Should find HTTP-related crates");
        let has_http = results
            .iter()
            .any(|r| r.use_case.to_lowercase().contains("http"));
        assert!(has_http, "Should have HTTP in results");
    }

    #[test]
    fn test_query_serialization() {
        let results = query("serialization", 5);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_query_random() {
        let results = query("random", 5);
        assert!(!results.is_empty());
        let has_rand = results
            .iter()
            .any(|r| r.crates.iter().any(|c| c.name == "rand"));
        assert!(has_rand);
    }
}
