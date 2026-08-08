//! just AST access via `just --dump --dump-format json`.
//!
//! Rather than re-implementing just's parser, we use just's own compiler
//! output. `just --dump --dump-format json` produces the full AST including
//! recipe bodies, parameters (with kind: singular/plus/star), dependencies,
//! docs, settings, and variable assignments.
//!
//! This gives us LSP-grade introspection without embedding just's internals.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Full just AST as returned by `just --dump --dump-format json`
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JustDump {
    #[serde(default)]
    pub aliases: HashMap<String, String>,
    #[serde(default)]
    pub assignments: HashMap<String, JustAssignment>,
    pub first: Option<String>,
    #[serde(default)]
    pub recipes: HashMap<String, JustRecipe>,
    pub settings: JustSettings,
    pub source: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JustAssignment {
    pub name: String,
    pub value: Option<serde_json::Value>,
    #[serde(default)]
    pub export: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JustRecipe {
    pub name: String,
    pub namepath: String,
    pub doc: Option<String>,
    /// Raw command body — complex nested structure from just's AST
    pub body: serde_json::Value,
    #[serde(default)]
    pub dependencies: Vec<JustDependency>,
    #[serde(default)]
    pub parameters: Vec<JustParameter>,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub quiet: bool,
    #[serde(default)]
    pub shebang: bool,
    #[serde(default)]
    pub attributes: Vec<serde_json::Value>,
    #[serde(default)]
    pub priors: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct JustParameter {
    pub name: String,
    pub default: Option<String>,
    #[serde(default)]
    pub export: bool,
    /// "singular" | "plus" (one-or-more) | "star" (zero-or-more)
    pub kind: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum JustDependency {
    Name(String),
    Recipe {
        recipe: String,
        #[serde(default)]
        arguments: Vec<serde_json::Value>,
    },
    Raw(serde_json::Value),
}

impl JustDependency {
    pub fn name(&self) -> String {
        match self {
            JustDependency::Name(name) => name.clone(),
            JustDependency::Recipe { recipe, .. } => recipe.clone(),
            JustDependency::Raw(value) => value.to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct JustSettings {
    #[serde(default)]
    pub dotenv_load: bool,
    #[serde(default)]
    pub export: bool,
    #[serde(default)]
    pub positional_arguments: bool,
    #[serde(default)]
    pub quiet: bool,
    pub shell: Option<serde_json::Value>,
    pub working_directory: Option<String>,
}

/// Live AST loaded from a justfile — use `reload()` to detect changes.
pub struct JustfileAst {
    pub path: PathBuf,
    pub dump: JustDump,
    /// Filesystem mtime when this AST was loaded — used for staleness check
    loaded_mtime: Option<SystemTime>,
}

impl JustfileAst {
    /// Load the AST by running `just --dump --dump-format json`.
    pub fn load(path: &Path) -> Result<Self> {
        let (dump, mtime) = Self::run_dump(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            dump,
            loaded_mtime: mtime,
        })
    }

    /// Check if the justfile has been modified since this AST was loaded.
    pub fn is_stale(&self) -> bool {
        let Ok(meta) = std::fs::metadata(&self.path) else {
            return true;
        };
        let Ok(current_mtime) = meta.modified() else {
            return true;
        };
        match self.loaded_mtime {
            Some(loaded) => current_mtime > loaded,
            None => true,
        }
    }

    /// Reload the AST if stale; return the diff (empty diff if not stale).
    pub fn reload(&mut self) -> Result<AstDiff> {
        if !self.is_stale() {
            return Ok(AstDiff::empty());
        }
        let (new_dump, new_mtime) = Self::run_dump(&self.path)?;
        let diff = self.diff_with(&new_dump);
        self.dump = new_dump;
        self.loaded_mtime = new_mtime;
        Ok(diff)
    }

    /// Diff this AST against a new one — reports structural changes at recipe level.
    pub fn diff_with(&self, other: &JustDump) -> AstDiff {
        let old_recipes: std::collections::HashSet<&str> =
            self.dump.recipes.keys().map(String::as_str).collect();
        let new_recipes: std::collections::HashSet<&str> =
            other.recipes.keys().map(String::as_str).collect();

        let added: Vec<String> = new_recipes
            .difference(&old_recipes)
            .map(|s| s.to_string())
            .collect();

        let removed: Vec<String> = old_recipes
            .difference(&new_recipes)
            .map(|s| s.to_string())
            .collect();

        let modified: Vec<RecipeDiff> = old_recipes
            .intersection(&new_recipes)
            .filter_map(|name| {
                let old = &self.dump.recipes[*name];
                let new = &other.recipes[*name];
                recipe_diff(name, old, new)
            })
            .collect();

        let old_vars: std::collections::HashSet<&str> =
            self.dump.assignments.keys().map(String::as_str).collect();
        let new_vars: std::collections::HashSet<&str> =
            other.assignments.keys().map(String::as_str).collect();

        let added_variables: Vec<String> = new_vars
            .difference(&old_vars)
            .map(|s| s.to_string())
            .collect();
        let removed_variables: Vec<String> = old_vars
            .difference(&new_vars)
            .map(|s| s.to_string())
            .collect();
        let has_changes = !added.is_empty()
            || !removed.is_empty()
            || !modified.is_empty()
            || !added_variables.is_empty()
            || !removed_variables.is_empty();
        AstDiff {
            added_recipes: added,
            removed_recipes: removed,
            modified_recipes: modified,
            added_variables,
            removed_variables,
            has_changes,
        }
    }

    /// Validate the justfile by running `just --list` (dry-run check).
    /// Returns validation warnings/errors as strings.
    pub fn validate(&self) -> Vec<String> {
        let output = std::process::Command::new("just")
            .args(["--justfile", &self.path.display().to_string(), "--list"])
            .output();
        match output {
            Err(e) => vec![format!("just validation failed: {}", e)],
            Ok(out) if !out.status.success() => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                vec![stderr]
            }
            Ok(_) => vec![],
        }
    }

    /// Convenient recipe iterator, sorted by name
    pub fn recipes_sorted(&self) -> Vec<&JustRecipe> {
        let mut recipes: Vec<&JustRecipe> = self.dump.recipes.values().collect();
        recipes.sort_by_key(|r| &r.name);
        recipes
    }

    fn run_dump(path: &Path) -> Result<(JustDump, Option<SystemTime>)> {
        let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();

        let working_dir = path.parent().unwrap_or_else(|| Path::new("."));

        let output = std::process::Command::new("just")
            .args([
                "--justfile",
                &path.display().to_string(),
                "--dump",
                "--dump-format",
                "json",
            ])
            .current_dir(working_dir)
            .output()
            .context("just --dump --dump-format json failed; ensure just >= 1.13.0 is installed")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("just --dump failed: {}", stderr);
        }

        let dump: JustDump =
            serde_json::from_slice(&output.stdout).context("failed to parse just JSON dump")?;

        Ok((dump, mtime))
    }
}

/// Structural diff between two justfile ASTs.
#[derive(Debug, Default)]
pub struct AstDiff {
    pub added_recipes: Vec<String>,
    pub removed_recipes: Vec<String>,
    pub modified_recipes: Vec<RecipeDiff>,
    pub added_variables: Vec<String>,
    pub removed_variables: Vec<String>,
    pub has_changes: bool,
}

impl AstDiff {
    pub fn empty() -> Self {
        Self {
            has_changes: false,
            ..Default::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        !self.has_changes
            && self.added_recipes.is_empty()
            && self.removed_recipes.is_empty()
            && self.modified_recipes.is_empty()
    }

    /// Human-readable summary of changes
    pub fn summary(&self) -> String {
        if self.is_empty() {
            return "no changes".to_string();
        }
        let mut parts = vec![];
        if !self.added_recipes.is_empty() {
            parts.push(format!(
                "+{} recipe(s): {}",
                self.added_recipes.len(),
                self.added_recipes.join(", ")
            ));
        }
        if !self.removed_recipes.is_empty() {
            parts.push(format!(
                "-{} recipe(s): {}",
                self.removed_recipes.len(),
                self.removed_recipes.join(", ")
            ));
        }
        if !self.modified_recipes.is_empty() {
            let names: Vec<&str> = self
                .modified_recipes
                .iter()
                .map(|d| d.name.as_str())
                .collect();
            parts.push(format!("~{} changed: {}", names.len(), names.join(", ")));
        }
        parts.join("; ")
    }
}

/// Diff for a single recipe.
#[derive(Debug)]
pub struct RecipeDiff {
    pub name: String,
    pub doc_changed: bool,
    pub body_changed: bool,
    pub parameters_changed: bool,
    pub dependencies_changed: bool,
}

fn recipe_diff(name: &str, old: &JustRecipe, new: &JustRecipe) -> Option<RecipeDiff> {
    let doc_changed = old.doc != new.doc;
    let body_changed = old.body != new.body;
    let parameters_changed = old.parameters != new.parameters;
    let dependencies_changed = old.dependencies != new.dependencies;

    if doc_changed || body_changed || parameters_changed || dependencies_changed {
        Some(RecipeDiff {
            name: name.to_string(),
            doc_changed,
            body_changed,
            parameters_changed,
            dependencies_changed,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_recipe(name: &str, doc: Option<&str>) -> JustRecipe {
        JustRecipe {
            name: name.to_string(),
            namepath: name.to_string(),
            doc: doc.map(str::to_string),
            body: serde_json::Value::Array(vec![]),
            dependencies: vec![],
            parameters: vec![],
            private: false,
            quiet: false,
            shebang: false,
            attributes: vec![],
            priors: 0,
        }
    }
    fn make_dump(recipes: &[(&str, Option<&str>)]) -> JustDump {
        let mut map = HashMap::new();
        for (name, doc) in recipes {
            map.insert(name.to_string(), make_recipe(name, *doc));
        }
        JustDump {
            aliases: HashMap::new(),
            assignments: HashMap::new(),
            first: None,
            recipes: map,
            settings: JustSettings::default(),
            source: None,
            warnings: vec![],
        }
    }
    #[test]
    fn identical_dumps_produce_empty_diff() {
        let old_dump = make_dump(&[("build", Some("Build")), ("test", None)]);
        let ast = JustfileAst {
            path: std::path::PathBuf::from("/fake/justfile"),
            dump: old_dump,
            loaded_mtime: None,
        };
        let same_dump = make_dump(&[("build", Some("Build")), ("test", None)]);
        let diff = ast.diff_with(&same_dump);
        assert!(
            diff.is_empty(),
            "identical justfiles must produce empty diff"
        );
    }
    #[test]
    fn added_recipe_produces_nonempty_diff() {
        let old_dump = make_dump(&[("build", None)]);
        let ast = JustfileAst {
            path: std::path::PathBuf::from("/fake/justfile"),
            dump: old_dump,
            loaded_mtime: None,
        };
        let new_dump = make_dump(&[("build", None), ("test", None)]);
        let diff = ast.diff_with(&new_dump);
        assert!(!diff.is_empty());
        assert!(diff.added_recipes.contains(&"test".to_string()));
    }
    #[test]
    fn astdiff_empty_is_empty() {
        assert!(AstDiff::empty().is_empty());
    }
}
