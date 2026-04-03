#[cfg(test)]
mod knowledge_base_tests {
    use super::super::*;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[test]
    fn test_blessing_metadata_creation() {
        let metadata = BlessingMetadata {
            blessing_id: "blessing:observe-infrastructure".to_string(),
            source_datum: "terraform".to_string(),
            discovered_at: chrono::Utc::now(),
            quality_score: 0.85,
            depends_on: vec!["blessing:auth".to_string()],
            layer_path: PathBuf::from("layers/observe-infrastructure.adapter"),
        };

        assert_eq!(metadata.blessing_id, "blessing:observe-infrastructure");
        assert_eq!(metadata.source_datum, "terraform");
        assert_eq!(metadata.quality_score, 0.85);
        assert_eq!(metadata.depends_on.len(), 1);
    }

    #[test]
    fn test_quality_score_validation() {
        // Valid score: 0.85
        let metadata = BlessingMetadata {
            blessing_id: "test".to_string(),
            source_datum: "datum".to_string(),
            discovered_at: chrono::Utc::now(),
            quality_score: 0.85,
            depends_on: vec![],
            layer_path: PathBuf::from("layers/test.adapter"),
        };

        assert!(metadata.quality_score >= 0.0 && metadata.quality_score <= 1.0);

        // Test edge cases
        let min_score = BlessingMetadata {
            blessing_id: "test".to_string(),
            source_datum: "datum".to_string(),
            discovered_at: chrono::Utc::now(),
            quality_score: 0.0,
            depends_on: vec![],
            layer_path: PathBuf::from("layers/test.adapter"),
        };
        assert_eq!(min_score.quality_score, 0.0);

        let max_score = BlessingMetadata {
            blessing_id: "test".to_string(),
            source_datum: "datum".to_string(),
            discovered_at: chrono::Utc::now(),
            quality_score: 1.0,
            depends_on: vec![],
            layer_path: PathBuf::from("layers/test.adapter"),
        };
        assert_eq!(max_score.quality_score, 1.0);
    }

    #[test]
    fn test_layer_metadata_creation() {
        let layer_metadata = LayerMetadata {
            blessing_id: "blessing:terraform-apply".to_string(),
            artifact_path: PathBuf::from("/cache/blessing_terraform_apply.gguf"),
            embedding_dim: 768,
            adapter_rank: 8,
            generated_at: chrono::Utc::now(),
            quality_score: 0.92,
        };

        assert_eq!(layer_metadata.blessing_id, "blessing:terraform-apply");
        assert_eq!(layer_metadata.embedding_dim, 768);
        assert_eq!(layer_metadata.adapter_rank, 8);
        assert!(layer_metadata.artifact_path.to_str().unwrap().contains(".gguf"));
    }

    #[test]
    fn test_adapter_path_generation() {
        let knowledge_base = KnowledgeBase {
            metadata: std::collections::HashMap::new(),
            layer_cache: std::collections::HashMap::new(),
            index_dir: PathBuf::from("/tmp/knowledge_index"),
            discovery_callback: None,
        };

        let adapter_path = knowledge_base.adapter_path_for("blessing:observe-infrastructure");

        assert!(adapter_path.to_str().unwrap().contains("blessing_observe_infrastructure"));
        assert!(adapter_path.to_str().unwrap().contains(".adapter"));
    }

    #[tokio::test]
    async fn test_knowledge_base_new() {
        let index_dir = tempfile::tempdir().unwrap();
        let index_path = index_dir.path().to_str().unwrap();

        let kb = KnowledgeBase::new(index_path).await;

        assert_eq!(kb.metadata.len(), 0);
        assert_eq!(kb.layer_cache.len(), 0);
        assert_eq!(kb.index_dir, PathBuf::from(index_path));
        assert!(kb.discovery_callback.is_none());
    }

    #[tokio::test]
    async fn test_discover_capability() {
        let index_dir = tempfile::tempdir().unwrap();
        let index_path = index_dir.path().to_str().unwrap();

        let mut kb = KnowledgeBase::new(index_path).await;

        let blessing = BlessingMetadata {
            blessing_id: "blessing:observe-infrastructure".to_string(),
            source_datum: "terraform".to_string(),
            discovered_at: chrono::Utc::now(),
            quality_score: 0.85,
            depends_on: vec!["blessing:auth".to_string()],
            layer_path: PathBuf::from("layers/observe-infrastructure.adapter"),
        };

        let context = serde_json::json!({
            "agent_role": "executor",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        kb.discover_capability(blessing.clone(), &context).await;

        assert_eq!(kb.metadata.len(), 1);
        assert!(kb.metadata.contains_key("blessing:observe-infrastructure"));

        let stored = kb.metadata.get("blessing:observe-infrastructure").unwrap();
        assert_eq!(stored.blessing_id, blessing.blessing_id);
        assert_eq!(stored.source_datum, blessing.source_datum);
        assert_eq!(stored.quality_score, blessing.quality_score);
    }

    #[tokio::test]
    async fn test_discover_multiple_capabilities() {
        let index_dir = tempfile::tempdir().unwrap();
        let index_path = index_dir.path().to_str().unwrap();

        let mut kb = KnowledgeBase::new(index_path).await;
        let context = serde_json::json!({});

        // Discover first blessing
        let blessing1 = BlessingMetadata {
            blessing_id: "blessing:observe".to_string(),
            source_datum: "datum1".to_string(),
            discovered_at: chrono::Utc::now(),
            quality_score: 0.80,
            depends_on: vec![],
            layer_path: PathBuf::from("layers/observe.adapter"),
        };

        // Discover second blessing
        let blessing2 = BlessingMetadata {
            blessing_id: "blessing:apply".to_string(),
            source_datum: "datum2".to_string(),
            discovered_at: chrono::Utc::now(),
            quality_score: 0.90,
            depends_on: vec!["blessing:observe".to_string()],
            layer_path: PathBuf::from("layers/apply.adapter"),
        };

        kb.discover_capability(blessing1, &context).await;
        kb.discover_capability(blessing2, &context).await;

        assert_eq!(kb.metadata.len(), 2);
        assert!(kb.metadata.contains_key("blessing:observe"));
        assert!(kb.metadata.contains_key("blessing:apply"));

        let stored_apply = kb.metadata.get("blessing:apply").unwrap();
        assert_eq!(stored_apply.depends_on.len(), 1);
        assert_eq!(stored_apply.depends_on[0], "blessing:observe");
    }

    #[tokio::test]
    async fn test_generate_layer() {
        let index_dir = tempfile::tempdir().unwrap();
        let index_path = index_dir.path().to_str().unwrap();

        let mut kb = KnowledgeBase::new(index_path).await;

        let layer = kb.generate_layer("blessing:terraform-apply").await;

        assert_eq!(layer.blessing_id, "blessing:terraform-apply");
        assert_eq!(layer.embedding_dim, 768);
        assert_eq!(layer.adapter_rank, 8);
        assert!(layer.quality_score >= 0.0 && layer.quality_score <= 1.0);

        // Verify it's cached
        assert!(kb.layer_cache.contains_key("blessing:terraform-apply"));
    }

    #[tokio::test]
    async fn test_semantic_discovery_callback_stub() {
        let index_dir = tempfile::tempdir().unwrap();
        let index_path = index_dir.path().to_str().unwrap();

        // 🦨 Phase 8: Callback registration stub
        // This test demonstrates the Phase 8 semantic discovery callback interface
        // Full implementation in assimilate module

        let mut kb = KnowledgeBase::new(index_path).await;

        // Stub: callback should be Option<Arc<dyn SemanticDiscoveryCallback>>
        assert!(kb.discovery_callback.is_none());

        // Phase 8: This would be populated by assimilate module
        // let callback = Arc::new(MockSemanticDiscovery::new());
        // kb.discovery_callback = Some(callback);

        // When callback is set, discover_capability would invoke:
        // if let Some(cb) = &self.discovery_callback {
        //     cb.on_capability_discovered(&blessing.blessing_id, &blessing).await;
        // }

        let blessing = BlessingMetadata {
            blessing_id: "blessing:test".to_string(),
            source_datum: "test".to_string(),
            discovered_at: chrono::Utc::now(),
            quality_score: 0.75,
            depends_on: vec![],
            layer_path: PathBuf::from("layers/test.adapter"),
        };

        let context = serde_json::json!({});
        kb.discover_capability(blessing.clone(), &context).await;

        // Verify capability was discovered even without callback
        assert!(kb.metadata.contains_key("blessing:test"));
    }

    #[test]
    fn test_knowledge_base_metadata_indexing() {
        // Test HashMap behavior with multiple metadata entries
        let mut metadata_map = std::collections::HashMap::new();

        let meta1 = BlessingMetadata {
            blessing_id: "blessing:auth".to_string(),
            source_datum: "security".to_string(),
            discovered_at: chrono::Utc::now(),
            quality_score: 0.95,
            depends_on: vec![],
            layer_path: PathBuf::from("layers/auth.adapter"),
        };

        let meta2 = BlessingMetadata {
            blessing_id: "blessing:compute".to_string(),
            source_datum: "infrastructure".to_string(),
            discovered_at: chrono::Utc::now(),
            quality_score: 0.88,
            depends_on: vec!["blessing:auth".to_string()],
            layer_path: PathBuf::from("layers/compute.adapter"),
        };

        metadata_map.insert(meta1.blessing_id.clone(), meta1);
        metadata_map.insert(meta2.blessing_id.clone(), meta2);

        assert_eq!(metadata_map.len(), 2);
        assert_eq!(metadata_map.get("blessing:auth").unwrap().quality_score, 0.95);
        assert_eq!(metadata_map.get("blessing:compute").unwrap().quality_score, 0.88);
    }
}
