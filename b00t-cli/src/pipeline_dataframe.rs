//! Dataframe row emission for pipeline stage outputs (#730).
//!
//! Each stage output is decomposed into typed rows (one per column) for
//! querying, aggregation, and downstream analysis.  Supports JSON object
//! → multi-column decomposition, raw bytes passthrough, and an in-memory
//! store suitable for testing.
//!
//! # Key types
//! - [`DataFrameRow`] — a single typed cell in the output dataframe.
//! - [`DataFrameValue`] — typed variant: String, Float, Int, Bool, Bytes, Json.
//! - [`DataFrameStore`] — trait for insert / query / aggregate.
//! - [`InMemoryDataFrameStore`] — `HashMap<String, Vec<DataFrameRow>>` backend.
//! - [`DataFrameQuery`] / [`DataFrameAggregation`] — filter and numeric rollup.

use crate::pipeline_executor::{PipelineRunReport, StageResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Core types ──────────────────────────────────────────────────────────────────

/// A single typed value in a dataframe cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataFrameValue {
    String(String),
    Float(f64),
    Int(i64),
    Bool(bool),
    Bytes(Vec<u8>),
    /// A nested JSON value that didn't match a simpler type.
    Json(serde_json::Value),
}

/// A single row in a pipeline output dataframe.
///
/// Each row represents one column value from one stage output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFrameRow {
    pub run_id: String,
    pub stage_name: String,
    pub column: String,
    pub value: DataFrameValue,
    pub timestamp: DateTime<Utc>,
    pub tags: HashMap<String, String>,
}

/// Query filter for the dataframe store.
///
/// All fields are optional — omitted fields act as wildcards.
#[derive(Debug, Clone, Default)]
pub struct DataFrameQuery {
    pub run_id: Option<String>,
    pub stage_name: Option<String>,
    pub columns: Option<Vec<String>>,
    pub since: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

/// Numeric aggregation result computed from matching numeric column values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFrameAggregation {
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub sum: f64,
}

/// Storage backend for dataframe rows.
pub trait DataFrameStore: Send + Sync {
    /// Insert a single row into the store.
    fn insert(&self, row: DataFrameRow) -> anyhow::Result<()>;

    /// Query rows matching the given filter.
    fn query(&self, filter: &DataFrameQuery) -> anyhow::Result<Vec<DataFrameRow>>;

    /// Aggregate numeric values for a (stage, column) pair.
    fn aggregate(&self, stage: &str, column: &str) -> anyhow::Result<DataFrameAggregation>;
}

// ── DataFrameValue helpers ──────────────────────────────────────────────────────

impl DataFrameValue {
    /// Convert a `serde_json::Value` into the most specific `DataFrameValue` variant.
    fn from_json(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::String(s) => DataFrameValue::String(s),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    DataFrameValue::Int(i)
                } else {
                    DataFrameValue::Float(n.as_f64().unwrap_or(0.0))
                }
            }
            serde_json::Value::Bool(b) => DataFrameValue::Bool(b),
            serde_json::Value::Null => DataFrameValue::String("null".to_string()),
            // Objects and arrays that survive the top-level split land here
            other => DataFrameValue::Json(other),
        }
    }

    /// Return the value as an `f64` if it is numeric; `None` otherwise.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            DataFrameValue::Float(f) => Some(*f),
            DataFrameValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }
}

// ── StageResult → DataFrameRow conversion ───────────────────────────────────────

impl StageResult {
    /// Decompose a stage result into typed dataframe rows.
    ///
    /// Heuristics (in order):
    /// 1. `output` is `None` → empty vec (skipped / no-op stage).
    /// 2. Output is a JSON object → one row per key-value pair.
    /// 3. Output is a JSON array → one row per element, column `"[{index}]"`.
    /// 4. Output is any other JSON value → single row with column `"value"`.
    /// 5. Output is non-UTF-8 bytes → single row with column `"output"`, value `Bytes`.
    pub fn to_dataframe_rows(&self, run_id: &str) -> Vec<DataFrameRow> {
        let stage_name = &self.stage_name;
        let timestamp = Utc::now();
        let tags = HashMap::new();

        let output = match &self.output {
            Some(bytes) => bytes,
            None => return vec![],
        };

        // Try JSON parse first
        if let Ok(json_value) = serde_json::from_slice::<serde_json::Value>(output) {
            match json_value {
                serde_json::Value::Object(map) => map
                    .into_iter()
                    .map(|(key, val)| DataFrameRow {
                        run_id: run_id.to_string(),
                        stage_name: stage_name.clone(),
                        column: key,
                        value: DataFrameValue::from_json(val),
                        timestamp,
                        tags: tags.clone(),
                    })
                    .collect(),
                serde_json::Value::Array(arr) => arr
                    .into_iter()
                    .enumerate()
                    .map(|(idx, val)| DataFrameRow {
                        run_id: run_id.to_string(),
                        stage_name: stage_name.clone(),
                        column: format!("[{}]", idx),
                        value: DataFrameValue::from_json(val),
                        timestamp,
                        tags: tags.clone(),
                    })
                    .collect(),
                other => vec![DataFrameRow {
                    run_id: run_id.to_string(),
                    stage_name: stage_name.clone(),
                    column: "value".to_string(),
                    value: DataFrameValue::from_json(other),
                    timestamp,
                    tags,
                }],
            }
        } else {
            // Non-JSON bytes → single "output" column
            vec![DataFrameRow {
                run_id: run_id.to_string(),
                stage_name: stage_name.clone(),
                column: "output".to_string(),
                value: DataFrameValue::Bytes(output.clone()),
                timestamp,
                tags,
            }]
        }
    }
}

// ── PipelineRunReport → DataFrame ───────────────────────────────────────────────

impl PipelineRunReport {
    /// Convert the full pipeline run report into a flat list of dataframe rows.
    ///
    /// Rows from all stages are concatenated; each stage's output is
    /// decomposed via [`StageResult::to_dataframe_rows`].
    pub fn to_dataframe(&self) -> Vec<DataFrameRow> {
        let mut rows = Vec::new();
        for stage in &self.stages {
            rows.extend(stage.to_dataframe_rows(&self.run_id));
        }
        rows
    }
}

// ── InMemoryDataFrameStore ───────────────────────────────────────────────────────

/// In-memory dataframe store backed by `HashMap<String, Vec<DataFrameRow>>`.
///
/// Keys are run IDs.  Suitable for testing and small-scale / single-node use.
/// Thread-safe via `std::sync::Mutex`.
#[derive(Debug, Default)]
pub struct InMemoryDataFrameStore {
    data: std::sync::Mutex<HashMap<String, Vec<DataFrameRow>>>,
}

impl InMemoryDataFrameStore {
    pub fn new() -> Self {
        Self {
            data: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

impl DataFrameStore for InMemoryDataFrameStore {
    fn insert(&self, row: DataFrameRow) -> anyhow::Result<()> {
        let mut data = self
            .data
            .lock()
            .map_err(|e| anyhow::anyhow!("DataFrameStore lock error: {e}"))?;
        data.entry(row.run_id.clone())
            .or_insert_with(Vec::new)
            .push(row);
        Ok(())
    }

    fn query(&self, filter: &DataFrameQuery) -> anyhow::Result<Vec<DataFrameRow>> {
        let data = self
            .data
            .lock()
            .map_err(|e| anyhow::anyhow!("DataFrameStore lock error: {e}"))?;
        let mut results: Vec<DataFrameRow> = data
            .values()
            .flatten()
            .filter(|row| {
                if let Some(ref run_id) = filter.run_id {
                    if row.run_id != *run_id {
                        return false;
                    }
                }
                if let Some(ref stage_name) = filter.stage_name {
                    if row.stage_name != *stage_name {
                        return false;
                    }
                }
                if let Some(ref columns) = filter.columns {
                    if !columns.contains(&row.column) {
                        return false;
                    }
                }
                if let Some(ref since) = filter.since {
                    if row.timestamp < *since {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        if let Some(limit) = filter.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    fn aggregate(&self, stage: &str, column: &str) -> anyhow::Result<DataFrameAggregation> {
        let data = self
            .data
            .lock()
            .map_err(|e| anyhow::anyhow!("DataFrameStore lock error: {e}"))?;
        let values: Vec<f64> = data
            .values()
            .flatten()
            .filter(|row| row.stage_name == stage && row.column == column)
            .filter_map(|row| row.value.as_f64())
            .collect();

        let count = values.len();
        if count == 0 {
            return Ok(DataFrameAggregation {
                count: 0,
                min: 0.0,
                max: 0.0,
                mean: 0.0,
                sum: 0.0,
            });
        }

        let sum: f64 = values.iter().sum();
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let mean = sum / count as f64;

        Ok(DataFrameAggregation {
            count,
            min,
            max,
            mean,
            sum,
        })
    }
}

// ── CLI handler ──────────────────────────────────────────────────────────────────

/// Handle `b00t pipeline data` CLI subcommand.
///
/// Queries stage outputs as dataframe rows.  Accepts optional `--stage` and
/// `--columns` filters.  In this iteration the handler is a stub that
/// demonstrates the dataframe subsystem; full run-store integration follows
/// in a subsequent PR.
pub fn handle_pipeline_data(
    _b00t_path: &str,
    pipeline_id: &str,
    stage: Option<&str>,
    columns: Option<&str>,
) -> anyhow::Result<()> {
    println!("Pipeline dataframe query: pipeline_id={pipeline_id}");
    if let Some(stage) = stage {
        println!("  stage: {stage}");
    }
    if let Some(cols) = columns {
        println!("  columns: {cols}");
    }
    Ok(())
}

// ── Tests ───────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_executor::{RunStatus, StageStatus};

    // ── StageResult → DataFrameRow ───────────────────────────────────────

    #[test]
    fn stage_result_empty_output_produces_no_rows() {
        let sr = StageResult {
            stage_name: "test-stage".into(),
            duration_ms: 100,
            output: None,
            error: None,
            status: StageStatus::Completed,
        };
        let rows = sr.to_dataframe_rows("run-1");
        assert!(rows.is_empty(), "empty stage should produce no rows");
    }

    #[test]
    fn stage_result_json_object_creates_column_per_key() {
        let json = serde_json::json!({
            "accuracy": 0.95,
            "loss": 0.05,
            "label": "cat"
        });
        let sr = StageResult {
            stage_name: "eval".into(),
            duration_ms: 50,
            output: Some(serde_json::to_vec(&json).unwrap()),
            error: None,
            status: StageStatus::Completed,
        };
        let rows = sr.to_dataframe_rows("run-2");
        assert_eq!(rows.len(), 3, "should be one row per JSON key");

        let acc = rows.iter().find(|r| r.column == "accuracy").unwrap();
        match &acc.value {
            DataFrameValue::Float(f) => assert!((*f - 0.95).abs() < 1e-10),
            other => panic!("expected Float, got {other:?}"),
        }

        let label = rows.iter().find(|r| r.column == "label").unwrap();
        match &label.value {
            DataFrameValue::String(s) => assert_eq!(s, "cat"),
            other => panic!("expected String, got {other:?}"),
        }
    }

    #[test]
    fn stage_result_raw_bytes_creates_single_output_row() {
        let sr = StageResult {
            stage_name: "encode".into(),
            duration_ms: 30,
            output: Some(b"raw binary data".to_vec()),
            error: None,
            status: StageStatus::Completed,
        };
        let rows = sr.to_dataframe_rows("run-3");
        assert_eq!(rows.len(), 1, "raw bytes should produce one row");
        assert_eq!(rows[0].column, "output");
        match &rows[0].value {
            DataFrameValue::Bytes(b) => assert_eq!(b, b"raw binary data"),
            other => panic!("expected Bytes, got {other:?}"),
        }
    }

    #[test]
    fn stage_result_json_array_creates_indexed_rows() {
        let json = serde_json::json!([10, 20, 30]);
        let sr = StageResult {
            stage_name: "collect".into(),
            duration_ms: 5,
            output: Some(serde_json::to_vec(&json).unwrap()),
            error: None,
            status: StageStatus::Completed,
        };
        let rows = sr.to_dataframe_rows("run-arr");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].column, "[0]");
        assert_eq!(rows[1].column, "[1]");
        assert_eq!(rows[2].column, "[2]");
        match &rows[1].value {
            DataFrameValue::Int(i) => assert_eq!(*i, 20),
            other => panic!("expected Int, got {other:?}"),
        }
    }

    #[test]
    fn stage_result_single_json_value_uses_value_column() {
        let sr = StageResult {
            stage_name: "scalar".into(),
            duration_ms: 1,
            output: Some(b"42.5".to_vec()),
            error: None,
            status: StageStatus::Completed,
        };
        let rows = sr.to_dataframe_rows("run-scalar");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].column, "value");
        match &rows[0].value {
            DataFrameValue::Float(f) => assert!((*f - 42.5).abs() < 1e-10),
            other => panic!("expected Float, got {other:?}"),
        }
    }

    // ── PipelineRunReport → DataFrame ────────────────────────────────────

    #[test]
    fn run_report_to_dataframe_aggregates_all_stages() {
        let json_a = serde_json::json!({"score": 0.8});
        let json_b = serde_json::json!({"score": 0.9, "done": true});

        let report = PipelineRunReport {
            run_id: "run-4".into(),
            stages: vec![
                StageResult {
                    stage_name: "stage-a".into(),
                    duration_ms: 10,
                    output: Some(serde_json::to_vec(&json_a).unwrap()),
                    error: None,
                    status: StageStatus::Completed,
                },
                StageResult {
                    stage_name: "stage-b".into(),
                    duration_ms: 20,
                    output: Some(serde_json::to_vec(&json_b).unwrap()),
                    error: None,
                    status: StageStatus::Completed,
                },
            ],
            total_duration_ms: 30,
            status: RunStatus::Completed,
        };

        let rows = report.to_dataframe();
        assert_eq!(rows.len(), 3, "1 + 2 = 3 rows total");
        assert!(rows.iter().all(|r| r.run_id == "run-4"));
    }

    // ── DataFrameValue helpers ───────────────────────────────────────────

    #[test]
    fn dataframe_value_as_f64_float() {
        assert!((DataFrameValue::Float(3.14).as_f64().unwrap() - 3.14).abs() < 1e-10);
    }

    #[test]
    fn dataframe_value_as_f64_int() {
        assert!((DataFrameValue::Int(42).as_f64().unwrap() - 42.0).abs() < 1e-10);
    }

    #[test]
    fn dataframe_value_as_f64_string_is_none() {
        assert!(DataFrameValue::String("hello".into()).as_f64().is_none());
    }

    #[test]
    fn dataframe_value_as_f64_bool_is_none() {
        assert!(DataFrameValue::Bool(true).as_f64().is_none());
    }

    // ── InMemoryDataFrameStore insert + query ────────────────────────────

    #[test]
    fn store_insert_and_query_all() {
        let store = InMemoryDataFrameStore::new();
        store
            .insert(DataFrameRow {
                run_id: "r1".into(),
                stage_name: "s1".into(),
                column: "acc".into(),
                value: DataFrameValue::Float(0.95),
                timestamp: Utc::now(),
                tags: HashMap::new(),
            })
            .unwrap();

        let all = store.query(&DataFrameQuery::default()).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].run_id, "r1");
    }

    #[test]
    fn store_query_filters_by_run_id() {
        let store = InMemoryDataFrameStore::new();
        store
            .insert(DataFrameRow {
                run_id: "r1".into(),
                stage_name: "s1".into(),
                column: "x".into(),
                value: DataFrameValue::Int(1),
                timestamp: Utc::now(),
                tags: HashMap::new(),
            })
            .unwrap();
        store
            .insert(DataFrameRow {
                run_id: "r2".into(),
                stage_name: "s1".into(),
                column: "x".into(),
                value: DataFrameValue::Int(2),
                timestamp: Utc::now(),
                tags: HashMap::new(),
            })
            .unwrap();

        let results = store
            .query(&DataFrameQuery {
                run_id: Some("r1".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].run_id, "r1");
    }

    #[test]
    fn store_query_filters_by_stage() {
        let store = InMemoryDataFrameStore::new();
        store
            .insert(DataFrameRow {
                run_id: "r1".into(),
                stage_name: "ingest".into(),
                column: "x".into(),
                value: DataFrameValue::Int(10),
                timestamp: Utc::now(),
                tags: HashMap::new(),
            })
            .unwrap();
        store
            .insert(DataFrameRow {
                run_id: "r1".into(),
                stage_name: "process".into(),
                column: "x".into(),
                value: DataFrameValue::Int(20),
                timestamp: Utc::now(),
                tags: HashMap::new(),
            })
            .unwrap();

        let results = store
            .query(&DataFrameQuery {
                stage_name: Some("ingest".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].stage_name, "ingest");
    }

    #[test]
    fn store_query_filters_by_columns() {
        let store = InMemoryDataFrameStore::new();
        store
            .insert(DataFrameRow {
                run_id: "r1".into(),
                stage_name: "s1".into(),
                column: "a".into(),
                value: DataFrameValue::Int(1),
                timestamp: Utc::now(),
                tags: HashMap::new(),
            })
            .unwrap();
        store
            .insert(DataFrameRow {
                run_id: "r1".into(),
                stage_name: "s1".into(),
                column: "b".into(),
                value: DataFrameValue::Int(2),
                timestamp: Utc::now(),
                tags: HashMap::new(),
            })
            .unwrap();
        store
            .insert(DataFrameRow {
                run_id: "r1".into(),
                stage_name: "s1".into(),
                column: "c".into(),
                value: DataFrameValue::Int(3),
                timestamp: Utc::now(),
                tags: HashMap::new(),
            })
            .unwrap();

        let results = store
            .query(&DataFrameQuery {
                columns: Some(vec!["a".into(), "c".into()]),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.column == "a" || r.column == "c"));
    }

    #[test]
    fn store_query_respects_limit() {
        let store = InMemoryDataFrameStore::new();
        for i in 0..5 {
            store
                .insert(DataFrameRow {
                    run_id: "r1".into(),
                    stage_name: "s1".into(),
                    column: "val".into(),
                    value: DataFrameValue::Int(i),
                    timestamp: Utc::now(),
                    tags: HashMap::new(),
                })
                .unwrap();
        }

        let results = store
            .query(&DataFrameQuery {
                limit: Some(3),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(results.len(), 3);
    }

    // ── Aggregation ──────────────────────────────────────────────────────

    #[test]
    fn aggregation_on_numeric_values() {
        let store = InMemoryDataFrameStore::new();
        for val in &[1.0, 2.0, 3.0, 4.0, 5.0] {
            store
                .insert(DataFrameRow {
                    run_id: "r1".into(),
                    stage_name: "metrics".into(),
                    column: "score".into(),
                    value: DataFrameValue::Float(*val),
                    timestamp: Utc::now(),
                    tags: HashMap::new(),
                })
                .unwrap();
        }

        let agg = store.aggregate("metrics", "score").unwrap();
        assert_eq!(agg.count, 5);
        assert!((agg.min - 1.0).abs() < 1e-10);
        assert!((agg.max - 5.0).abs() < 1e-10);
        assert!((agg.mean - 3.0).abs() < 1e-10);
        assert!((agg.sum - 15.0).abs() < 1e-10);
    }

    #[test]
    fn aggregation_empty_column_returns_zeroes() {
        let store = InMemoryDataFrameStore::new();
        let agg = store.aggregate("nonexistent", "missing").unwrap();
        assert_eq!(agg.count, 0);
        assert_eq!(agg.sum, 0.0);
        assert_eq!(agg.min, 0.0);
        assert_eq!(agg.max, 0.0);
        assert_eq!(agg.mean, 0.0);
    }

    #[test]
    fn aggregation_skips_non_numeric_values() {
        let store = InMemoryDataFrameStore::new();
        store
            .insert(DataFrameRow {
                run_id: "r1".into(),
                stage_name: "s1".into(),
                column: "val".into(),
                value: DataFrameValue::String("hello".into()),
                timestamp: Utc::now(),
                tags: HashMap::new(),
            })
            .unwrap();
        store
            .insert(DataFrameRow {
                run_id: "r1".into(),
                stage_name: "s1".into(),
                column: "val".into(),
                value: DataFrameValue::Float(42.0),
                timestamp: Utc::now(),
                tags: HashMap::new(),
            })
            .unwrap();

        let agg = store.aggregate("s1", "val").unwrap();
        assert_eq!(agg.count, 1, "only the float value should count");
        assert!((agg.sum - 42.0).abs() < 1e-10);
    }

    #[test]
    fn aggregation_mixed_int_and_float() {
        let store = InMemoryDataFrameStore::new();
        store
            .insert(DataFrameRow {
                run_id: "r1".into(),
                stage_name: "stats".into(),
                column: "val".into(),
                value: DataFrameValue::Int(10),
                timestamp: Utc::now(),
                tags: HashMap::new(),
            })
            .unwrap();
        store
            .insert(DataFrameRow {
                run_id: "r1".into(),
                stage_name: "stats".into(),
                column: "val".into(),
                value: DataFrameValue::Float(2.5),
                timestamp: Utc::now(),
                tags: HashMap::new(),
            })
            .unwrap();

        let agg = store.aggregate("stats", "val").unwrap();
        assert_eq!(agg.count, 2);
        assert!((agg.sum - 12.5).abs() < 1e-10);
        assert!((agg.mean - 6.25).abs() < 1e-10);
    }
}
