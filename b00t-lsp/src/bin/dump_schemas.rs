//! dump-schemas — emit the b00t datum JSON Schema for taplo (tier-1 editor support).
//!
//! Usage: dump-schemas [out-path]   (default: _b00t_/schemas/b00t-datum.schema.json)

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("_b00t_/schemas/b00t-datum.schema.json"));
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let schema = b00t_lsp::schema::datum_schema();
    std::fs::write(
        &out,
        format!("{}\n", serde_json::to_string_pretty(&schema)?),
    )?;
    println!("wrote {}", out.display());
    Ok(())
}
