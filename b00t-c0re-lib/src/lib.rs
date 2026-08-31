//! # b00t-c0re-lib
//!
//! Core shared library for b00t ecosystem providing:
//! - Template rendering with b00t context variables
//! - Common data structures and utilities
//! - Shared functionality between b00t-cli and b00t-mcp
//! - Single source of truth for version management

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Version management - single source of truth for the b00t ecosystem
pub mod version {
    /// The current version of the b00t ecosystem
    /// 🤓 This is the SINGLE SOURCE OF TRUTH - all other crates reference this
    pub const VERSION: &str = env!("CARGO_PKG_VERSION");

    /// Get the current b00t ecosystem version
    pub fn get() -> &'static str {
        VERSION
    }

    /// Check if we're running a development/git build
    pub fn is_dev_build() -> bool {
        VERSION.contains("git") || VERSION.contains("dev")
    }
}

pub mod aaiii;
pub mod agent_coordination;
pub mod agent_manager;
pub mod agent_subtype;
pub mod ai_artifact;
pub mod ai_capability;
pub mod ai_client;
pub mod ato_client;
pub mod b00t_config;
pub mod context;
pub mod datum_ai_model;
// 🤓 datum_lsp stub removed: superseded by the b00t-lsp crate (real tower-lsp server);
//    the stub bin shared target/debug/b00t-lsp and clobbered it on rebuild.
pub mod data_fabric;
pub mod doc_pipeline;
pub mod z3_examples;
pub mod dual_grok;
pub mod dual_install;
pub mod events;
pub mod codebase_memory;
pub mod datum_types;
pub mod gate_result;
pub mod grok;
pub mod interaction;
pub mod irontology_bridge;
pub mod knowledge;
pub mod kv_store;
pub mod learn;
pub mod lfmf;
pub mod lfmf_telemetry;
pub mod lsp_proxy;
pub mod man_page;
pub mod mcp_proxy;
pub mod mcp_registry;
pub mod ooda;
pub mod pipeline_nodes;
pub mod query_bus;
pub mod rag;
pub mod reasoning;
pub mod redis;
pub mod reviewer;
pub mod rhai_engine;
pub mod runtime_env;
pub mod satisfies;
pub mod secret_validation;
pub mod reviewer_gate;
pub mod sm0l_dispatch;
pub mod state_introspection;
pub mod sudo_operator;
pub mod soul_dataframerr;
pub mod template;
pub mod utils;

// Re-export commonly used types
pub use agent_subtype::AgentSubtype;
pub use ai_artifact::{
    ArtifactExtractionConstraint, ArtifactExtractionManifest, ArtifactId, BoundingBoxRegion,
    ExtractedArtifact, OcrProviderOutput, OcrTextRegion, Photo, PhotoArtifactExtractionRequest,
    PhotoId, PhotoSource, PromptArgumentMap, PromptParameterKind, PromptParameterSpec,
    PromptTemplate, PromptTemplateId, PromptValue, Sam3ArtifactExtractionInput,
    Sam3ArtifactExtractionNode, SegmentedRegion, StructuredRegion, TextRegion,
};
pub use agent_manager::{
    AgentConfig, AgentHandle, AgentManager, ExecutorConfig, invoke_agent_executor,
};
pub use ai_client::{AiClientConfig, AiProviderConfig, B00tAiClient, ChatMessage};
pub use b00t_config::{AiConfiguration, B00tUnifiedConfig, CloudServicesConfig, UserConfig};
pub use context::B00tContext;
pub use datum_types::{LearnMetadata, UsageExample, deserialize_usage};
pub use doc_pipeline::{
    ChunkMetadata, Connective, DocumentFormat, Evidence, EvidenceType,
    FOLFormula, FOLStereotype, FullPipelineResult, PipelineStage, Predicate, ProvenancePointer,
    Quantifier, ReqIFMetadata, Requirement, RequirementStatus, RequirementType,
    SemanticChunk, SerializableFOLFormula, StageResult, SysMLv2Stereotype,
    Category, Endurant, Perdurant, Quality, Relator, RelatorType, Role,
};
pub use dual_grok::{
    ControlCodeEvent, ControlEventCapability, ControlEventReceipt, ControlEventSink, ControlReply,
    DualGrokClient, DualIngestResult, DualQueryItem, DualQueryResult, GrokBackend,
    StubControlEventSink, default_control_event_sink,
};
pub use events::{B00tEvent, write_event, write_event_obj, events_path};
pub use gate_result::{GateDecision, GateResult, ZellijGate};
pub use grok::{AskResult, ChunkResult, ChunkSummary, DigestResult, GrokClient, LearnResult};
pub use interaction::{
    AgentAction, EisenhowerQuadrant, InputRequest, InteractionMode, MenuItem, UserResponse,
};
pub use irontology_bridge::{
    DatumNode, IntoIrontologyRecord, IntoRagDocument, IrontologyBridgeClient,
    IrontologyIngestResult, IrontologyQueryItem, compiled_knowledge_backend,
    compiled_knowledge_backend_data_path,
};
pub use knowledge::{DisplayOpts, KnowledgeSource};
pub use kv_store::{KvBackend, KvConfig, KvStore, ZellijKvEntry};
pub use lfmf::{Lesson, LfmfConfig, LfmfSystem};
pub use man_page::{ManPage, ManSection};
pub use mcp_proxy::{GenericMcpProxy, McpToolDefinition, McpToolRequest, McpToolResponse};
pub use mcp_registry::{
    McpRegistry, McpServerConfig, McpServerRegistration, create_registration_from_datum,
};
pub use ooda::*;
pub use rag::{DocumentSource, LoaderType, RagLightConfig, RagLightManager};
pub use rhai_engine::RhaiEngine;
pub use secret_validation::{
    AwsValidation, CloudflareValidation, QdrantValidation, SecretValidator,
};
pub use state_introspection::{
    StateMachineIntrospection, StateTransitionDescriptor, StateTypeDescriptor,
};
pub use template::TemplateRenderer;

/// Common configuration structure for b00t components
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct B00tConfig {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub env: Option<HashMap<String, String>>,
}

/// Result type alias for b00t operations
pub type B00tResult<T> = Result<T, anyhow::Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_available() {
        assert!(!version::VERSION.is_empty());
    }
}
pub mod datum_credential;
pub mod store;
