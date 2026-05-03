use std::fs;
use serde_json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read the TOML file
    let content = fs::read_to_string("/home/brianh/.dotfiles/_b00t_/b00t-mcp.mcp.toml")?;
    println!("TOML content:");
    println!("{}", content);
    
    // Parse it using the same structure
    let config: toml::Value = toml::from_str(&content)?;
    println!("\nParsed TOML:");
    println!("{:#?}", config);
    
    Ok(())
}