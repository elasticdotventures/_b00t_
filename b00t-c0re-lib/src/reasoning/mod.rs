//! First-order reasoning engine over b00t knowledge graph triples.
//!
//! Layers:
//!   graph_rules — crepe Horn clause derivation (reachability, dependency, trait impls)
//!   analytics   — ascent lattice analytics (shortest path, skill frequency)
//!
//! # Example
//! ```rust
//! use b00t_c0re_lib::reasoning::ReasoningEngine;
//!
//! let triples = vec![
//!     ("a".into(), "b00t:dependsOn".into(), "b".into()),
//!     ("b".into(), "b00t:dependsOn".into(), "c".into()),
//! ];
//! let result = ReasoningEngine::run(triples);
//! assert!(result.horn.reachable_from("a").contains(&"c".to_string()));
//! assert_eq!(result.analytics.shortest_path("a", "c"), Some(2));
//! ```

pub mod adjacency;
pub mod analytics;
pub mod bound_checker;
pub mod graph_rules;
pub mod neumann_bridge;
pub mod trait_lower;
#[cfg(test)]
mod tests;

pub use adjacency::find_adjacent;
pub use analytics::AnalyticsResults;
pub use graph_rules::HornResults;

/// Combined result of running both Horn derivation and lattice analytics.
pub struct ReasoningResult {
    pub horn: HornResults,
    pub analytics: AnalyticsResults,
}

/// Entry point: run both reasoning layers over a set of SPO triples.
pub struct ReasoningEngine;

impl ReasoningEngine {
    pub fn run(triples: Vec<(String, String, String)>) -> ReasoningResult {
        let horn = graph_rules::derive(triples.clone());
        let analytics = analytics::analyze(triples);
        ReasoningResult { horn, analytics }
    }

    /// Convenience: run and immediately query reachability.
    pub fn reachable_from(
        triples: Vec<(String, String, String)>,
        subject: &str,
    ) -> Vec<String> {
        let result = Self::run(triples);
        result.horn.reachable_from(subject)
    }

    /// Convenience: run and return top-N skills by recall frequency.
    pub fn top_skills(triples: Vec<(String, String, String)>, n: usize) -> Vec<(String, u32)> {
        let result = Self::run(triples);
        result.analytics.top_skills(n)
    }
}
