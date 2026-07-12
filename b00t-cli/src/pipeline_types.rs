//! Typed error routing for pipeline stages — PipelineError taxonomy and ErrorRoute
//! with glob-based matching.
//!
//! # Architecture
//!
//! All pipeline execution errors are modelled as a single `PipelineError` enum
//! with well-typed variants. Each variant carries structured context (not just
//! string messages) so that `ErrorRoute` can match and route them declaratively.
//!
//! # Glob Matching
//!
//! `ErrorRoute::matches()` supports simple glob patterns:
//! - `"*"` matches every variant
//! - `"Transcode*"` matches any variant whose Debug name starts with "Transcode"
//! - `"ResourceExhausted"` matches only the exact variant name (case-sensitive)
//!
//! # Retry Semantics
//!
//! `ErrorRoute` carries `max_retries` and `backoff_ms` so that retry logic can
//! be driven by the route, not by the caller. When retries are exhausted the
//! optional `fallback_output` provides a default `StagePort` to emit instead of
//! failing the pipeline.
//!
//! # Parallel Types
//!
//! `PortMediaType`, `PortDirection`, and `StagePort` are defined here as minimal
//! standalone copies so that issue #722 has no type dependency on issue #719
//! (which defines the full versions). They will be reconciled in a follow-up.

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════
// Section A: PipelineError — typed, matchable error taxonomy
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PipelineError {
    /// Input failed validation — carries the validation message.
    InputValidation(String),

    /// A required resource is exhausted — carries what was needed vs. available.
    ResourceExhausted {
        /// Description of the resource that was needed
        needed: String,
        /// Description of what was actually available
        available: String,
    },

    /// A pipeline stage crashed unexpectedly — carries the stage name + reason.
    StageCrashed(String),

    /// A stage exceeded its time limit — carries which stage and how long it ran.
    Timeout {
        /// Name of the stage that timed out
        stage: String,
        /// Elapsed wall-clock time in milliseconds
        elapsed_ms: u64,
    },

    /// A port received data of an unexpected media type.
    MediaTypeMismatch {
        /// The media type the port expected
        expected: PortMediaType,
        /// The media type that was actually delivered
        got: PortMediaType,
    },

    /// A transcoding operation failed — carries the error message.
    TranscodeError(String),
}

// ── From impls ────────────────────────────────────────────────────────────

impl From<anyhow::Error> for PipelineError {
    fn from(err: anyhow::Error) -> Self {
        PipelineError::InputValidation(err.to_string())
    }
}

impl From<String> for PipelineError {
    fn from(msg: String) -> Self {
        PipelineError::InputValidation(msg)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Section B: Parallel types (minimal — issue #719 will define the full types)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PortMediaType {
    Video,
    Audio,
    Image,
    Json,
    Parquet,
    Bytes,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StagePort {
    pub direction: PortDirection,
    pub media_type: PortMediaType,
    pub description: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// Section C: ErrorRoute — declarative error routing with retry
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorRoute {
    /// Glob-style pattern to match against PipelineError variant names.
    pub match_pattern: String,

    /// Name of the stage to route matching errors to (for retry or recovery).
    pub route_to_stage: String,

    /// Maximum number of automatic retries before giving up.
    pub max_retries: u32,

    /// Base backoff in milliseconds between retries.
    pub backoff_ms: u64,

    /// Optional fallback output to emit when retries are exhausted.
    pub fallback_output: Option<StagePort>,

    /// Retry counter — incremented by the caller on each retry.
    /// Not serialised — reset for each error-routing session.
    #[serde(skip)]
    pub retry_count: u32,
}

impl ErrorRoute {
    /// Returns `true` when the given `error`'s variant name matches this
    /// route's `match_pattern` using simple glob semantics.
    pub fn matches(&self, error: &PipelineError) -> bool {
        let variant_name = error.variant_name();
        glob_match(&self.match_pattern, variant_name)
    }

    /// Returns the number of retries remaining before exhaustion.
    pub fn retries_left(&self) -> u32 {
        self.max_retries.saturating_sub(self.retry_count)
    }

    /// Record one retry attempt (increments `retry_count`).
    pub fn record_retry(&mut self) {
        self.retry_count = self.retry_count.saturating_add(1);
    }

    /// Returns `true` if the route can still be retried (`retries_left() > 0`).
    pub fn can_retry(&self) -> bool {
        self.retries_left() > 0
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Section D: Glob matching — simple, intentional, no regex dependency
// ═══════════════════════════════════════════════════════════════════════════

fn glob_match(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else {
        pattern == name
    }
}

impl PipelineError {
    fn variant_name(&self) -> &str {
        match self {
            PipelineError::InputValidation(_) => "InputValidation",
            PipelineError::ResourceExhausted { .. } => "ResourceExhausted",
            PipelineError::StageCrashed(_) => "StageCrashed",
            PipelineError::Timeout { .. } => "Timeout",
            PipelineError::MediaTypeMismatch { .. } => "MediaTypeMismatch",
            PipelineError::TranscodeError(_) => "TranscodeError",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── PipelineError construction ──────────────────────────────────────

    #[test]
    fn input_validation_holds_string() {
        let err = PipelineError::InputValidation("bad input".into());
        assert_eq!(
            format!("{err:?}"),
            "InputValidation(\"bad input\")"
        );
    }

    #[test]
    fn resource_exhausted_holds_needed_and_available() {
        let err = PipelineError::ResourceExhausted {
            needed: "512 MiB".into(),
            available: "256 MiB".into(),
        };
        assert_eq!(
            format!("{err:?}"),
            "ResourceExhausted { needed: \"512 MiB\", available: \"256 MiB\" }"
        );
    }

    #[test]
    fn stage_crashed_holds_reason() {
        let err = PipelineError::StageCrashed("OOM".into());
        assert_eq!(format!("{err:?}"), "StageCrashed(\"OOM\")");
    }

    #[test]
    fn timeout_holds_stage_and_elapsed() {
        let err = PipelineError::Timeout {
            stage: "transcode".into(),
            elapsed_ms: 30_000,
        };
        assert_eq!(
            format!("{err:?}"),
            "Timeout { stage: \"transcode\", elapsed_ms: 30000 }"
        );
    }

    #[test]
    fn media_type_mismatch_holds_expected_and_got() {
        let err = PipelineError::MediaTypeMismatch {
            expected: PortMediaType::Video,
            got: PortMediaType::Audio,
        };
        assert_eq!(
            format!("{err:?}"),
            "MediaTypeMismatch { expected: Video, got: Audio }"
        );
    }

    #[test]
    fn transcode_error_holds_message() {
        let err = PipelineError::TranscodeError("unsupported codec".into());
        assert_eq!(
            format!("{err:?}"),
            "TranscodeError(\"unsupported codec\")"
        );
    }

    // ── From impls ──────────────────────────────────────────────────────

    #[test]
    fn from_anyhow_error_wraps_as_input_validation() {
        let any_err = anyhow::anyhow!("disk full");
        let pipe_err: PipelineError = any_err.into();
        assert_eq!(pipe_err.variant_name(), "InputValidation");
    }

    #[test]
    fn from_string_wraps_as_input_validation() {
        let pipe_err: PipelineError = "something went wrong".to_string().into();
        assert_eq!(pipe_err.variant_name(), "InputValidation");
    }

    // ── Variant name extraction ─────────────────────────────────────────

    #[test]
    fn variant_name_all_variants() {
        let cases: Vec<(PipelineError, &str)> = vec![
            (PipelineError::InputValidation("x".into()), "InputValidation"),
            (
                PipelineError::ResourceExhausted {
                    needed: "a".into(),
                    available: "b".into(),
                },
                "ResourceExhausted",
            ),
            (PipelineError::StageCrashed("x".into()), "StageCrashed"),
            (
                PipelineError::Timeout { stage: "x".into(), elapsed_ms: 1 },
                "Timeout",
            ),
            (
                PipelineError::MediaTypeMismatch {
                    expected: PortMediaType::Json,
                    got: PortMediaType::Bytes,
                },
                "MediaTypeMismatch",
            ),
            (PipelineError::TranscodeError("x".into()), "TranscodeError"),
        ];
        for (err, expected) in cases {
            assert_eq!(err.variant_name(), expected);
        }
    }

    // ── Glob matching ───────────────────────────────────────────────────

    #[test]
    fn glob_match_exact() {
        assert!(glob_match("TranscodeError", "TranscodeError"));
        assert!(!glob_match("TranscodeError", "Timeout"));
    }

    #[test]
    fn glob_match_prefix_wildcard() {
        assert!(glob_match("Transcode*", "TranscodeError"));
        assert!(glob_match("Transcode*", "Transcode"));
        assert!(!glob_match("Transcode*", "Timeout"));
    }

    #[test]
    fn glob_match_catch_all() {
        assert!(glob_match("*", "InputValidation"));
        assert!(glob_match("*", "ResourceExhausted"));
        assert!(glob_match("*", "StageCrashed"));
        assert!(glob_match("*", "Timeout"));
        assert!(glob_match("*", "MediaTypeMismatch"));
        assert!(glob_match("*", "TranscodeError"));
    }

    #[test]
    fn glob_match_empty_pattern_does_not_match() {
        assert!(!glob_match("", "TranscodeError"));
    }

    #[test]
    fn glob_match_star_only_is_catch_all() {
        assert!(glob_match("*", "AnythingAtAll"));
    }

    // ── ErrorRoute::matches ─────────────────────────────────────────────

    #[test]
    fn route_matches_exact_variant_name() {
        let route = ErrorRoute {
            match_pattern: "TranscodeError".into(),
            route_to_stage: "recovery".into(),
            max_retries: 3,
            backoff_ms: 100,
            fallback_output: None,
            retry_count: 0,
        };
        let err = PipelineError::TranscodeError("bad codec".into());
        assert!(route.matches(&err));
    }

    #[test]
    fn route_does_not_match_different_variant() {
        let route = ErrorRoute {
            match_pattern: "TranscodeError".into(),
            route_to_stage: "recovery".into(),
            max_retries: 3,
            backoff_ms: 100,
            fallback_output: None,
            retry_count: 0,
        };
        let err = PipelineError::Timeout {
            stage: "transcode".into(),
            elapsed_ms: 30_000,
        };
        assert!(!route.matches(&err));
    }

    #[test]
    fn route_matches_glob_prefix() {
        let route = ErrorRoute {
            match_pattern: "Transcode*".into(),
            route_to_stage: "recovery".into(),
            max_retries: 3,
            backoff_ms: 100,
            fallback_output: None,
            retry_count: 0,
        };
        assert!(route.matches(&PipelineError::TranscodeError("x".into())));
        assert!(!route.matches(&PipelineError::Timeout {
            stage: "x".into(),
            elapsed_ms: 1,
        }));
    }

    #[test]
    fn route_matches_catch_all() {
        let route = ErrorRoute {
            match_pattern: "*".into(),
            route_to_stage: "recovery".into(),
            max_retries: 0,
            backoff_ms: 0,
            fallback_output: None,
            retry_count: 0,
        };
        assert!(route.matches(&PipelineError::InputValidation("x".into())));
        assert!(route.matches(&PipelineError::TranscodeError("x".into())));
        assert!(route.matches(&PipelineError::Timeout {
            stage: "x".into(),
            elapsed_ms: 1,
        }));
    }

    #[test]
    fn route_matches_resource_exhausted_by_exact_name() {
        let route = ErrorRoute {
            match_pattern: "ResourceExhausted".into(),
            route_to_stage: "scale-up".into(),
            max_retries: 2,
            backoff_ms: 500,
            fallback_output: None,
            retry_count: 0,
        };
        let err = PipelineError::ResourceExhausted {
            needed: "GPU".into(),
            available: "none".into(),
        };
        assert!(route.matches(&err));
    }

    // ── Retry lifecycle ─────────────────────────────────────────────────

    #[test]
    fn retry_succeeds_within_limit() {
        let mut route = ErrorRoute {
            match_pattern: "*".into(),
            route_to_stage: "retry-stage".into(),
            max_retries: 3,
            backoff_ms: 100,
            fallback_output: None,
            retry_count: 0,
        };

        assert_eq!(route.retries_left(), 3);
        assert!(route.can_retry());

        route.record_retry();
        assert_eq!(route.retries_left(), 2);
        assert!(route.can_retry());

        route.record_retry();
        assert_eq!(route.retries_left(), 1);
        assert!(route.can_retry());

        route.record_retry();
        assert_eq!(route.retries_left(), 0);
        assert!(!route.can_retry());
    }

    #[test]
    fn retry_exhausted_returns_false_for_can_retry() {
        let mut route = ErrorRoute {
            match_pattern: "*".into(),
            route_to_stage: "retry-stage".into(),
            max_retries: 2,
            backoff_ms: 100,
            fallback_output: None,
            retry_count: 0,
        };

        route.record_retry();
        route.record_retry();

        assert!(!route.can_retry());
        assert_eq!(route.retries_left(), 0);
    }

    #[test]
    fn retry_zero_max_immediately_exhausted() {
        let route = ErrorRoute {
            match_pattern: "*".into(),
            route_to_stage: "retry-stage".into(),
            max_retries: 0,
            backoff_ms: 0,
            fallback_output: None,
            retry_count: 0,
        };

        assert!(!route.can_retry());
        assert_eq!(route.retries_left(), 0);
    }

    #[test]
    fn retry_count_does_not_overflow() {
        let mut route = ErrorRoute {
            match_pattern: "*".into(),
            route_to_stage: "retry-stage".into(),
            max_retries: u32::MAX,
            backoff_ms: 1,
            fallback_output: None,
            retry_count: u32::MAX,
        };
        route.record_retry();
        assert_eq!(route.retry_count, u32::MAX);
        assert_eq!(route.retries_left(), 0);
    }

    // ── ErrorRoute construction ─────────────────────────────────────────

    #[test]
    fn route_default_retry_count_is_zero() {
        let route = ErrorRoute {
            match_pattern: "Timeout".into(),
            route_to_stage: "scaler".into(),
            max_retries: 1,
            backoff_ms: 250,
            fallback_output: None,
            retry_count: 0,
        };
        assert_eq!(route.retry_count, 0);
        assert_eq!(route.retries_left(), 1);
    }

    #[test]
    fn route_with_fallback_output() {
        let fallback = StagePort {
            direction: PortDirection::Output,
            media_type: PortMediaType::Json,
            description: Some("fallback empty result".into()),
        };
        let route = ErrorRoute {
            match_pattern: "*".into(),
            route_to_stage: "recovery".into(),
            max_retries: 1,
            backoff_ms: 100,
            fallback_output: Some(fallback),
            retry_count: 0,
        };
        assert!(route.fallback_output.is_some());
        assert_eq!(
            route.fallback_output.as_ref().unwrap().media_type,
            PortMediaType::Json
        );
    }

    // ── PortMediaType / PortDirection / StagePort ───────────────────────

    #[test]
    fn stage_port_round_trip() {
        let port = StagePort {
            direction: PortDirection::Input,
            media_type: PortMediaType::Video,
            description: Some("input video stream".into()),
        };
        assert_eq!(port.direction, PortDirection::Input);
        assert_eq!(port.media_type, PortMediaType::Video);
        assert_eq!(port.description.as_deref(), Some("input video stream"));
    }

    #[test]
    fn stage_port_description_can_be_none() {
        let port = StagePort {
            direction: PortDirection::Output,
            media_type: PortMediaType::Bytes,
            description: None,
        };
        assert!(port.description.is_none());
    }

    #[test]
    fn port_media_type_all_variants() {
        let variants = vec![
            PortMediaType::Video,
            PortMediaType::Audio,
            PortMediaType::Image,
            PortMediaType::Json,
            PortMediaType::Parquet,
            PortMediaType::Bytes,
            PortMediaType::Error,
        ];
        let debug = format!("{:?}", variants);
        assert!(debug.contains("Video"));
        assert!(debug.contains("Audio"));
        assert!(debug.contains("Image"));
        assert!(debug.contains("Json"));
        assert!(debug.contains("Parquet"));
        assert!(debug.contains("Bytes"));
        assert!(debug.contains("Error"));
    }

    // ── Serialisation round-trips ───────────────────────────────────────

    #[test]
    fn pipeline_error_serialize_round_trip() {
        let err = PipelineError::Timeout {
            stage: "encode".into(),
            elapsed_ms: 5000,
        };
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: PipelineError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, deserialized);
    }

    #[test]
    fn error_route_serialize_skips_retry_count() {
        let route = ErrorRoute {
            match_pattern: "*".into(),
            route_to_stage: "recovery".into(),
            max_retries: 3,
            backoff_ms: 100,
            fallback_output: None,
            retry_count: 5,
        };
        let json = serde_json::to_string(&route).unwrap();
        assert!(!json.contains("retry_count"));
    }

    #[test]
    fn error_route_deserialize_defaults_retry_count_to_zero() {
        let json = r#"{
            "match_pattern": "Timeout",
            "route_to_stage": "scaler",
            "max_retries": 2,
            "backoff_ms": 500,
            "fallback_output": null
        }"#;
        let route: ErrorRoute = serde_json::from_str(json).unwrap();
        assert_eq!(route.retry_count, 0);
        assert_eq!(route.max_retries, 2);
    }

    #[test]
    fn stage_port_serialize_round_trip() {
        let port = StagePort {
            direction: PortDirection::Output,
            media_type: PortMediaType::Image,
            description: Some("generated thumbnail".into()),
        };
        let json = serde_json::to_string(&port).unwrap();
        let deserialized: StagePort = serde_json::from_str(&json).unwrap();
        assert_eq!(port, deserialized);
    }
}
