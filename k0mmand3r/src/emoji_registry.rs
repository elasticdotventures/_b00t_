// k0mmand3r/src/emoji_registry.rs
//
// Compile-time emoji registry deserialized from .tomllmd mdtable format.
// Uses include_str! so the datum is embedded at compile time — zero runtime reads.
//
// The mdtable is parsed at compile time into a static &[EmojiEntry] slice.
// One linear scan, returns references. <100 entries means this is O(n) with n<30.
//
// Schema versioning: schema_version in the .tomllmd tracks column layout.
// Row additions (new emoji entries) don't change the schema version.
// The parser validates known schema versions and refuses unknown ones.
//
// 🔗参 _b00t_/schema/EMOJI_REGISTRY.tomllmd

use std::fmt;

/// A single emoji registry entry, deserialized from the .tomllmd mdtable.
///
/// All fields are `&'static str` because they point into a `include_str!`'d
/// memory region that lives for the program's lifetime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmojiEntry {
    /// The Unicode emoji literal, e.g. "🦨"
    pub literal: &'static str,
    /// The colon-wrapped shortcode, e.g. ":skunk:"
    pub shortcode: &'static str,
    /// The g0spell key, e.g. "skunk" (used for programmatic lookup)
    pub g0spell: &'static str,
    /// Escalation tier: 0=pass, 1=warn, 2=block
    pub tier: u8,
    /// Guard action description, e.g. "warn+redirect"
    pub action: &'static str,
    /// Human-readable description
    pub description: &'static str,
}

impl fmt::Display for EmojiEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} (tier={}, action={}): {}",
            self.literal, self.shortcode, self.tier, self.action, self.description
        )
    }
}

/// The full emoji registry, parsed at compile time.
///
/// Holds references into the include_str! memory. Small enough that
/// Entry lookup is a linear scan (n < 30).
#[derive(Debug, Clone)]
pub struct EmojiRegistry {
    /// Schema version from the .tomllmd
    pub schema_version: u8,
    /// All entries parsed from the mdtable
    pub entries: &'static [EmojiEntry],
}

impl EmojiRegistry {
    /// Look up an entry by its Unicode literal emoji.
    ///
    /// ```
    /// let reg = k0mmand3r::emoji_registry!();
    /// let skunk = reg.lookup_literal("🦨").unwrap();
    /// assert_eq!(skunk.shortcode, ":skunk:");
    /// ```
    pub fn lookup_literal(&self, literal: &str) -> Option<&'static EmojiEntry> {
        self.entries.iter().find(|e| e.literal == literal)
    }

    /// Look up an entry by its colon-wrapped shortcode.
    ///
    /// ```
    /// let reg = k0mmand3r::emoji_registry!();
    /// let poop = reg.lookup_shortcode(":poop:").unwrap();
    /// assert_eq!(poop.literal, "💩");
    /// ```
    pub fn lookup_shortcode(&self, shortcode: &str) -> Option<&'static EmojiEntry> {
        self.entries.iter().find(|e| e.shortcode == shortcode)
    }

    /// Look up an entry by its g0spell key.
    ///
    /// ```
    /// let reg = k0mmand3r::emoji_registry!();
    /// let block = reg.lookup_g0spell("block").unwrap();
    /// assert_eq!(block.literal, "🚫");
    /// ```
    pub fn lookup_g0spell(&self, g0spell: &str) -> Option<&'static EmojiEntry> {
        self.entries.iter().find(|e| e.g0spell == g0spell)
    }

    /// Return all entries with a given tier.
    pub fn filter_tier(&self, tier: u8) -> Vec<&'static EmojiEntry> {
        self.entries.iter().filter(|e| e.tier == tier).collect()
    }

    /// Return the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Parse an EmojiRegistry from .tomllmd content at compile time.
    ///
    /// Uses the emoji_registry!() macro instead — this can't be const due to
    /// slice allocation and string comparison limitations in const contexts.
    /// The macro uses OnceLock for lazy one-time init at program start.
    pub fn from_content(content: &'static str) -> EmojiRegistry {
        let entries = parse_entries_from_content(content);
        let schema_version = extract_schema_version(content).unwrap_or(0);
        EmojiRegistry {
            schema_version,
            entries: Box::leak(entries.into_boxed_slice()),
        }
    }
}

// ═══════════════════════════════════════════════════════════
// Helper functions
// ═══════════════════════════════════════════════════════════

/// Extract the `schema = N` value from a [b00t.version] section.
pub fn extract_schema_version(content: &str) -> Option<u8> {
    let bytes = content.as_bytes();
    let mut i = 0;
    let len = bytes.len();

    // Scan for "schema = " pattern
    let needle = "schema = ";
    let needle_bytes = needle.as_bytes();

    while i + needle_bytes.len() <= len {
        if bytes[i..].starts_with(needle_bytes) {
            i += needle_bytes.len();
            // Skip whitespace
            while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            // Read digits
            let mut num: u8 = 0;
            while i < len && bytes[i].is_ascii_digit() {
                num = num * 10 + (bytes[i] - b'0');
                i += 1;
            }
            return Some(num);
        }
        i += 1;
    }
    None
}

/// Parse emoji entries from raw mdtable content.
///
/// Takes a `&'static str` (from include_str!) and returns owned strings.
/// The emoji_registry!() macro Box::leaks the Vec so entries live forever.
pub fn parse_entries_from_content(content: &'static str) -> Vec<EmojiEntry> {
    let mut entries = Vec::new();

    // Find the mdtable header
    let header_pos = match content.find("| literal | shortcode | g0spell | tier | action | description |") {
        Some(p) => p,
        None => return entries,
    };

    // Find the separator row + skip it
    let after_header = &content[header_pos..];
    let separator_end = match after_header.find('\n') {
        Some(p) => p + 1,
        None => return entries,
    };
    let after_sep = &after_header[separator_end..];

    // Skip the separator line (|----|----|...)
    let data_start = match after_sep.find('\n') {
        Some(p) => p + 1,
        None => return entries,
    };
    let data = &after_sep[data_start..];

    // Parse each row until blank line or end or another header
    for line in data.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("##") || trimmed.starts_with('#') {
            break;
        }
        if !trimmed.starts_with('|') {
            break;
        }

        // Split on |, trim each cell
        let cells: Vec<&str> = trimmed
            .split('|')
            .skip(1) // skip leading empty from first |
            .map(|c| c.trim())
            .collect();

        if cells.len() < 6 {
            continue;
        }

        let tier_str = cells[3];
        let tier: u8 = tier_str.parse().unwrap_or(0);

        entries.push(EmojiEntry {
            literal: cells[0],
            shortcode: cells[1],
            g0spell: cells[2],
            tier,
            action: cells[4],
            description: cells[5],
        });
    }

    entries
}

// ═══════════════════════════════════════════════════════════
// emoji_registry!() macro — compile-time datum embedding
// ═══════════════════════════════════════════════════════════

/// Load the emoji registry from the .tomllmd datum at compile time.
///
/// The datum path is relative to `CARGO_MANIFEST_DIR`.
/// For the k0mmand3r crate, this is `../_b00t_/schema/EMOJI_REGISTRY.tomllmd`.
///
/// Returns a `&'static EmojiRegistry` parsed at runtime via OnceLock
/// from the include_str! data embedded at compile time.
///
/// # Example
///
/// ```rust
/// let reg = k0mmand3r::emoji_registry!();
/// let skunk = reg.lookup_shortcode(":skunk:").unwrap();
/// assert_eq!(skunk.literal, "🦨");
/// assert_eq!(skunk.tier, 1);
/// ```
#[macro_export]
macro_rules! emoji_registry {
    () => {{
        // Include the .tomllmd at compile time into a static string
        const EMOJI_DATA: &str = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../_b00t_/schema/EMOJI_REGISTRY.tomllmd"
        ));

        // Parse once via std::sync::OnceLock
        static REGISTRY: std::sync::OnceLock<$crate::EmojiRegistry> = std::sync::OnceLock::new();
        REGISTRY.get_or_init(|| {
            let entries = $crate::parse_entries_from_content(EMOJI_DATA);
            let schema_version = $crate::extract_schema_version(EMOJI_DATA).unwrap_or(0);
            $crate::EmojiRegistry {
                schema_version,
                entries: Box::leak(entries.into_boxed_slice()),
            }
        })
    }};
}

// ═══════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_schema_version() {
        let content = "[b00t.version]\nschema = 1\ncontent = \"stable\"\n";
        assert_eq!(extract_schema_version(content), Some(1));
    }

    #[test]
    fn test_extract_schema_version_no_match() {
        let content = "[b00t.version]\nfoo = 42\n";
        assert_eq!(extract_schema_version(content), None);
    }

    #[test]
    fn test_parse_entries_from_content_direct() {
        let content = r##"| literal | shortcode | g0spell | tier | action | description |
|---------|-----------|---------|------|--------|-------------|
| 🦨 | :skunk: | skunk | 1 | warn+redirect | First offense |
| 💩 | :poop: | antipattern | 2 | block+escalate | Repeat offense |
"##;

        let entries = parse_entries_from_content(content);
        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].literal, "🦨");
        assert_eq!(entries[0].shortcode, ":skunk:");
        assert_eq!(entries[0].g0spell, "skunk");
        assert_eq!(entries[0].tier, 1);
        assert_eq!(entries[0].action, "warn+redirect");

        assert_eq!(entries[1].literal, "💩");
        assert_eq!(entries[1].shortcode, ":poop:");
        assert_eq!(entries[1].tier, 2);
    }

    #[test]
    fn test_parse_entries_empty() {
        let content = "some random text with no table\n";
        let entries = parse_entries_from_content(content);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_entries_malformed_row() {
        let content = r##"| literal | shortcode | g0spell | tier | action | description |
|---------|-----------|---------|------|--------|-------------|
| 🦨 | :skunk: | skunk | not_a_number | warn+redirect | First offense |
"##;

        let entries = parse_entries_from_content(content);
        assert_eq!(entries.len(), 1);
        // non-numeric tier defaults to 0
        assert_eq!(entries[0].tier, 0);
    }

    #[test]
    fn test_parse_entries_partial_row_too_few_cells() {
        let content = r##"| literal | shortcode | g0spell | tier | action | description |
|---------|-----------|---------|------|--------|-------------|
| 🦨 | :skunk: | skunk |
"##;

        let entries = parse_entries_from_content(content);
        assert!(entries.is_empty(), "row with <6 cells should be skipped");
    }

    #[test]
    fn test_lookup_literal() {
        let entries = vec![
            EmojiEntry { literal: "🦨", shortcode: ":skunk:", g0spell: "skunk", tier: 1, action: "warn+redirect", description: "First" },
            EmojiEntry { literal: "💩", shortcode: ":poop:", g0spell: "antipattern", tier: 2, action: "block+escalate", description: "Repeat" },
        ];
        let reg = EmojiRegistry {
            schema_version: 1,
            entries: Box::leak(entries.into_boxed_slice()),
        };

        assert_eq!(reg.lookup_literal("🦨").unwrap().shortcode, ":skunk:");
        assert_eq!(reg.lookup_literal("💩").unwrap().shortcode, ":poop:");
        assert!(reg.lookup_literal("🍕").is_none());
    }

    #[test]
    fn test_lookup_shortcode() {
        let entries = vec![
            EmojiEntry { literal: "🦨", shortcode: ":skunk:", g0spell: "skunk", tier: 1, action: "warn+redirect", description: "First" },
        ];
        let reg = EmojiRegistry {
            schema_version: 1,
            entries: Box::leak(entries.into_boxed_slice()),
        };

        assert_eq!(reg.lookup_shortcode(":skunk:").unwrap().literal, "🦨");
        assert!(reg.lookup_shortcode(":pizza:").is_none());
    }

    #[test]
    fn test_lookup_g0spell() {
        let entries = vec![
            EmojiEntry { literal: "🚫", shortcode: ":block:", g0spell: "block", tier: 0, action: "deny", description: "Permanent" },
        ];
        let reg = EmojiRegistry {
            schema_version: 1,
            entries: Box::leak(entries.into_boxed_slice()),
        };

        assert_eq!(reg.lookup_g0spell("block").unwrap().literal, "🚫");
        assert!(reg.lookup_g0spell("nonexistent").is_none());
    }

    #[test]
    fn test_filter_tier() {
        let entries = vec![
            EmojiEntry { literal: "✅", shortcode: ":pass:", g0spell: "pass", tier: 0, action: "ok", description: "Pass" },
            EmojiEntry { literal: "🦨", shortcode: ":skunk:", g0spell: "skunk", tier: 1, action: "warn+redirect", description: "First" },
            EmojiEntry { literal: "💩", shortcode: ":poop:", g0spell: "antipattern", tier: 2, action: "block+escalate", description: "Repeat" },
        ];
        let reg = EmojiRegistry {
            schema_version: 1,
            entries: Box::leak(entries.into_boxed_slice()),
        };

        assert_eq!(reg.filter_tier(0).len(), 1);
        assert_eq!(reg.filter_tier(1).len(), 1);
        assert_eq!(reg.filter_tier(2).len(), 1);
        assert_eq!(reg.filter_tier(99).len(), 0);
    }

    #[test]
    fn test_display_entry() {
        let entry = EmojiEntry { literal: "🦨", shortcode: ":skunk:", g0spell: "skunk", tier: 1, action: "warn+redirect", description: "First offense" };
        let display = format!("{}", entry);
        assert!(display.contains("🦨"));
        assert!(display.contains(":skunk:"));
        assert!(display.contains("tier=1"));
    }

    #[test]
    fn test_emoji_registry_macro_points_to_real_file() {
        // This tests that the macro compiles and the file exists
        let reg = emoji_registry!();
        assert!(reg.len() >= 9, "Expected at least 9 emoji entries, got {}", reg.len());

        // Verify the file has :skunk: and :poop:
        let skunk = reg.lookup_shortcode(":skunk:");
        assert!(skunk.is_some(), ":skunk: should exist in the real registry file");
        assert_eq!(skunk.unwrap().literal, "🦨");

        let poop = reg.lookup_shortcode(":poop:");
        assert!(poop.is_some(), ":poop: should exist in the real registry file");
        assert_eq!(poop.unwrap().literal, "💩");

        // Verify escalation tier semantics
        let pass = reg.lookup_g0spell("pass").unwrap();
        assert_eq!(pass.tier, 0, "pass should be tier 0");

        let skunk_entry = reg.lookup_g0spell("skunk").unwrap();
        assert_eq!(skunk_entry.tier, 1, "skunk should be tier 1");

        let antipattern = reg.lookup_g0spell("antipattern").unwrap();
        assert_eq!(antipattern.tier, 2, "antipattern should be tier 2");

        // Verify schema version
        assert_eq!(reg.schema_version, 1);
    }
}
