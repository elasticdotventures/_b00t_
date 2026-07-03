//! Deployment-blocking E2E tests: every default graph endpoint must return valid output.

#[cfg(test)]
mod e2e {
    use std::time::Duration;

    const BASE: &str = "http://localhost:31337";

    fn check(path: &str, label: &str, key: &str, min_len: usize) {
        let url = format!("{BASE}{path}");
        let resp = ureq::get(&url)
            .timeout(Duration::from_secs(60))
            .call()
            .unwrap_or_else(|e| panic!("{label}: GET failed: {e}"));
        let data: serde_json::Value = resp.into_json()
            .unwrap_or_else(|e| panic!("{label}: JSON malformed: {e}"));
        if let Some(err) = data.get("error").and_then(|v| v.as_str()) {
            if !err.is_empty() {
                panic!("{label}: error: {err}");
            }
        }
        let content = data.get(key).and_then(|v| v.as_str()).unwrap_or("");
        assert!(
            content.len() >= min_len,
            "{label}: expected >= {min_len} bytes, got {} (start: {:.80})",
            content.len(),
            content
        );
    }

    #[test]
    fn all_default_graphs_render() {
        check("/api/admin/processes", "pipeline", "mermaid", 200);
        check("/api/admin/viz/task", "task", "mermaid", 50);
        check("/api/admin/viz/entangle", "entangle", "mermaid", 200);
        check("/api/admin/viz/isometric/demo", "isometric_demo", "svg", 500);
        check("/api/admin/viz/isometric", "isometric", "svg", 50);
    }
}
