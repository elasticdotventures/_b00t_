//! Pure, unit-testable pieces of b00t-pgduck split out of main.rs so this
//! crate has a `[lib]` target at all - main.rs alone (bin-only) meant
//! `cargo test --lib -p b00t-pgduck` (this repo's pre-push hook always
//! runs `--lib`, see .git/hooks/pre-push) failed outright with "no
//! library targets found", blocking every push touching this crate
//! regardless of what changed. Everything here is connection/IO-free by
//! design - the wire-protocol handler code in main.rs stays there and
//! calls into this crate.

use pgwire::api::Type;

/// DuckDB's `column_type()` returns arrow's DataType, not a decl-type
/// string like SQLite - map the common scalars pgwire's wire protocol
/// has a real Type for; unmapped kinds (List/Struct/Array/Map/Union/
/// Dictionary/etc) fall back to UNKNOWN rather than erroring, same "not
/// all types" honesty as the upstream sqlite.rs example this was
/// adapted from.
pub fn arrow_type_to_pg_type(dt: &duckdb::arrow::datatypes::DataType) -> Type {
    use duckdb::arrow::datatypes::DataType;
    match dt {
        DataType::Boolean => Type::BOOL,
        DataType::Int8 | DataType::UInt8 => Type::CHAR,
        DataType::Int16 | DataType::UInt16 => Type::INT2,
        DataType::Int32 | DataType::UInt32 => Type::INT4,
        DataType::Int64 | DataType::UInt64 => Type::INT8,
        DataType::Float32 => Type::FLOAT4,
        DataType::Float64 => Type::FLOAT8,
        DataType::Utf8 | DataType::LargeUtf8 => Type::TEXT,
        DataType::Binary | DataType::LargeBinary => Type::BYTEA,
        DataType::Date32 | DataType::Date64 => Type::DATE,
        DataType::Timestamp(_, _) => Type::TIMESTAMP,
        _ => Type::UNKNOWN,
    }
}

/// DuckDB's `DESCRIBE <query>` meta-statement returns column_type as
/// free-form SQL type text (e.g. "INTEGER", "VARCHAR", "DECIMAL(10,2)"),
/// not the typed arrow DataType arrow_type_to_pg_type maps above - match
/// on the leading type name, ignoring any parenthesized precision/scale.
/// Same "not all types" honesty as arrow_type_to_pg_type: unmapped kinds
/// fall back to UNKNOWN.
pub fn duckdb_type_str_to_pg_type(type_str: &str) -> Type {
    let base = type_str.split('(').next().unwrap_or(type_str).trim().to_uppercase();
    match base.as_str() {
        "BOOLEAN" | "BOOL" => Type::BOOL,
        "TINYINT" | "UTINYINT" => Type::CHAR,
        "SMALLINT" | "USMALLINT" | "INT2" => Type::INT2,
        "INTEGER" | "UINTEGER" | "INT" | "INT4" => Type::INT4,
        "BIGINT" | "UBIGINT" | "INT8" | "HUGEINT" | "UHUGEINT" => Type::INT8,
        "FLOAT" | "REAL" | "FLOAT4" => Type::FLOAT4,
        "DOUBLE" | "FLOAT8" => Type::FLOAT8,
        "VARCHAR" | "TEXT" | "STRING" | "CHAR" | "BPCHAR" => Type::TEXT,
        "BLOB" | "BYTEA" | "VARBINARY" => Type::BYTEA,
        "DATE" => Type::DATE,
        "TIMESTAMP" | "TIMESTAMP WITH TIME ZONE" | "TIMESTAMPTZ" | "DATETIME" => Type::TIMESTAMP,
        _ => Type::UNKNOWN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duckdb_type_str_maps_known_scalars() {
        assert_eq!(duckdb_type_str_to_pg_type("INTEGER"), Type::INT4);
        assert_eq!(duckdb_type_str_to_pg_type("BIGINT"), Type::INT8);
        assert_eq!(duckdb_type_str_to_pg_type("VARCHAR"), Type::TEXT);
        assert_eq!(duckdb_type_str_to_pg_type("BOOLEAN"), Type::BOOL);
        assert_eq!(duckdb_type_str_to_pg_type("DOUBLE"), Type::FLOAT8);
    }

    #[test]
    fn duckdb_type_str_strips_precision_scale() {
        // DESCRIBE returns "DECIMAL(10,2)" for a scaled decimal column -
        // the leading type name still has to match despite the suffix.
        assert_eq!(duckdb_type_str_to_pg_type("VARCHAR(255)"), Type::TEXT);
    }

    #[test]
    fn duckdb_type_str_falls_back_to_unknown() {
        assert_eq!(duckdb_type_str_to_pg_type("DECIMAL(10,2)"), Type::UNKNOWN);
        assert_eq!(duckdb_type_str_to_pg_type("STRUCT(a INTEGER)"), Type::UNKNOWN);
    }

    #[test]
    fn duckdb_type_str_is_case_insensitive() {
        assert_eq!(duckdb_type_str_to_pg_type("integer"), Type::INT4);
        assert_eq!(duckdb_type_str_to_pg_type("Varchar"), Type::TEXT);
    }

    #[test]
    fn arrow_type_maps_known_scalars() {
        use duckdb::arrow::datatypes::DataType;
        assert_eq!(arrow_type_to_pg_type(&DataType::Boolean), Type::BOOL);
        assert_eq!(arrow_type_to_pg_type(&DataType::Int64), Type::INT8);
        assert_eq!(arrow_type_to_pg_type(&DataType::Utf8), Type::TEXT);
    }

    #[test]
    fn arrow_type_falls_back_to_unknown() {
        use duckdb::arrow::datatypes::DataType;
        assert_eq!(arrow_type_to_pg_type(&DataType::Null), Type::UNKNOWN);
    }
}
