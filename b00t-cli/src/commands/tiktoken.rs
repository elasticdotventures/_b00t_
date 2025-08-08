use anyhow::Result;
use tiktoken_rs::o200k_base;

pub fn handle_tiktoken(text: &str) -> Result<()> {
    let bpe = o200k_base().map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {e}"))?;
    let tokens = bpe.encode_with_special_tokens(text);
    println!("Token count: {}", tokens.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_count_basic() {
        let text = "This is a test sentence.";
        let bpe = o200k_base().unwrap();
        let tokens = bpe.encode_with_special_tokens(text);
        // The expected token count may vary by tokenizer version, but should be > 0
        assert!(tokens.len() > 0);
    }
}
