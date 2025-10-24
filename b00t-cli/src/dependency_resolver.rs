use anyhow::{bail, Result};
use std::collections::{HashMap, HashSet};
use crate::BootDatum;

/// Dependency resolver with topological sort and cycle detection
pub struct DependencyResolver<'a> {
    datums: HashMap<String, &'a BootDatum>,
}

impl<'a> DependencyResolver<'a> {
    /// Create a new resolver from a collection of datums
    pub fn new(datums: Vec<&'a BootDatum>) -> Self {
        let mut datum_map = HashMap::new();
        for datum in datums {
            // Key format: "name.type" (e.g., "docker.cli", "postgres.docker")
            let type_str = datum.datum_type.as_ref()
                .map(|t| format!("{:?}", t).to_lowercase())
                .unwrap_or_else(|| "unknown".to_string());
            let key = format!("{}.{}", datum.name, type_str);
            datum_map.insert(key, datum);
        }
        Self { datums: datum_map }
    }

    /// Resolve dependencies for a datum and return installation order
    /// Returns Vec of datum keys in topologically sorted order (dependencies first)
    pub fn resolve(&self, datum_key: &str) -> Result<Vec<String>> {
        let mut order = Vec::new();
        let mut visiting = HashSet::new();
        let mut visited = HashSet::new();

        self.visit(datum_key, &mut order, &mut visiting, &mut visited)?;

        // No need to reverse - DFS post-order already gives us correct order
        Ok(order)
    }

    /// Recursive DFS visit for topological sort with cycle detection
    fn visit(
        &self,
        datum_key: &str,
        order: &mut Vec<String>,
        visiting: &mut HashSet<String>,
        visited: &mut HashSet<String>,
    ) -> Result<()> {
        if visited.contains(datum_key) {
            return Ok(()); // Already processed
        }

        if visiting.contains(datum_key) {
            // Cycle detected - build cycle path for error message
            bail!("Circular dependency detected involving: {}", datum_key);
        }

        // Get datum from map
        let datum = self.datums.get(datum_key)
            .ok_or_else(|| anyhow::anyhow!("Datum not found: {}", datum_key))?;

        visiting.insert(datum_key.to_string());

        // Visit all dependencies first
        if let Some(depends_on) = &datum.depends_on {
            for dep in depends_on {
                self.visit(dep, order, visiting, visited)?;
            }
        }

        visiting.remove(datum_key);
        visited.insert(datum_key.to_string());
        order.push(datum_key.to_string());

        Ok(())
    }

    /// Resolve multiple datums and return merged installation order
    /// Deduplicates common dependencies
    pub fn resolve_many(&self, datum_keys: &[String]) -> Result<Vec<String>> {
        let mut all_deps = Vec::new();
        let mut seen = HashSet::new();

        for key in datum_keys {
            let deps = self.resolve(key)?;
            for dep in deps {
                if !seen.contains(&dep) {
                    seen.insert(dep.clone());
                    all_deps.push(dep);
                }
            }
        }

        Ok(all_deps)
    }

    /// Check if datum has any dependencies (direct or transitive)
    pub fn has_dependencies(&self, datum_key: &str) -> bool {
        if let Some(datum) = self.datums.get(datum_key) {
            datum.depends_on.as_ref().map_or(false, |deps| !deps.is_empty())
        } else {
            false
        }
    }

    /// Get direct dependencies of a datum
    pub fn get_direct_dependencies(&self, datum_key: &str) -> Vec<String> {
        self.datums.get(datum_key)
            .and_then(|d| d.depends_on.as_ref())
            .map(|deps| deps.clone())
            .unwrap_or_default()
    }

    /// Validate all datums in the collection for:
    /// - Missing dependencies
    /// - Circular dependencies
    /// Returns list of validation errors
    pub fn validate_all(&self) -> Vec<String> {
        let mut errors = Vec::new();

        for datum_key in self.datums.keys() {
            // Check for missing dependencies
            if let Some(datum) = self.datums.get(datum_key) {
                if let Some(depends_on) = &datum.depends_on {
                    for dep in depends_on {
                        if !self.datums.contains_key(dep) {
                            errors.push(format!(
                                "Datum '{}' depends on missing datum '{}'",
                                datum_key, dep
                            ));
                        }
                    }
                }
            }

            // Check for circular dependencies
            match self.resolve(datum_key) {
                Err(e) => {
                    if e.to_string().contains("Circular dependency") {
                        errors.push(format!(
                            "Circular dependency detected starting from '{}'",
                            datum_key
                        ));
                    }
                }
                Ok(_) => {}
            }
        }

        errors
    }

    /// Build dependency graph for visualization/debugging
    /// Returns adjacency list: datum_key -> [dependency_keys]
    pub fn build_graph(&self) -> HashMap<String, Vec<String>> {
        let mut graph = HashMap::new();

        for (key, datum) in &self.datums {
            let deps = datum.depends_on.as_ref()
                .map(|d| d.clone())
                .unwrap_or_default();
            graph.insert(key.clone(), deps);
        }

        graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BootDatum, DatumType};

    fn create_test_datum(name: &str, r#type: &str, depends_on: Option<Vec<String>>) -> BootDatum {
        let datum_type = match r#type {
            "cli" => Some(DatumType::Cli),
            "mcp" => Some(DatumType::Mcp),
            "docker" => Some(DatumType::Docker),
            "k8s" => Some(DatumType::K8s),
            _ => Some(DatumType::Unknown),
        };

        BootDatum {
            name: name.to_string(),
            datum_type,
            desires: None,
            hint: format!("Test datum {}", name),
            install: None,
            update: None,
            version: None,
            version_regex: None,
            command: None,
            args: None,
            vsix_id: None,
            script: None,
            image: None,
            docker_args: None,
            oci_uri: None,
            resource_path: None,
            chart_path: None,
            namespace: None,
            values_file: None,
            keywords: None,
            package_name: None,
            env: None,
            require: None,
            aliases: None,
            depends_on,
            members: None,
            mcp: None,
        }
    }

    #[test]
    fn test_no_dependencies() {
        let datum = create_test_datum("docker", "cli", None);
        let resolver = DependencyResolver::new(vec![&datum]);

        let result = resolver.resolve("docker.cli").unwrap();
        assert_eq!(result, vec!["docker.cli"]);
    }

    #[test]
    fn test_linear_dependencies() {
        // c -> b -> a
        let datum_a = create_test_datum("a", "cli", None);
        let datum_b = create_test_datum("b", "cli", Some(vec!["a.cli".to_string()]));
        let datum_c = create_test_datum("c", "cli", Some(vec!["b.cli".to_string()]));

        let resolver = DependencyResolver::new(vec![&datum_a, &datum_b, &datum_c]);

        let result = resolver.resolve("c.cli").unwrap();
        assert_eq!(result, vec!["a.cli", "b.cli", "c.cli"]);
    }

    #[test]
    fn test_diamond_dependencies() {
        // d -> [b, c] -> a
        let datum_a = create_test_datum("a", "cli", None);
        let datum_b = create_test_datum("b", "cli", Some(vec!["a.cli".to_string()]));
        let datum_c = create_test_datum("c", "cli", Some(vec!["a.cli".to_string()]));
        let datum_d = create_test_datum("d", "cli", Some(vec!["b.cli".to_string(), "c.cli".to_string()]));

        let resolver = DependencyResolver::new(vec![&datum_a, &datum_b, &datum_c, &datum_d]);

        let result = resolver.resolve("d.cli").unwrap();
        // a should come first, b and c after, d last
        assert_eq!(result[0], "a.cli");
        assert_eq!(result[3], "d.cli");
    }

    #[test]
    fn test_circular_dependency_detection() {
        // a -> b -> c -> a (cycle)
        let datum_a = create_test_datum("a", "cli", Some(vec!["b.cli".to_string()]));
        let datum_b = create_test_datum("b", "cli", Some(vec!["c.cli".to_string()]));
        let datum_c = create_test_datum("c", "cli", Some(vec!["a.cli".to_string()]));

        let resolver = DependencyResolver::new(vec![&datum_a, &datum_b, &datum_c]);

        let result = resolver.resolve("a.cli");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circular dependency"));
    }

    #[test]
    fn test_self_dependency_detection() {
        // a -> a (self-cycle)
        let datum_a = create_test_datum("a", "cli", Some(vec!["a.cli".to_string()]));

        let resolver = DependencyResolver::new(vec![&datum_a]);

        let result = resolver.resolve("a.cli");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circular dependency"));
    }

    #[test]
    fn test_missing_dependency() {
        let datum_a = create_test_datum("a", "cli", Some(vec!["missing.cli".to_string()]));

        let resolver = DependencyResolver::new(vec![&datum_a]);

        let result = resolver.resolve("a.cli");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_resolve_many() {
        let datum_a = create_test_datum("a", "cli", None);
        let datum_b = create_test_datum("b", "cli", Some(vec!["a.cli".to_string()]));
        let datum_c = create_test_datum("c", "cli", Some(vec!["a.cli".to_string()]));

        let resolver = DependencyResolver::new(vec![&datum_a, &datum_b, &datum_c]);

        let result = resolver.resolve_many(&["b.cli".to_string(), "c.cli".to_string()]).unwrap();

        // a.cli should appear only once, followed by b and c
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], "a.cli");
        assert!(result.contains(&"b.cli".to_string()));
        assert!(result.contains(&"c.cli".to_string()));
    }

    #[test]
    fn test_validate_all() {
        let datum_a = create_test_datum("a", "cli", None);
        let datum_b = create_test_datum("b", "cli", Some(vec!["missing.cli".to_string()]));
        let datum_c = create_test_datum("c", "cli", Some(vec!["d.cli".to_string()]));
        let datum_d = create_test_datum("d", "cli", Some(vec!["c.cli".to_string()]));

        let resolver = DependencyResolver::new(vec![&datum_a, &datum_b, &datum_c, &datum_d]);

        let errors = resolver.validate_all();

        // Should detect missing dependency and circular dependency
        assert!(errors.len() >= 2);
        assert!(errors.iter().any(|e| e.contains("missing datum")));
        assert!(errors.iter().any(|e| e.contains("Circular dependency")));
    }
}
