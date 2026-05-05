//! # cloudflare-b00t
//!
//! Cloudflare capability provider for b00t — typed ProviderRegistry for agentic
//! discovery of cloud capabilities. Agents query by capability kind, never
//! hardcode provider names.
//!
//! # Why this exists
//! Without a registry, agents must spelunk through datums and traits to discover
//! what cloud providers exist and what they can do. This wastes context and
//! causes "stumbling" — agents trying capabilities that don't exist.
//!
//! # Quick start
//! ```rust,no_run
//! use cloudflare_b00t::registry::{Capability, ProviderRegistry};
//!
//! let registry = ProviderRegistry::global();
//! let inference = registry.find_by_capability(Capability::Inference);
//! for provider in inference {
//!     println!("{} @ {} (priority {})", provider.name, provider.endpoint, provider.priority);
//! }
//! ```

pub mod registry;
pub mod provider;

pub use registry::{Capability, ProviderInfo, ProviderRegistry};
pub use provider::CloudflareProvider;
