//! Type registry macro — single source of truth for typed `.tomllm`/`.toml` file systems.
//!
//! ## The problem
//! Any datum/typed-TOML system needs to map type variants to file suffixes in multiple places:
//! enum definition, `Display`, serde renames, filename detection, file search order.
//! Without a macro, that mapping is duplicated 5+ times — each update requires N edits.
//!
//! ## The solution: `define_typed_registry!`
//! Declare your type table once. The macro generates:
//! - The enum with correct serde renames (no `rename_all` footguns)
//! - `base_suffix(&self) -> Option<&'static str>` — None for Unknown/fallback
//! - `all_base_suffixes() -> &'static [&'static str]` — for ordered file search
//! - `from_filename(filename: &str) -> Self` — detects .tomllm and .toml transparently
//! - `Display` — using the display field
//! - `TomllmRegistry` impl — enables generic `Loader` operations
//!
//! ## Usage
//! ```rust,ignore
//! use tomllm::define_typed_registry;
//!
//! define_typed_registry! {
//!     /// My typed datum system
//!     pub enum MyType {
//!         //  Variant      base suffix    serde name       display
//!         Database      => ".database"  [ serde="database",    display="database"   ],
//!         Role          => ".role"      [ serde="role",         display="role"       ],
//!         Agent         => ".agent"     [ serde="agent",        display="agent"      ],
//!         HiveProfile   => ".hive"      [ serde="hive_profile", display="hive_profile" ],
//!     }
//! }
//!
//! // Generated methods:
//! let base = MyType::Role.base_suffix();           // Some(".role")
//! let all  = MyType::all_base_suffixes();          // [".database", ".role", ".agent", ".hive"]
//! let t    = MyType::from_filename("foo.role.tomllm"); // MyType::Role
//! ```
//!
//! ## Integration with `tomllm::loader`
//! ```rust,ignore
//! use tomllm::loader::resolve_path;
//! // Load any type — .tomllm first, .toml fallback, auto-detected:
//! for base in MyType::all_base_suffixes() {
//!     if let Some(path) = resolve_path(dir, "executive", base) {
//!         let cfg: MyConfig = toml::from_str(&std::fs::read_to_string(path)?)?;
//!         return Ok(cfg);
//!     }
//! }
//! ```

/// Trait implemented by enums generated with `define_typed_registry!`.
/// Enables generic file resolution without knowing the concrete type.
pub trait TomllmRegistry: Sized + 'static {
    /// Base file suffix for this type (e.g. `".role"`).
    /// `None` for the fallback/unknown variant — resolves to plain `.tomllm`/`.toml`.
    fn base_suffix(&self) -> Option<&'static str>;

    /// All base suffixes in declaration order (most-specific first).
    /// Used to drive ordered file search: for each base try `.tomllm` then `.toml`.
    fn all_base_suffixes() -> &'static [&'static str];

    /// Detect type from filename by checking suffixes.
    /// Transparently matches both `.tomllm` and `.toml` variants.
    fn from_filename(filename: &str) -> Self;
}

/// Generate a typed registry enum with all boilerplate derived from a single declaration.
///
/// Each entry: `Variant => "base_suffix" [ serde="name", display="text" ]`
/// A special `_Unknown` fallback is always appended with `base_suffix() = None`.
///
/// See module docs for full usage example.
#[macro_export]
macro_rules! define_typed_registry {
    (
        $(#[$meta:meta])*
        $vis:vis enum $Name:ident {
            $(
                $(#[$vmeta:meta])*
                $Variant:ident => $base:literal [ serde=$serde_name:literal, display=$display:literal ]
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(::serde::Deserialize, ::serde::Serialize, ::std::fmt::Debug, ::std::clone::Clone, ::std::cmp::PartialEq, ::std::default::Default)]
        $vis enum $Name {
            /// Fallback — no typed extension; resolves to plain .tomllm / .toml
            #[serde(rename = "unknown")]
            #[default]
            Unknown,
            $(
                $(#[$vmeta])*
                #[serde(rename = $serde_name)]
                $Variant,
            )*
        }

        impl $Name {
            /// Base suffix for this variant's typed file extension.
            /// `None` for `Unknown` — caller should fall back to plain `.tomllm`/`.toml`.
            pub fn base_suffix(&self) -> ::std::option::Option<&'static str> {
                match self {
                    $Name::Unknown => None,
                    $($Name::$Variant => Some($base),)*
                }
            }

            /// All base suffixes in declaration order.
            /// 🤓 iterate this + `tomllm::loader::resolve_path` to implement typed file search.
            pub fn all_base_suffixes() -> &'static [&'static str] {
                &[$($base,)*]
            }

            /// Detect type from filename — matches `.tomllm` and `.toml` transparently.
            /// Returns `Unknown` if no suffix matches.
            pub fn from_filename(filename: &str) -> Self {
                $(
                    if filename.ends_with(::std::concat!($base, ".tomllm"))
                        || filename.ends_with(::std::concat!($base, ".toml"))
                    {
                        return $Name::$Variant;
                    }
                )*
                $Name::Unknown
            }
        }

        impl ::std::fmt::Display for $Name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                match self {
                    $Name::Unknown => write!(f, "unknown"),
                    $($Name::$Variant => write!(f, $display),)*
                }
            }
        }

        impl $crate::registry::TomllmRegistry for $Name {
            fn base_suffix(&self) -> ::std::option::Option<&'static str> {
                $Name::base_suffix(self)
            }
            fn all_base_suffixes() -> &'static [&'static str] {
                $Name::all_base_suffixes()
            }
            fn from_filename(filename: &str) -> Self {
                $Name::from_filename(filename)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::TomllmRegistry;

    define_typed_registry! {
        /// Test registry
        pub enum TestType {
            Role        => ".role"      [ serde="role",         display="role"        ],
            Agent       => ".agent"     [ serde="agent",        display="agent"       ],
            HiveProfile => ".hive"      [ serde="hive_profile", display="hive_profile" ],
            AiModel     => ".ai_model"  [ serde="ai_model",     display="ai_model"    ],
            Ai          => ".ai"        [ serde="ai",           display="AI"          ],
        }
    }

    #[test]
    fn test_base_suffix() {
        assert_eq!(TestType::Role.base_suffix(), Some(".role"));
        assert_eq!(TestType::HiveProfile.base_suffix(), Some(".hive"));
        assert_eq!(TestType::Unknown.base_suffix(), None);
    }

    #[test]
    fn test_all_base_suffixes_ordered() {
        let bases = TestType::all_base_suffixes();
        assert_eq!(bases[0], ".role");
        assert_eq!(bases[2], ".hive");
        assert!(bases.contains(&".ai_model"));
        assert!(bases.contains(&".ai"));
    }

    #[test]
    fn test_from_filename_tomllm() {
        assert_eq!(TestType::from_filename("executive.role.tomllm"), TestType::Role);
        assert_eq!(TestType::from_filename("foo.hive.tomllm"), TestType::HiveProfile);
        assert_eq!(TestType::from_filename("bar.baz"), TestType::Unknown);
    }

    #[test]
    fn test_from_filename_toml() {
        assert_eq!(TestType::from_filename("executive.role.toml"), TestType::Role);
        assert_eq!(TestType::from_filename("ralph.agent.toml"), TestType::Agent);
    }

    #[test]
    fn test_from_filename_no_ambiguity() {
        // .ai_model.toml must NOT match .ai suffix
        assert_eq!(TestType::from_filename("foo.ai_model.toml"), TestType::AiModel);
        assert_eq!(TestType::from_filename("foo.ai.toml"), TestType::Ai);
    }

    #[test]
    fn test_display() {
        assert_eq!(TestType::HiveProfile.to_string(), "hive_profile");
        assert_eq!(TestType::Unknown.to_string(), "unknown");
        assert_eq!(TestType::Ai.to_string(), "AI");
    }

    #[test]
    fn test_serde_roundtrip() {
        // serde: "hive_profile" deserializes to HiveProfile (not "hiveprofile")
        let t: TestType = toml::from_str("value = \"hive_profile\"")
            .ok()
            .and_then(|v: toml::Value| toml::from_str(&format!("x = {:?}", v.get("value").unwrap().as_str().unwrap())).ok())
            .unwrap_or_else(|| {
                // Direct serde test
                let s = format!("\"{}\"", "hive_profile");
                serde_json::from_str::<TestType>(&s).unwrap_or(TestType::Unknown)
            });
        // Direct JSON serde test is cleaner:
        let t2: TestType = serde_json::from_str("\"hive_profile\"").unwrap();
        assert_eq!(t2, TestType::HiveProfile);
        let t3: TestType = serde_json::from_str("\"ai_model\"").unwrap();
        assert_eq!(t3, TestType::AiModel);
        let t4: TestType = serde_json::from_str("\"unknown\"").unwrap();
        assert_eq!(t4, TestType::Unknown);
    }

    #[test]
    fn test_trait_object_usage() {
        // Verify TomllmRegistry trait works
        fn check_suffix<R: TomllmRegistry>(t: &R) -> bool {
            t.base_suffix().is_some()
        }
        assert!(check_suffix(&TestType::Role));
        assert!(!check_suffix(&TestType::Unknown));
    }
}
