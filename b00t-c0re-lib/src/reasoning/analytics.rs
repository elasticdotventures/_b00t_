//! Lattice analytics over SPO triples using ascent.
//!
//! shortest_path is anchored on direct edges (not transitive path closure) so hop
//! counts are correct: a→b→c gives shortest_path(a,c) = Dual(2), not Dual(1).

use ascent::ascent;
use ascent::lattice::Dual;
use std::collections::HashMap;

fn is_edge_pred(p: &str) -> bool {
    matches!(p, "b00t:relatedTo" | "b00t:dependsOn" | "b00t:informedBy")
        || p.contains("DependsOn")
        || p.contains("relatedTo")
}

ascent! {
    struct AnalyticsProgram;

    relation triple(String, String, String);
    relation edge(String, String);           // direct relationship edges only
    relation path(String, String);           // transitive closure (reachability)
    lattice shortest_path(String, String, Dual<u32>);

    // Classify direct edges from relationship predicates
    edge(x.clone(), y.clone()) <-- triple(x, p, y), if is_edge_pred(p.as_str());

    // Transitive closure for reachability (boolean)
    path(x.clone(), y.clone()) <-- edge(x, y);
    path(x.clone(), z.clone()) <-- edge(x, y), path(y, z);

    // Shortest path: base case on direct edges (NOT transitive path — avoids Dual(1) shortcut)
    shortest_path(x.clone(), y.clone(), Dual(1)) <-- edge(x, y);
    // Extend one hop: existing shortest_path from y + direct edge x→y
    shortest_path(x.clone(), z.clone(), Dual(d + 1)) <--
        edge(x, y),
        shortest_path(y, z, ?Dual(d));
}

/// Results of running lattice analytics.
pub struct AnalyticsResults {
    pub shortest_paths: Vec<(String, String, u32)>,
    pub path_count: usize,
    pub skill_frequency: HashMap<String, u32>,
}

impl AnalyticsResults {
    pub fn shortest_path(&self, from: &str, to: &str) -> Option<u32> {
        self.shortest_paths.iter().find(|(f, t, _)| f == from && t == to).map(|(_, _, d)| *d)
    }

    pub fn top_skills(&self, n: usize) -> Vec<(String, u32)> {
        let mut v: Vec<_> = self.skill_frequency.iter().map(|(k, v)| (k.clone(), *v)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.truncate(n);
        v
    }
}

/// Run lattice analytics over the given SPO triples.
pub fn analyze(triples: Vec<(String, String, String)>) -> AnalyticsResults {
    let mut prog = AnalyticsProgram::default();
    prog.triple = triples.clone();
    prog.run();

    let shortest_paths = prog
        .shortest_path
        .into_iter()
        .map(|(from, to, Dual(d))| (from, to, d))
        .collect();

    let path_count = prog.path.len();

    let mut skill_frequency: HashMap<String, u32> = HashMap::new();
    for (_, p, skill) in &triples {
        if p.as_str() == "b00t:informedBy" {
            *skill_frequency.entry(skill.clone()).or_insert(0) += 1;
        }
    }

    AnalyticsResults { shortest_paths, path_count, skill_frequency }
}
