//! Soul-aware flash sheets.
//!
//! Flash sheets are lightweight, typed sheet projections over b00t dataframe
//! streams. The core runtime lives here so domain systems such as ledgrrr can
//! consume it as a library, register guards/hooks, and project their own cells
//! without owning the generic sheet model.

use crate::dataframe_receipt::{ColumnValue, Dataframe, Datatype};
use std::collections::BTreeMap;
use std::sync::Arc;
use thiserror::Error;

pub type SheetMetadata = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoulKind {
    Sheet,
    Row,
    Column,
    Cell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolicRule {
    pub id: String,
    pub expression: String,
    pub metadata: SheetMetadata,
}

impl SymbolicRule {
    pub fn new(id: impl Into<String>, expression: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            expression: expression.into(),
            metadata: SheetMetadata::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoulConcept {
    pub id: String,
    pub kind: SoulKind,
    pub metadata: SheetMetadata,
    pub symbolic_rules: Vec<SymbolicRule>,
}

impl SoulConcept {
    pub fn new(id: impl Into<String>, kind: SoulKind) -> Self {
        Self {
            id: id.into(),
            kind,
            metadata: SheetMetadata::new(),
            symbolic_rules: Vec::new(),
        }
    }

    pub fn with_rule(mut self, rule: SymbolicRule) -> Self {
        self.symbolic_rules.push(rule);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellAddress {
    pub row_id: String,
    pub column_id: String,
}

impl CellAddress {
    pub fn new(row_id: impl Into<String>, column_id: impl Into<String>) -> Self {
        Self {
            row_id: row_id.into(),
            column_id: column_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetRow {
    pub soul: SoulConcept,
}

impl SheetRow {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            soul: SoulConcept::new(format!("row:{id}"), SoulKind::Row),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetColumn {
    pub soul: SoulConcept,
    pub datatype: Datatype,
    pub nullable: bool,
}

impl SheetColumn {
    pub fn new(id: impl Into<String>, datatype: Datatype, nullable: bool) -> Self {
        let id = id.into();
        Self {
            soul: SoulConcept::new(format!("column:{id}"), SoulKind::Column),
            datatype,
            nullable,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CellExpression {
    Literal(ColumnValue),
    Formula(String),
    Symbol(String),
}

impl CellExpression {
    pub fn literal(value: ColumnValue) -> Self {
        Self::Literal(value)
    }

    pub fn formula(expression: impl Into<String>) -> Self {
        Self::Formula(expression.into())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SheetCell {
    pub soul: SoulConcept,
    pub expression: CellExpression,
    pub metadata: SheetMetadata,
}

impl SheetCell {
    pub fn new(address: &CellAddress, expression: CellExpression) -> Self {
        Self {
            soul: SoulConcept::new(
                format!("cell:{}:{}", address.row_id, address.column_id),
                SoulKind::Cell,
            ),
            expression,
            metadata: SheetMetadata::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CellChange {
    pub address: CellAddress,
    pub old: Option<SheetCell>,
    pub new: SheetCell,
}

#[derive(Debug, Error)]
pub enum FlashSheetError {
    #[error("unknown row: {0}")]
    UnknownRow(String),
    #[error("unknown column: {0}")]
    UnknownColumn(String),
    #[error("guard {guard} rejected cell change: {reason}")]
    GuardRejected { guard: String, reason: String },
    #[error("hook {hook} failed: {reason}")]
    HookFailed { hook: String, reason: String },
    #[error("dataframe has mismatched column lengths")]
    MismatchedDataframe,
}

pub type GuardResult = Result<(), String>;
pub type HookResult = Result<(), String>;

pub trait CellGuard: Send + Sync {
    fn check(&self, change: &CellChange, sheet: &FlashSheet) -> GuardResult;
}

impl<F> CellGuard for F
where
    F: Fn(&CellChange, &FlashSheet) -> GuardResult + Send + Sync,
{
    fn check(&self, change: &CellChange, sheet: &FlashSheet) -> GuardResult {
        self(change, sheet)
    }
}

pub trait CellHook: Send + Sync {
    fn on_change(&self, change: &CellChange, sheet: &FlashSheet) -> HookResult;
}

impl<F> CellHook for F
where
    F: Fn(&CellChange, &FlashSheet) -> HookResult + Send + Sync,
{
    fn on_change(&self, change: &CellChange, sheet: &FlashSheet) -> HookResult {
        self(change, sheet)
    }
}

struct RegisteredGuard {
    name: String,
    guard: Arc<dyn CellGuard>,
}

struct RegisteredHook {
    name: String,
    hook: Arc<dyn CellHook>,
}

pub struct FlashSheet {
    pub soul: SoulConcept,
    pub rows: BTreeMap<String, SheetRow>,
    pub columns: BTreeMap<String, SheetColumn>,
    pub cells: BTreeMap<CellAddress, SheetCell>,
    pub metadata: SheetMetadata,
    guards: Vec<RegisteredGuard>,
    hooks: Vec<RegisteredHook>,
}

impl FlashSheet {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            soul: SoulConcept::new(format!("sheet:{id}"), SoulKind::Sheet),
            rows: BTreeMap::new(),
            columns: BTreeMap::new(),
            cells: BTreeMap::new(),
            metadata: SheetMetadata::new(),
            guards: Vec::new(),
            hooks: Vec::new(),
        }
    }

    pub fn add_row(&mut self, row_id: impl Into<String>) {
        let row_id = row_id.into();
        self.rows
            .entry(row_id.clone())
            .or_insert_with(|| SheetRow::new(row_id));
    }

    pub fn add_column(&mut self, column_id: impl Into<String>, datatype: Datatype, nullable: bool) {
        let column_id = column_id.into();
        self.columns
            .entry(column_id.clone())
            .or_insert_with(|| SheetColumn::new(column_id, datatype, nullable));
    }

    pub fn register_guard(&mut self, name: impl Into<String>, guard: impl CellGuard + 'static) {
        self.guards.push(RegisteredGuard {
            name: name.into(),
            guard: Arc::new(guard),
        });
    }

    pub fn register_hook(&mut self, name: impl Into<String>, hook: impl CellHook + 'static) {
        self.hooks.push(RegisteredHook {
            name: name.into(),
            hook: Arc::new(hook),
        });
    }

    pub fn set_cell(
        &mut self,
        address: CellAddress,
        expression: CellExpression,
    ) -> Result<CellChange, FlashSheetError> {
        if !self.rows.contains_key(&address.row_id) {
            return Err(FlashSheetError::UnknownRow(address.row_id));
        }
        if !self.columns.contains_key(&address.column_id) {
            return Err(FlashSheetError::UnknownColumn(address.column_id));
        }

        let old = self.cells.get(&address).cloned();
        let new = SheetCell::new(&address, expression);
        let change = CellChange {
            address: address.clone(),
            old,
            new: new.clone(),
        };

        for registered in &self.guards {
            registered.guard.check(&change, self).map_err(|reason| {
                FlashSheetError::GuardRejected {
                    guard: registered.name.clone(),
                    reason,
                }
            })?;
        }

        self.cells.insert(address, new);

        for registered in &self.hooks {
            registered.hook.on_change(&change, self).map_err(|reason| {
                FlashSheetError::HookFailed {
                    hook: registered.name.clone(),
                    reason,
                }
            })?;
        }

        Ok(change)
    }

    pub fn cell(&self, address: &CellAddress) -> Option<&SheetCell> {
        self.cells.get(address)
    }

    pub fn from_dataframe(
        id: impl Into<String>,
        dataframe: &Dataframe,
    ) -> Result<Self, FlashSheetError> {
        let mut sheet = Self::new(id);
        let row_count = dataframe.row_count();

        if dataframe
            .columns
            .iter()
            .any(|column| column.len() != row_count)
        {
            return Err(FlashSheetError::MismatchedDataframe);
        }

        for field in &dataframe.fields {
            sheet.add_column(&field.name, field.datatype, field.nullable);
        }

        for row_idx in 0..row_count {
            let row_id = format!("r{}", row_idx + 1);
            sheet.add_row(&row_id);
            for (field_idx, field) in dataframe.fields.iter().enumerate() {
                let value = dataframe.columns[field_idx][row_idx].clone();
                sheet.set_cell(
                    CellAddress::new(&row_id, &field.name),
                    CellExpression::Literal(value),
                )?;
            }
        }

        Ok(sheet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataframe_receipt::{ColumnValue, DataframeReceipt, FocusRecord};
    use serde::Deserialize;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Debug, Deserialize)]
    struct FixtureRow {
        row_id: String,
        column_id: String,
        value: String,
    }

    #[test]
    fn cell_change_runs_guard_and_hook() {
        let fired = Arc::new(AtomicUsize::new(0));
        let fired_hook = fired.clone();
        let mut sheet = FlashSheet::new("exec");
        sheet.add_row("r1");
        sheet.add_column("status", Datatype::Utf8, false);
        sheet.register_guard(
            "non-empty",
            |change: &CellChange, _sheet: &FlashSheet| match &change.new.expression {
                CellExpression::Literal(ColumnValue::Utf8(value)) if value.is_empty() => {
                    Err("empty status".to_string())
                }
                _ => Ok(()),
            },
        );
        sheet.register_hook("count", move |_change: &CellChange, _sheet: &FlashSheet| {
            fired_hook.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        sheet
            .set_cell(
                CellAddress::new("r1", "status"),
                CellExpression::literal(ColumnValue::Utf8("ready".to_string())),
            )
            .unwrap();

        assert_eq!(fired.load(Ordering::SeqCst), 1);
        assert!(matches!(
            sheet.set_cell(
                CellAddress::new("r1", "status"),
                CellExpression::literal(ColumnValue::Utf8(String::new())),
            ),
            Err(FlashSheetError::GuardRejected { .. })
        ));
    }

    #[test]
    fn fixture_cells_have_soul_concepts() {
        let fixture = include_str!("../tests/fixtures/flash_sheet_cells.json");
        let rows: Vec<FixtureRow> = serde_json::from_str(fixture).unwrap();
        let mut sheet = FlashSheet::new("fixture");
        sheet.add_column("status", Datatype::Utf8, false);
        sheet.add_column("score", Datatype::Utf8, true);

        for row in rows {
            sheet.add_row(&row.row_id);
            sheet
                .set_cell(
                    CellAddress::new(&row.row_id, &row.column_id),
                    CellExpression::literal(ColumnValue::Utf8(row.value)),
                )
                .unwrap();
        }

        let address = CellAddress::new("r1", "status");
        let cell = sheet.cell(&address).unwrap();
        assert_eq!(sheet.soul.kind, SoulKind::Sheet);
        assert_eq!(sheet.rows["r1"].soul.kind, SoulKind::Row);
        assert_eq!(sheet.columns["status"].soul.kind, SoulKind::Column);
        assert_eq!(cell.soul.kind, SoulKind::Cell);
    }

    #[test]
    fn dataframe_receipt_projects_to_flash_sheet() {
        let record = FocusRecord {
            experiment_id: "exp-1".into(),
            variant: "treatment".into(),
            personality: "guardian".into(),
            score: 0.91,
            agent_id: "agent-a".into(),
            reasoning_review: "accepted".into(),
            occurred_at_ms: 1_700_000_000_000,
        };

        let dataframe = record.to_dataframe();
        let sheet = FlashSheet::from_dataframe("focus", &dataframe).unwrap();

        let address = CellAddress::new("r1", "agent_id");
        assert_eq!(
            sheet.cell(&address).unwrap().expression,
            CellExpression::Literal(ColumnValue::Utf8("agent-a".into()))
        );
    }

    #[test]
    fn row_column_and_cell_accept_symbolic_rules() {
        let mut sheet = FlashSheet::new("rules");
        sheet
            .soul
            .symbolic_rules
            .push(SymbolicRule::new("sheet-total", "SUM(score)"));
        sheet.add_row("r1");
        sheet.add_column("score", Datatype::Float64, false);
        sheet
            .rows
            .get_mut("r1")
            .unwrap()
            .soul
            .symbolic_rules
            .push(SymbolicRule::new("row-ready", "status == 'ready'"));
        sheet
            .columns
            .get_mut("score")
            .unwrap()
            .soul
            .symbolic_rules
            .push(SymbolicRule::new("score-range", "score >= 0 && score <= 1"));

        sheet
            .set_cell(
                CellAddress::new("r1", "score"),
                CellExpression::formula("A1 * confidence"),
            )
            .unwrap();

        assert_eq!(sheet.soul.symbolic_rules.len(), 1);
        assert_eq!(sheet.rows["r1"].soul.symbolic_rules.len(), 1);
        assert_eq!(sheet.columns["score"].soul.symbolic_rules.len(), 1);
    }
}
