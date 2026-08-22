use serde::{Deserialize, Serialize};
use ufo_types::{Stereotyped, UfoStereotype};

// ── DatumType — b00t's typed datum registry ────────────────────────────────
// 🤓 single source of truth: add new variants ONLY here. The macro below derives:
//    from_type_token, base_suffix, all_base_suffixes, from_filename, extension_for_type.
//    DO NOT add manual match arms elsewhere — use the generated methods.
//
// 🎨 Display classification: each variant belongs to one SemanticClass.
//    Shape, color, icon are derived from the class — NO per-variant hardcoding.
//    New variants: just pick a class.  Runtime registration: toggle tracing per class.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatumType {
    Database,
    HiveProfile,
    Agent,
    Config,
    Docker,
    Skill,
    Stack,
    Repo,
    Role,
    Bash,
    Vscode,
    K8s,
    /// Raw Kubernetes Pod manifest deployed via `podman kube play` — no Helm,
    /// no docker-compose, no cluster. See datum_podman.rs.
    Podman,
    /// SysML v2 LSP + bundled MCP server wrapper (daltskin/sysml-v2-lsp, npm).
    /// See datum_sysml_v2_lsp.rs — spike, not yet wired into commands/mcp.rs's
    /// install/sync flow (use a generic `.mcp.toml`/DatumType::Mcp datum for
    /// that today, same as `_b00t_/ssh-mcp.mcp.toml`).
    SysmlV2Lsp,
    Apt,
    Nix,
    Mcp,
    Cli,
    Api,
    Job,
    Ai,
    Justfile,
    Pipeline,
    Hardware,
    Overlay,
    /// Encrypted credential datum — `.credential.toml` (encrypted at rest via OS keyring).
    /// 🤓 Stores cloud provider access keys (R2, S3, OpenAI, etc.). Queryable via datum system.
    ///    Agents discover available credentials with: b00t datum list --type credential
    ///    Encryption key lives in OS keyring (b00t/master-key), never on disk.
    Credential,
    /// Polyseme — resolves to multiple concrete datums from one topic name
    Polyseme,
    Runtime,
    Training,
    McpServer,
    Schema,
    Hook,
    Gate,
    Plan,
    Vendor,
    Ooda,
    Unknown,
}

// ── Semantic classification — the display derives from what a type IS ─────
// 🤓 Single reasonable default per class.  New variants: just pick a class.
//    Runtime registration: SemanticClass::tracing_enabled() per class.

/// Semantic taxonomy for datum types.  Shape/color/icon are derived from class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SemanticClass {
    /// Infrastructure — nodes that provision, host, or configure systems
    Infra,
    /// Agent — autonomous actors, roles, models, learning
    Agent,
    /// Protocol — MCP servers, APIs, schemas, wire formats
    Protocol,
    /// Skill — executable capabilities, jobs, hooks, gates
    Skill,
    /// Tool — CLI tools, configs, build scripts, plans, vendored deps
    Tool,
    /// Repo — source trees, workspaces, editor configs, packages
    Repo,
    /// Data — databases, store profiles
    Data,
    /// Secret — encrypted credentials, polyseme black boxes
    Secret,
    /// Fallback for unclassified or incubating types
    Unknown,
}

// ── SVG shape templates ────────────────────────────────────────────────────
const SVG_CIRCLE: &str = "<circle r='24' cx='28' cy='28' />";
const SVG_RECTANGLE: &str = "<rect x='4' y='4' width='48' height='48' rx='8' />";
const SVG_DIAMOND: &str = "<polygon points='28,4 52,28 28,52 4,28' />";
const SVG_HEXAGON: &str = "<polygon points='28,4 48,16 48,40 28,52 8,40 8,16' />";
const SVG_TRIANGLE: &str = "<polygon points='28,6 50,48 6,48' />";
const SVG_VEE: &str = "<polygon points='4,4 28,52 52,4' />";

impl SemanticClass {
    /// Shape for graph/chart rendering.
    pub const fn shape(&self) -> &'static str {
        match self {
            Self::Infra => "hexagon",
            Self::Agent => "circle",
            Self::Protocol => "diamond",
            Self::Skill => "triangle",
            Self::Tool => "rectangle",
            Self::Repo => "vee",
            Self::Data => "rectangle",
            Self::Secret => "circle",
            Self::Unknown => "rectangle",
        }
    }

    /// Fill color (hex).
    pub const fn color(&self) -> &'static str {
        match self {
            Self::Infra => "#326ce5",
            Self::Agent => "#059669",
            Self::Protocol => "#7c3aed",
            Self::Skill => "#d97706",
            Self::Tool => "#0d9488",
            Self::Repo => "#be123c",
            Self::Data => "#475569",
            Self::Secret => "#1e293b",
            Self::Unknown => "#1e293b",
        }
    }

    /// Border/stroke color.
    pub const fn border_color(&self) -> &'static str {
        match self {
            Self::Infra => "#5b9cf5",
            Self::Agent => "#34d399",
            Self::Protocol => "#a78bfa",
            Self::Skill => "#fbbf24",
            Self::Tool => "#2dd4bf",
            Self::Repo => "#fb7185",
            Self::Data => "#94a3b8",
            Self::Secret => "#475569",
            Self::Unknown => "#475569",
        }
    }

    /// Emoji or unicode icon for compact rendering.
    pub const fn icon(&self) -> &'static str {
        match self {
            Self::Infra => "☸",
            Self::Agent => "🤖",
            Self::Protocol => "🔌",
            Self::Skill => "🛠️",
            Self::Tool => "⌨️",
            Self::Repo => "📁",
            Self::Data => "🗄️",
            Self::Secret => "🔐",
            Self::Unknown => "❓",
        }
    }

    /// SVG shape template fragment.
    pub const fn svg_template(&self) -> &'static str {
        match self {
            Self::Infra => SVG_HEXAGON,
            Self::Agent => SVG_CIRCLE,
            Self::Protocol => SVG_DIAMOND,
            Self::Skill => SVG_TRIANGLE,
            Self::Tool => SVG_RECTANGLE,
            Self::Repo => SVG_VEE,
            Self::Data => SVG_RECTANGLE,
            Self::Secret => SVG_CIRCLE,
            Self::Unknown => SVG_RECTANGLE,
        }
    }

    /// CSS class for styling hooks.
    pub const fn css_class(&self) -> &'static str {
        match self {
            Self::Infra => "sc-infra",
            Self::Agent => "sc-agent",
            Self::Protocol => "sc-protocol",
            Self::Skill => "sc-skill",
            Self::Tool => "sc-tool",
            Self::Repo => "sc-repo",
            Self::Data => "sc-data",
            Self::Secret => "sc-secret",
            Self::Unknown => "sc-unknown",
        }
    }
}

impl DatumType {
    /// Classify this datum type into its semantic class.
    /// 🎨 This is the ONLY place variant→class mapping lives.
    ///    New variants: add one line here. No other code changes needed.
    pub const fn semantic_class(&self) -> SemanticClass {
        match self {
            Self::K8s
            | Self::Docker
            | Self::Podman
            | Self::Hardware
            | Self::Overlay
            | Self::Runtime
            | Self::Nix => SemanticClass::Infra,
            Self::Agent | Self::Role | Self::Ai | Self::Training => SemanticClass::Agent,
            Self::Mcp | Self::McpServer | Self::Api | Self::Schema | Self::SysmlV2Lsp => {
                SemanticClass::Protocol
            }
            Self::Skill | Self::Job | Self::Hook | Self::Gate | Self::Pipeline => {
                SemanticClass::Skill
            }
            Self::Config
            | Self::Bash
            | Self::Cli
            | Self::Justfile
            | Self::Plan
            | Self::Vendor
            | Self::Ooda => SemanticClass::Tool,
            Self::Stack | Self::Repo | Self::Vscode | Self::Apt => SemanticClass::Repo,
            Self::Database | Self::HiveProfile => SemanticClass::Data,
            Self::Polyseme | Self::Credential => SemanticClass::Secret,
            Self::Unknown => SemanticClass::Unknown,
        }
    }

    /// Stereotype hierarchy: which types does this type imply?
    /// e.g. McpServer implies Mcp (server → protocol), Runtime implies Cli (can run → can check)
    ///
    /// 🤓 NOT the same relation as [`ufo_stereotype()`](Stereotyped::ufo_stereotype)'s
    ///    Kind/SubKind lattice below. `implies()` is capability entailment
    ///    ("having this type means you also have that capability"); the UFO
    ///    lattice is structural subtyping (Guizzardi 2005 §4.2.1–4.2.2). The
    ///    two may diverge — see `ufo_stereotype()`'s doc comment for the
    ///    inverse cross-reference and the one point where they're pinned to
    ///    agree (`McpServer`).
    pub const fn implies(&self) -> &'static [DatumType] {
        match self {
            Self::McpServer => &[Self::Mcp],
            Self::Runtime => &[Self::Cli],
            Self::Agent => &[Self::Runtime],
            Self::Ai => &[Self::Agent],
            Self::Role => &[Self::Agent],
            _ => &[],
        }
    }

    /// Is this type implied by (i.e., less specific than) another?
    pub fn is_implied_by(&self, other: &DatumType) -> bool {
        other.implies().contains(self)
    }

    // 🎨 display() defined below with DatumDisplay struct
}

impl Stereotyped for DatumType {
    /// UFO Kind/SubKind lattice for datum types (Guizzardi 2005 §4.2.1–4.2.2).
    /// 🤓 SINGLE SOURCE OF TRUTH — this is the ONLY place the lattice is
    ///    declared. Callers (ontology sparql, BootDatum) delegate here;
    ///    do not re-derive parent/child relationships anywhere else.
    ///
    /// Most variants are their own rigid Kind. Only variants with a clear,
    /// pre-existing structural relationship are modeled as SubKind. This is
    /// a DIFFERENT, stricter relation than `implies()` above (capability
    /// entailment) — they may diverge; see `implies()`'s doc comment.
    fn ufo_stereotype(&self) -> UfoStereotype {
        match self {
            // ── container/orchestration engines — SubKind of abstract ContainerRuntime ──
            Self::Docker => UfoStereotype::SubKind {
                name: "Docker".into(),
                parent: "ContainerRuntime".into(),
            },
            Self::Podman => UfoStereotype::SubKind {
                name: "Podman".into(),
                parent: "ContainerRuntime".into(),
            },
            Self::K8s => UfoStereotype::SubKind {
                name: "K8s".into(),
                parent: "ContainerRuntime".into(),
            },

            // ── executable surface — SubKind of abstract Executable ─────────────────────
            Self::Cli => UfoStereotype::SubKind {
                name: "Cli".into(),
                parent: "Executable".into(),
            },
            Self::Bash => UfoStereotype::SubKind {
                name: "Bash".into(),
                parent: "Executable".into(),
            },
            Self::Justfile => UfoStereotype::SubKind {
                name: "Justfile".into(),
                parent: "Executable".into(),
            },

            // ── package managers — SubKind of abstract PackageManager ───────────────────
            Self::Apt => UfoStereotype::SubKind {
                name: "Apt".into(),
                parent: "PackageManager".into(),
            },
            Self::Nix => UfoStereotype::SubKind {
                name: "Nix".into(),
                parent: "PackageManager".into(),
            },

            // ── MCP: McpServer is-a Mcp (matches implies(): McpServer => [Mcp]) ──────────
            // Deliberately using Mcp itself as parent (not inventing an abstract "MCP"
            // label) — Mcp is already a real, addressable Kind elsewhere in the codebase;
            // a third parallel label would duplicate it (see #905's warning against
            // parallel vocabularies).
            Self::McpServer => UfoStereotype::SubKind {
                name: "McpServer".into(),
                parent: "Mcp".into(),
            },

            // ── everything else: rigid Kind, name = variant name (Debug format gives
            //    exact variant name for fieldless enum variants — zero maintenance,
            //    stays in sync automatically as variants are added) ────────────────────
            other => UfoStereotype::Kind(format!("{other:?}")),
        }
    }
}

macro_rules! datum_type_table {
    ($($variant:ident => [$($token:literal),+] => $suffix:literal),* $(,)?) => {
        /// Map a TOML type token string to a DatumType variant; returns None for unknown tokens.
        pub fn from_type_token(s: &str) -> Option<Self> {
            match s {
                $($($token => Some(Self::$variant),)+)*
                _ => None,
            }
        }

        /// File suffix for this datum type (e.g., ".cli", ".mcp").
        pub fn base_suffix(&self) -> &'static str {
            match self {
                $(Self::$variant => $suffix,)*
                Self::Unknown => ".toml",
            }
        }

        /// All known base suffixes (excluding Unknown).
        pub fn all_base_suffixes() -> Vec<&'static str> {
            vec![$($suffix,)*]
        }

        /// All non-Unknown variants as a const slice.
        pub fn all_variants() -> &'static [Self] {
            &[$(Self::$variant,)*]
        }

        /// Determine DatumType from a filename (e.g. "mold.cli.toml" → Cli).
        /// 🤓 legacy .ai_model.toml suffix handled explicitly — model is sub-class of Ai.
        pub fn from_filename(filename: &str) -> Self {
            for t in Self::all_variants() {
                let base = t.base_suffix();
                if filename.ends_with(base)
                    || filename.ends_with(&format!("{base}.toml"))
                    || filename.ends_with(&format!("{base}.tomllmd"))
                    || filename.ends_with(&format!("{base}.tomllm"))
                {
                    return *t;
                }
            }
            // Legacy: .ai_model.toml / .ai_model.tomllmd / .ai_model.tomllm → Ai
            if filename.ends_with(".ai_model.toml")
                || filename.ends_with(".ai_model.tomllmd")
                || filename.ends_with(".ai_model.tomllm")
                || filename.ends_with(".ai_model")
            {
                return Self::Ai;
            }
            Self::Unknown
        }

        /// Preferred file extension for this type (e.g., ".cli.toml").
        pub fn extension(&self) -> &'static str {
            match self {
                $(Self::$variant => concat!($suffix, ".toml"),)*
                Self::Unknown => ".toml",
            }
        }
    };
}

impl DatumType {
    datum_type_table! {
        Database    => ["database", "db"]             => ".database",
        HiveProfile => ["hive", "hive_profile"]      => ".hive",
        Agent       => ["agent"]                     => ".agent",
        Config      => ["config"]                    => ".config",
        Docker      => ["docker"]                    => ".docker",
        Skill       => ["skill"]                     => ".skill",
        Stack       => ["stack"]                     => ".stack",
        Repo        => ["repo"]                      => ".repo",
        Role        => ["role"]                      => ".role",
        Bash        => ["bash"]                      => ".bash",
        Vscode      => ["vscode"]                    => ".vscode",
        K8s         => ["k8s"]                       => ".k8s",
        Podman      => ["podman", "podman_kube", "kube"] => ".podman",
        SysmlV2Lsp  => ["sysml_v2_lsp", "sysml-v2-lsp", "sysml2lsp"] => ".sysml_v2_lsp",
        Apt         => ["apt"]                       => ".apt",
        Nix         => ["nix"]                       => ".nix",
        Mcp         => ["mcp"]                       => ".mcp",
        Cli         => ["cli", "verifier"]           => ".cli",
        Api         => ["api"]                       => ".api",
        Job         => ["job"]                       => ".job",
        // Ai is the umbrella; model/ai_model tokens map here (reverse dot: name.model.ai.tomllmd)
        Ai          => ["ai", "model", "ai_model"]   => ".ai",
        Justfile    => ["justfile"]                  => ".justfile",
        Pipeline    => ["pipeline"]                  => ".pipeline",
        Hardware    => ["hardware"]                  => ".hardware",
        Overlay     => ["overlay"]                   => ".overlay",
        Credential  => ["credential", "credentials"]  => ".credential",
        Polyseme    => ["polyseme"]                  => ".polyseme",
        Runtime     => ["runtime"]                   => ".runtime",
        Training    => ["training"]                  => ".training",
        McpServer   => ["mcp_server", "mcp-server"]  => ".mcp_server",
        Schema      => ["schema"]                    => ".schema",
        Hook        => ["hook"]                      => ".hook",
        Gate        => ["gate"]                      => ".gate",
        Plan        => ["plan"]                      => ".plan",
        Vendor      => ["vendor"]                    => ".vendor",
        Ooda        => ["ooda"]                      => ".ooda",
    }

    /// Preferred file extension for writing new datum files.
    /// Defaults to `{base_suffix}.toml`; special-cases Role (bare .toml),
    /// Justfile (bare .justfile).
    /// 🤓 Model datums use reverse-dot: <name>.model.ai.tomllmd
    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Role => ".toml",
            Self::Justfile => ".justfile",
            Self::Unknown => ".toml",
            other => other.extension(),
        }
    }
}

impl DatumType {
    /// mti TypeID prefix inferred from base_suffix() — e.g. ".skill" → "skill", ".mcp" → "mcp".
    /// No manual mapping needed; stays in sync with datum_type_table! automatically.
    pub fn type_prefix(&self) -> &'static str {
        self.base_suffix().trim_start_matches('.')
    }

    /// Type-graph nodes for all DatumType variants, auto-derived from datum_type_table!.
    /// Zero-maintenance: adding a new variant automatically appears here.
    pub fn datum_nodes() -> Vec<b00t_reflect_types::HolonNode> {
        Self::all_variants()
            .iter()
            .map(|v| b00t_reflect_types::HolonNode {
                id: format!("datum_type::{}", v.type_prefix()),
                label: v.type_prefix().to_string(),
                kind: "datum_type".to_string(),
                z_layer: None,
                semantic_type: None,
            })
            .collect()
    }
}

impl std::fmt::Display for DatumType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.base_suffix())
    }
}

/// Visual display descriptor — derived from [SemanticClass] at compile time.
/// 🤓 Rust types own their display.  Subsystems with code provide CSS animations
///    via css_class.  No TOML overrides — code is the single source of truth.
#[derive(Serialize, Debug, Clone)]
pub struct DatumDisplay {
    pub shape: String,
    pub color: String,
    pub border_color: String,
    pub icon: String,
    pub css_class: String,
    pub svg: String,
}

impl DatumType {
    /// Display descriptor derived from semantic class.
    pub fn display(&self) -> DatumDisplay {
        let sc = self.semantic_class();
        DatumDisplay {
            shape: sc.shape().into(),
            color: sc.color().into(),
            border_color: sc.border_color().into(),
            icon: sc.icon().into(),
            css_class: sc.css_class().into(),
            svg: sc.svg_template().into(),
        }
    }
}

impl DatumDisplay {
    pub fn to_cytoscape_style(&self) -> serde_json::Value {
        serde_json::json!({
            "shape": self.shape,
            "background-color": self.color,
            "border-color": self.border_color,
            "icon": self.icon,
            "css_class": self.css_class,
        })
    }
}

#[cfg(test)]
mod ufo_stereotype_tests {
    use super::*;

    #[test]
    fn every_variant_produces_a_stereotype_without_panicking() {
        for v in DatumType::all_variants() {
            let _ = v.ufo_stereotype();
        }
        let _ = DatumType::Unknown.ufo_stereotype();
    }

    #[test]
    fn container_orchestration_cluster_shares_parent() {
        for v in [DatumType::Docker, DatumType::Podman, DatumType::K8s] {
            match v.ufo_stereotype() {
                UfoStereotype::SubKind { parent, .. } => assert_eq!(parent, "ContainerRuntime"),
                other => panic!("expected SubKind for {v:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn executable_cluster_shares_parent() {
        for v in [DatumType::Cli, DatumType::Bash, DatumType::Justfile] {
            match v.ufo_stereotype() {
                UfoStereotype::SubKind { parent, .. } => assert_eq!(parent, "Executable"),
                other => panic!("expected SubKind for {v:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn package_manager_cluster_shares_parent() {
        for v in [DatumType::Apt, DatumType::Nix] {
            match v.ufo_stereotype() {
                UfoStereotype::SubKind { parent, .. } => assert_eq!(parent, "PackageManager"),
                other => panic!("expected SubKind for {v:?}, got {other:?}"),
            }
        }
    }

    #[test]
    fn mcp_server_is_subkind_of_mcp_matching_implies() {
        // Pins the one point where implies() and the lattice coincide.
        assert_eq!(DatumType::McpServer.implies(), &[DatumType::Mcp]);
        assert_eq!(
            DatumType::McpServer.ufo_stereotype(),
            UfoStereotype::SubKind {
                name: "McpServer".into(),
                parent: "Mcp".into()
            }
        );
    }

    #[test]
    fn unknown_is_plain_kind() {
        assert_eq!(
            DatumType::Unknown.ufo_stereotype(),
            UfoStereotype::Kind("Unknown".into())
        );
    }
}
