//! Generic lazy discovery-chain walk: cycle guard + depth cap (#898).
//!
//! One walker, two callers: b00t-cli's `blessing --manifest` role graph
//! (depends_on -> unlocks, previously single-hop only -- see
//! b00t-cli/src/commands/blessing.rs) and ScopeStore's own `$.b00t.manifest`
//! provider-discovery chain (#893's "each discovery unlocks the next").
//! Both are the same shape (visit a node, expand it into more nodes, never
//! revisit, stop past a depth cap) -- worth one generic implementation, not
//! two hand-rolled graph walks.

use std::collections::HashSet;
use std::hash::Hash;

/// Walk a lazily-expanding chain starting from `roots`: repeatedly resolve
/// each node's children via `expand`, in discovery order.
///
/// - **Cycle guard**: a node already visited is never re-queued or
///   re-expanded, so cyclic graphs (A -> B -> A) terminate.
/// - **Depth cap**: a node at exactly `max_depth` is still recorded as
///   discovered, but its own children are not expanded further -- this is
///   a bound on traversal, not an error condition (a graph deeper than the
///   cap is truncated, not rejected).
///
/// Generic over the node identity `N` (anything `Eq + Hash + Clone`) and
/// the expansion function, so callers supply their own notion of "what
/// does this node unlock/publish."
pub fn walk_lazy_chain<N, F>(roots: impl IntoIterator<Item = N>, max_depth: usize, mut expand: F) -> Vec<N>
where
    N: Eq + Hash + Clone,
    F: FnMut(&N) -> Vec<N>,
{
    let mut visited: HashSet<N> = HashSet::new();
    let mut order: Vec<N> = Vec::new();
    // Stack-based (LIFO), not queue-based: discovery order is depth-first,
    // which matches "each discovery unlocks the next" -- a node's own
    // children are surfaced immediately after it, not after every sibling.
    let mut frontier: Vec<(N, usize)> = roots.into_iter().map(|n| (n, 0)).collect();
    frontier.reverse(); // preserve caller's root order in `order`

    while let Some((node, depth)) = frontier.pop() {
        if visited.contains(&node) {
            continue;
        }
        visited.insert(node.clone());
        order.push(node.clone());

        if depth >= max_depth {
            continue;
        }

        let mut children = expand(&node);
        children.reverse(); // preserve expand()'s own order
        for child in children {
            if !visited.contains(&child) {
                frontier.push((child, depth + 1));
            }
        }
    }

    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn chain_of(edges: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
        edges
            .iter()
            .map(|(k, vs)| (k.to_string(), vs.iter().map(|v| v.to_string()).collect()))
            .collect()
    }

    #[test]
    fn single_root_no_children() {
        let graph: HashMap<String, Vec<String>> = HashMap::new();
        let order = walk_lazy_chain(["a".to_string()], 10, |n| graph.get(n).cloned().unwrap_or_default());
        assert_eq!(order, vec!["a".to_string()]);
    }

    #[test]
    fn linear_chain_fully_discovered_within_depth() {
        let graph = chain_of(&[("a", &["b"]), ("b", &["c"]), ("c", &[])]);
        let order = walk_lazy_chain(["a".to_string()], 10, |n| graph.get(n).cloned().unwrap_or_default());
        assert_eq!(order, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    #[test]
    fn cycle_terminates_instead_of_looping_forever() {
        // a -> b -> a  (direct cycle)
        let graph = chain_of(&[("a", &["b"]), ("b", &["a"])]);
        let order = walk_lazy_chain(["a".to_string()], 100, |n| graph.get(n).cloned().unwrap_or_default());
        assert_eq!(order, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn diamond_shared_dependency_visited_once() {
        //   a
        //  / \
        // b   c
        //  \ /
        //   d
        let graph = chain_of(&[("a", &["b", "c"]), ("b", &["d"]), ("c", &["d"]), ("d", &[])]);
        let order = walk_lazy_chain(["a".to_string()], 10, |n| graph.get(n).cloned().unwrap_or_default());
        assert_eq!(order.len(), 4, "d must appear exactly once: {order:?}");
        assert_eq!(order.iter().filter(|n| n.as_str() == "d").count(), 1);
    }

    #[test]
    fn depth_cap_stops_expansion_but_still_records_the_boundary_node() {
        // a -> b -> c -> d, max_depth = 1: a(depth0) discovered+expanded,
        // b(depth1) discovered but NOT expanded (c, d never reached).
        let graph = chain_of(&[("a", &["b"]), ("b", &["c"]), ("c", &["d"]), ("d", &[])]);
        let order = walk_lazy_chain(["a".to_string()], 1, |n| graph.get(n).cloned().unwrap_or_default());
        assert_eq!(order, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn multiple_roots_all_discovered() {
        let graph: HashMap<String, Vec<String>> = HashMap::new();
        let order = walk_lazy_chain(
            ["a".to_string(), "b".to_string()],
            10,
            |n| graph.get(n).cloned().unwrap_or_default(),
        );
        assert_eq!(order, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn zero_depth_cap_still_records_roots_without_expanding() {
        let graph = chain_of(&[("a", &["b"])]);
        let order = walk_lazy_chain(["a".to_string()], 0, |n| graph.get(n).cloned().unwrap_or_default());
        assert_eq!(order, vec!["a".to_string()]);
    }
}
