//! `b00t soul` — agentic soul management
//!
//! The soul is the persistent identity of a b00t agent instance.
//! Lives at ~/._b00t_/SOUL.tomllm — survives sessions, encodes accumulated
//! knowledge, role state, and tribal memory.
//!
//! Inspired by moltis per-agent memory workspaces + session persistence.

use anyhow::Result;
use clap::Parser;

use crate::memory_provider::{FileMemory, MemoryProvider, soul_path};

#[derive(Parser)]
pub enum SoulCommands {
    #[clap(about = "Show current soul state (~/._b00t_/SOUL.tomllm)")]
    Status {
        #[clap(long, help = "Output as JSON")]
        json: bool,
    },

    #[clap(about = "Read a key from soul memory")]
    Get {
        #[clap(help = "Key to read")]
        key: String,
    },

    #[clap(about = "Write a key to soul memory")]
    Set {
        #[clap(help = "Key")]
        key: String,
        #[clap(help = "Value")]
        value: String,
    },

    #[clap(about = "Show soul file path")]
    Path,

    #[clap(about = "Reset soul memory (clears all keys — irreversible)")]
    Reset {
        #[clap(long, help = "Confirm reset without prompt")]
        confirm: bool,
    },
}

pub fn handle_soul_command(cmd: &SoulCommands) -> Result<()> {
    let path = soul_path();
    let mem = FileMemory::new(path.clone());

    match cmd {
        SoulCommands::Status { json } => {
            if !path.exists() {
                if *json {
                    println!("{{\"soul\": null, \"path\": \"{}\"}}", path.display());
                } else {
                    println!("Soul: uninitialized");
                    println!("  Path: {}", path.display());
                    println!("  Tip:  b00t soul set <key> <value> to initialize");
                }
                return Ok(());
            }

            let raw = std::fs::read_to_string(&path)?;

            if *json {
                // Strip comments, parse, emit JSON
                let stripped: String = raw
                    .lines()
                    .filter(|l| !l.trim_start().starts_with('#'))
                    .collect::<Vec<_>>()
                    .join("\n");
                #[derive(serde::Deserialize, serde::Serialize)]
                struct SoulStore {
                    #[serde(default)]
                    data: std::collections::HashMap<String, String>,
                }
                let store: SoulStore = toml::from_str(&stripped).unwrap_or(SoulStore {
                    data: Default::default(),
                });
                println!("{}", serde_json::to_string_pretty(&store.data)?);
            } else {
                println!("Soul: {}", path.display());
                println!();
                // Print non-comment lines
                for line in raw.lines() {
                    if !line.trim_start().starts_with('#') || line.starts_with("# b00t:map") {
                        println!("  {}", line);
                    }
                }
            }
            Ok(())
        }

        SoulCommands::Get { key } => {
            match mem.read(key)? {
                Some(val) => println!("{}", val),
                None => {
                    eprintln!("soul: key '{}' not found", key);
                    std::process::exit(1);
                }
            }
            Ok(())
        }

        SoulCommands::Set { key, value } => {
            mem.write(key, value)?;
            println!("soul: {} = {}", key, value);
            Ok(())
        }

        SoulCommands::Path => {
            println!("{}", path.display());
            Ok(())
        }

        SoulCommands::Reset { confirm } => {
            if !confirm {
                eprintln!("soul reset clears all memory. Use --confirm to proceed.");
                eprintln!("  b00t soul reset --confirm");
                std::process::exit(1);
            }
            if path.exists() {
                std::fs::remove_file(&path)?;
            }
            println!("soul: reset — {}", path.display());
            Ok(())
        }
    }
}
