//! b00t-admin library — WASM codegen, twin simulation, and type introspection.
//!
//! Provides the core traits and types backing the admin dashboard:
//! - `WasmCodegen` trait: generate WASM, Cython, and type diagrams from pipeline types
//! - `DigitalTwin`: stateful simulation with tick/rollback/subscribers
//! - `TypeSchema` / `reflect_type`: runtime type introspection for any serde type

pub use b00t_c0re_lib::doc_pipeline;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

// ═══════════════════════════════════════════════════════════════════════════
// Section A: WASM Codegen Trait
// ═══════════════════════════════════════════════════════════════════════════

/// Trait for types that can generate WASM modules, Cython stubs, and type diagrams.
///
/// Implemented for key pipeline types (Evidence, Requirement) so the dashboard
/// can show codegen outputs for each type in the pipeline.
pub trait WasmCodegen {
    /// Generate a WebAssembly Text Format (WAT) module representing this type's structure.
    fn to_wasm_module(&self) -> String;

    /// Generate a Cython (.pyx) type stub for FFI interop with Python.
    fn to_cython(&self) -> String;

    /// Generate a Mermaid/ASCII type diagram showing fields and relationships.
    fn to_type_diagram(&self) -> String;
}

// ── WasmCodegen impl for Evidence ────────────────────────────────────────

impl WasmCodegen for doc_pipeline::Evidence {
    fn to_wasm_module(&self) -> String {
        let id = &self.evidence_id;
        format!(
            r#";; WASM module for Evidence: {id}
(module
  (import "pipeline" "log" (func $log (param i32 i32)))
  (memory (export "memory") 1)

  ;; Evidence struct layout:
  ;;   offset 0: evidence_id    (string ptr+len)
  ;;   offset 8: chunk_id       (string ptr+len)
  ;;   offset 16: source_id     (string ptr+len)
  ;;   offset 24: statement     (string ptr+len)
  ;;   offset 32: evidence_type (i32 enum)
  ;;   offset 36: confidence    (f32)
  ;;   offset 40: provenance    (nested struct)
  ;;   offset 56: extracted_at  (i64 timestamp)

  (type $evidence_t (struct
    (field $evidence_id    (mut i32))    ;; string offset
    (field $chunk_id       (mut i32))
    (field $source_id      (mut i32))
    (field $statement_ptr  (mut i32))
    (field $statement_len  (mut i32))
    (field $evidence_type  (mut i32))    ;; 0=Claim, 1=Statistic, 2=Constraint, 3=Definition, 4=Observation
    (field $confidence     (mut f32))
    (field $line_start     (mut i32))
    (field $line_end       (mut i32))
    (field $extracted_at   (mut i64))
  ))

  (func $get_confidence (param $ev i32) (result f32)
    local.get $ev
    struct.get $evidence_t $confidence
  )

  (func $set_status_valid (param $ev i32)
    ;; Evidence with confidence > 0.5 is considered valid
    local.get $ev
    call $get_confidence
    f32.const 0.5
    f32.gt
    (if (then
      local.get $ev
      i32.const 1        ;; status = valid
      i32.store offset=40 ;; store in provenance field area
    ))
  )

  (export "get_confidence" (func $get_confidence))
  (export "set_status_valid" (func $set_status_valid))
)"#
        )
    }

    fn to_cython(&self) -> String {
        format!(
            r#"# cython: language_level=3
# Cython type stub for Evidence (b00t doc_pipeline)

from libc.stdint cimport uint8_t, uint32_t, uint64_t, int64_t
from libc.string cimport const_char

cdef extern from "b00t_pipeline.h":
    ctypedef enum EvidenceType:
        EVIDENCE_CLAIM = 0
        EVIDENCE_STATISTIC = 1
        EVIDENCE_CONSTRAINT = 2
        EVIDENCE_DEFINITION = 3
        EVIDENCE_OBSERVATION = 4

    ctypedef struct ProvenancePointer:
        const_char* source_id
        const_char* chunk_id
        uint32_t    line_start
        uint32_t    line_end
        const_char* quote_snippet

    ctypedef struct Evidence:
        const_char*        evidence_id
        const_char*        chunk_id
        const_char*        source_id
        const_char*        statement
        EvidenceType       evidence_type
        float              confidence
        const_char*        extraction_method
        const_char*        source_quote
        ProvenancePointer  provenance
        int64_t            extracted_at  # unix timestamp

cdef class PyEvidence:
    cdef Evidence _c_evidence
    cdef str _evidence_id

    def __cinit__(self, str evidence_id, str chunk_id, str source_id,
                  str statement, str evidence_type, float confidence):
        self._evidence_id = evidence_id
        self._c_evidence.evidence_id = evidence_id.encode('utf-8')
        self._c_evidence.confidence = confidence

    @property
    def confidence(self) -> float:
        return self._c_evidence.confidence

    cpdef bint is_high_confidence(self):
        return self._c_evidence.confidence > 0.75
"#
        )
    }

    fn to_type_diagram(&self) -> String {
        format!(
            r#"```mermaid
classDiagram
    class Evidence {{
        +String evidence_id
        +String chunk_id
        +String source_id
        +String statement
        +EvidenceType evidence_type
        +f32 confidence
        +ProvenancePointer provenance
        +DateTime extracted_at
    }}

    class ProvenancePointer {{
        +String source_id
        +String chunk_id
        +usize line_start
        +usize line_end
        +String quote_snippet
    }}

    class EvidenceType {{
        <<enumeration>>
        Claim
        Statistic
        Constraint
        Definition
        Observation
    }}

    class SemanticChunk {{
        +String chunk_id
        +String source_id
        +usize chunk_index
        +String content
        +Vec~f32~ embedding
        +f32 confidence
    }}

    Evidence --> ProvenancePointer : has
    Evidence --> SemanticChunk : extracted from
    Evidence --> EvidenceType : classified as
```
Evidence ID: {}
Confidence: {:.2}
Type: {:?}
Statement: {}..."#,
            self.evidence_id,
            self.confidence,
            self.evidence_type,
            &self.statement.chars().take(80).collect::<String>()
        )
    }
}

// ── WasmCodegen impl for Requirement ─────────────────────────────────────

impl WasmCodegen for doc_pipeline::Requirement {
    fn to_wasm_module(&self) -> String {
        format!(
            r#";; WASM module for Requirement: {}
(module
  (import "env" "assert" (func $assert (param i32)))
  (memory (export "memory") 1)

  ;; Requirement struct layout (SysMLv2 + ReqIF compatible)
  (type $requirement_t (struct
    (field $req_id       (mut i32))    ;; string ptr
    (field $text_ptr     (mut i32))
    (field $text_len     (mut i32))
    (field $req_type     (mut i32))    ;; enum: 0=Functional, 1=NonFunctional, etc.
    (field $priority     (mut i32))    ;; 1-5
    (field $status       (mut i32))    ;; enum: 0=Proposed, 1=Approved, 2=Verified, 3=Rejected, 4=Implemented
    (field $derived_count (mut i32))   ;; number of derived-from evidence items
    (field $satisfies_count (mut i32))
    (field $created_at   (mut i64))
  ))

  (func $validate_priority (param $req i32) (result i32)
    local.get $req
    struct.get $requirement_t $priority
    (if (result i32)
      (i32.le_u (local.get 0) (i32.const 5))
      (then i32.const 1)
      (else i32.const 0)
    )
  )

  (func $is_derived (param $req i32) (result i32)
    local.get $req
    struct.get $requirement_t $derived_count
    i32.const 0
    i32.gt_u
  )

  (export "validate_priority" (func $validate_priority))
  (export "is_derived" (func $is_derived))
  (export "memory" (memory 0))
)"#,
            self.req_id
        )
    }

    fn to_cython(&self) -> String {
        format!(
            r#"# cython: language_level=3
# Cython type stub for Requirement (b00t doc_pipeline, SysMLv2 + ReqIF)

from libc.stdint cimport uint8_t, uint32_t, uint64_t, int64_t
from libc.string cimport const_char

cdef extern from "b00t_pipeline.h":
    ctypedef enum RequirementType:
        REQ_FUNCTIONAL = 0
        REQ_NONFUNCTIONAL = 1
        REQ_CONSTRAINT = 2
        REQ_INTERFACE = 3
        REQ_DESIGN = 4
        REQ_STAKEHOLDER = 5

    ctypedef enum RequirementStatus:
        STATUS_PROPOSED = 0
        STATUS_APPROVED = 1
        STATUS_VERIFIED = 2
        STATUS_REJECTED = 3
        STATUS_IMPLEMENTED = 4

    ctypedef struct ReqIFMetadata:
        const_char* reqif_id
        const_char* object_type
        int64_t     last_change
        const_char* tool_id

    ctypedef struct Requirement:
        const_char*       req_id
        const_char*       text
        RequirementType   req_type
        uint8_t           priority
        RequirementStatus status
        uint32_t          derived_count
        const_char**      derived_from       # array of evidence IDs
        ReqIFMetadata*    reqif

cdef class PyRequirement:
    cdef Requirement _c_req
    cdef str _req_id
    cdef list _derived_from

    def __cinit__(self, str req_id, str text, str req_type, int priority):
        self._req_id = req_id
        self._c_req.req_id = req_id.encode('utf-8')
        self._c_req.text = text.encode('utf-8')
        self._c_req.priority = <uint8_t>priority
        self._derived_from = []

    @property
    def priority(self) -> int:
        return self._c_req.priority

    cpdef void add_derived_from(self, str evidence_id):
        self._derived_from.append(evidence_id)

    cpdef bint is_high_priority(self):
        return self._c_req.priority <= 2
"#
        )
    }

    fn to_type_diagram(&self) -> String {
        format!(
            r#"```mermaid
classDiagram
    class Requirement {{
        +String req_id
        +String text
        +RequirementType req_type
        +u8 priority
        +Option~String~ rationale
        +Vec~String~ derived_from
        +Vec~String~ satisfies
        +RequirementStatus status
        +String source_id
        +DateTime created_at
    }}

    class RequirementType {{
        <<enumeration>>
        Functional
        NonFunctional
        Constraint
        Interface
        DesignConstraint
        Stakeholder
    }}

    class RequirementStatus {{
        <<enumeration>>
        Proposed
        Approved
        Verified
        Rejected
        Implemented
    }}

    class Evidence {{
        +String evidence_id
        +String statement
        +f32 confidence
    }}

    class SysMLv2Stereotype {{
        <<enumeration>>
        Requirement
        FunctionalRequirement
        InterfaceRequirement
        PerformanceRequirement
    }}

    Requirement --> RequirementType : typed as
    Requirement --> RequirementStatus : lifecycle
    Requirement --> Evidence : derived_from
    Requirement --> SysMLv2Stereotype : stereotyped
```
Requirement ID: {}
Type: {:?}
Priority: {}
Status: {:?}
Rationale: {}"#,
            self.req_id,
            self.req_type,
            self.priority,
            self.status,
            self.rationale.as_deref().unwrap_or("none")
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Section B: Digital Twin Simulation
// ═══════════════════════════════════════════════════════════════════════════

/// A subscriber handle for receiving twin state updates.
pub type TwinSubscriber<T> = broadcast::Receiver<TwinUpdate<T>>;

/// An update event emitted to twin subscribers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwinUpdate<T> {
    /// Monotonic tick counter
    pub tick: u64,
    /// Timestamp of this update
    pub timestamp: DateTime<Utc>,
    /// Current state snapshot
    pub state: T,
    /// Delta applied (if any)
    pub delta: Option<serde_json::Value>,
    /// Event type: "tick", "delta", "rollback"
    pub event_type: String,
}

/// A digital twin — stateful simulation with history, subscribers, and rollback.
///
/// Generic over `T` so it can simulate any pipeline state (FullPipelineResult,
/// or individual stage states). Supports:
/// - `tick()` — advance one step with optional transformation
/// - `apply_delta()` — merge a partial update into the state
/// - `rollback()` — revert to a previous state from history
/// - `subscribe()` — receive live updates via broadcast channel
pub struct DigitalTwin<T> {
    /// Current simulation state
    state: T,
    /// History of (timestamp, state) pairs
    history: Vec<(DateTime<Utc>, T)>,
    /// Broadcast sender for live subscribers
    tx: broadcast::Sender<TwinUpdate<T>>,
    /// Monotonic tick counter
    tick_count: u64,
    /// Simulation name / identifier
    name: String,
}

impl<T: Clone + Serialize + DeserializeOwned + std::fmt::Debug> DigitalTwin<T> {
    /// Create a new digital twin with the given initial state.
    pub fn new(name: impl Into<String>, initial: T) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            state: initial,
            history: Vec::new(),
            tx,
            tick_count: 0,
            name: name.into(),
        }
    }

    /// Advance the simulation by one tick, applying an optional transformation.
    ///
    /// If `transform` is Some, the function is called with the current state
    /// and should return the new state. If None, the state is snapshot as-is.
    pub fn tick<F>(&mut self, transform: Option<F>)
    where
        F: FnOnce(&T) -> T,
    {
        let now = Utc::now();

        // Save current state to history before mutating
        self.history.push((now, self.state.clone()));

        // Apply transformation if provided
        if let Some(f) = transform {
            self.state = f(&self.state);
        }

        self.tick_count += 1;

        let update = TwinUpdate {
            tick: self.tick_count,
            timestamp: now,
            state: self.state.clone(),
            delta: None,
            event_type: "tick".into(),
        };
        let _ = self.tx.send(update);
    }

    /// Apply a partial (JSON) delta to the current state.
    ///
    /// The delta is merged into the state using serde_json value merging.
    pub fn apply_delta(&mut self, delta: serde_json::Value) -> Result<(), String> {
        let now = Utc::now();
        self.history.push((now, self.state.clone()));

        // Merge delta into current state by serializing state to Value,
        // merging, then deserializing back.
        let mut current_value = serde_json::to_value(&self.state)
            .map_err(|e| format!("Failed to serialize state: {e}"))?;

        if let (serde_json::Value::Object(cur_map), serde_json::Value::Object(delta_map)) =
            (&mut current_value, &delta)
        {
            for (k, v) in delta_map {
                cur_map.insert(k.clone(), v.clone());
            }
        } else {
            current_value = delta.clone();
        }

        self.state = serde_json::from_value(current_value)
            .map_err(|e| format!("Failed to deserialize merged state: {e}"))?;

        self.tick_count += 1;

        let update = TwinUpdate {
            tick: self.tick_count,
            timestamp: now,
            state: self.state.clone(),
            delta: Some(delta),
            event_type: "delta".into(),
        };
        let _ = self.tx.send(update);
        Ok(())
    }

    /// Rollback to a previous state by history index.
    ///
    /// Returns `Ok(())` if rollback succeeded, or an error if the index
    /// is out of bounds.
    pub fn rollback(&mut self, history_index: usize) -> Result<(), String> {
        if history_index >= self.history.len() {
            return Err(format!(
                "Rollback index {} out of bounds (history has {} entries)",
                history_index,
                self.history.len()
            ));
        }

        let now = Utc::now();
        let (_, previous_state) = self.history[history_index].clone();
        self.state = previous_state;
        self.tick_count += 1;

        let update = TwinUpdate {
            tick: self.tick_count,
            timestamp: now,
            state: self.state.clone(),
            delta: None,
            event_type: "rollback".into(),
        };
        let _ = self.tx.send(update);
        Ok(())
    }

    /// Subscribe to live twin updates.
    pub fn subscribe(&self) -> TwinSubscriber<T> {
        self.tx.subscribe()
    }

    /// Get the current state reference.
    pub fn state(&self) -> &T {
        &self.state
    }

    /// Get history entries count.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Get current tick count.
    pub fn tick_count(&self) -> u64 {
        self.tick_count
    }

    /// Get the simulation name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get a snapshot of the current state and metadata.
    pub fn snapshot(&self) -> TwinSnapshot<T> {
        TwinSnapshot {
            name: self.name.clone(),
            tick: self.tick_count,
            history_len: self.history.len(),
            subscriber_count: self.tx.receiver_count(),
            state: self.state.clone(),
        }
    }
}

/// A read-only snapshot of a digital twin's current state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwinSnapshot<T> {
    pub name: String,
    pub tick: u64,
    pub history_len: usize,
    pub subscriber_count: usize,
    pub state: T,
}

// ═══════════════════════════════════════════════════════════════════════════
// Section C: Type Introspection
// ═══════════════════════════════════════════════════════════════════════════

/// Schema describing a reflected type — holds JSON schema + diagram data.
///
/// Type schemas are built manually via `build_type_schema()` in the server
/// for known doc_pipeline types, or via `reflect_type<T>()` when `T` implements
/// `JsonSchema`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeSchema {
    /// Type name (e.g., "Evidence", "Requirement")
    pub name: String,
    /// Rust module path
    pub module_path: String,
    /// JSON Schema (Draft-07 or later)
    pub json_schema: serde_json::Value,
    /// Mermaid class diagram fragment
    pub mermaid_diagram: String,
    /// Field descriptions
    pub fields: Vec<FieldSchema>,
    /// UFO ontological stereotype (Endurant, Perdurant, Relator, Role, Category)
    pub ufo_stereotype: Option<String>,
    /// Whether this type implements WasmCodegen
    pub has_wasm_codegen: bool,
}

/// A single field in a reflected type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldSchema {
    /// Field name
    pub name: String,
    /// Rust type as string
    pub rust_type: String,
    /// Whether the field is optional
    pub is_optional: bool,
    /// Whether the field is a collection
    pub is_collection: bool,
    /// JSON schema for this field
    pub schema: serde_json::Value,
    /// Doc comment or description
    pub description: Option<String>,
}

/// Reflect a Rust type into a TypeSchema using schemars.
///
/// Requires `T: JsonSchema` in addition to serde traits.
/// For types that don't implement `JsonSchema`, use the manual registry
/// approach with `build_type_schema()` in the server binary.
pub fn reflect_type<T: Serialize + DeserializeOwned + JsonSchema>() -> TypeSchema {
    let type_name = std::any::type_name::<T>();
    let short_name = type_name.rsplit("::").next().unwrap_or(type_name);

    // Generate JSON schema via schemars
    let root_schema = schemars::schema_for!(T);
    let json_schema = serde_json::to_value(&root_schema).unwrap_or_default();

    // Build Mermaid diagram from the schema's JSON representation
    let mut fields = Vec::new();
    let mut mermaid_lines = vec![
        "classDiagram".to_string(),
        format!("    class {short_name} {{"),
    ];

    // Navigate the root schema JSON to extract properties
    if let Some(props) = json_schema
        .get("schema")
        .and_then(|s| s.get("properties"))
        .and_then(|p| p.as_object())
    {
        let required: Vec<&str> = json_schema
            .get("schema")
            .and_then(|s| s.get("required"))
            .and_then(|r| r.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        for (field_name, field_value) in props {
            let rust_type = infer_json_type(field_value);
            let is_optional = !required.contains(&field_name.as_str());
            let is_collection = field_value
                .get("type")
                .map(|t| t == "array")
                .unwrap_or(false);
            let desc = field_value
                .get("description")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());

            let prefix = if is_optional { "+Option~" } else { "+" };
            let suffix = if is_optional { "~" } else { "" };
            let col = if is_collection { "[]" } else { "" };
            mermaid_lines.push(format!(
                "        {prefix}{rust_type}{col}{suffix} {field_name}"
            ));

            fields.push(FieldSchema {
                name: field_name.clone(),
                rust_type,
                is_optional,
                is_collection,
                schema: field_value.clone(),
                description: desc,
            });
        }
    }
    mermaid_lines.push("    }".to_string());

    let mermaid_diagram = mermaid_lines.join("\n");

    // Determine UFO stereotype
    let ufo_stereotype = match short_name {
        "DocumentSource" => Some("Endurant".to_string()),
        "Requirement" => Some("Endurant+Role".to_string()),
        "SemanticChunk" => Some("Perdurant".to_string()),
        "Evidence" => Some("Relator".to_string()),
        "EvidenceType" | "RequirementType" | "RequirementStatus" | "DocumentFormat"
        | "PipelineStage" => Some("Category".to_string()),
        "Predicate" | "FOLFormula" => Some("Category".to_string()),
        _ => None,
    };

    let has_wasm_codegen = matches!(
        short_name,
        "Evidence" | "Requirement" | "SemanticChunk" | "DocumentSource"
    );

    TypeSchema {
        name: short_name.to_string(),
        module_path: type_name.to_string(),
        json_schema,
        mermaid_diagram,
        fields,
        ufo_stereotype,
        has_wasm_codegen,
    }
}

/// Infer a Rust-like type name from a schemars JSON schema value.
fn infer_json_type(value: &serde_json::Value) -> String {
    let type_str = value.get("type").and_then(|t| t.as_str());
    let format_str = value.get("format").and_then(|f| f.as_str());

    match format_str {
        Some("date-time") => return "DateTime".into(),
        Some("uri") => return "Url".into(),
        Some("uuid") => return "Uuid".into(),
        _ => {}
    }

    match type_str {
        Some("string") => "String".into(),
        Some("integer") => "i64".into(),
        Some("number") => "f64".into(),
        Some("boolean") => "bool".into(),
        Some("array") => "Vec".into(),
        Some("object") => {
            if let Some(ref_path) = value.get("$ref").and_then(|r| r.as_str()) {
                ref_path.rsplit('/').next().unwrap_or("Object").to_string()
            } else {
                "HashMap".into()
            }
        }
        Some("null") => "()".into(),
        _ => "Value".into(),
    }
}

/// Convenience: reflect a type and return just the Mermaid diagram.
pub fn type_mermaid<T: Serialize + DeserializeOwned + JsonSchema>() -> String {
    reflect_type::<T>().mermaid_diagram
}

/// Convenience: reflect a type and return just the JSON schema.
pub fn type_json_schema<T: Serialize + DeserializeOwned + JsonSchema>() -> serde_json::Value {
    reflect_type::<T>().json_schema
}

// ═══════════════════════════════════════════════════════════════════════════
// Section D: Pipeline State Snapshot (for admin API)
// ═══════════════════════════════════════════════════════════════════════════

/// Lightweight pipeline state for the admin dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineStateSnapshot {
    /// Whether the pipeline has run
    pub has_pipeline: bool,
    /// Source document info
    pub source_id: Option<String>,
    pub source_title: Option<String>,
    /// Counts
    pub chunk_count: usize,
    pub evidence_count: usize,
    pub requirement_count: usize,
    pub fol_formula_count: usize,
    /// Pipeline metadata
    pub pipeline_version: Option<String>,
    pub total_duration_ms: Option<u64>,
    pub executed_at: Option<DateTime<Utc>>,
}

impl Default for PipelineStateSnapshot {
    fn default() -> Self {
        Self {
            has_pipeline: false,
            source_id: None,
            source_title: None,
            chunk_count: 0,
            evidence_count: 0,
            requirement_count: 0,
            fol_formula_count: 0,
            pipeline_version: None,
            total_duration_ms: None,
            executed_at: None,
        }
    }
}

impl From<doc_pipeline::FullPipelineResult> for PipelineStateSnapshot {
    fn from(result: doc_pipeline::FullPipelineResult) -> Self {
        Self {
            has_pipeline: true,
            source_id: Some(result.source.source_id),
            source_title: Some(result.source.title),
            chunk_count: result.chunks.len(),
            evidence_count: result.evidences.len(),
            requirement_count: result.requirements.len(),
            fol_formula_count: result.fol_formulas.len(),
            pipeline_version: Some(result.pipeline_version),
            total_duration_ms: Some(result.total_duration_ms),
            executed_at: Some(result.executed_at),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Section E: Registered Type Names (for the type explorer)
// ═══════════════════════════════════════════════════════════════════════════

/// Returns the list of doc_pipeline type names available for introspection.
pub fn registered_type_names() -> Vec<&'static str> {
    vec![
        "DocumentSource",
        "DocumentFormat",
        "SemanticChunk",
        "ChunkMetadata",
        "Evidence",
        "EvidenceType",
        "ProvenancePointer",
        "Requirement",
        "RequirementType",
        "RequirementStatus",
        "SysMLv2Stereotype",
        "ReqIFMetadata",
        "PipelineStage",
        "StageResult",
        "FullPipelineResult",
        "Quantifier",
        "Connective",
        "RelatorType",
    ]
}
