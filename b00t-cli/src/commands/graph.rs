//! `b00t graph` — emit the datum + identity/authz graph as SPO triples (SP5-06).
//!
//! The bridge between the triple *compilers* (`datum_triples`, `identity_triples`,
//! this crate) and the `store-oxigraph` substrate in `b00t-c0re-lib`, which
//! only builds `--no-default-features --features store-oxigraph`. This
//! subcommand runs in the normal `b00t-cli` build and writes JSONL that the
//! `b00t-c0re-lib` `b00t-graph` example bin consumes.

use anyhow::Result;
use clap::Subcommand;
use std::io::Write;

#[derive(Subcommand, Debug)]
pub enum GraphCommands {
    /// Emit the composed datum + identity/authz graph as JSONL `[s,p,o]`.
    EmitTriples {
        /// Tenant overlay to include (default: base tree only).
        #[arg(long)]
        tenant: Option<String>,
        /// Output file (default: stdout).
        #[arg(long)]
        out: Option<String>,
    },
}

pub fn execute(cmd: &GraphCommands, b00t_path: &str) -> Result<()> {
    match cmd {
        GraphCommands::EmitTriples { tenant, out } => {
            let mut triples = crate::datum_triples::compile_datum_triples(b00t_path)?;
            triples.extend(crate::identity_triples::compile_identity_triples(
                b00t_path,
                tenant.as_deref(),
            )?);
            triples.sort();
            triples.dedup();

            let mut sink: Box<dyn Write> = match out {
                Some(p) => Box::new(std::fs::File::create(p)?),
                None => Box::new(std::io::stdout()),
            };
            for (s, p, o) in &triples {
                writeln!(sink, "{}", serde_json::to_string(&[s, p, o])?)?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_triples_writes_jsonl_arrays() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rust.cli.toml"),
            "[b00t]\nname = \"rust.cli\"\ntype = \"cli\"\ndepends_on = [\"cargo\"]\n",
        )
        .unwrap();
        let out = dir.path().join("g.jsonl");
        execute(
            &GraphCommands::EmitTriples {
                tenant: None,
                out: Some(out.to_string_lossy().into()),
            },
            dir.path().to_str().unwrap(),
        )
        .unwrap();
        let body = std::fs::read_to_string(&out).unwrap();
        assert!(!body.is_empty());
        assert!(body.lines().all(|l| {
            let v: Vec<String> = serde_json::from_str(l).unwrap();
            v.len() == 3
        }));
        assert!(body.contains("b00t:dependsOn"));
    }
}
