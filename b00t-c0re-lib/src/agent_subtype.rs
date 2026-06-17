/// Cardinal agent subtypes — classify agents by deployment modality.
///
/// Subtypes are encoded in `type_tags` (content classification), NOT in `DatumType`
/// (which encodes file structure only per b00t DatumType policy).
///
/// An agent `.agent.toml` includes a tag like `"cli"`, `"sdk"`, `"ide.vsix"`, or `"gui"`.
/// The `AgentSubtype` enum provides typed access with display labels for the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentSubtype {
    /// Command-line interface agent (b00t-cli, bash wrappers, REPL agents).
    Cli,
    /// Library/SDK agent — embedded in application code or SDKs.
    Sdk,
    /// VS Code extension agent (`.vsix` packaged).
    IdeVsix,
    /// GUI application agent (Electron, Tauri, etc.).
    Gui,
    /// Subtype not specified or not recognized.
    #[default]
    Unknown,
}

impl AgentSubtype {
    /// Extract `AgentSubtype` from a slice of `type_tags`.
    ///
    /// Checks for `"cli"`, `"sdk"`, `"ide.vsix"` / `"vsix"`, `"gui"` in that order.
    pub fn from_type_tags(tags: &[impl AsRef<str>]) -> Self {
        for tag in tags {
            match tag.as_ref() {
                "cli" => return Self::Cli,
                "sdk" => return Self::Sdk,
                "ide.vsix" | "vsix" => return Self::IdeVsix,
                "gui" => return Self::Gui,
                _ => {}
            }
        }
        Self::Unknown
    }

    /// Short display label for dashboard output.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Sdk => "sdk",
            Self::IdeVsix => "ide.vsix",
            Self::Gui => "gui",
            Self::Unknown => "-",
        }
    }

    /// Icon for concise dashboard display.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Cli => "🖥",
            Self::Sdk => "📦",
            Self::IdeVsix => "🔌",
            Self::Gui => "🖼",
            Self::Unknown => "?",
        }
    }

    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

impl std::fmt::Display for AgentSubtype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_tag_recognized() {
        let tags = vec!["agent", "cli"];
        assert_eq!(AgentSubtype::from_type_tags(&tags), AgentSubtype::Cli);
    }

    #[test]
    fn sdk_tag_recognized() {
        assert_eq!(
            AgentSubtype::from_type_tags(&["agent", "sdk"]),
            AgentSubtype::Sdk
        );
    }

    #[test]
    fn vsix_alt_spelling() {
        assert_eq!(
            AgentSubtype::from_type_tags(&["vsix"]),
            AgentSubtype::IdeVsix
        );
        assert_eq!(
            AgentSubtype::from_type_tags(&["ide.vsix"]),
            AgentSubtype::IdeVsix
        );
    }

    #[test]
    fn gui_tag_recognized() {
        assert_eq!(
            AgentSubtype::from_type_tags(&["gui"]),
            AgentSubtype::Gui
        );
    }

    #[test]
    fn unknown_when_no_subtype_tag() {
        assert_eq!(
            AgentSubtype::from_type_tags(&["agent", "frontier"]),
            AgentSubtype::Unknown
        );
    }

    #[test]
    fn empty_tags_unknown() {
        let empty: Vec<&str> = vec![];
        assert_eq!(AgentSubtype::from_type_tags(&empty), AgentSubtype::Unknown);
    }

    #[test]
    fn display_labels() {
        assert_eq!(AgentSubtype::Cli.label(), "cli");
        assert_eq!(AgentSubtype::IdeVsix.label(), "ide.vsix");
        assert_eq!(AgentSubtype::Unknown.label(), "-");
    }

    #[test]
    fn first_match_wins() {
        // cli appears before sdk — cli wins
        assert_eq!(
            AgentSubtype::from_type_tags(&["cli", "sdk"]),
            AgentSubtype::Cli
        );
    }
}
