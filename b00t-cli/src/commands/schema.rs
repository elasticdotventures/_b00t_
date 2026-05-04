//! `b00t schema` — schema datum management (diff, generate, import).
//!
//! # Usage
//! ```bash
//! b00t-cli schema diff focus focus-v2
//! b00t-cli schema import schema.json --name my_schema
//! ```

use crate::datum_schema::{AbDataHeader, AbDataSchema, DataType, FocusSchema};
use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

/// Diff two schema datums by name, loaded from `{path}/{name}.schema.tomllmd`.
pub fn handle_schema_diff(path: &str, schema_a: &str, schema_b: &str) -> Result<()> {
    let path_a = format!("{}/{}.schema.tomllmd", path, schema_a);
    let path_b = format!("{}/{}.schema.tomllmd", path, schema_b);

    let loaded_a = FocusSchema::load(&path_a)
        .map_err(|e| anyhow::anyhow!("failed to load '{}': {}", path_a, e))?;
    let loaded_b = FocusSchema::load(&path_b)
        .map_err(|e| anyhow::anyhow!("failed to load '{}': {}", path_b, e))?;

    let headers_a = loaded_a.headers();
    let headers_b = loaded_b.headers();

    // Build lookup maps
    let map_a: HashMap<&str, &AbDataHeader> =
        headers_a.iter().map(|h| (h.name.as_str(), h)).collect();
    let map_b: HashMap<&str, &AbDataHeader> =
        headers_b.iter().map(|h| (h.name.as_str(), h)).collect();

    let names_a: HashSet<&str> = headers_a.iter().map(|h| h.name.as_str()).collect();
    let names_b: HashSet<&str> = headers_b.iter().map(|h| h.name.as_str()).collect();

    // Added headers (in B not in A)
    let mut added: Vec<&str> = names_b.difference(&names_a).copied().collect();
    added.sort();
    // Removed headers (in A not in B)
    let mut removed: Vec<&str> = names_a.difference(&names_b).copied().collect();
    removed.sort();

    // Changed headers (same name, different type/nullable/ordinal)
    let mut changed: Vec<(&str, &AbDataHeader, &AbDataHeader)> = Vec::new();
    let mut common: Vec<&str> = names_a.intersection(&names_b).copied().collect();
    common.sort();
    for name in &common {
        let ha = map_a.get(name).unwrap();
        let hb = map_b.get(name).unwrap();
        if ha.data_type != hb.data_type || ha.nullable != hb.nullable || ha.ordinal != hb.ordinal
        {
            changed.push((name, ha, hb));
        }
    }

    println!("=== Schema Diff: {} vs {} ===", schema_a, schema_b);
    println!();

    if removed.is_empty() && added.is_empty() && changed.is_empty() {
        println!("No differences found.");
        return Ok(());
    }

    if !removed.is_empty() {
        println!("--- Removed headers (in A but not in B) ---");
        for name in &removed {
            println!("  - {}", name);
        }
        println!();
    }

    if !added.is_empty() {
        println!("--- Added headers (in B but not in A) ---");
        for name in &added {
            println!("  + {}", name);
        }
        println!();
    }

    if !changed.is_empty() {
        println!("--- Changed headers ---");
        for (name, ha, hb) in &changed {
            println!("  {}:", name);
            println!("    type:     {:?} -> {:?}", ha.data_type, hb.data_type);
            println!("    nullable: {} -> {}", ha.nullable, hb.nullable);
            println!("    ordinal:  {} -> {}", ha.ordinal, hb.ordinal);
        }
        println!();
    }

    Ok(())
}

/// Map a JSON type string to `DataType`.
fn json_type_to_data_type(type_str: &str) -> DataType {
    match type_str.to_lowercase().trim() {
        "string" | "str" | "text" | "varchar" => DataType::String,
        "integer" | "int" | "int64" | "long" => DataType::Int64,
        "number" | "float" | "float64" | "double" | "decimal" => DataType::Decimal,
        "boolean" | "bool" => DataType::Bool,
        "datetime" | "date" | "timestamp" => DataType::DateTime,
        "json" | "object" => DataType::Json,
        _ => DataType::String,
    }
}

/// Import schema from a JSON file containing `{ "columns": [...] }`.
/// Writes a `.schema.tomllmd` datum to the output directory.
pub fn handle_schema_import(path: &str, name: &str, output: Option<PathBuf>) -> Result<()> {
    let content =
        std::fs::read_to_string(path).map_err(|e| anyhow!("failed to read '{}': {}", path, e))?;
    let json: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| anyhow!("invalid JSON in '{}': {}", path, e))?;

    let columns = json["columns"]
        .as_array()
        .ok_or_else(|| anyhow!("expected top-level 'columns' array in '{}'", path))?;

    if columns.is_empty() {
        anyhow::bail!("'columns' array is empty in '{}'", path);
    }

    let mut headers: Vec<AbDataHeader> = Vec::new();

    for (i, col) in columns.iter().enumerate() {
        let col_name = col["name"]
            .as_str()
            .ok_or_else(|| anyhow!("column {i}: missing 'name' field"))?;
        let col_type = col["type"].as_str().unwrap_or("string");
        let nullable = col["nullable"].as_bool().unwrap_or(true);
        let description = col["description"].as_str().unwrap_or("");

        headers.push(AbDataHeader::new(col_name, json_type_to_data_type(col_type), nullable, description, i));
    }

    // Build the output path
    let out_dir = output.unwrap_or_else(|| PathBuf::from("_b00t_"));
    let out_path = out_dir.join(format!("{}.schema.tomllmd", name));

    // Generate the .tomllmd content
    let mut out = String::new();
    out.push_str(&format!(
        r#"# 🤖 AUTO-GENERATED from `b00t schema import {path}`
generated_by = "b00t schema import"

[b00t]
name = "{name}"
type = "schema"
version = "0.1"
hint = "Imported schema: {name}"
keywords = ["schema", "imported"]

[b00t.spec]
canonical = ""
license = ""

[b00t.schema.{name}]
spec_version = "0.1"
header_count = {}

[b00t.schema.{name}.headers]

"#,
        headers.len()
    ));

    for h in &headers {
        let ctype = if h.data_type.is_numeric() {
            "metric"
        } else {
            "dimension"
        };
        let feat = if !h.nullable { "mandatory" } else { "optional" };
        let type_str = match h.data_type {
            DataType::String => "string",
            DataType::Int64 => "int64",
            DataType::Float64 | DataType::Decimal => "metric",
            DataType::Bool => "boolean",
            DataType::DateTime => "datetime",
            DataType::Json => "json",
            _ => "string",
        };
        out.push_str(&format!(
            "# {}: {} ({} | {} | nullable={})\n",
            h.name, h.description, ctype, feat, h.nullable
        ));
        out.push_str(&format!(
            "{} = {{ type = \"{}\", feature = \"{}\", nullable = {}, ordinal = {} }}\n",
            h.name, type_str, ctype, h.nullable, h.ordinal
        ));
    }

    out.push_str("\n# b00t:map v1\n");
    out.push_str(&format!(
        "# summary: Imported schema '{}' — {} headers\n",
        name,
        headers.len()
    ));
    out.push_str("# tags: schema, imported\n");

    // Ensure output directory exists
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(&out_path, &out)?;
    println!("✅ Schema datum written to {}", out_path.display());
    println!("   Name: {}", name);
    println!("   Headers: {}", headers.len());

    Ok(())
}
