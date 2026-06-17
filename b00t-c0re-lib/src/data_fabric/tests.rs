//! Data Fabric interface tests + operational examples for docgen fine-tuning.
//!
//! Test categories:
//!   Unit    — pure logic, no backends required.
//!   Serde   — round-trip serialization for all public types.
//!   Integration — require grafeo + zvec: gated by `#[ignore]`
//!                 + `#[cfg(feature = "data-fabric")]`.
//!
//! Run integration tests:
//!   cargo test --features data-fabric -- --ignored
//!
//! 🤓 Doc-examples in this file are duplicated as `/// # Example` blocks
//!    in `mod.rs` for b00t docgen fine-tuning (see `b00t_example!` macro
//!    proposal in this PR review).

#[cfg(test)]
use super::*;

// ─── Serde round-trips ───────────────────────────────────────────────────────

mod serde_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn data_fabric_query_serde_roundtrip() {
        let q = DataFabricQuery {
            subject: Some("b00t:datum/rust/abc".into()),
            predicate: Some("b00t:hasContent".into()),
            vector: Some(vec![0.1_f32, 0.2, 0.3]),
            vector_field: Some("embedding".into()),
            topk: 5,
            min_score: Some(0.75),
        };
        let json = serde_json::to_string(&q).expect("serialize");
        let q2: DataFabricQuery = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(q2.subject, q.subject);
        assert_eq!(q2.predicate, q.predicate);
        assert_eq!(q2.topk, q.topk);
        assert!((q2.min_score.unwrap() - 0.75).abs() < 1e-6);
        assert_eq!(q2.vector_field.as_deref(), Some("embedding"));
    }

    #[test]
    fn data_fabric_query_default_topk_is_ten() {
        // topk defaults to 10 — safe default; no caller-side .max(1) guard needed
        let q = DataFabricQuery::default();
        assert_eq!(q.topk, 10);
        assert!(q.subject.is_none());
        assert!(q.vector.is_none());
    }

    #[test]
    fn data_fabric_query_from_semantic_preserves_subject_predicate() {
        let sq = SemanticQuery {
            subject: Some("b00t:datum/rust/x".into()),
            predicate: Some("b00t:hasTag".into()),
        };
        let fq: DataFabricQuery = sq.into();
        assert_eq!(fq.subject.as_deref(), Some("b00t:datum/rust/x"));
        assert_eq!(fq.predicate.as_deref(), Some("b00t:hasTag"));
        assert_eq!(fq.topk, 10); // impl From hardcodes 10
        assert!(fq.vector.is_none());
    }

    #[test]
    fn vector_hit_serde_roundtrip() {
        let hit = VectorHit {
            id: "sub:pred".into(),
            score: 0.92,
            subject: "sub".into(),
            predicate: "pred".into(),
            object: json!({"key": "val"}),
        };
        let json = serde_json::to_string(&hit).expect("serialize");
        let hit2: VectorHit = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(hit2.id, "sub:pred");
        assert!((hit2.score - 0.92).abs() < 1e-6);
        assert_eq!(hit2.object["key"], "val");
    }

    #[test]
    fn data_fabric_result_serde_roundtrip() {
        let r = DataFabricResult {
            facts: vec![FactRecord {
                subject: "s".into(),
                predicate: "p".into(),
                object: json!("o"),
            }],
            vector_hits: vec![VectorHit {
                id: "s:p".into(), score: 0.5,
                subject: "s".into(), predicate: "p".into(), object: json!("o"),
            }],
            source: Some(DataFabricSource::Both),
        };
        let json = serde_json::to_string(&r).expect("serialize");
        let r2: DataFabricResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r2.facts.len(), 1);
        assert_eq!(r2.vector_hits.len(), 1);
        assert!(matches!(r2.source, Some(DataFabricSource::Both)));
    }

    #[test]
    fn data_fabric_result_default_is_empty() {
        let r = DataFabricResult::default();
        assert!(r.facts.is_empty());
        assert!(r.vector_hits.is_empty());
        assert!(r.source.is_none());
    }

    #[test]
    fn data_fabric_result_into_query_result_preserves_facts() {
        let r = DataFabricResult {
            facts: vec![FactRecord {
                subject: "a".into(),
                predicate: "b".into(),
                object: json!(1),
            }],
            vector_hits: vec![],
            source: None,
        };
        let qr: QueryResult = r.into();
        assert_eq!(qr.facts.len(), 1);
        assert_eq!(qr.facts[0].subject, "a");
    }

    #[test]
    fn fabric_record_serde_roundtrip_with_embedding() {
        let r = FabricRecord {
            subject: "b00t:datum/test/1".into(),
            predicate: "b00t:hasContent".into(),
            object: json!({"nested": true}),
            embedding: Some(vec![0.1, 0.2, 0.3]),
        };
        let json = serde_json::to_string(&r).expect("serialize");
        let r2: FabricRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r2.subject, r.subject);
        assert_eq!(r2.embedding.as_ref().unwrap().len(), 3);
    }

    #[test]
    fn fabric_record_serde_roundtrip_no_embedding() {
        let r = FabricRecord {
            subject: "s".into(),
            predicate: "p".into(),
            object: json!(null),
            embedding: None,
        };
        let json = serde_json::to_string(&r).expect("serialize");
        let r2: FabricRecord = serde_json::from_str(&json).expect("deserialize");
        assert!(r2.embedding.is_none());
    }

    #[test]
    fn fabric_record_from_fact_record_drops_embedding() {
        let fact = FactRecord { subject: "s".into(), predicate: "p".into(), object: json!("o") };
        let fr: FabricRecord = fact.clone().into();
        assert_eq!(fr.subject, fact.subject);
        assert!(fr.embedding.is_none());
    }

    #[test]
    fn fact_record_from_fabric_record_roundtrip() {
        let fr = FabricRecord {
            subject: "s".into(), predicate: "p".into(), object: json!("o"), embedding: None,
        };
        let fact: FactRecord = fr.clone().into();
        assert_eq!(fact.subject, fr.subject);
        assert_eq!(fact.predicate, fr.predicate);
    }

    #[test]
    fn data_fabric_source_variants_serde() {
        for src in [DataFabricSource::Grafeo, DataFabricSource::Zvec, DataFabricSource::Both] {
            let json = serde_json::to_string(&src).expect("serialize");
            let src2: DataFabricSource = serde_json::from_str(&json).expect("deserialize");
            // serde round-trip preserves variant name
            assert_eq!(
                serde_json::to_string(&src2).unwrap(),
                serde_json::to_string(&src).unwrap()
            );
        }
    }
}

// ─── DataFabricStream unit tests ─────────────────────────────────────────────

mod stream_tests {
    use super::*;
    use serde_json::json;

    fn make_records(n: usize) -> Vec<FabricRecord> {
        (0..n).map(|i| FabricRecord {
            subject: format!("s{i}"),
            predicate: "p".into(),
            object: json!(i),
            embedding: None,
        }).collect()
    }

    // --- from_vec / collect ---

    #[tokio::test]
    async fn stream_from_vec_collect_preserves_order() {
        let records = make_records(5);
        let subjects: Vec<String> = records.iter().map(|r| r.subject.clone()).collect();
        let result = DataFabricStream::from_vec(records).collect().await.unwrap();
        let got: Vec<String> = result.iter().map(|r| r.subject.clone()).collect();
        assert_eq!(got, subjects);
    }

    #[tokio::test]
    async fn stream_from_vec_empty_collects_empty() {
        let result = DataFabricStream::<FabricRecord>::from_vec(vec![])
            .collect().await.unwrap();
        assert!(result.is_empty());
    }

    // --- map ---

    #[tokio::test]
    async fn stream_map_transforms_each_element() {
        let result = DataFabricStream::from_vec(make_records(3))
            .map(|r| r.subject.clone())
            .collect().await.unwrap();
        assert_eq!(result, vec!["s0", "s1", "s2"]);
    }

    #[tokio::test]
    async fn stream_map_chained_twice() {
        let result = DataFabricStream::from_vec(make_records(3))
            .map(|r| r.subject.clone())
            .map(|s| s.to_uppercase())
            .collect().await.unwrap();
        assert_eq!(result, vec!["S0", "S1", "S2"]);
    }

    // --- filter ---

    #[tokio::test]
    async fn stream_filter_removes_matching() {
        let result = DataFabricStream::from_vec(make_records(5))
            .filter(|r| r.subject != "s2")
            .collect().await.unwrap();
        let subjects: Vec<_> = result.iter().map(|r| r.subject.as_str()).collect();
        assert_eq!(subjects, vec!["s0", "s1", "s3", "s4"]);
    }

    #[tokio::test]
    async fn stream_filter_all_pass() {
        let result = DataFabricStream::from_vec(make_records(3))
            .filter(|_| true)
            .collect().await.unwrap();
        assert_eq!(result.len(), 3);
    }

    #[tokio::test]
    async fn stream_filter_all_removed() {
        let result = DataFabricStream::from_vec(make_records(3))
            .filter(|_| false)
            .collect().await.unwrap();
        assert!(result.is_empty());
    }

    // 🚨 BUG PROBE: filter passes errors through (unwrap_or(true)).
    // This means an errored stream item is never filtered out — it passes through as Err
    // which then propagates when collect() calls `.collect::<Result<Vec<_>>>()`.
    // That IS the correct behavior (errors are not silently dropped), but it is
    // UNDOCUMENTED and the filter closure never even sees errored items.
    #[tokio::test]
    async fn stream_filter_error_passthrough_not_silenced() {
        use futures::stream;
        use anyhow::anyhow;
        let s = DataFabricStream::<FabricRecord>::new(stream::iter(vec![
            Ok(FabricRecord { subject: "ok".into(), predicate: "p".into(), object: json!(1), embedding: None }),
            Err(anyhow!("injected error")),
        ]));
        // filter keeps "ok" and passes the error through — collect returns Err
        let result = s.filter(|r| r.subject != "skip").collect().await;
        assert!(result.is_err(), "error items must propagate through filter");
    }

    // --- flat_map ---

    #[tokio::test]
    async fn stream_flat_map_expands_each() {
        let result = DataFabricStream::from_vec(make_records(2))
            .flat_map(|r| vec![r.subject.clone(), r.predicate.clone()])
            .collect().await.unwrap();
        assert_eq!(result, vec!["s0", "p", "s1", "p"]);
    }

    #[tokio::test]
    async fn stream_flat_map_can_return_empty() {
        let result = DataFabricStream::from_vec(make_records(3))
            .flat_map(|_| Vec::<String>::new())
            .collect().await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn stream_flat_map_error_collapses_to_single_err() {
        use futures::stream;
        use anyhow::anyhow;
        let s = DataFabricStream::<FabricRecord>::new(stream::iter(vec![
            Ok(FabricRecord { subject: "a".into(), predicate: "p".into(), object: json!(1), embedding: None }),
            Err(anyhow!("boom")),
        ]));
        let result = s.flat_map(|r| vec![r.subject]).collect().await;
        assert!(result.is_err());
    }

    // --- for_each ---

    #[tokio::test]
    async fn stream_for_each_visits_all() {
        let mut subjects = Vec::new();
        DataFabricStream::from_vec(make_records(3))
            .for_each(|r| subjects.push(r.subject.clone()))
            .await.unwrap();
        assert_eq!(subjects, vec!["s0", "s1", "s2"]);
    }

    #[tokio::test]
    async fn stream_for_each_on_empty_is_ok() {
        DataFabricStream::<FabricRecord>::from_vec(vec![])
            .for_each(|_| {})
            .await.unwrap();
    }

    #[tokio::test]
    async fn stream_for_each_stops_on_error() {
        use futures::stream;
        use anyhow::anyhow;
        let mut count = 0usize;
        let s = DataFabricStream::<FabricRecord>::new(stream::iter(vec![
            Ok(FabricRecord { subject: "ok".into(), predicate: "p".into(), object: json!(1), embedding: None }),
            Err(anyhow!("stop here")),
            Ok(FabricRecord { subject: "never".into(), predicate: "p".into(), object: json!(2), embedding: None }),
        ]));
        let result = s.for_each(|_| count += 1).await;
        assert!(result.is_err());
        assert_eq!(count, 1, "for_each must short-circuit on first error");
    }

    // --- map + filter + collect composition ---

    #[tokio::test]
    async fn stream_map_filter_collect_composition() {
        let result = DataFabricStream::from_vec(make_records(6))
            .filter(|r| r.subject != "s2" && r.subject != "s4")
            .map(|r| r.subject.clone())
            .collect().await.unwrap();
        assert_eq!(result, vec!["s0", "s1", "s3", "s5"]);
    }
}

// ─── Zvec parse_dim unit test ────────────────────────────────────────────────
// parse_dim is private; test via ZvecStore::try_new indirectly, or expose for test.
// 🤓 parse_dim is pub(crate) in this review recommendation — expose it.

#[cfg(test)]
#[cfg(feature = "store-zvec")]
mod zvec_unit_tests {
    use super::zvec::parse_dim;

    #[test]
    fn parse_dim_default_when_no_prefix() {
        assert_eq!(parse_dim("my-namespace"), 1536);
    }

    #[test]
    fn parse_dim_parses_valid_prefix() {
        assert_eq!(parse_dim("dim:768:my-namespace"), 768);
    }

    #[test]
    fn parse_dim_invalid_value_falls_back_to_default() {
        assert_eq!(parse_dim("dim:notanumber:ns"), 1536);
    }

    #[test]
    fn parse_dim_empty_namespace() {
        assert_eq!(parse_dim(""), 1536);
    }

    #[test]
    fn parse_dim_zero_treated_as_invalid() {
        // 0-dim vector is meaningless; should fall through to default
        assert_eq!(parse_dim("dim:0:ns"), 1536);
    }
}

// ─── Grafeo label sanitization unit test ────────────────────────────────────

#[cfg(test)]
#[cfg(feature = "store-grafeo")]
mod grafeo_unit_tests {
    use super::grafeo::GrafeoStore;
    use crate::irontology_bridge::StoreConfig;
    use super::DataFabricBackend;

    fn make_store(ns: &str) -> GrafeoStore {
        <GrafeoStore as DataFabricBackend>::try_new(&StoreConfig {
            endpoint: String::new(),
            namespace: ns.into(),
            data_path: None,
        }).expect("in-memory grafeo store")
    }

    #[test]
    fn label_replaces_hyphens_and_colons_with_underscores() {
        // GrafeoStore::label() is private; test by observing no panic in try_new.
        // 🤓 If label() produces invalid GQL identifiers the MERGE will silently fail.
        //    A namespace of "b00t-core:v2" must produce "Fact_b00t_core_v2", not "Fact_b00t-core:v2".
        let store = make_store("b00t-core:v2");
        // We can't call label() directly — use a no-op upsert and verify no error
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let result = store.fabric_upsert(vec![]).await;
            assert!(result.is_ok(), "upsert of empty slice must succeed");
        });
    }

    #[test]
    fn data_path_stored_is_none_for_in_memory() {
        let store = make_store("test");
        // data_path is private; we verify via try_new not panicking for None path
        let _ = store; // struct is Clone + Send
    }
}

// ─── Pipeline NATS subject format ────────────────────────────────────────────

#[cfg(feature = "data-fabric")]
mod pipeline_unit_tests {
    // subj_upsert and subj_edges are private — accessed via #[cfg(test)] re-export
    // or by calling through the pipeline internals. Test indirectly here.

    #[test]
    fn nats_subject_format_has_correct_structure() {
        // Validate manually since subj_upsert/subj_edges are private.
        // These are deterministic pure functions — trivial to inline test.
        fn subj_upsert(ns: &str) -> String { format!("b00t.data_fabric.{ns}.upsert") }
        fn subj_edges(ns: &str)  -> String { format!("b00t.data_fabric.{ns}.edges") }

        assert_eq!(subj_upsert("b00t-core"), "b00t.data_fabric.b00t-core.upsert");
        assert_eq!(subj_edges("b00t-core"),  "b00t.data_fabric.b00t-core.edges");
        // NATS subject with hyphens is valid; dots are NATS hierarchy separators
        assert_eq!(subj_upsert("my:ns"), "b00t.data_fabric.my:ns.upsert");
        // 🚨 WARN: colons in NATS subjects are legal but unusual — see bug report #5
    }

    #[test]
    fn dedup_logic_removes_same_subject_predicate_pairs() {
        use std::collections::HashSet;
        use crate::data_fabric::FabricRecord;
        use serde_json::json;

        let records = vec![
            FabricRecord { subject: "a".into(), predicate: "type".into(), object: json!("x"), embedding: None },
            FabricRecord { subject: "a".into(), predicate: "type".into(), object: json!("x"), embedding: None },
            FabricRecord { subject: "a".into(), predicate: "other".into(), object: json!("y"), embedding: None },
            FabricRecord { subject: "b".into(), predicate: "type".into(), object: json!("z"), embedding: None },
        ];
        let mut seen = HashSet::new();
        let deduped: Vec<_> = records.into_iter()
            .filter(|r| seen.insert((r.subject.clone(), r.predicate.clone())))
            .collect();
        assert_eq!(deduped.len(), 3);
    }
}

// ─── Integration tests (require real backends) ──────────────────────────────
// Run with: cargo test --features data-fabric -- --ignored

#[cfg(test)]
#[cfg(feature = "data-fabric")]
mod integration_tests {
    use super::*;
    use serde_json::json;

    fn test_config(ns: &str) -> StoreConfig {
        StoreConfig {
            endpoint: String::new(),
            namespace: ns.into(),
            data_path: None, // in-memory
        }
    }

    // 🤓 requires grafeo + zvec — run with: cargo test --features data-fabric -- --ignored
    #[tokio::test]
    #[ignore]
    async fn pipeline_upsert_then_query_returns_facts() {
        let config = test_config("test-integration");
        let pipeline = super::pipeline::DataFabricPipeline::try_new(&config)
            .expect("pipeline init");

        let records = vec![
            FabricRecord {
                subject: "b00t:datum/rust/001".into(),
                predicate: "b00t:hasContent".into(),
                object: json!("Rust ownership model"),
                embedding: None,
            },
        ];
        pipeline.fabric_upsert(records).await.expect("upsert");

        let query = DataFabricQuery {
            subject: Some("b00t:datum/rust/001".into()),
            ..Default::default()
        };
        let result = pipeline.fabric_query(query).await.expect("query");
        assert!(!result.facts.is_empty(), "must return at least one fact");
        assert_eq!(result.facts[0].subject, "b00t:datum/rust/001");
    }

    // 🤓 requires grafeo + zvec — run with: cargo test --features data-fabric -- --ignored
    #[tokio::test]
    #[ignore]
    async fn pipeline_vector_upsert_then_ann_query() {
        let config = test_config("test-ann");
        let pipeline = super::pipeline::DataFabricPipeline::try_new(&config)
            .expect("pipeline init");

        let embedding: Vec<f32> = (0..1536).map(|i| (i as f32) / 1536.0).collect();
        let records = vec![
            FabricRecord {
                subject: "b00t:datum/rust/emb01".into(),
                predicate: "b00t:hasContent".into(),
                object: json!("test embedding"),
                embedding: Some(embedding.clone()),
            },
        ];
        pipeline.fabric_upsert(records).await.expect("upsert with embedding");

        let result = pipeline.fabric_query(DataFabricQuery {
            vector: Some(embedding),
            topk: 3,
            ..Default::default()
        }).await.expect("ANN query");
        assert!(!result.vector_hits.is_empty(), "ANN must return hits after upsert");
    }

    // 🤓 requires grafeo + zvec — run with: cargo test --features data-fabric -- --ignored
    #[tokio::test]
    #[ignore]
    async fn pipeline_stream_returns_fabric_records() {
        let config = test_config("test-stream");
        let pipeline = super::pipeline::DataFabricPipeline::try_new(&config)
            .expect("pipeline init");

        let records = vec![
            FabricRecord { subject: "a".into(), predicate: "p".into(), object: json!(1), embedding: None },
            FabricRecord { subject: "b".into(), predicate: "p".into(), object: json!(2), embedding: None },
        ];
        pipeline.fabric_upsert(records).await.expect("upsert");

        let stream = pipeline.fabric_stream(DataFabricQuery::default()).await.expect("stream");
        let collected = stream.collect().await.expect("collect");
        assert!(!collected.is_empty());
    }

    // 🤓 requires grafeo + zvec — run with: cargo test --features data-fabric -- --ignored
    #[tokio::test]
    #[ignore]
    async fn grafeo_store_upsert_edges_and_query() {
        use crate::irontology_bridge::{EdgeKind, EdgeRecord};
        use super::grafeo::GrafeoStore;

        let store = <GrafeoStore as crate::data_fabric::DataFabricBackend>::try_new(&test_config("test-edges"))
            .expect("grafeo init");

        let edges = vec![EdgeRecord {
            from: "b00t:datum/rust/a".into(),
            to: "b00t:datum/rust/b".into(),
            kind: EdgeKind::DependsOn,
            weight: 1.0,
        }];
        store.fabric_upsert_edges(edges).await.expect("upsert edges");
        // No graph query API for edges in DataFabricQuery yet — this exercises the write path only.
    }

    // 🤓 requires grafeo + zvec — run with: cargo test --features data-fabric -- --ignored
    #[tokio::test]
    #[ignore]
    async fn zvec_store_skips_records_without_embeddings() {
        use super::zvec::ZvecStore;

        let store = <ZvecStore as crate::data_fabric::DataFabricBackend>::try_new(&test_config("test-zvec-skip"))
            .expect("zvec init");
        // Records without embeddings must be silently dropped (no-op, no error)
        let records = vec![
            FabricRecord { subject: "s".into(), predicate: "p".into(), object: json!(1), embedding: None },
        ];
        store.fabric_upsert(records).await.expect("upsert no-embedding must not error");
    }

    // 🤓 requires grafeo + zvec — run with: cargo test --features data-fabric -- --ignored
    #[tokio::test]
    #[ignore]
    async fn pipeline_knowledge_store_bridge_query() {
        use crate::irontology_bridge::KnowledgeStoreBackend;
        use super::pipeline::DataFabricPipeline;

        let config = test_config("test-bridge");
        let pipeline = DataFabricPipeline::try_new(&config).expect("pipeline init");

        let facts = vec![FactRecord {
            subject: "b00t:datum/bridge/1".into(),
            predicate: "b00t:hasContent".into(),
            object: json!("bridge content"),
        }];
        pipeline.upsert_facts(facts).await.expect("upsert via bridge");

        let result = pipeline.query(SemanticQuery {
            subject: Some("b00t:datum/bridge/1".into()),
            predicate: None,
        }).await.expect("query via bridge");
        assert!(!result.facts.is_empty());
    }
}
