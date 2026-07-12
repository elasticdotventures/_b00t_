//! Enhanced assimilation pipeline — content-type-aware crawling + concept extraction.
//!
//! Orchestrates: ContentRouter → ConceptExtractor → CrawlEngine → IndexDispatcher
//! Activated by `b00t grok assimilate --enhanced`.

pub mod content_router;
pub mod concept_extractor;
pub mod crawl_engine;
pub mod domain_router;
pub mod pdf_extractor;
pub mod index_dispatcher;

use anyhow::Result;
use crawl_engine::{CrawlConfig, CrawledDoc};

/// Configuration for enhanced assimilation.
pub struct EnhancedConfig {
    pub max_depth: u32,
    pub max_pages: u32,
    pub delay_ms: u64,
    pub index_targets: Vec<String>,
}

impl Default for EnhancedConfig {
    fn default() -> Self {
        Self {
            max_depth: 2,
            max_pages: 20,
            delay_ms: 500,
            index_targets: vec![],
        }
    }
}

/// Run the full enhanced assimilation pipeline on a URL or file.
pub async fn run_enhanced(
    source: &str,
    topic: &str,
    config: &EnhancedConfig,
) -> Result<Vec<CrawledDoc>> {
    let router = content_router::ContentRouter::new();
    let extractor = concept_extractor::ConceptExtractor::new();

    // Parse initial source
    eprintln!("→ routing: {source}");
    let parsed = router.route(source).await?;

    // Extract concepts + links from initial page.
    // 🤓 Concept extraction is enrichment, not a gate: if no sm0l/ch0nky model
    //    is reachable, degrade to an empty extraction so the fetched content
    //    still lands in the datum (parse_response already degrades the same way).
    eprintln!("→ extracting concepts (sm0l)…");
    let extraction = match extractor.extract(&parsed.text).await {
        Ok(extraction) => extraction,
        Err(e) => {
            eprintln!("⚠️  concept extraction unavailable ({e}) — continuing without concepts");
            concept_extractor::ConceptExtraction {
                concepts: vec![],
                links: vec![],
            }
        }
    };

    let initial_doc = CrawledDoc {
        url: source.to_string(),
        content: parsed,
        extraction: extraction.clone(),
        depth: 0,
    };

    let mut all_docs = vec![initial_doc];

    // Crawl links if depth > 0
    if config.max_depth > 0 && !extraction.links.is_empty() {
        eprintln!("→ crawling {} link(s), depth≤{}, same-origin", extraction.links.len(), config.max_depth);
        let crawl_cfg = CrawlConfig {
            seed_url: source.to_string(),
            max_depth: config.max_depth,
            max_pages: config.max_pages,
            delay_ms: config.delay_ms,
        };
        let crawled = crawl_engine::crawl(&extraction.links, &crawl_cfg, &router, &extractor).await?;
        all_docs.extend(crawled);
    }

    eprintln!("→ {} document(s) assimilated", all_docs.len());

    // Index into targets
    if !config.index_targets.is_empty() {
        let dispatcher = index_dispatcher::IndexDispatcher::discover(&config.index_targets)?;
        let report = dispatcher.ingest(&all_docs, topic)?;
        eprintln!("→ indexed: {report}");
    }

    Ok(all_docs)
}
