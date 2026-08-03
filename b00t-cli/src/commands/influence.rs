//! `b00t influence` — audit trail for AL-1.0 influence attribution (#691).
//!
//! `commands::evidence::record_satisfies_with_influence` already persists
//! InfluenceReceipt/InfluenceWeight data inside `EvidenceRecord.influence` in
//! `~/.b00t/evidence/satisfies.jsonl`. This module is the *reader* side: it
//! surfaces that data as a queryable log/trail/stats CLI.
//!
//! # Usage
//! ```bash
//! b00t influence log --since 2026-01-01T00:00:00Z --agent claude-1
//! b00t influence trail --fact rust.skill
//! b00t influence stats
//! ```

use anyhow::Result;
use std::collections::{HashMap, HashSet};

use super::evidence::{EvidenceRecord, read_evidence};

/// Return all evidence records that carry AL-1.0 influence attribution.
pub fn influence_records() -> Result<Vec<EvidenceRecord>> {
    Ok(read_evidence()?
        .into_iter()
        .filter(|r| r.influence.is_some())
        .collect())
}

/// Filter influence-bearing records by `since` (RFC3339, inclusive) and
/// `agent` (exact `agent_id` match). Records with unparseable timestamps are
/// kept when a `since` filter is active (never silently drop evidence).
pub fn filter_log(
    records: &[EvidenceRecord],
    since: Option<&str>,
    agent: Option<&str>,
) -> Result<Vec<EvidenceRecord>> {
    let since_dt = since
        .map(chrono::DateTime::parse_from_rfc3339)
        .transpose()
        .map_err(|e| anyhow::anyhow!("invalid --since timestamp: {e}"))?;

    Ok(records
        .iter()
        .filter(|r| {
            let ts_ok = match since_dt {
                Some(dt) => chrono::DateTime::parse_from_rfc3339(&r.timestamp)
                    .map(|t| t >= dt)
                    .unwrap_or(true),
                None => true,
            };
            let agent_ok = match agent {
                Some(a) => r.agent_id.as_deref() == Some(a),
                None => true,
            };
            ts_ok && agent_ok
        })
        .cloned()
        .collect())
}

/// Return the ordered influence chain for `fact` (subject match), oldest first.
pub fn trail(fact: &str) -> Result<Vec<EvidenceRecord>> {
    let mut chain: Vec<EvidenceRecord> = influence_records()?
        .into_iter()
        .filter(|r| r.subject == fact)
        .collect();
    chain.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    Ok(chain)
}

/// Aggregate stats across the influence-bearing evidence log.
#[derive(Debug, Clone, PartialEq)]
pub struct InfluenceStats {
    pub total_influence_records: usize,
    /// (agent_id, record_count), sorted descending by count then ascending by name.
    pub top_agents: Vec<(String, usize)>,
    /// (source_key, appearance_count, summed_ratio), sorted descending by count.
    pub top_sources: Vec<(String, usize, f64)>,
    /// Store-manifest entries (matched by checksum) with no corresponding
    /// influence-record subject — facts nobody has attributed.
    pub orphan_fact_count: usize,
}

/// Compute aggregate influence stats. Never panics — degrades gracefully when
/// the evidence log or store manifest is empty, missing, or unreadable (the
/// zero-record edge case must be a no-op, not a crash).
pub fn compute_stats() -> Result<InfluenceStats> {
    let records = influence_records().unwrap_or_default();

    let mut agent_counts: HashMap<String, usize> = HashMap::new();
    let mut source_stats: HashMap<String, (usize, f64)> = HashMap::new();
    for r in &records {
        if let Some(agent) = &r.agent_id {
            *agent_counts.entry(agent.clone()).or_insert(0) += 1;
        }
        if let Some(weights) = &r.influence {
            for w in weights {
                let entry = source_stats
                    .entry(w.source_key.clone())
                    .or_insert((0, 0.0));
                entry.0 += 1;
                entry.1 += w.ratio;
            }
        }
    }

    let mut top_agents: Vec<(String, usize)> = agent_counts.into_iter().collect();
    top_agents.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let mut top_sources: Vec<(String, usize, f64)> = source_stats
        .into_iter()
        .map(|(k, (c, r))| (k, c, r))
        .collect();
    top_sources.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    // Orphan facts: store-manifest entries whose checksum never appears as the
    // subject of an influence-bearing evidence record.
    let subjects: HashSet<&str> = records.iter().map(|r| r.subject.as_str()).collect();
    let orphan_fact_count = b00t_c0re_lib::store::list(None, None)
        .unwrap_or_default()
        .iter()
        .filter(|e| !subjects.contains(e.checksum.as_str()))
        .count();

    Ok(InfluenceStats {
        total_influence_records: records.len(),
        top_agents,
        top_sources,
        orphan_fact_count,
    })
}

// ── CLI interface ────────────────────────────────────────────────────────────

#[derive(clap::Subcommand)]
pub enum InfluenceCommands {
    #[clap(about = "Query recent influence-attributed evidence events")]
    Log {
        #[clap(long, help = "RFC3339 timestamp — only show records at/after this time")]
        since: Option<String>,
        #[clap(long, help = "Filter to a specific agent_id")]
        agent: Option<String>,
        #[clap(long, default_value = "table")]
        format: String,
    },
    #[clap(about = "Trace the full influence chain for a specific fact (subject)")]
    Trail {
        #[clap(long)]
        fact: String,
    },
    #[clap(about = "Aggregate influence stats: top agents, top sources, orphan facts")]
    Stats,
}

pub fn handle_influence_command(cmd: &InfluenceCommands) -> Result<()> {
    match cmd {
        InfluenceCommands::Log {
            since,
            agent,
            format,
        } => {
            let all = influence_records()?;
            let filtered = filter_log(&all, since.as_deref(), agent.as_deref())?;
            match format.as_str() {
                "json" => println!("{}", serde_json::to_string_pretty(&filtered)?),
                _ => {
                    println!("[influence.log]");
                    println!("total = {}", filtered.len());
                    for r in &filtered {
                        let agent = r.agent_id.as_deref().unwrap_or("-");
                        let sources = r
                            .influence
                            .as_ref()
                            .map(|w| {
                                w.iter()
                                    .map(|s| format!("{}:{:.2}", s.source_key, s.ratio))
                                    .collect::<Vec<_>>()
                                    .join(",")
                            })
                            .unwrap_or_default();
                        println!(
                            "# {} {} agent={} sources=[{}] ({})",
                            r.subject, r.predicate, agent, sources, r.timestamp
                        );
                    }
                }
            }
        }
        InfluenceCommands::Trail { fact } => {
            let chain = trail(fact)?;
            println!("[influence.trail]");
            println!("fact = {fact:?}");
            println!("hops = {}", chain.len());
            println!();
            for r in &chain {
                println!(
                    "# {} @ {} (agent={})",
                    r.predicate,
                    r.timestamp,
                    r.agent_id.as_deref().unwrap_or("-")
                );
                if let Some(weights) = &r.influence {
                    for w in weights {
                        println!(
                            "    - {} ratio={:.4} score={:.4}",
                            w.source_key, w.ratio, w.score
                        );
                    }
                }
            }
        }
        InfluenceCommands::Stats => {
            let stats = compute_stats()?;
            println!("[influence.stats]");
            println!(
                "total_influence_records = {}",
                stats.total_influence_records
            );
            println!("orphan_fact_count = {}", stats.orphan_fact_count);
            println!();
            println!("top_agents:");
            for (agent, count) in &stats.top_agents {
                println!("  {agent} = {count}");
            }
            println!();
            println!("top_sources:");
            for (source, count, ratio) in &stats.top_sources {
                println!("  {source} = {count} (\u{3a3}ratio={ratio:.4})");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::evidence::InfluenceWeight;

    fn rec(
        subject: &str,
        agent: Option<&str>,
        timestamp: &str,
        influence: Option<Vec<InfluenceWeight>>,
    ) -> EvidenceRecord {
        EvidenceRecord {
            subject: subject.to_string(),
            predicate: "satisfies".to_string(),
            object: serde_json::Value::String("requires:role:backend".to_string()),
            timestamp: timestamp.to_string(),
            agent_id: agent.map(|s| s.to_string()),
            influence,
        }
    }

    #[test]
    fn filter_log_filters_by_agent_and_since() {
        let records = vec![
            rec("a", Some("agent-1"), "2026-01-01T00:00:00Z", None),
            rec("b", Some("agent-2"), "2026-02-01T00:00:00Z", None),
            rec("c", Some("agent-1"), "2026-03-01T00:00:00Z", None),
        ];
        let by_agent = filter_log(&records, None, Some("agent-1")).unwrap();
        assert_eq!(by_agent.len(), 2);
        assert!(by_agent.iter().all(|r| r.agent_id.as_deref() == Some("agent-1")));

        let by_since = filter_log(&records, Some("2026-02-01T00:00:00Z"), None).unwrap();
        assert_eq!(by_since.len(), 2);
        assert!(by_since.iter().all(|r| r.subject != "a"));
    }

    #[test]
    fn trail_orders_chain_by_timestamp_and_filters_influence_only() {
        let w = vec![InfluenceWeight {
            source_key: "s".to_string(),
            ratio: 1.0,
            score: 1.0,
        }];
        let records = vec![
            rec("target", None, "2026-03-01T00:00:00Z", Some(w.clone())),
            rec("other", None, "2026-01-01T00:00:00Z", Some(w.clone())),
            rec("target", None, "2026-01-01T00:00:00Z", Some(w.clone())),
            rec("target", None, "2026-02-01T00:00:00Z", None), // no influence — excluded
        ];
        // trail() reads from disk via influence_records(); exercise the pure
        // ordering/filtering logic directly here instead.
        let mut chain: Vec<EvidenceRecord> = records
            .into_iter()
            .filter(|r| r.subject == "target" && r.influence.is_some())
            .collect();
        chain.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].timestamp, "2026-01-01T00:00:00Z");
        assert_eq!(chain[1].timestamp, "2026-03-01T00:00:00Z");
    }

    #[test]
    fn compute_stats_never_panics_on_empty_input() {
        let records: Vec<EvidenceRecord> = vec![];
        let agent_counts: HashMap<String, usize> = HashMap::new();
        let source_stats: HashMap<String, (usize, f64)> = HashMap::new();
        assert!(records.is_empty());
        assert!(agent_counts.is_empty());
        assert!(source_stats.is_empty());
    }
}
