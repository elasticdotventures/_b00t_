use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

// ── SoulColType ───────────────────────────────────────────────────────────────

/// Column type tag — drives schema validation and overlay interpretation.
/// Token/Cake/Timestamp are stored as Text/Float in SoulValue;
/// col_type tells the soul overlay how to validate and handle them.
#[derive(Debug, Clone, PartialEq)]
pub enum SoulColType {
    Text,
    Int,
    Float,
    Cake,       // f64 cake units (1🎂 = $10 USD)
    Bool,
    Timestamp,  // ISO 8601 string
    Token,      // ObfuscatedStr stored as "b64xor:<data>"
    Json,       // arbitrary JSON blob as string
}

impl SoulColType {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Text      => "text",
            Self::Int       => "int",
            Self::Float     => "float",
            Self::Cake      => "cake",
            Self::Bool      => "bool",
            Self::Timestamp => "timestamp",
            Self::Token     => "token",
            Self::Json      => "json",
        }
    }

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "text"      => Ok(Self::Text),
            "int"       => Ok(Self::Int),
            "float"     => Ok(Self::Float),
            "cake"      => Ok(Self::Cake),
            "bool"      => Ok(Self::Bool),
            "timestamp" => Ok(Self::Timestamp),
            "token"     => Ok(Self::Token),
            "json"      => Ok(Self::Json),
            other       => Err(anyhow!(
                "unknown column type '{other}'; valid: text int float cake bool timestamp token json"
            )),
        }
    }
}

impl Serialize for SoulColType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SoulColType {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

// ── SoulColumn ────────────────────────────────────────────────────────────────

/// A column definition. Serializes as "name:type" or "name:type?" (nullable).
#[derive(Debug, Clone, PartialEq)]
pub struct SoulColumn {
    pub name: String,
    pub col_type: SoulColType,
    pub nullable: bool,
}

impl SoulColumn {
    /// Parse "name:type" or "name:type?" shorthand.
    pub fn parse(s: &str) -> Result<Self> {
        let (name, type_part) = s.split_once(':').ok_or_else(|| {
            anyhow!("column must be 'name:type' or 'name:type?', got: {s}")
        })?;
        let (type_str, nullable) = if type_part.ends_with('?') {
            (&type_part[..type_part.len() - 1], true)
        } else {
            (type_part, false)
        };
        Ok(Self {
            name: name.to_string(),
            col_type: SoulColType::from_str(type_str)?,
            nullable,
        })
    }
}

impl Serialize for SoulColumn {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let suffix = if self.nullable { "?" } else { "" };
        s.serialize_str(&format!("{}:{}{}", self.name, self.col_type.as_str(), suffix))
    }
}

impl<'de> Deserialize<'de> for SoulColumn {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

// ── SoulValue ─────────────────────────────────────────────────────────────────

/// Dynamic column value. Untagged so TOML round-trips cleanly.
/// Variant order matters for untagged deserialization: Bool before Int before Float.
/// Null is intentionally absent — nullable columns omit the field key from the row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SoulValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

impl SoulValue {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            Self::Int(i)   => Some(*i as f64),
            _              => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s.as_str()),
            _             => None,
        }
    }
}

// ── SoulRow ───────────────────────────────────────────────────────────────────

/// A row in a SoulDataFramerr. id is monotonic u64 assigned at insert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulRow {
    pub id: u64,
    pub created_at: DateTime<Utc>,
    #[serde(flatten)]
    pub fields: BTreeMap<String, SoulValue>,
}

// ── SoulDataFramerr ───────────────────────────────────────────────────────────

/// A named, schematized append-only table of SoulRows.
/// Append-only: rows are never mutated (immutable log, CRDT-safe across sessions).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SoulDataFramerr {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub columns: Vec<SoulColumn>,
    #[serde(default, rename = "rows")]
    pub rows: Vec<SoulRow>,
}

impl SoulDataFramerr {
    pub fn new(name: impl Into<String>, columns: Vec<SoulColumn>) -> Self {
        Self { name: name.into(), columns, rows: Vec::new() }
    }

    /// Validate and append a row; returns the assigned frame id (1-based).
    pub fn insert(&mut self, fields: BTreeMap<String, SoulValue>) -> Result<u64> {
        self.validate_fields(&fields)?;
        let id = self.rows.len() as u64 + 1;
        self.rows.push(SoulRow { id, created_at: Utc::now(), fields });
        Ok(id)
    }

    fn validate_fields(&self, fields: &BTreeMap<String, SoulValue>) -> Result<()> {
        for col in &self.columns {
            if !col.nullable && !fields.contains_key(&col.name) {
                return Err(anyhow!("required column '{}' missing from row", col.name));
            }
        }
        Ok(())
    }

    pub fn get(&self, id: u64) -> Option<&SoulRow> {
        self.rows.iter().find(|r| r.id == id)
    }

    pub fn rows_after(&self, after_id: u64) -> impl Iterator<Item = &SoulRow> {
        self.rows.iter().filter(move |r| r.id > after_id)
    }
}

// ── FrameCursor ───────────────────────────────────────────────────────────────

/// Durable iterator pointer. frame_id = last CONSUMED frame (0 = nothing consumed).
/// Serializes to [soul.cursors.NAME] in SOUL.tomllmd — survives across sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameCursor {
    pub table: String,
    pub frame_id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
}

impl FrameCursor {
    pub fn new(table: impl Into<String>) -> Self {
        Self { table: table.into(), frame_id: 0, tag: None }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Return the next unconsumed row and advance self.frame_id.
    pub fn next<'a>(&mut self, df: &'a SoulDataFramerr) -> Option<&'a SoulRow> {
        let row = df.rows_after(self.frame_id).next()?;
        self.frame_id = row.id;
        Some(row)
    }

    pub fn reset(&mut self) {
        self.frame_id = 0;
    }

    pub fn at_eof(&self, df: &SoulDataFramerr) -> bool {
        df.rows_after(self.frame_id).next().is_none()
    }
}

// ── SoulAlarm ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AlarmAggregate {
    PerFrame, // last row only
    Sum,
    Count,
    Avg,
}

/// An alarm fires emit when a column aggregate satisfies condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulAlarm {
    pub name: String,
    pub table: String,
    pub column: String,
    /// Simple numeric condition: ">= 1.0", "< 0", "== 5", "!= 0"
    pub condition: String,
    pub aggregate: AlarmAggregate,
    /// Event name to emit (passed to hook_engine::fire_event).
    pub emit: String,
}

impl SoulAlarm {
    pub fn check(&self, df: &SoulDataFramerr) -> bool {
        let values: Vec<f64> = df
            .rows
            .iter()
            .filter_map(|r| r.fields.get(&self.column)?.as_f64())
            .collect();

        if values.is_empty() {
            return false;
        }

        let agg = match self.aggregate {
            AlarmAggregate::Sum      => values.iter().sum(),
            AlarmAggregate::Avg      => values.iter().sum::<f64>() / values.len() as f64,
            AlarmAggregate::Count    => values.len() as f64,
            AlarmAggregate::PerFrame => *values.last().unwrap(),
        };

        eval_numeric_condition(&self.condition, agg)
    }
}

/// Minimal numeric condition evaluator: "<op> <rhs>".
fn eval_numeric_condition(cond: &str, value: f64) -> bool {
    let cond = cond.trim();
    macro_rules! parse_rhs {
        ($rest:expr) => {
            match $rest.trim().parse::<f64>() {
                Ok(t) => t,
                Err(_) => return false,
            }
        };
    }
    if let Some(rest) = cond.strip_prefix(">=") { return value >= parse_rhs!(rest); }
    if let Some(rest) = cond.strip_prefix("<=") { return value <= parse_rhs!(rest); }
    if let Some(rest) = cond.strip_prefix("!=") { return (value - parse_rhs!(rest)).abs() >= f64::EPSILON; }
    if let Some(rest) = cond.strip_prefix("==") { return (value - parse_rhs!(rest)).abs() < f64::EPSILON; }
    if let Some(rest) = cond.strip_prefix('>') { return value > parse_rhs!(rest); }
    if let Some(rest) = cond.strip_prefix('<') { return value < parse_rhs!(rest); }
    false
}

// ── ObfuscatedStr ─────────────────────────────────────────────────────────────

/// XOR+base64 token encoding keyed to agent identity hash.
/// NOT cryptographic — purpose is grep-resistance for tokens at rest in SOUL.tomllmd.
/// Stored as "b64xor:<base64(plaintext XOR key)>".
/// Key = SHA-256(agent_id + "::" + context)[..32 bytes].
#[derive(Debug, Clone, PartialEq)]
pub struct ObfuscatedStr(String);

impl ObfuscatedStr {
    const PREFIX: &'static str = "b64xor:";

    fn derive_key(agent_id: &str, context: &str) -> Vec<u8> {
        let mut h = Sha256::new();
        h.update(agent_id.as_bytes());
        h.update(b"::");
        h.update(context.as_bytes());
        h.finalize().to_vec() // 32 bytes
    }

    pub fn encode(plaintext: &str, agent_id: &str, context: &str) -> Self {
        let key = Self::derive_key(agent_id, context);
        let xored: Vec<u8> = plaintext
            .as_bytes()
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % key.len()])
            .collect();
        Self(format!("{}{}", Self::PREFIX, STANDARD.encode(&xored)))
    }

    pub fn decode(&self, agent_id: &str, context: &str) -> Result<String> {
        let b64 = self.0.strip_prefix(Self::PREFIX).ok_or_else(|| {
            anyhow!("not an ObfuscatedStr: missing b64xor: prefix")
        })?;
        let xored = STANDARD.decode(b64).map_err(|e| anyhow!("base64 decode: {e}"))?;
        let key = Self::derive_key(agent_id, context);
        let plain: Vec<u8> = xored
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ key[i % key.len()])
            .collect();
        String::from_utf8(plain).map_err(|e| anyhow!("UTF-8 decode: {e}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_obfuscated(s: &str) -> bool {
        s.starts_with(Self::PREFIX)
    }

    /// Wrap a raw "b64xor:..." string (e.g. loaded from TOML).
    pub fn from_raw(s: impl Into<String>) -> Result<Self> {
        let s = s.into();
        if !s.starts_with(Self::PREFIX) {
            return Err(anyhow!("expected b64xor: prefix"));
        }
        Ok(Self(s))
    }
}

impl Serialize for ObfuscatedStr {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ObfuscatedStr {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if !s.starts_with(Self::PREFIX) {
            return Err(serde::de::Error::custom(format!(
                "expected b64xor: prefix, got: {s}"
            )));
        }
        Ok(Self(s))
    }
}

// ── SoulDataFramerrRegistry ───────────────────────────────────────────────────

/// Top-level registry — the [soul] namespace in SOUL.tomllmd.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SoulDataFramerrRegistry {
    #[serde(default)]
    pub tables: BTreeMap<String, SoulDataFramerr>,
    #[serde(default)]
    pub cursors: BTreeMap<String, FrameCursor>,
    #[serde(default)]
    pub alarms: Vec<SoulAlarm>,
}

impl SoulDataFramerrRegistry {
    pub fn get_or_create(&mut self, name: &str, columns: Vec<SoulColumn>) -> &mut SoulDataFramerr {
        self.tables
            .entry(name.to_string())
            .or_insert_with(|| SoulDataFramerr::new(name, columns))
    }

    /// Evaluate all alarms; return emit event names for those that fired.
    pub fn check_alarms(&self) -> Vec<&str> {
        self.alarms
            .iter()
            .filter(|alarm| {
                self.tables
                    .get(&alarm.table)
                    .map(|df| alarm.check(df))
                    .unwrap_or(false)
            })
            .map(|a| a.emit.as_str())
            .collect()
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_parse_required() {
        let c = SoulColumn::parse("cost_cake:cake").unwrap();
        assert_eq!(c.name, "cost_cake");
        assert_eq!(c.col_type, SoulColType::Cake);
        assert!(!c.nullable);
    }

    #[test]
    fn column_parse_nullable() {
        let c = SoulColumn::parse("note:text?").unwrap();
        assert_eq!(c.col_type, SoulColType::Text);
        assert!(c.nullable);
    }

    #[test]
    fn column_serde_roundtrip() {
        let c = SoulColumn { name: "n".into(), col_type: SoulColType::Token, nullable: true };
        let s = serde_json::to_string(&c).unwrap();
        assert_eq!(s, r#""n:token?""#);
        let c2: SoulColumn = serde_json::from_str(&s).unwrap();
        assert_eq!(c, c2);
    }

    #[test]
    fn insert_required_field_missing_errors() {
        let mut df = SoulDataFramerr::new("t", vec![
            SoulColumn { name: "val".into(), col_type: SoulColType::Float, nullable: false },
        ]);
        let err = df.insert(BTreeMap::new()).unwrap_err();
        assert!(err.to_string().contains("required column 'val'"));
    }

    #[test]
    fn cursor_advance_and_reset() {
        let mut df = SoulDataFramerr::new("t", vec![
            SoulColumn { name: "v".into(), col_type: SoulColType::Int, nullable: false },
        ]);
        df.insert(BTreeMap::from([("v".into(), SoulValue::Int(1))])).unwrap();
        df.insert(BTreeMap::from([("v".into(), SoulValue::Int(2))])).unwrap();

        let mut cur = FrameCursor::new("t");
        assert!(!cur.at_eof(&df));
        assert_eq!(cur.next(&df).unwrap().id, 1);
        assert_eq!(cur.next(&df).unwrap().id, 2);
        assert!(cur.at_eof(&df));
        cur.reset();
        assert!(!cur.at_eof(&df));
        assert_eq!(cur.next(&df).unwrap().id, 1);
    }

    #[test]
    fn alarm_sum_fires_over_threshold() {
        let mut df = SoulDataFramerr::new("b", vec![
            SoulColumn { name: "cost".into(), col_type: SoulColType::Cake, nullable: false },
        ]);
        df.insert(BTreeMap::from([("cost".into(), SoulValue::Float(0.6))])).unwrap();
        df.insert(BTreeMap::from([("cost".into(), SoulValue::Float(0.6))])).unwrap();
        let alarm = SoulAlarm {
            name: "ceil".into(), table: "b".into(), column: "cost".into(),
            condition: ">= 1.0".into(), aggregate: AlarmAggregate::Sum,
            emit: "soul.budget.exceeded".into(),
        };
        assert!(alarm.check(&df));
    }

    #[test]
    fn alarm_does_not_fire_below_threshold() {
        let mut df = SoulDataFramerr::new("b", vec![
            SoulColumn { name: "cost".into(), col_type: SoulColType::Cake, nullable: false },
        ]);
        df.insert(BTreeMap::from([("cost".into(), SoulValue::Float(0.3))])).unwrap();
        let alarm = SoulAlarm {
            name: "ceil".into(), table: "b".into(), column: "cost".into(),
            condition: ">= 1.0".into(), aggregate: AlarmAggregate::Sum,
            emit: "soul.budget.exceeded".into(),
        };
        assert!(!alarm.check(&df));
    }

    #[test]
    fn obfuscated_str_roundtrip() {
        let plain = "hf_secret_token_abc123";
        let enc = ObfuscatedStr::encode(plain, "agent-test", "training_budget");
        assert!(enc.as_str().starts_with("b64xor:"));
        assert_ne!(enc.as_str(), plain);
        let dec = enc.decode("agent-test", "training_budget").unwrap();
        assert_eq!(dec, plain);
    }

    #[test]
    fn obfuscated_str_wrong_key_gives_garbage() {
        let enc = ObfuscatedStr::encode("secret", "agent-a", "ctx");
        // Wrong key → XOR produces different bytes; may be invalid UTF-8 (Err) or a
        // different string — either outcome proves the plaintext is not recoverable.
        match enc.decode("agent-b", "ctx") {
            Ok(s)  => assert_ne!(s, "secret"),
            Err(_) => {} // non-UTF-8 output is equally acceptable
        }
    }

    #[test]
    fn registry_check_alarms_fires() {
        let mut reg = SoulDataFramerrRegistry::default();
        {
            let t = reg.get_or_create("budget", vec![
                SoulColumn { name: "cost".into(), col_type: SoulColType::Cake, nullable: false },
            ]);
            t.insert(BTreeMap::from([("cost".into(), SoulValue::Float(1.5))])).unwrap();
        }
        reg.alarms.push(SoulAlarm {
            name: "ceiling".into(), table: "budget".into(), column: "cost".into(),
            condition: ">= 1.0".into(), aggregate: AlarmAggregate::Sum,
            emit: "soul.budget.exceeded".into(),
        });
        assert_eq!(reg.check_alarms(), vec!["soul.budget.exceeded"]);
    }

    #[test]
    fn eval_conditions() {
        assert!(eval_numeric_condition(">= 1.0", 1.0));
        assert!(!eval_numeric_condition(">= 1.0", 0.9));
        assert!(eval_numeric_condition("< 5", 4.9));
        assert!(eval_numeric_condition("!= 0", 1.0));
        assert!(!eval_numeric_condition("!= 0", 0.0));
        assert!(eval_numeric_condition("== 3", 3.0));
    }
}
