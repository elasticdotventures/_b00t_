//! AbDataSchema trait — generic type invariant for protocol transformation handlers.
//!
//! # Naming convention
//! | Term | Meaning | Example |
//! |------|---------|---------|
//! | `AbDataSchema` | A schema definition (columns, types, constraints) | `FocusSchema`, `GrpcSchema` |
//! | `AbDataHeader` | A single column within a schema | `AbDataHeader { name, data_type, nullable }` |
//! | `AbDataFrame` | A batch of rows conforming to a schema | `AbDataFrame { headers, rows }` |
//! | `AbDataSequence` | A lazy `.next()` iterator over frames | stream of AbDataFrame from a protocol |
//!
//! # Invariant
//! An `AbDataSchema` guarantees that any `AbDataFrame` produced by `validate()` conforms.
//! Protocol handlers transform between formats using this guarantee.
//!
//! # Implementations
//! - `FocusSchema` — FOCUS v1.3 CostAndUsage + ContractCommitment
//! - `GrpcSchema` — protobuf-backed message schemas (placeholder)
//! - `SqlSchema` — SQL DDL / table schemas (placeholder)
//! - `ArrowSchema` — Arrow Schema wrapper (placeholder)
//!
//! # Schema documentation
//! `FocusSchema::generate_tomllmd()` produces the `focus.schema.tomllmd` file.
//! This is the ONLY source of that file — never hand-edit it.
//! The .tomllmd is a zero-drift documentation artifact generated from code.

use anyhow::Result;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use toml::Value;

#[derive(Parser, Clone)]
pub struct SchemaGenerateArgs {
    #[arg(long, help = "Output path (default: _b00t_/focus.schema.tomllmd)")]
    pub output: Option<std::path::PathBuf>,
}

// ─── Header (column definition) ──────────────────────────────────────────────

/// Data types supported across all schema variants.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataType {
    String,
    Int64,
    Float64,
    Decimal,
    Bool,
    DateTime,
    Binary,
    Json,
    Struct(Vec<AbDataHeader>),
    List(Box<DataType>),
}

impl DataType {
    pub fn is_numeric(&self) -> bool { matches!(self, Self::Int64 | Self::Float64 | Self::Decimal) }
    pub fn is_text(&self) -> bool { matches!(self, Self::String | Self::Json) }
}

/// A single column — the header definition within a schema.
/// `AbDataHeader` is the invariant type for column metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbDataHeader {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub description: String,
    pub ordinal: usize,
}

impl AbDataHeader {
    pub fn new(name: &str, data_type: DataType, nullable: bool, description: &str, ordinal: usize) -> Self {
        Self { name: name.into(), data_type, nullable, description: description.into(), ordinal }
    }
}

// ─── Frame (row batch) ───────────────────────────────────────────────────────

/// A single cell value — the union of all supported types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CellValue {
    String(String),
    Int64(i64),
    Float64(f64),
    Bool(bool),
    Null,
}

impl From<&str> for CellValue { fn from(s: &str) -> Self { Self::String(s.to_string()) } }
impl From<i64> for CellValue { fn from(i: i64) -> Self { Self::Int64(i) } }
impl From<f64> for CellValue { fn from(f: f64) -> Self { Self::Float64(f) } }
impl From<bool> for CellValue { fn from(b: bool) -> Self { Self::Bool(b) } }

/// A batch of rows — the payload that protocol handlers transform.
/// Guaranteed to conform to its schema after `AbDataSchema::validate()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbDataFrame {
    pub headers: Vec<AbDataHeader>,
    pub rows: Vec<Vec<CellValue>>,
}

impl AbDataFrame {
    pub fn new(headers: Vec<AbDataHeader>) -> Self { Self { headers, rows: Vec::new() } }
    pub fn push(&mut self, row: Vec<CellValue>) -> Result<(), SchemaError> {
        if row.len() != self.headers.len() {
            return Err(SchemaError(format!("row width {} != schema width {}", row.len(), self.headers.len())));
        }
        self.rows.push(row);
        Ok(())
    }
    pub fn row_count(&self) -> usize { self.rows.len() }
    pub fn header_count(&self) -> usize { self.headers.len() }
    pub fn header_index(&self, name: &str) -> Option<usize> {
        self.headers.iter().position(|h| h.name == name)
    }
    pub fn cell(&self, row: usize, col: &str) -> Option<&CellValue> {
        let ci = self.header_index(col)?;
        self.rows.get(row).and_then(|r| r.get(ci))
    }
}

// ─── Sequence (lazy iterator over frames) ────────────────────────────────────

/// A lazy producer of AbDataFrames. Protocol handlers implement this
/// to stream data frame-by-frame rather than loading everything at once.
pub trait AbDataSequence: Iterator<Item = Result<AbDataFrame, SchemaError>> + Send {
    /// Schema that every yielded frame conforms to.
    fn schema(&self) -> &dyn AbDataSchema;
}

// ─── Schema errors ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaError(pub String);

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ─── Schema trait ────────────────────────────────────────────────────────────

/// Generic type invariant for protocol transformation handlers.
///
/// Every data format (FOCUS, gRPC, SQL, Arrow) implements this trait.
/// Multiple frames can share one schema — the schema is the invariant,
/// frames are the payloads.
pub trait AbDataSchema: Send + Sync {
    /// Human-readable name (e.g. "focus", "inference-grpc", "user-sql").
    fn name(&self) -> &str;
    /// Schema version string.
    fn version(&self) -> &str;
    /// Header definitions — the canonical column set.
    fn headers(&self) -> Vec<AbDataHeader>;
    /// Look up a single header by name.
    fn header(&self, name: &str) -> Option<AbDataHeader> {
        self.headers().into_iter().find(|h| h.name == name)
    }
    /// Validate a frame against this schema. Returns the frame or errors.
    fn validate(&self, frame: AbDataFrame, mode: MatchMode) -> Result<AbDataFrame, Vec<SchemaError>> {
        let mut errors = Vec::new();
        let hdrs = self.headers();
        if frame.headers.len() != hdrs.len() {
            errors.push(SchemaError(format!("header count mismatch: expected {} got {}", hdrs.len(), frame.headers.len())));
            return Err(errors);
        }
        for (i, (exp, act)) in hdrs.iter().zip(frame.headers.iter()).enumerate() {
            if mode == MatchMode::ByName && exp.name != act.name {
                errors.push(SchemaError(format!("header {i} name mismatch: expected '{}' got '{}'", exp.name, act.name)));
            }
        }
        if !errors.is_empty() { return Err(errors); }
        for (ri, row) in frame.rows.iter().enumerate() {
            if row.len() != hdrs.len() {
                errors.push(SchemaError(format!("row {ri}: width {} != schema width {}", row.len(), hdrs.len())));
                continue;
            }
            for (ci, (hdr, val)) in hdrs.iter().zip(row.iter()).enumerate() {
                if matches!(val, CellValue::Null) && !hdr.nullable {
                    errors.push(SchemaError(format!("row {ri}, hdr {} '{}': null but not nullable", ci, hdr.name)));
                }
            }
        }
        if errors.is_empty() { Ok(frame) } else { Err(errors) }
    }
    /// Encode a validated frame to format-specific bytes.
    fn encode(&self, frame: &AbDataFrame) -> Vec<u8>;
    /// Generate schema documentation as a .tomllmd string (zero-drift).
    fn generate_tomllmd(&self) -> String { String::new() }
}

/// How headers are matched during validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode { Positional, ByName }

// ─── Transform between schemas ───────────────────────────────────────────────

/// Generate the focus.schema.tomllmd file from FocusSchema code.
/// Run after changing FocusSchema to regenerate the documentation artifact.
pub fn handle_schema_generate(args: &SchemaGenerateArgs) -> Result<()> {
    let schema = FocusSchema::new();
    let toml = schema.generate_tomllmd();
    let path = args.output.clone().unwrap_or_else(|| std::path::PathBuf::from("_b00t_/focus.schema.tomllmd"));
    std::fs::write(&path, &toml)?;
    eprintln!("generated {} ({} headers)", path.display(), schema.headers().len());
    Ok(())
}

pub fn transform<S1: AbDataSchema, S2: AbDataSchema>(
    from: &S1, to: &S2, frame: AbDataFrame,
) -> Result<AbDataFrame, Vec<SchemaError>> {
    if from.headers().len() != to.headers().len() {
        return Err(vec![SchemaError(format!("schema width mismatch: from {} headers, to {} headers", from.headers().len(), to.headers().len()))]);
    }
    Ok(AbDataFrame { headers: to.headers(), rows: frame.rows })
}

// ─── Registry ────────────────────────────────────────────────────────────────

static REGISTRY: std::sync::OnceLock<std::sync::Mutex<Vec<(String, String)>>> =
    std::sync::OnceLock::new();

pub fn register_schema(name: &str, version: &str) {
    if let Ok(mut reg) = REGISTRY.get_or_init(|| std::sync::Mutex::new(Vec::new())).lock() {
        reg.push((name.to_string(), version.to_string()));
    }
}

pub fn list_schemas() -> Vec<(String, String)> {
    REGISTRY.get_or_init(|| std::sync::Mutex::new(Vec::new())).lock()
        .map(|reg| reg.clone()).unwrap_or_default()
}

// ─── Concrete: FOCUS v1.3 ────────────────────────────────────────────────────

/// FOCUS v1.3 CostAndUsage dataset. Canonical schema — defined in Rust.
/// `.tomllmd` is generated from this struct, never hand-edited.
pub struct FocusSchema {
    pub headers: Vec<AbDataHeader>,
}

/// A validation requirement (REQIF-style but native to the schema datum).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbDataRequirement {
    pub id: String,
    pub statement: String,
    pub header: String,
    pub constraint: String,
}

impl FocusSchema {
    pub fn new() -> Self {
        let mut o = 0usize;
        let mut h = |n: &str, dt: DataType, nu: bool, d: &str| {
            let hdr = AbDataHeader::new(n, dt, nu, d, o); o += 1; hdr
        };
        Self { headers: vec![
            h("BillingAccountId",   DataType::String,   false, "Provider-assigned billing account identifier"),
            h("BillingCurrency",    DataType::String,   false, "ISO 4217 currency code"),
            h("ServiceProviderName",DataType::String,   false, "Service provider name"),
            h("ServiceName",        DataType::String,   false, "Service name"),
            h("SkuId",              DataType::String,   false, "SKU identifier"),
            h("BilledCost",         DataType::Decimal,  false, "Invoice-basis cost after discounts"),
            h("EffectiveCost",      DataType::Decimal,  false, "Cost after all discounts and amortization"),
            h("ChargeCategory",     DataType::String,   false, "Usage/Purchase/Tax/Credit/Adjustment"),
            h("ChargeFrequency",    DataType::String,   false, "OneTime/Recurring/UsageBased"),
            h("ChargePeriodStart",  DataType::DateTime, false, "Charge period start"),
            h("ChargePeriodEnd",    DataType::DateTime, false, "Charge period end"),
            h("BillingPeriodStart", DataType::DateTime, false, "Billing period start"),
            h("BillingPeriodEnd",   DataType::DateTime, false, "Billing period end"),
            h("BillingAccountName", DataType::String,   true,  "Billing account display name"),
            h("ChargeClass",        DataType::String,   true,  "Correction or null"),
            h("ChargeDescription",  DataType::String,   true,  "Human-readable description"),
            h("ConsumedQuantity",   DataType::Decimal,  true,  "Resources consumed"),
            h("ConsumedUnit",       DataType::String,   true,  "Unit for consumed qty"),
            h("ContractedCost",     DataType::Decimal,  true,  "Cost after contracted discounts"),
            h("InvoiceId",          DataType::String,   true,  "Invoice identifier"),
            h("InvoiceIssuerName",  DataType::String,   true,  "Invoice issuer"),
            h("ListCost",           DataType::Decimal,  true,  "Undiscounted list price"),
            h("PricingQuantity",    DataType::Decimal,  true,  "Pricing quantity"),
            h("PricingUnit",        DataType::String,   true,  "Pricing unit"),
            h("RegionId",           DataType::String,   true,  "Region ID"),
            h("RegionName",         DataType::String,   true,  "Region name"),
            h("ResourceId",         DataType::String,   true,  "Resource ID"),
            h("ResourceName",       DataType::String,   true,  "Resource name"),
            h("ServiceCategory",    DataType::String,   true,  "Service category"),
            h("ServiceSubcategory", DataType::String,   true,  "Service subcategory"),
            h("SubAccountId",       DataType::String,   true,  "Sub-account ID"),
            h("SubAccountName",     DataType::String,   true,  "Sub-account name"),
            h("AvailabilityZone",   DataType::String,   true,  "Availability zone"),
            h("CommitmentDiscountId", DataType::String, true,  "Commitment discount ID"),
            h("CommitmentDiscountType", DataType::String, true, "Commitment discount type"),
            h("CommitmentDiscountStatus", DataType::String, true, "Used/Unused"),
            h("HostProviderName",   DataType::String,   true,  "Host provider"),
            h("x_ExperimentId",     DataType::String,   true,  "ledgrrr: experiment ID"),
            h("x_Variant",          DataType::String,   true,  "ledgrrr: control/treatment"),
            h("x_Personality",      DataType::String,   true,  "ledgrrr: psychometric label"),
            h("x_ExperimentScore",  DataType::Decimal,  true,  "ledgrrr: 0.0–1.0"),
            h("x_AgentId",          DataType::String,   true,  "ledgrrr: agent ID"),
            h("x_ReasoningReview",  DataType::String,   true,  "ledgrrr: reviewer verdict"),
        ]}
    }

    /// Load FocusSchema from a `.tomllmd` file at runtime.
    /// Reads the `[b00t.schema.focus.headers]` table and constructs
    /// AbDataHeader entries with type mapping:
    /// - `"metric"` → `DataType::Decimal`
    /// - `"dimension"` → `DataType::String`
    pub fn load(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let root: Value = toml::from_str(&content)?;

        let headers_table = root
            .get("b00t")
            .and_then(|v| v.get("schema"))
            .and_then(|v| v.get("focus"))
            .and_then(|v| v.get("headers"))
            .and_then(|v| v.as_table())
            .ok_or_else(|| anyhow::anyhow!("missing [b00t.schema.focus.headers] in {path}"))?;

        let mut headers: Vec<AbDataHeader> = Vec::new();
        for (name, entry) in headers_table {
            let tbl = entry
                .as_table()
                .ok_or_else(|| anyhow::anyhow!("header {name} value is not a table"))?;

            let type_str = tbl.get("type").and_then(|v| v.as_str()).unwrap_or("dimension");
            let data_type = match type_str {
                "metric" => DataType::Decimal,
                _ => DataType::String,
            };

            let nullable = tbl.get("nullable").and_then(|v| v.as_bool()).unwrap_or(true);
            let ordinal = tbl.get("ordinal").and_then(|v| v.as_integer()).unwrap_or(0) as usize;

            headers.push(AbDataHeader::new(name, data_type, nullable, "", ordinal));
        }

        headers.sort_by_key(|h| h.ordinal);
        Ok(Self { headers })
    }
}

impl FocusSchema {
    /// Validation requirements embedded in the schema datum.
    /// Replaces the separate reqif.yaml file — zero drift.
    pub fn requirements(&self) -> Vec<AbDataRequirement> {
        vec![
            AbDataRequirement { id: "REQ-FOCUS-001".into(), statement: "Every record MUST have a BillingAccountId".into(), header: "BillingAccountId".into(), constraint: "required".into() },
            AbDataRequirement { id: "REQ-FOCUS-002".into(), statement: "Every record MUST have a non-null BilledCost".into(), header: "BilledCost".into(), constraint: "required".into() },
            AbDataRequirement { id: "REQ-FOCUS-003".into(), statement: "Control/treatment variants MUST have matching experiment_id".into(), header: "x_ExperimentId".into(), constraint: "pairwise_match".into() },
            AbDataRequirement { id: "REQ-FOCUS-004".into(), statement: "EffectiveCost MUST be <= BilledCost".into(), header: "EffectiveCost".into(), constraint: "lte:BilledCost".into() },
            AbDataRequirement { id: "REQ-FOCUS-005".into(), statement: "ChargeCategory MUST be one of Usage/Purchase/Tax/Credit/Adjustment".into(), header: "ChargeCategory".into(), constraint: "enum".into() },
            AbDataRequirement { id: "REQ-FOCUS-006".into(), statement: "A recommendation MUST be present when focus_delta is computed".into(), header: "recommendation".into(), constraint: "required".into() },
        ]
    }
}

impl AbDataSchema for FocusSchema {
    fn name(&self) -> &str { "focus" }
    fn version(&self) -> &str { "1.3" }
    fn headers(&self) -> Vec<AbDataHeader> { self.headers.clone() }
    fn encode(&self, frame: &AbDataFrame) -> Vec<u8> {
        serde_json::to_vec(frame).unwrap_or_default()
    }
    fn generate_tomllmd(&self) -> String {
        let mut out = String::new();
        // TOML requires the first non-comment line to be a valid key or table.
        // Use a known-good first key, then header comments after.
        out.push_str("generated_by = \"FocusSchema::generate_tomllmd()\"\n");
        out.push_str("# 🤖 AUTO-GENERATED — DO NOT EDIT\n");
        out.push_str("# Regenerate: b00t schema generate\n");
        let n = self.headers.len();
        out.push_str(&format!("# Source: {n} headers from datum_schema.rs FocusSchema struct\n\n"));
        out.push_str("[b00t]\n");
        out.push_str(&format!("name = \"focus\"\ntype = \"schema\"\nversion = \"{}\"\n", self.version()));
        out.push_str(&format!("hint = \"FOCUS v{} CostAndUsage — auto-generated. DO NOT EDIT.\"\n", self.version()));
        out.push_str("keywords = [\"focus\", \"finops\", \"schema\", \"auto-gen\"]\n\n");
        out.push_str("[b00t.spec]\n");
        out.push_str("canonical = \"https://focus.finops.org/focus-specification/v1-3/\"\n");
        out.push_str("license = \"CC-BY-4.0\"\n\n");
        out.push_str("[b00t.schema.focus]\n");
        out.push_str(&format!("spec_version = \"{}\"\n", self.version()));
        out.push_str(&format!("header_count = {}\n\n", self.headers.len()));
        out.push_str("[b00t.schema.focus.headers]\n\n");
        for h in &self.headers {
            let ctype = if h.data_type.is_numeric() { "Metric" } else { "Dimension" };
            let feat = if !h.nullable { "Mandatory" } else { "Optional" };
            out.push_str(&format!("# {}: {} ({} | {} | nullable={})\n", h.name, h.description, ctype, feat, h.nullable));
            out.push_str(&format!("{} = {{ type = \"{}\", feature = \"{}\", nullable = {}, ordinal = {} }}\n", h.name, ctype.to_lowercase(), feat.to_lowercase(), h.nullable, h.ordinal));
        }
        out.push_str("\n# b00t:map v1\n");
        out.push_str(&format!("# summary: FOCUS v1.3 CostAndUsage — {} headers, auto-generated. Source: FocusSchema struct.\n", self.headers.len()));
        out.push_str("# tags: focus, finops, schema, v1.3, auto-gen\n# tier: frontier\n# cmds: b00t schema generate\n");
        out
    }
}

// ─── Placeholder implementations ─────────────────────────────────────────────

pub struct GrpcSchema { pub service_name: String, pub method_name: String, pub headers: Vec<AbDataHeader> }
impl GrpcSchema {
    pub fn new(service: &str, method: &str) -> Self { Self { service_name: service.into(), method_name: method.into(), headers: Vec::new() } }
}
impl AbDataSchema for GrpcSchema {
    fn name(&self) -> &str { &self.service_name }
    fn version(&self) -> &str { &self.method_name }
    fn headers(&self) -> Vec<AbDataHeader> { self.headers.clone() }
    fn encode(&self, frame: &AbDataFrame) -> Vec<u8> { serde_json::to_vec(frame).unwrap_or_default() }
}

pub struct SqlSchema { pub table_name: String, pub headers: Vec<AbDataHeader> }
impl SqlSchema {
    pub fn new(table: &str) -> Self { Self { table_name: table.into(), headers: Vec::new() } }
}
impl AbDataSchema for SqlSchema {
    fn name(&self) -> &str { &self.table_name }
    fn version(&self) -> &str { "0" }
    fn headers(&self) -> Vec<AbDataHeader> { self.headers.clone() }
    fn encode(&self, frame: &AbDataFrame) -> Vec<u8> { serde_json::to_vec(frame).unwrap_or_default() }
}

pub struct ArrowSchema { pub headers: Vec<AbDataHeader> }
impl ArrowSchema {
    pub fn new(headers: Vec<AbDataHeader>) -> Self { Self { headers } }
}
impl AbDataSchema for ArrowSchema {
    fn name(&self) -> &str { "arrow" }
    fn version(&self) -> &str { "58" }
    fn headers(&self) -> Vec<AbDataHeader> { self.headers.clone() }
    fn encode(&self, frame: &AbDataFrame) -> Vec<u8> { serde_json::to_vec(frame).unwrap_or_default() }
}

// ─── Concrete Sequence: focus records from JSONL ─────────────────────────────

/// Reads FOCUS records from a JSONL file, yielding one AbDataFrame per call.
pub struct FocusJsonlSequence {
    schema: FocusSchema,
    reader: Option<std::io::BufReader<std::fs::File>>,
    exhausted: bool,
}

impl FocusJsonlSequence {
    pub fn open(path: &str) -> Result<Self, std::io::Error> {
        let file = std::fs::File::open(path)?;
        Ok(Self {
            schema: FocusSchema::new(),
            reader: Some(std::io::BufReader::new(file)),
            exhausted: false,
        })
    }
}

impl Iterator for FocusJsonlSequence {
    type Item = Result<AbDataFrame, SchemaError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.exhausted { return None; }
        let reader = self.reader.as_mut()?;
        let mut buf = String::new();
        use std::io::BufRead;
        match reader.read_line(&mut buf) {
            Ok(0) => { self.exhausted = true; None }
            Ok(_) => {
                let json: serde_json::Value = serde_json::from_str(&buf).ok()?;
                let mut frame = AbDataFrame::new(self.schema.headers());
                // parse JSON object → single row
                let mut row = Vec::new();
                for hdr in &self.schema.headers {
                    let val = match json.get(&hdr.name) {
                        Some(v) if v.is_null() => CellValue::Null,
                        Some(v) if v.is_string() => CellValue::String(v.as_str().unwrap().to_string()),
                        Some(v) if v.is_number() => CellValue::Float64(v.as_f64().unwrap_or(0.0)),
                        Some(v) if v.is_boolean() => CellValue::Bool(v.as_bool().unwrap_or(false)),
                        _ => CellValue::Null,
                    };
                    row.push(val);
                }
                let _ = frame.push(row);
                Some(Ok(frame))
            }
            Err(e) => Some(Err(SchemaError(e.to_string()))),
        }
    }
}

impl AbDataSequence for FocusJsonlSequence {
    fn schema(&self) -> &dyn AbDataSchema { &self.schema }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ab_naming_frame_push_validates() {
        let headers = vec![
            AbDataHeader::new("a", DataType::String, false, "", 0),
            AbDataHeader::new("b", DataType::Int64, false, "", 1),
        ];
        let mut frame = AbDataFrame::new(headers);
        assert!(frame.push(vec!["x".into(), 42i64.into()]).is_ok());
        assert!(frame.push(vec!["y".into()]).is_err());
    }

    #[test]
    fn test_focus_schema_headers() {
        let s = FocusSchema::new();
        assert!(s.headers().len() >= 13);
        assert!(s.header("BilledCost").is_some());
        assert!(s.header("ServiceName").is_some());
    }

    #[test]
    fn test_validate_header_count_mismatch() {
        let s = FocusSchema::new();
        let frame = AbDataFrame::new(vec![AbDataHeader::new("x", DataType::String, false, "", 0)]);
        assert!(s.validate(frame, MatchMode::Positional).is_err());
    }

    #[test]
    fn test_validate_rejects_nullable_violation() {
        let headers = vec![AbDataHeader::new("req", DataType::String, false, "", 0)];
        let mut frame = AbDataFrame::new(headers.clone());
        frame.push(vec![CellValue::Null]).unwrap();
        let s = ArrowSchema::new(headers);
        assert!(s.validate(frame, MatchMode::Positional).is_err());
    }

    #[test]
    fn test_transform_preserves_rows() {
        let focus = FocusSchema::new();
        let mut frame = AbDataFrame::new(focus.headers());
        let mut row = Vec::new();
        for _ in 0..focus.headers().len() { row.push(CellValue::Null); }
        frame.push(row).unwrap();
        let arrow = ArrowSchema::new(focus.headers());
        let result = transform(&focus, &arrow, frame);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().row_count(), 1);
    }

    #[test]
    fn test_cell_value_conversions() {
        assert!(matches!(<CellValue as From<&str>>::from("hi"), CellValue::String(_)));
        assert!(matches!(<CellValue as From<i64>>::from(42i64), CellValue::Int64(_)));
        assert!(matches!(<CellValue as From<f64>>::from(3.14), CellValue::Float64(_)));
        assert!(matches!(<CellValue as From<bool>>::from(true), CellValue::Bool(_)));
    }

    #[test]
    fn test_frame_cell_by_header_name() {
        let headers = vec![
            AbDataHeader::new("id", DataType::Int64, false, "", 0),
            AbDataHeader::new("name", DataType::String, false, "", 1),
        ];
        let mut frame = AbDataFrame::new(headers);
        frame.push(vec![1i64.into(), "alice".into()]).unwrap();
        assert_eq!(frame.cell(0, "name"), Some(&CellValue::String("alice".into())));
        assert_eq!(frame.cell(0, "id"), Some(&CellValue::Int64(1)));
        assert!(frame.cell(0, "missing").is_none());
    }

    #[test]
    fn test_focus_schema_generates_tomllmd() {
        let s = FocusSchema::new();
        let toml = s.generate_tomllmd();
        assert!(toml.contains("focus"));
        assert!(toml.contains("1.3"));
        assert!(toml.contains("BillingAccountId"));
        assert!(toml.contains("DO NOT EDIT"));
    }

    #[test]
    fn test_focus_schema_file_matches_generated() {
        // CI gate: the checked-in focus.schema.tomllmd MUST match
        // FocusSchema::generate_tomllmd(). Run `b00t schema generate`
        // to regenerate if this test fails.
        // Resolve path relative to CARGO_MANIFEST_DIR or fall back to cwd
        let root = std::env::var("CARGO_MANIFEST_DIR")
            .or_else(|_| std::env::current_dir().map(|p| p.to_string_lossy().to_string()))
            .unwrap_or_else(|_| ".".to_string());
        let s = FocusSchema::new();
        let generated = s.generate_tomllmd();
        let path = format!("{root}/../_b00t_/focus.schema.tomllmd");
        let on_disk = std::fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("{path} not found — run `b00t schema generate` first"));
        // Normalize line endings for comparison
        let generated_normalized = generated.replace("\r\n", "\n");
        let on_disk_normalized = on_disk.replace("\r\n", "\n");
        assert_eq!(
            generated_normalized, on_disk_normalized,
            "focus.schema.tomllmd is stale — regenerate with `b00t schema generate`"
        );
    }

    #[test]
    fn test_focus_schema_load_from_file() {
        let root = std::env::var("CARGO_MANIFEST_DIR")
            .or_else(|_| std::env::current_dir().map(|p| p.to_string_lossy().to_string()))
            .unwrap_or_else(|_| ".".to_string());
        let path = format!("{root}/../_b00t_/focus.schema.tomllmd");
        let s = FocusSchema::load(&path).expect("should load focus.schema.tomllmd");
        assert_eq!(s.headers().len(), 43, "expected 43 headers from file");
        let billed = s.header("BilledCost").expect("BilledCost header should exist");
        assert_eq!(billed.data_type, DataType::Decimal, "BilledCost: metric -> Decimal");
        assert_eq!(billed.nullable, false, "BilledCost should be non-nullable");
    }

    #[test]
    fn test_ab_data_type_checks() {
        assert!(DataType::Float64.is_numeric());
        assert!(DataType::String.is_text());
        assert!(!DataType::Bool.is_numeric());
    }

    #[test]
    fn test_register_and_list_schemas() {
        register_schema("focus", "1.3");
        let list = list_schemas();
        assert!(list.iter().any(|(n, v)| n == "focus" && v == "1.3"));
    }

    #[test]
    fn test_grpc_schema_placeholder() {
        let s = GrpcSchema::new("InferenceService", "Predict");
        assert_eq!(s.name(), "InferenceService");
    }

    #[test]
    fn test_sql_schema_placeholder() {
        let s = SqlSchema::new("experiment_scores");
        assert_eq!(s.name(), "experiment_scores");
    }

    #[test]
    fn test_focus_jsonl_sequence() {
        // File doesn't exist — expect None from iterator
        let mut seq = FocusJsonlSequence::open("/tmp/nonexistent-focus.jsonl").ok();
        assert!(seq.is_none());
    }
}
