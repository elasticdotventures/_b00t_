//! Unified knowledge management system
//!
//! Aggregates knowledge from multiple sources with intelligent priority:
//! 1. LFMF lessons (tribal knowledge)
//! 2. Learn content (curated docs)
//! 3. Man pages (system docs)
//! 4. RAG results (vector DB)

use crate::learn::get_learn_lesson;
use crate::{ChunkResult, GrokClient, LfmfSystem, ManPage};
use anyhow::Result;
use std::env;

#[derive(Debug, Clone)]
pub struct KnowledgeSource {
    pub topic: String,
    pub lfmf_lessons: Vec<String>,
    pub learn_content: Option<String>,
    pub man_page: Option<ManPage>,
    pub rag_results: Vec<ChunkResult>,
}

#[derive(Debug, Clone)]
pub struct DisplayOpts {
    pub force_man: bool,
    pub toc_only: bool,
    pub section: Option<usize>,
    pub concise: bool,
}

impl Default for DisplayOpts {
    fn default() -> Self {
        Self {
            force_man: false,
            toc_only: false,
            section: None,
            concise: false,
        }
    }
}

impl KnowledgeSource {
    /// Gather all knowledge for a topic
    pub async fn gather(topic: &str, b00t_path: &str) -> Result<Self> {
        let mut knowledge = KnowledgeSource {
            topic: topic.to_string(),
            lfmf_lessons: Vec::new(),
            learn_content: None,
            man_page: None,
            rag_results: Vec::new(),
        };

        // 1. Try to load LFMF lessons
        if let Ok(config) = LfmfSystem::load_config(b00t_path) {
            let mut lfmf_system = LfmfSystem::new(config);
            let skip_grok = env::var("B00T_LEARN_SKIP_GROK")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);

            if !skip_grok {
                // Initialize vector DB (non-fatal if fails)
                let _ = lfmf_system.initialize().await;
            }
            if let Ok(lessons) = lfmf_system.list_lessons(topic, Some(10)).await {
                knowledge.lfmf_lessons = lessons;
            }
        }

        // 2. Try to load learn content
        if let Ok(content) = get_learn_lesson(b00t_path, topic) {
            knowledge.learn_content = Some(content);
        }

        // 3. Try to parse man page
        if let Ok(man) = ManPage::from_command(topic) {
            knowledge.man_page = Some(man);
        }

        // 4. Try RAG search (if grok is available)
        let client = GrokClient::new();
        if let Ok(results) = client.ask(topic, Some(topic), Some(10)).await {
            knowledge.rag_results = results.results;
        }

        Ok(knowledge)
    }

    /// Display unified knowledge with priority
    pub fn display(&self, opts: &DisplayOpts) -> Result<()> {
        // Force man page display if requested
        if opts.force_man {
            return self.display_man_page(opts);
        }

        // Show TOC only if requested
        if opts.toc_only {
            return self.display_toc();
        }

        // Display header
        self.display_header();

        // Display in priority order
        if !self.lfmf_lessons.is_empty() {
            self.display_lfmf_lessons()?;
        }

        if let Some(ref content) = self.learn_content {
            self.display_learn_content(content, opts.concise)?;
        }

        if let Some(ref man) = self.man_page {
            if self.lfmf_lessons.is_empty() && self.learn_content.is_none() {
                // Only show man page if no other sources
                self.display_man_inline(man, opts)?;
            } else {
                // Just mention it's available
                println!(
                    "\n📄 **Man Page Available**: `b00t learn {} --man`\n",
                    self.topic
                );
            }
        }

        if !self.rag_results.is_empty() && self.learn_content.is_none() {
            self.display_rag_results()?;
        }

        // Show helpful hints
        self.display_hints();

        Ok(())
    }

    fn display_header(&self) {
        println!("╔═══════════════════════════════════════╗");
        println!("║  Learning: {:<28}║", self.topic);
        println!("╠═══════════════════════════════════════╣");

        if !self.lfmf_lessons.is_empty() {
            println!(
                "║ 📚 LFMF Lessons: {:<21}║",
                format!("{} lessons", self.lfmf_lessons.len())
            );
        }
        if self.learn_content.is_some() {
            println!("║ 📖 Learn Content: {}.md{:<14}║", self.topic, "");
        }
        if self.man_page.is_some() {
            let version = self
                .man_page
                .as_ref()
                .and_then(|m| m.version.as_deref())
                .unwrap_or("unknown");
            println!(
                "║ 📄 Man Page: {:<26}║",
                format!("{} v{}", self.topic, version)
            );
        }
        if !self.rag_results.is_empty() {
            println!(
                "║ 🧠 RAG Results: {:<21}║",
                format!("{} chunks", self.rag_results.len())
            );
        }

        println!("╚═══════════════════════════════════════╝\n");
    }

    fn display_lfmf_lessons(&self) -> Result<()> {
        println!("## 📚 LFMF Lessons (Priority: 🔥)\n");

        for (idx, lesson) in self.lfmf_lessons.iter().enumerate() {
            println!("{}. {}", idx + 1, lesson);
        }
        println!();

        Ok(())
    }

    fn display_learn_content(&self, content: &str, concise: bool) -> Result<()> {
        println!("## 📖 Learn Content (_b00t_/learn/{}.md)\n", self.topic);

        if concise {
            // Show first 20 lines
            let lines: Vec<&str> = content.lines().take(20).collect();
            println!("{}", lines.join("\n"));
            if content.lines().count() > 20 {
                println!("\n... ({} more lines)", content.lines().count() - 20);
            }
        } else {
            println!("{}", content);
        }
        println!();

        Ok(())
    }

    fn display_man_page(&self, opts: &DisplayOpts) -> Result<()> {
        if let Some(ref man) = self.man_page {
            if let Some(section_num) = opts.section {
                // Display specific section
                if let Some(section) = man.get_section(section_num) {
                    println!("## {}\n", section.title);
                    println!("{}\n", section.content);
                } else {
                    anyhow::bail!("Section {} not found", section_num);
                }
            } else {
                // Display full man page
                let markdown = man.to_markdown(opts.concise);
                println!("{}", markdown);
            }
        } else {
            anyhow::bail!("No man page available for '{}'", self.topic);
        }

        Ok(())
    }

    fn display_man_inline(&self, man: &ManPage, opts: &DisplayOpts) -> Result<()> {
        println!(
            "## 📄 Man Page ({})\n",
            man.version.as_deref().unwrap_or("unknown")
        );

        if opts.concise {
            let markdown = man.to_markdown(true);
            println!("{}", markdown);
        } else {
            // Show first few sections
            for section in man.sections.iter().take(3) {
                println!("### {}\n", section.title);
                let lines: Vec<&str> = section.content.lines().take(10).collect();
                println!("{}", lines.join("\n"));
                if section.content.lines().count() > 10 {
                    println!("...\n");
                }
            }
            println!("\nFull manual: `b00t learn {} --man`\n", self.topic);
        }

        Ok(())
    }

    fn display_rag_results(&self) -> Result<()> {
        println!("## 🧠 RAG Knowledge Base\n");

        for (idx, chunk) in self.rag_results.iter().take(5).enumerate() {
            println!(
                "{}. {}",
                idx + 1,
                chunk.content.lines().next().unwrap_or("")
            );
        }
        println!();

        Ok(())
    }

    fn display_toc(&self) -> Result<()> {
        println!("Table of Contents: {}\n", self.topic);

        let mut section_num = 1;

        if !self.lfmf_lessons.is_empty() {
            println!(
                "[{}] LFMF Lessons ({} items)",
                section_num,
                self.lfmf_lessons.len()
            );
            section_num += 1;
        }

        if self.learn_content.is_some() {
            println!(
                "[{}] Learn Content (_b00t_/learn/{}.md)",
                section_num, self.topic
            );
            section_num += 1;
        }

        if let Some(ref man) = self.man_page {
            println!("\nMan Page Sections:");
            for (idx, section) in man.sections.iter().enumerate() {
                println!("[{}] {}", section_num + idx, section.title);
            }
        }

        println!("\nUse: b00t learn {} --section <num>", self.topic);

        Ok(())
    }

    fn display_hints(&self) {
        println!("\n**Quick Actions:**");

        if !self.lfmf_lessons.is_empty() {
            println!(
                "• Record lesson: `b00t learn {} --record \"<topic>: <body>\"`",
                self.topic
            );
            println!(
                "• Search lessons: `b00t learn {} --search \"<query>\"`",
                self.topic
            );
        }

        if self.man_page.is_some() {
            println!("• View man page: `b00t learn {} --man`", self.topic);
            println!("• Show TOC: `b00t learn {} --toc`", self.topic);
        }

        println!(
            "• Query RAG: `b00t learn {} --ask \"<question>\"`",
            self.topic
        );
    }

    /// Check if any knowledge exists
    pub fn has_knowledge(&self) -> bool {
        !self.lfmf_lessons.is_empty()
            || self.learn_content.is_some()
            || self.man_page.is_some()
            || !self.rag_results.is_empty()
    }
}
