//! BFS crawl engine — same-origin link following with depth limit.
//!
//! Polite crawling: rate-limited, dedup'd, bounded by max_pages.

use crate::assimilate::concept_extractor::{ConceptExtractor, ConceptExtraction, ExtractedLink};
use crate::assimilate::content_router::{ContentRouter, ParsedContent};
use anyhow::Result;
use std::collections::HashSet;
use std::thread;
use std::time::Duration;
use url::Url;

/// Crawl configuration.
pub struct CrawlConfig {
    pub seed_url: String,
    pub max_depth: u32,
    pub max_pages: u32,
    pub delay_ms: u64,
}

/// A crawled document with its extraction.
pub struct CrawledDoc {
    pub url: String,
    pub content: ParsedContent,
    pub extraction: ConceptExtraction,
    pub depth: u32,
}

/// Crawl links using BFS, same-origin only, depth-limited.
pub fn crawl(
    seed_links: &[ExtractedLink],
    config: &CrawlConfig,
    router: &ContentRouter,
    extractor: &ConceptExtractor,
) -> Result<Vec<CrawledDoc>> {
    let seed_origin = Url::parse(&config.seed_url)
        .ok()
        .and_then(|u| u.host_str().map(|h| h.to_string()));

    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(config.seed_url.clone());

    let mut results: Vec<CrawledDoc> = Vec::new();
    let mut queue: Vec<(String, u32)> = seed_links
        .iter()
        .filter_map(|l| {
            let normalized = normalize_url(&l.url, &config.seed_url)?;
            if is_same_origin(&normalized, seed_origin.as_deref()) {
                Some((normalized, 1))
            } else {
                None
            }
        })
        .filter(|(url, _)| !visited.contains(url))
        .collect();

    let pages_remaining = config.max_pages.saturating_sub(1) as usize; // -1 for seed

    while let Some((url, depth)) = queue.pop() {
        if results.len() >= pages_remaining || depth > config.max_depth {
            break;
        }
        if visited.contains(&url) {
            continue;
        }
        visited.insert(url.clone());

        // Polite delay
        if config.delay_ms > 0 {
            thread::sleep(Duration::from_millis(config.delay_ms));
        }

        eprintln!("  [{depth}/{max_depth}] {url}", max_depth = config.max_depth);

        match router.route(&url) {
            Ok(parsed) => {
                match extractor.extract(&parsed.text) {
                    Ok(extraction) => {
                        // Queue child links
                        for link in &extraction.links {
                            if let Some(abs) = normalize_url(&link.url, &url) {
                                if is_same_origin(&abs, seed_origin.as_deref())
                                    && !visited.contains(&abs)
                                    && depth < config.max_depth
                                {
                                    queue.push((abs, depth + 1));
                                }
                            }
                        }
                        results.push(CrawledDoc {
                            url: url.clone(),
                            content: parsed,
                            extraction,
                            depth,
                        });
                    }
                    Err(e) => {
                        eprintln!("  ⚠️  extraction failed for {url}: {e}");
                    }
                }
            }
            Err(e) => {
                eprintln!("  ⚠️  fetch failed for {url}: {e}");
            }
        }
    }

    Ok(results)
}

/// Normalize a URL: resolve relative URLs against base, strip fragments.
pub fn normalize_url(url: &str, base: &str) -> Option<String> {
    // Already absolute
    if let Ok(parsed) = Url::parse(url) {
        let mut cleaned = parsed.clone();
        cleaned.set_fragment(None);
        return Some(cleaned.to_string());
    }
    // Resolve relative URL against base
    let base_url = Url::parse(base).ok()?;
    let resolved = base_url.join(url).ok()?;
    let mut cleaned = resolved.clone();
    cleaned.set_fragment(None);
    Some(cleaned.to_string())
}

/// Check if a URL is same-origin as the seed host.
pub fn is_same_origin(url: &str, seed_host: Option<&str>) -> bool {
    match (Url::parse(url).ok(), seed_host) {
        (Some(parsed), Some(host)) => parsed.host_str() == Some(host),
        (Some(_), None) => true,  // No seed host — allow all
        (None, _) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_absolute_url() {
        let result = normalize_url("https://example.com/page#section", "https://other.com").unwrap();
        assert_eq!(result, "https://example.com/page");
    }

    #[test]
    fn test_normalize_relative_url() {
        let result = normalize_url("/docs/intro", "https://example.com/guide").unwrap();
        assert_eq!(result, "https://example.com/docs/intro");
    }

    #[test]
    fn test_normalize_relative_with_dot() {
        let result = normalize_url("../api", "https://example.com/v1/page").unwrap();
        assert_eq!(result, "https://example.com/api");
    }

    #[test]
    fn test_is_same_origin_match() {
        assert!(is_same_origin(
            "https://example.com/page",
            Some("example.com")
        ));
    }

    #[test]
    fn test_is_same_origin_mismatch() {
        assert!(!is_same_origin(
            "https://other.com/page",
            Some("example.com")
        ));
    }

    #[test]
    fn test_is_same_origin_no_seed() {
        assert!(is_same_origin("https://anything.com/page", None));
    }
}
