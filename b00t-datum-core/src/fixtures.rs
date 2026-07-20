/// Load a `.tomllmd` / `.tomllm` / `.toml` datum as a [`TomllmDoc`] fixture in tests.
///
/// The path is resolved relative to the crate's `CARGO_MANIFEST_DIR` at runtime.
///
/// # Examples
///
/// ```rust,no_run
/// # use b00t_datum_core::b00t_datum;
/// let doc = b00t_datum!("_b00t_/datums/PRD-TEST.tomllmd");
/// assert_eq!(doc.tier(), Some("frontier"));
/// ```
///
/// With an absolute path:
/// ```rust,no_run
/// # use b00t_datum_core::b00t_datum;
/// let doc = b00t_datum!(abs "/home/user/.b00t/_b00t_/datums/PRD-TEST.tomllmd");
/// ```
#[macro_export]
macro_rules! b00t_datum {
    // Relative path: resolved from CARGO_MANIFEST_DIR
    ($rel:literal) => {{
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = base.join($rel);
        $crate::TomllmDoc::from_path(&path).expect(concat!("b00t_datum! failed to load: ", $rel))
    }};
    // Absolute path
    (abs $abs:literal) => {{
        let path = std::path::Path::new($abs);
        $crate::TomllmDoc::from_path(path).expect(concat!("b00t_datum! failed to load: ", $abs))
    }};
}

/// In-memory TOMLLM fixture builder for tests — avoids file I/O.
///
/// # Example
///
/// ```rust
/// # use b00t_datum_core::fixtures::TomllmdFixture;
/// let doc = TomllmdFixture::new()
///     .datum_type("prd")
///     .type_tags(&["prd", "ooda"])
///     .tier("frontier")
///     .complexity(6)
///     .build();
/// assert_eq!(doc.tier(), Some("frontier"));
/// ```
pub struct TomllmdFixture {
    datum_type: Option<String>,
    type_tags: Vec<String>,
    tier: Option<String>,
    complexity: Option<u8>,
    extra_toml: String,
}

impl TomllmdFixture {
    pub fn new() -> Self {
        Self {
            datum_type: None,
            type_tags: Vec::new(),
            tier: None,
            complexity: None,
            extra_toml: String::new(),
        }
    }

    pub fn datum_type(mut self, t: &str) -> Self {
        self.datum_type = Some(t.to_string());
        self
    }

    pub fn type_tags(mut self, tags: &[&str]) -> Self {
        self.type_tags = tags.iter().map(|s| format!("\"{s}\"")).collect();
        self
    }

    pub fn tier(mut self, tier: &str) -> Self {
        self.tier = Some(tier.to_string());
        self
    }

    pub fn complexity(mut self, c: u8) -> Self {
        self.complexity = Some(c);
        self
    }

    /// Append raw TOML to the fixture (for extra sections like `[prd]`).
    pub fn toml(mut self, raw: &str) -> Self {
        self.extra_toml.push('\n');
        self.extra_toml.push_str(raw);
        self
    }

    /// Build a `TomllmDoc` from the fixture specification (no file I/O).
    pub fn build(self) -> crate::tomllmd::TomllmDoc {
        use crate::tomllmd::TomllmdExt;

        let dtype = self.datum_type.as_deref().unwrap_or("prd");
        let tags_toml = if self.type_tags.is_empty() {
            "[]".to_string()
        } else {
            format!("[{}]", self.type_tags.join(", "))
        };

        let mut src = format!(
            "[b00t.schema]\nversion = \"1\"\ntype = \"{dtype}\"\ntype_tags = {tags_toml}\n"
        );
        src.push_str(&self.extra_toml);

        let tier = self.tier.unwrap_or_default();
        let complexity = self.complexity.map_or(String::new(), |c| c.to_string());
        src.push_str(&format!(
            "\n# b00t:map v1\n# tier: {tier}\n# complexity: {complexity}\n"
        ));

        crate::tomllmd::TomllmDoc::from_str(&src, TomllmdExt::Tomllmd, "fixture.tomllmd".into())
            .expect("TomllmdFixture::build produced invalid TOML")
    }
}

impl Default for TomllmdFixture {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_builder_basic() {
        let doc = TomllmdFixture::new()
            .datum_type("prd")
            .type_tags(&["prd", "ooda"])
            .tier("frontier")
            .complexity(6)
            .build();

        assert_eq!(doc.schema.datum_type.as_deref(), Some("prd"));
        assert!(doc.type_tags().contains(&"ooda".to_string()));
        assert_eq!(doc.tier(), Some("frontier"));
        assert_eq!(doc.complexity(), Some(6));
    }

    #[test]
    fn fixture_builder_extra_toml() {
        let doc = TomllmdFixture::new()
            .datum_type("prd")
            .toml("[prd]\nid = \"PRD-TEST\"\nstatus = \"proposed\"")
            .build();
        assert!(doc.sections.contains_key("prd"));
    }

    #[test]
    fn fixture_builder_defaults() {
        let doc = TomllmdFixture::new().build();
        assert_eq!(doc.schema.datum_type.as_deref(), Some("prd"));
        assert_eq!(doc.type_tags().len(), 0);
        assert_eq!(doc.tier(), None); // empty string → None
    }
}
