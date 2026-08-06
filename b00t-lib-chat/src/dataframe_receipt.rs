//! Dataframe-typed receipts → Apache Arrow.
//!
//! Every subsystem that executes billable/auditable work emits a *typed
//! receipt*. The [`DataframeReceipt`] trait is the contract each subsystem
//! implements: given a receipt, produce a columnar [`Dataframe`]. The dataframe
//! is Arrow-shaped (named columns + Arrow-compatible datatypes), so the same
//! receipt can be dumped to Parquet, queried with DataFusion, or streamed to a
//! finops warehouse.
//!
//! The local [`Dataframe`] type compiles with **no** external dependency, so
//! the trait is always available. When the `dataframe` feature is enabled,
//! `Dataframe::to_arrow()` materializes a real `arrow::record_batch::RecordBatch`
//! — the concrete Arrow implementation the operator requires. The FOCUS record
//! (ledgrr's experiment/finops record) is the reference [`DataframeReceipt`]
//! implementation.

use serde::{Deserialize, Serialize};

/// Arrow-compatible logical datatype for a receipt column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Datatype {
    Null,
    Bool,
    Int64,
    Float64,
    Utf8,
    /// Milliseconds since the Unix epoch.
    TimestampMs,
}

impl Datatype {
    /// Map to the corresponding `arrow` datatype (feature `dataframe`).
    #[cfg(feature = "dataframe")]
    pub fn to_arrow(&self) -> arrow::datatypes::DataType {
        use arrow::datatypes::DataType;
        match self {
            Datatype::Null => DataType::Null,
            Datatype::Bool => DataType::Boolean,
            Datatype::Int64 => DataType::Int64,
            Datatype::Float64 => DataType::Float64,
            Datatype::Utf8 => DataType::Utf8,
            Datatype::TimestampMs => {
                DataType::Timestamp(arrow::datatypes::TimeUnit::Millisecond, None)
            }
        }
    }
}

/// A single typed column value (Arrow-compatible).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ColumnValue {
    Null,
    Bool(bool),
    Int64(i64),
    Float64(f64),
    Utf8(String),
    TimestampMs(i64),
}

impl ColumnValue {
    pub fn as_bool(&self) -> bool {
        matches!(self, ColumnValue::Bool(true))
    }
    pub fn as_i64(&self) -> i64 {
        match self {
            ColumnValue::Int64(v) => *v,
            ColumnValue::TimestampMs(v) => *v,
            _ => 0,
        }
    }
    pub fn as_f64(&self) -> f64 {
        match self {
            ColumnValue::Float64(v) => *v,
            ColumnValue::Int64(v) => *v as f64,
            _ => 0.0,
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            ColumnValue::Utf8(s) => s,
            _ => "",
        }
    }
}

/// A columnar, Arrow-shaped receipt dataframe (one or more rows).
#[derive(Debug, Clone)]
pub struct Dataframe {
    pub fields: Vec<Field>,
    pub columns: Vec<Vec<ColumnValue>>,
}

/// A named, typed column descriptor.
#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub datatype: Datatype,
    pub nullable: bool,
}

impl Dataframe {
    /// New dataframe with the given field schema; columns are empty.
    pub fn new(fields: Vec<Field>) -> Self {
        let columns = vec![Vec::new(); fields.len()];
        Self { fields, columns }
    }

    /// Append one row. `values` must align 1:1 with `fields`.
    pub fn push_row(&mut self, values: Vec<ColumnValue>) {
        debug_assert_eq!(values.len(), self.fields.len());
        for (col, v) in self.columns.iter_mut().zip(values.into_iter()) {
            col.push(v);
        }
    }

    pub fn row_count(&self) -> usize {
        self.columns.first().map(|c| c.len()).unwrap_or(0)
    }

    /// Materialize a real Apache Arrow `RecordBatch` from this dataframe.
    #[cfg(feature = "dataframe")]
    pub fn to_arrow(&self) -> arrow::error::Result<arrow::record_batch::RecordBatch> {
        use arrow::array::*;
        use arrow::datatypes::{Field as AField, Schema};
        use arrow::record_batch::RecordBatch;
        use std::sync::Arc;

        let a_fields: Vec<Arc<AField>> = self
            .fields
            .iter()
            .map(|f| Arc::new(AField::new(&f.name, f.datatype.to_arrow(), f.nullable)))
            .collect();
        let schema = Arc::new(Schema::new(a_fields));

        let mut arrays: Vec<ArrayRef> = Vec::with_capacity(self.fields.len());
        for (field, col) in self.fields.iter().zip(self.columns.iter()) {
            let arr: ArrayRef = match field.datatype {
                Datatype::Null => Arc::new(NullArray::new(col.len())),
                Datatype::Bool => {
                    let mut b = BooleanBuilder::new();
                    for v in col {
                        b.append_value(v.as_bool());
                    }
                    Arc::new(b.finish())
                }
                Datatype::Int64 => {
                    let mut b = Int64Builder::new();
                    for v in col {
                        b.append_value(v.as_i64());
                    }
                    Arc::new(b.finish())
                }
                Datatype::Float64 => {
                    let mut b = Float64Builder::new();
                    for v in col {
                        b.append_value(v.as_f64());
                    }
                    Arc::new(b.finish())
                }
                Datatype::Utf8 => {
                    let mut b = StringBuilder::new();
                    for v in col {
                        b.append_value(v.as_str());
                    }
                    Arc::new(b.finish())
                }
                Datatype::TimestampMs => {
                    let mut b = TimestampMillisecondBuilder::new();
                    for v in col {
                        b.append_value(v.as_i64());
                    }
                    Arc::new(b.finish())
                }
            };
            arrays.push(arr);
        }
        RecordBatch::try_new(schema, arrays)
    }
}

/// Contract every subsystem implements for its typed receipt.
///
/// `to_dataframe()` returns a single-row (or multi-row) Arrow-shaped dataframe
/// describing the receipt. Subsystems call this to feed finops/audit pipelines.
pub trait DataframeReceipt {
    /// Logical schema (column names + datatypes).
    fn fields(&self) -> Vec<Field>;
    /// The receipt's rows as columnar values.
    fn rows(&self) -> Vec<Vec<ColumnValue>>;
    /// Convenience: build the dataframe (schema + rows).
    fn to_dataframe(&self) -> Dataframe {
        let mut df = Dataframe::new(self.fields());
        for row in self.rows() {
            df.push_row(row);
        }
        df
    }
}

// ── Reference implementation: FOCUS record ─────────────────────────────────
//
// FOCUS is ledgrr's experiment/finops record (see b00t-cli datum_schema.rs:
// x_ExperimentId, x_Variant, x_Personality, x_ExperimentScore, x_AgentId,
// x_ReasoningReview). Any subsystem running an experiment emits one.

/// A FOCUS record — the canonical dataframe-typed finops receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusRecord {
    pub experiment_id: String,
    pub variant: String,
    pub personality: String,
    pub score: f64,
    pub agent_id: String,
    pub reasoning_review: String,
    pub occurred_at_ms: i64,
}

impl DataframeReceipt for FocusRecord {
    fn fields(&self) -> Vec<Field> {
        use Datatype::*;
        vec![
            Field {
                name: "experiment_id".into(),
                datatype: Utf8,
                nullable: false,
            },
            Field {
                name: "variant".into(),
                datatype: Utf8,
                nullable: false,
            },
            Field {
                name: "personality".into(),
                datatype: Utf8,
                nullable: true,
            },
            Field {
                name: "score".into(),
                datatype: Float64,
                nullable: false,
            },
            Field {
                name: "agent_id".into(),
                datatype: Utf8,
                nullable: false,
            },
            Field {
                name: "reasoning_review".into(),
                datatype: Utf8,
                nullable: true,
            },
            Field {
                name: "occurred_at_ms".into(),
                datatype: TimestampMs,
                nullable: false,
            },
        ]
    }

    fn rows(&self) -> Vec<Vec<ColumnValue>> {
        vec![vec![
            ColumnValue::Utf8(self.experiment_id.clone()),
            ColumnValue::Utf8(self.variant.clone()),
            ColumnValue::Utf8(self.personality.clone()),
            ColumnValue::Float64(self.score),
            ColumnValue::Utf8(self.agent_id.clone()),
            ColumnValue::Utf8(self.reasoning_review.clone()),
            ColumnValue::TimestampMs(self.occurred_at_ms),
        ]]
    }
}

// ── ledgrrr UsageReceipt is also a dataframe-typed receipt ──────────────────

impl DataframeReceipt for crate::ledgrrr::UsageReceipt {
    fn fields(&self) -> Vec<Field> {
        use Datatype::*;
        vec![
            Field {
                name: "receipt_id".into(),
                datatype: Utf8,
                nullable: false,
            },
            Field {
                name: "agent_id".into(),
                datatype: Utf8,
                nullable: false,
            },
            Field {
                name: "project".into(),
                datatype: Utf8,
                nullable: false,
            },
            Field {
                name: "capability".into(),
                datatype: Utf8,
                nullable: false,
            },
            Field {
                name: "units".into(),
                datatype: Int64,
                nullable: false,
            },
            Field {
                name: "occurred_at".into(),
                datatype: TimestampMs,
                nullable: false,
            },
            Field {
                name: "finops_code".into(),
                datatype: Utf8,
                nullable: true,
            },
            Field {
                name: "constraint_satisfied".into(),
                datatype: Bool,
                nullable: false,
            },
        ]
    }

    fn rows(&self) -> Vec<Vec<ColumnValue>> {
        vec![vec![
            ColumnValue::Utf8(self.receipt_id.clone()),
            ColumnValue::Utf8(self.agent_id.clone()),
            ColumnValue::Utf8(self.project.clone()),
            ColumnValue::Utf8(self.capability.clone()),
            ColumnValue::Int64(self.units as i64),
            ColumnValue::TimestampMs(self.occurred_at as i64),
            ColumnValue::Utf8(self.finops_code.clone().unwrap_or_default()),
            ColumnValue::Bool(self.constraint_satisfied),
        ]]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_record_builds_a_dataframe_row() {
        let f = FocusRecord {
            experiment_id: "exp-1".into(),
            variant: "treatment".into(),
            personality: "meticulous".into(),
            score: 0.87,
            agent_id: "alpha".into(),
            reasoning_review: "accepted".into(),
            occurred_at_ms: 1_700_000_000_000,
        };
        let df = f.to_dataframe();
        assert_eq!(df.row_count(), 1);
        assert_eq!(df.fields.len(), 7);
        assert_eq!(df.columns[4][0].as_str(), "alpha");
        assert_eq!(df.columns[3][0].as_f64(), 0.87);
    }

    #[cfg(feature = "dataframe")]
    #[test]
    fn focus_record_materializes_arrow() {
        let f = FocusRecord {
            experiment_id: "exp-2".into(),
            variant: "control".into(),
            personality: "low".into(),
            score: 0.42,
            agent_id: "beta".into(),
            reasoning_review: "rejected".into(),
            occurred_at_ms: 1_700_000_000_123,
        };
        let batch = f.to_dataframe().to_arrow().unwrap();
        assert_eq!(batch.num_rows(), 1);
        assert_eq!(batch.num_columns(), 7);
        assert_eq!(batch.schema().field(0).name(), "experiment_id");
    }
}
