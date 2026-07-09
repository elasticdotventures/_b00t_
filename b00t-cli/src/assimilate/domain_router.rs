// Domain-specific MCP routing — intercepts known domains and uses MCP servers
// instead of raw webfetch. Institutional knowledge: arxiv.org → arxiv-mcp-server.
//
// Pattern: each domain gets a regex to extract the resource ID, then an MCP
// tool chain to fetch + parse the content. New domains are added here.

use anyhow::{Result, bail};
use regex::Regex;

/// A domain handler: regex to match URLs, MCP tool chain to process them.
#[derive(Debug, Clone)]
pub struct McpDomainHandler {
    /// Regex to match and extract resource ID (capture group 1 = paper ID)
    pub url_pattern: Regex,
    /// MCP server name (must be registered in b00t config)
    pub mcp_server: String,
    /// Tool to download the resource
    pub download_tool: String,
    /// Tool to read the resource
    pub read_tool: String,
    /// Human-readable domain label
    pub domain_label: String,
}

/// Registry of domain → MCP handler mappings.
/// New institutional domains are added here. Each entry maps a URL pattern
/// (extracting the resource ID) to an MCP server + tool chain.
pub fn institutional_mcp_handlers() -> Vec<McpDomainHandler> {
    vec![
        // arxiv.org — academic paper repository
        // Matches: https://arxiv.org/abs/2401.12345
        //          https://arxiv.org/abs/2401.12345v2
        //          https://arxiv.org/pdf/2401.12345
        //          https://arxiv.org/pdf/2401.12345.pdf
        McpDomainHandler {
            url_pattern: Regex::new(
                r"arxiv\.org/(?:abs|pdf)/(\d{4}\.\d{4,5})(?:v\d+)?(?:\.pdf)?$"
            ).expect("valid arxiv URL regex"),
            mcp_server: "arxiv-mcp-server".into(),
            download_tool: "download_paper".into(),
            read_tool: "read_paper".into(),
            domain_label: "arXiv".into(),
        },
        // huggingface.co — models, datasets, spaces, papers
        // Matches: https://huggingface.co/<user>/<repo>
        //          https://huggingface.co/datasets/<user>/<repo>
        //          https://huggingface.co/spaces/<user>/<repo>
        //          https://huggingface.co/papers/<id>
        McpDomainHandler {
            url_pattern: Regex::new(
                r"huggingface\.co/(?:(?:datasets|spaces)/)?([^/]+/[^/?#]+)"
            ).expect("valid huggingface URL regex"),
            mcp_server: "huggingface".into(),
            download_tool: "hf_doc_fetch".into(),
            read_tool: "hf_doc_fetch".into(),
            domain_label: "HuggingFace".into(),
        },
    ]
}

/// Extract the resource ID from a URL matching a known domain pattern.
pub fn extract_mcp_resource_id(url: &str) -> Option<(&McpDomainHandler, String)> {
    for handler in institutional_mcp_handlers() {
        if let Some(caps) = handler.url_pattern.captures(url) {
            let resource_id = caps.get(1).map(|m| m.as_str().to_string())?;
            return Some((&handler, resource_id));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arxiv_url_parsing() {
        let re = Regex::new(r"arxiv\.org/(?:abs|pdf)/(\d{4}\.\d{4,5})(?:v\d+)?(?:\.pdf)?$").unwrap();

        let cases = vec![
            ("https://arxiv.org/abs/2401.12345", Some("2401.12345")),
            ("https://arxiv.org/abs/2401.12345v2", Some("2401.12345")),
            ("https://arxiv.org/pdf/2401.12345", Some("2401.12345")),
            ("https://arxiv.org/pdf/2401.12345.pdf", Some("2401.12345")),
            ("http://arxiv.org/abs/1706.03762", Some("1706.03762")),
            ("https://arxiv.org/abs/2401.123456", Some("2401.123456")),
            ("https://example.com/paper", None),
        ];

        for (url, expected) in cases {
            let result = re.captures(url).and_then(|c| c.get(1).map(|m| m.as_str()));
            assert_eq!(result, expected, "URL: {url}");
        }
    }

    #[test]
    fn test_extract_mcp_resource_id() {
        let (handler, id) = extract_mcp_resource_id("https://arxiv.org/abs/2401.12345").unwrap();
        assert_eq!(handler.mcp_server, "arxiv-mcp-server");
        assert_eq!(handler.download_tool, "download_paper");
        assert_eq!(id, "2401.12345");

        assert!(extract_mcp_resource_id("https://example.com/paper").is_none());
    }

    #[test]
    fn test_huggingface_url_parsing() {
        let re =
            Regex::new(r"huggingface\.co/(?:(?:datasets|spaces)/)?([^/]+/[^/?#]+)").unwrap();

        let cases = vec![
            ("https://huggingface.co/meta-llama/Llama-3.1-8B", Some("meta-llama/Llama-3.1-8B")),
            ("https://huggingface.co/datasets/squad/squad", Some("squad/squad")),
            ("https://huggingface.co/spaces/facebook/metaseq", Some("facebook/metaseq")),
            ("https://huggingface.co/papers/2401.12345", Some("papers/2401.12345")),
            ("https://huggingface.co/google/gemma-2-2b-it?tab=readme", Some("google/gemma-2-2b-it")),
            ("https://example.com/paper", None),
        ];

        for (url, expected) in cases {
            let result = re.captures(url).and_then(|c| c.get(1).map(|m| m.as_str()));
            assert_eq!(result, expected, "URL: {url}");
        }
    }

    #[test]
    fn test_huggingface_mcp_routing() {
        let (handler, id) =
            extract_mcp_resource_id("https://huggingface.co/meta-llama/Llama-3.1-8B").unwrap();
        assert_eq!(handler.mcp_server, "huggingface");
        assert_eq!(handler.download_tool, "hf_doc_fetch");
        assert_eq!(id, "meta-llama/Llama-3.1-8B");

        let (handler, id) =
            extract_mcp_resource_id("https://huggingface.co/datasets/squad/squad").unwrap();
        assert_eq!(handler.mcp_server, "huggingface");
        assert_eq!(id, "squad/squad");

        let (handler, id) =
            extract_mcp_resource_id("https://huggingface.co/spaces/facebook/metaseq").unwrap();
        assert_eq!(handler.mcp_server, "huggingface");
        assert_eq!(handler.domain_label, "HuggingFace");
        assert_eq!(id, "facebook/metaseq");
    }
}
