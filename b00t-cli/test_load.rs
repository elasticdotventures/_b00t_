use b00t_cli::*;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = "/home/brianh/.dotfiles/_b00t_/aws-knowledge-test.mcp.toml";
    match std::fs::read_to_string(path) {
        Ok(content) => {
            println\!("File content:");
            println\!("{}", content);
            println\!("\nParsing...");
            match toml::from_str::<UnifiedConfig>(&content) {
                Ok(config) => {
                    println\!("✅ Successfully parsed\!");
                    println\!("Name: {}", config.b00t.name);
                    println\!("Type: {:?}", config.b00t.datum_type);
                    println\!("MCP CLI methods: {:?}", config.b00t.mcp_cli);
                    println\!("MCP HTTP method: {:?}", config.b00t.mcp_http);
                },
                Err(e) => {
                    println\!("❌ Parse error: {}", e);
                    return Err(Box::new(e));
                }
            }
        },
        Err(e) => {
            println\!("❌ File read error: {}", e);
            return Err(Box::new(e));
        }
    }
    Ok(())
}
EOF < /dev/null
