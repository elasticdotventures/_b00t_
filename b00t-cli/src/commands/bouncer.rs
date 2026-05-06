// b00t-bouncer CLI integration
// Bouncer pattern gatekeeper commands

use anyhow::{Result, anyhow};
use clap::{Args, Subcommand};
use serde_json;
use std::path::PathBuf;

/// Bouncer pattern gatekeeper commands
#[derive(Args, Debug)]
pub struct BouncerArgs {
    #[clap(subcommand)]
    pub command: BouncerCommands,
}

#[derive(Debug, Clone, Subcommand)]
pub enum BouncerCommands {
    /// Validate input through bouncer gates
    Validate {
        /// Input string to validate
        #[clap(short, long)]
        input: String,
        /// Output string to validate (for output gates)
        #[clap(short, long)]
        output: Option<String>,
    },
    /// View audit log
    Audit {
        /// Path to audit log (default: .b00t/bouncer-audit.jsonl)
        #[clap(short, long)]
        log_path: Option<String>,
    },
    /// Show bouncer configuration
    Config,
}

/// Handle bouncer commands
pub fn handle_bouncer(args: &BouncerArgs) -> Result<()> {
    match &args.command {
        BouncerCommands::Validate { input, output } => {
            handle_validate(input, output.as_deref())
        }
        BouncerCommands::Audit { log_path } => {
            handle_audit(log_path.as_deref())
        }
        BouncerCommands::Config => {
            handle_config()
        }
    }
}

/// Handle validate command
fn handle_validate(input: &str, output: Option<&str>) -> Result<()> {
    // Import bouncer crate
    use b00t_bouncer::Bouncer;
    
    let bouncer = Bouncer::new();
    
    // Validate input
    let input_result = bouncer.validate_input(input);
    println!("Input validation: {:?}", input_result);
    
    // Validate output if provided
    if let Some(output_str) = output {
        let output_result = bouncer.validate_output(output_str);
        println!("Output validation: {:?}", output_result);
    }
    
    Ok(())
}

/// Handle audit command
fn handle_audit(log_path: Option<&str>) -> Result<()> {
    let path = log_path.unwrap_or(".b00t/bouncer-audit.jsonl");
    let path_buf = PathBuf::from(path);
    
    if !path_buf.exists() {
        println!("No audit log found at: {}", path);
        return Ok(());
    }
    
    let content = std::fs::read_to_string(&path_buf)?;
    
    // Parse and display JSONL entries
    for line in content.lines() {
        if line.is_empty() {
            continue;
        }
        
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(entry) => {
                println!("{}: {} ({})", 
                    entry["timestamp"], 
                    entry["gate"], 
                    entry["decision"]
                );
            }
            Err(e) => {
                eprintln!("Failed to parse audit entry: {}", e);
            }
        }
    }
    
    Ok(())
}

/// Handle config command
fn handle_config() -> Result<()> {
    // Import bouncer crate
    use b00t_bouncer::{Bouncer, BouncerConfig};
    
    let bouncer = Bouncer::new();
    let config = bouncer.config.clone();
    
    println!("Bouncer Configuration:");
    println!("  Enabled: {}", config.enabled);
    println!("  Audit Log: {}", config.audit_log);
    
    println!("\nInput Gates:");
    println!("  Sanitize: {}", config.input_gates.sanitize.enabled);
    println!("  Credential Check: {}", config.input_gates.credential_check.enabled);
    println!("  Permission Check: {}", config.input_gates.permission_check.enabled);
    println!("  Rate Limit: {}", config.input_gates.rate_limit.enabled);
    
    println!("\nOutput Gates:");
    println!("  Contract Validation: {}", config.output_gates.contract_validation.enabled);
    println!("  Security Scan: {}", config.output_gates.security_scan.enabled);
    println!("  Quality Check: {}", config.output_gates.quality_check.enabled);
    
    Ok(())
}
