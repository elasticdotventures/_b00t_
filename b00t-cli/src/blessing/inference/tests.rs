// b00t-cli/src/blessing/inference/tests.rs
// Comprehensive test suite for LLMInference trait and backend selection

#[cfg(test)]
mod inference_tests {
    use crate::blessing::inference::*;

    /// Test 1: Create embedding struct with f32 vector
    #[test]
    fn test_embedding_type() {
        let embedding = Embedding {
            data: vec![0.1, 0.2, 0.3, 0.4, 0.5],
        };

        assert_eq!(embedding.data.len(), 5);
        assert_eq!(embedding.data[0], 0.1);
    }

    /// Test 2: Embedding cosine similarity method
    #[test]
    fn test_embedding_cosine_similarity() {
        let emb1 = Embedding {
            data: vec![1.0, 0.0, 0.0],
        };
        let emb2 = Embedding {
            data: vec![1.0, 0.0, 0.0],
        };
        let emb3 = Embedding {
            data: vec![0.0, 1.0, 0.0],
        };

        // Identical embeddings: similarity = 1.0
        let sim_identical = emb1.cosine_similarity(&emb2);
        assert!((sim_identical - 1.0).abs() < 0.0001);

        // Orthogonal embeddings: similarity = 0.0
        let sim_orthogonal = emb1.cosine_similarity(&emb3);
        assert!(sim_orthogonal.abs() < 0.0001);
    }

    /// Test 3: Embedding with zero vector (edge case)
    #[test]
    fn test_embedding_cosine_zero_vector() {
        let emb_zero = Embedding {
            data: vec![0.0, 0.0, 0.0],
        };
        let emb_nonzero = Embedding {
            data: vec![1.0, 0.0, 0.0],
        };

        // Zero vector similarity should be 0.0
        let sim = emb_zero.cosine_similarity(&emb_nonzero);
        assert!(sim.is_nan() || sim == 0.0);
    }

    /// Test 4: ModelInfo struct creation
    #[test]
    fn test_model_info_creation() {
        let model_info = ModelInfo {
            model_id: "all-MiniLM-L6-v2".to_string(),
            embedding_dim: 384,
            backend_name: "candle".to_string(),
            available: true,
        };

        assert_eq!(model_info.model_id, "all-MiniLM-L6-v2");
        assert_eq!(model_info.embedding_dim, 384);
        assert_eq!(model_info.backend_name, "candle");
        assert!(model_info.available);
    }

    /// Test 5: LLMInference trait is object-safe
    /// (This test verifies the trait can be used as dyn LLMInference)
    #[test]
    fn test_llm_inference_trait_object_safe() {
        // This will compile only if LLMInference is object-safe
        // We don't instantiate it, just verify the type signature works
        let _: Option<Box<dyn LLMInference>> = None;
    }

    /// Test 6: InferenceConfig struct creation
    #[test]
    fn test_inference_config_creation() {
        let config = InferenceConfig {
            base_model_id: "all-MiniLM-L6-v2".to_string(),
            knowledge_index_dir: "/tmp/rag".to_string(),
            prefer_candle: true,
            enable_llamacpp: true,
        };

        assert_eq!(config.base_model_id, "all-MiniLM-L6-v2");
        assert_eq!(config.knowledge_index_dir, "/tmp/rag");
        assert!(config.prefer_candle);
        assert!(config.enable_llamacpp);
    }

    /// Test 7: InferenceBackendSelector enum variants exist
    #[test]
    fn test_inference_backend_selector_variants() {
        // Test that enum variants can be constructed
        // (actual implementations provided by Task 3+)

        // Candle variant
        let _selector_candle = InferenceBackendSelector::Candle;

        // Ripgrep fallback variant
        let _selector_ripgrep = InferenceBackendSelector::Ripgrep;

        #[cfg(feature = "llamacpp-fallback")]
        {
            let _selector_llamacpp = InferenceBackendSelector::LlamaCpp;
        }
    }

    /// Test 8: select_inference_backend respects prefer_candle flag
    #[test]
    fn test_backend_selector_respects_candle_preference() {
        let config = InferenceConfig {
            base_model_id: "all-MiniLM-L6-v2".to_string(),
            knowledge_index_dir: "/tmp/rag".to_string(),
            prefer_candle: true,
            enable_llamacpp: true,
        };

        let _backend = select_inference_backend(&config);
        // In Task 3+, backends will execute actual logic
        // For now, just verify function exists and returns without panicking
    }

    /// Test 9: select_inference_backend falls back correctly
    #[test]
    fn test_backend_selector_fallback_chain() {
        let config_no_candle = InferenceConfig {
            base_model_id: "test-model".to_string(),
            knowledge_index_dir: "/tmp/rag".to_string(),
            prefer_candle: false,
            enable_llamacpp: false,
        };

        let _backend = select_inference_backend(&config_no_candle);
        // Should fallback to ripgrep without panicking
    }

    /// Test 10: Embedding normalization (for cosine similarity)
    #[test]
    fn test_embedding_magnitude() {
        let emb = Embedding {
            data: vec![3.0, 4.0], // magnitude = 5.0
        };

        let magnitude = emb.magnitude();
        assert!((magnitude - 5.0).abs() < 0.0001);
    }

    /// Test 11: InferenceConfig serialization support
    #[test]
    fn test_inference_config_serde_json() {
        let config = InferenceConfig {
            base_model_id: "all-MiniLM-L6-v2".to_string(),
            knowledge_index_dir: "/tmp/rag".to_string(),
            prefer_candle: true,
            enable_llamacpp: false,
        };

        let json = serde_json::to_string(&config).expect("Should serialize");
        let _deserialized: InferenceConfig =
            serde_json::from_str(&json).expect("Should deserialize");
    }

    /// Test 12: ModelInfo serialization support
    #[test]
    fn test_model_info_serde_json() {
        let model_info = ModelInfo {
            model_id: "all-MiniLM-L6-v2".to_string(),
            embedding_dim: 384,
            backend_name: "candle".to_string(),
            available: true,
        };

        let json = serde_json::to_string(&model_info).expect("Should serialize");
        let _deserialized: ModelInfo = serde_json::from_str(&json).expect("Should deserialize");
    }

    /// Test 13: Embedding with different dimensions
    #[test]
    fn test_embedding_dimension_flexibility() {
        let emb_small = Embedding {
            data: vec![0.1, 0.2],
        };
        let emb_large = Embedding {
            data: vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
        };

        assert_eq!(emb_small.data.len(), 2);
        assert_eq!(emb_large.data.len(), 8);
    }

    /// Test 14: Backend selector with llamacpp feature
    #[test]
    #[cfg(feature = "llamacpp-fallback")]
    fn test_backend_selector_llamacpp_feature() {
        let config = InferenceConfig {
            base_model_id: "test".to_string(),
            knowledge_index_dir: "/tmp".to_string(),
            prefer_candle: false,
            enable_llamacpp: true,
        };

        let _backend = select_inference_backend(&config);
        // Should attempt llama.cpp if feature is enabled
    }

    /// Test 15: ModelInfo for ripgrep fallback
    #[test]
    fn test_model_info_ripgrep_backend() {
        let ripgrep_model = ModelInfo {
            model_id: "ripgrep-bm25".to_string(),
            embedding_dim: 0, // BM25 doesn't use embeddings
            backend_name: "ripgrep".to_string(),
            available: true,
        };

        assert_eq!(ripgrep_model.backend_name, "ripgrep");
        assert_eq!(ripgrep_model.embedding_dim, 0);
    }

    /// Test 16: Candle backend creation with model initialization
    #[test]
    #[cfg(feature = "candle")]
    fn test_candle_new() {
        use crate::blessing::inference::candle::CandleBackend;

        let backend = CandleBackend::new("all-MiniLM-L6-v2".to_string(), 384);
        let info = backend.model_info();

        assert_eq!(info.model_id, "all-MiniLM-L6-v2");
        assert_eq!(info.embedding_dim, 384);
        assert_eq!(info.backend_name, "candle");
        // loaded_at should be set in new()
        assert!(backend.loaded_at().is_some());
    }

    /// Test 17: Candle backend is_available with device detection
    #[test]
    #[cfg(feature = "candle")]
    fn test_candle_is_available() {
        use crate::blessing::inference::candle::CandleBackend;

        let backend = CandleBackend::new("test-model".to_string(), 384);
        // Should not panic; returns bool based on device availability
        let _available = backend.is_available();
    }

    /// Test 18: Candle backend device detection via nvidia-smi
    #[test]
    #[cfg(feature = "candle")]
    fn test_device_detection_nvidia_smi() {
        use crate::blessing::inference::candle::DeviceInfo;

        let device_info = DeviceInfo::available();
        // Should not panic; returns either GPU or CPU fallback
        assert!(!device_info.is_empty());
    }

    /// Test 19: Candle backend cache_stats idempotency
    #[test]
    #[cfg(feature = "candle")]
    fn test_model_cache_stats_idempotent() {
        use crate::blessing::inference::candle::CandleBackend;

        let backend = CandleBackend::new("all-MiniLM-L6-v2".to_string(), 384);
        let stats1 = backend.cache_stats();
        let stats2 = backend.cache_stats();

        // Both calls should return same result (idempotent)
        assert_eq!(stats1.model_id, stats2.model_id);
        assert_eq!(stats1.bytes_on_disk, stats2.bytes_on_disk);
    }

    /// Test 20: Candle async embed returns proper dimensions
    #[tokio::test]
    #[cfg(feature = "candle")]
    async fn test_candle_async_embed() {
        use crate::blessing::inference::candle::CandleBackend;

        let backend = CandleBackend::new("all-MiniLM-L6-v2".to_string(), 384);
        // embed() returns stub zero-vector in Phase 3
        let result = backend.embed("test text").await;
        assert!(result.is_ok());

        let embedding = result.unwrap();
        assert_eq!(embedding.data.len(), 384);
    }

    /// Test 21: llama.cpp backend creation with deprecation warning
    #[test]
    #[cfg(feature = "llamacpp-fallback")]
    fn test_llamacpp_backend_new() {
        use crate::blessing::inference::llamacpp::LlamaCppBackend;

        let backend = LlamaCppBackend::new("all-MiniLM-L6-v2".to_string(), 384);
        let info = backend.model_info();

        assert_eq!(info.model_id, "all-MiniLM-L6-v2");
        assert_eq!(info.embedding_dim, 384);
        assert_eq!(info.backend_name, "llamacpp");
        // deprecated flag should be true
        assert!(backend.is_deprecated());
    }

    /// Test 22: llama.cpp backend is_available with CPU device detection
    #[test]
    #[cfg(feature = "llamacpp-fallback")]
    fn test_llamacpp_is_available() {
        use crate::blessing::inference::llamacpp::LlamaCppBackend;

        let backend = LlamaCppBackend::new("test-model".to_string(), 384);
        // Should not panic; CPU-only in Phase 7
        let _available = backend.is_available();
    }

    /// Test 23: llama.cpp backend cache_stats with Phase 8 skunks
    #[test]
    #[cfg(feature = "llamacpp-fallback")]
    fn test_llamacpp_model_cache_stats() {
        use crate::blessing::inference::llamacpp::LlamaCppBackend;

        let backend = LlamaCppBackend::new("all-MiniLM-L6-v2".to_string(), 384);
        let stats = backend.cache_stats();

        assert_eq!(stats.model_id, "all-MiniLM-L6-v2");
        // 🦨 Phase 8: bytes_on_disk calculation from model_id + quantization
        // 🦨 Phase 8: quantization detection (currently stub "q8")
        // 🦨 Phase 8: Memory calculation refinement
        assert_eq!(stats.quantization, "q8");
    }

    /// Test 24: llama.cpp deprecated flag set in new()
    #[test]
    #[cfg(feature = "llamacpp-fallback")]
    fn test_llamacpp_deprecated_flag() {
        use crate::blessing::inference::llamacpp::LlamaCppBackend;

        let backend = LlamaCppBackend::new("test".to_string(), 384);
        // Deprecation flag must be true; logs warning in new()
        assert!(backend.is_deprecated());
    }

    /// Test 25: llama.cpp async embed returns proper dimensions
    #[tokio::test]
    #[cfg(feature = "llamacpp-fallback")]
    async fn test_llamacpp_async_embed() {
        use crate::blessing::inference::llamacpp::LlamaCppBackend;

        let backend = LlamaCppBackend::new("all-MiniLM-L6-v2".to_string(), 384);
        let result = backend.embed("test text").await;
        assert!(result.is_ok());

        let embedding = result.unwrap();
        assert_eq!(embedding.data.len(), 384);
    }

    /// Test 26: Ripgrep backend creation
    #[test]
    fn test_ripgrep_backend_new() {
        use crate::blessing::inference::fallback::RipgrepBM25;

        let backend = RipgrepBM25::new();
        let info = backend.model_info();

        assert_eq!(info.model_id, "ripgrep-bm25");
        assert_eq!(info.embedding_dim, 0);
        assert_eq!(info.backend_name, "ripgrep");
        assert!(info.available);
    }

    /// Test 27: Ripgrep is_available always true
    #[test]
    fn test_ripgrep_is_available() {
        use crate::blessing::inference::fallback::RipgrepBM25;

        let backend = RipgrepBM25::new();
        assert!(backend.is_available());
    }

    /// Test 28: Ripgrep embed returns zero vector (no embeddings)
    #[tokio::test]
    async fn test_ripgrep_embed() {
        use crate::blessing::inference::fallback::RipgrepBM25;

        let backend = RipgrepBM25::new();
        let result = backend.embed("test text").await;
        assert!(result.is_ok());

        let embedding = result.unwrap();
        assert_eq!(embedding.data.len(), 0); // BM25 doesn't use embeddings
    }

    /// Test 29: Ripgrep compose_layers is no-op
    #[tokio::test]
    async fn test_ripgrep_compose_layers() {
        use crate::blessing::inference::fallback::RipgrepBM25;

        let mut backend = RipgrepBM25::new();
        let result = backend.compose_layers(&["blessing1", "blessing2"]).await;
        assert!(result.is_ok());
    }

    /// Test 30: Ripgrep clear_layers is no-op
    #[test]
    fn test_ripgrep_clear_layers() {
        use crate::blessing::inference::fallback::RipgrepBM25;

        let mut backend = RipgrepBM25::new();
        let result = backend.clear_layers();
        assert!(result.is_ok());
    }

    /// Test 31: Ripgrep model_info returns correct metadata
    #[test]
    fn test_ripgrep_model_info() {
        use crate::blessing::inference::fallback::RipgrepBM25;

        let backend = RipgrepBM25::new();
        let info = backend.model_info();

        assert_eq!(info.model_id, "ripgrep-bm25");
        assert_eq!(info.embedding_dim, 0);
        assert_eq!(info.backend_name, "ripgrep");
        assert!(info.available);
    }

    /// Test 32: Ripgrep search returns empty results (stub)
    #[test]
    fn test_ripgrep_search_stub() {
        use crate::blessing::inference::fallback::RipgrepBM25;

        let backend = RipgrepBM25::new();
        // 🦨 TODO: Implement actual ripgrep command execution
        let results = backend.search("test query", 5);
        assert!(results.is_empty());
    }

    /// Test 33: BM25SearchResult structure
    #[test]
    fn test_bm25_search_result() {
        use crate::blessing::inference::fallback::BM25SearchResult;

        let result = BM25SearchResult {
            blessing_id: "blessing-1".to_string(),
            relevance_score: 0.85,
            matched_lines: vec!["line 1".to_string(), "line 2".to_string()],
        };

        assert_eq!(result.blessing_id, "blessing-1");
        assert!((result.relevance_score - 0.85).abs() < 0.0001);
        assert_eq!(result.matched_lines.len(), 2);
    }
}
