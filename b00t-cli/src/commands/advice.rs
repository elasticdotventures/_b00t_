use anyhow::Result;
use b00t_c0re_lib::{LfmfSystem, LfmfConfig};

/// Handle b00t advice command - syntax therapist functionality
/// Uses shared LFMF system from b00t-c0re-lib
pub async fn handle_advice(path: &str, tool: &str, query: &str, count: Option<usize>) -> Result<()> {
    // Load configuration
    let config = LfmfSystem::load_config(path)?;
    let mut lfmf_system = LfmfSystem::new(config);

    // Try to initialize vector database (non-fatal if fails)
    if let Err(e) = lfmf_system.initialize().await {
        println!("🔄 Vector database unavailable ({}), using filesystem fallback", e);
    }

    // Handle different query types  
    let results = match query.to_lowercase().as_str() {
        "list" => lfmf_system.list_lessons(tool, count).await?,
        query if query.starts_with("search ") => {
            let search_query = query.strip_prefix("search ").unwrap_or("");
            if search_query.trim().is_empty() {
                return Err(anyhow::anyhow!("Search query cannot be empty"));
            }
            lfmf_system.get_advice(tool, search_query, count).await?
        },
        _ => lfmf_system.get_advice(tool, query, count).await?,
    };

    // Display results
    if results.is_empty() {
        println!("No advice found for '{}' in tool '{}'", query, tool);
        println!("💡 Record lessons with: b00t lfmf {} \"<topic>: <lesson>\"", tool);
    } else {
        println!("Found {} advice entries for '{}' in tool '{}':", results.len(), query, tool);
        for (i, result) in results.iter().enumerate() {
            println!("{}. {}", i + 1, result);
        }
    }

    Ok(())
}