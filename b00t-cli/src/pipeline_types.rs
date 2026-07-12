//! Typed error routing for pipeline stages

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PipelineError {
    InputValidation(String),
    ResourceExhausted { needed: String, available: String },
    StageCrashed(String),
    Timeout { stage: String, elapsed_ms: u64 },
    MediaTypeMismatch { expected: PortMediaType, got: PortMediaType },
    TranscodeError(String),
}

impl From<anyhow::Error> for PipelineError {
    fn from(err: anyhow::Error) -> Self { PipelineError::InputValidation(err.to_string()) }
}

impl From<String> for PipelineError {
    fn from(msg: String) -> Self { PipelineError::InputValidation(msg) }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PortMediaType { Video, Audio, Image, Json, Parquet, Bytes, Error }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PortDirection { Input, Output }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StagePort {
    pub direction: PortDirection,
    pub media_type: PortMediaType,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorRoute {
    pub match_pattern: String,
    pub route_to_stage: String,
    pub max_retries: u32,
    pub backoff_ms: u64,
    pub fallback_output: Option<StagePort>,
    #[serde(skip)]
    pub retry_count: u32,
}

impl ErrorRoute {
    pub fn matches(&self, error: &PipelineError) -> bool {
        glob_match(&self.match_pattern, error.variant_name())
    }
    pub fn retries_left(&self) -> u32 { self.max_retries.saturating_sub(self.retry_count) }
    pub fn record_retry(&mut self) { self.retry_count = self.retry_count.saturating_add(1); }
    pub fn can_retry(&self) -> bool { self.retries_left() > 0 }
}

fn glob_match(pattern: &str, name: &str) -> bool {
    if pattern == "*" { return true; }
    if let Some(prefix) = pattern.strip_suffix('*') { name.starts_with(prefix) }
    else { pattern == name }
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

    #[test] fn input_validation_holds_string() {
        let err = PipelineError::InputValidation("bad input".into());
        assert_eq!(format!("{err:?}"), "InputValidation(\"bad input\")");
    }
    #[test] fn resource_exhausted_holds_needed_and_available() {
        let err = PipelineError::ResourceExhausted { needed: "512 MiB".into(), available: "256 MiB".into() };
        assert_eq!(format!("{err:?}"), "ResourceExhausted { needed: \"512 MiB\", available: \"256 MiB\" }");
    }
    #[test] fn from_anyhow_error() { let e: PipelineError = anyhow::anyhow!("disk full").into(); assert_eq!(e.variant_name(), "InputValidation"); }
    #[test] fn from_string() { let e: PipelineError = "oops".to_string().into(); assert_eq!(e.variant_name(), "InputValidation"); }
    #[test] fn variant_name_all() {
        for (e, n) in [
            (PipelineError::InputValidation("x".into()), "InputValidation"),
            (PipelineError::ResourceExhausted { needed: "a".into(), available: "b".into() }, "ResourceExhausted"),
            (PipelineError::StageCrashed("x".into()), "StageCrashed"),
            (PipelineError::Timeout { stage: "x".into(), elapsed_ms: 1 }, "Timeout"),
            (PipelineError::MediaTypeMismatch { expected: PortMediaType::Json, got: PortMediaType::Bytes }, "MediaTypeMismatch"),
            (PipelineError::TranscodeError("x".into()), "TranscodeError"),
        ] { assert_eq!(e.variant_name(), n); }
    }
    #[test] fn glob_exact() { assert!(glob_match("TranscodeError", "TranscodeError")); assert!(!glob_match("TranscodeError", "Timeout")); }
    #[test] fn glob_prefix() { assert!(glob_match("Transcode*", "TranscodeError")); assert!(!glob_match("Transcode*", "Timeout")); }
    #[test] fn glob_catch_all() { assert!(glob_match("*", "InputValidation")); assert!(glob_match("*", "Anything")); }
    #[test] fn glob_empty() { assert!(!glob_match("", "TranscodeError")); }
    #[test] fn route_exact_match() {
        let r = ErrorRoute { match_pattern: "TranscodeError".into(), route_to_stage: "r".into(), max_retries: 3, backoff_ms: 100, fallback_output: None, retry_count: 0 };
        assert!(r.matches(&PipelineError::TranscodeError("bad".into())));
        assert!(!r.matches(&PipelineError::Timeout { stage: "t".into(), elapsed_ms: 1 }));
    }
    #[test] fn route_glob_match() {
        let r = ErrorRoute { match_pattern: "Transcode*".into(), route_to_stage: "r".into(), max_retries: 3, backoff_ms: 100, fallback_output: None, retry_count: 0 };
        assert!(r.matches(&PipelineError::TranscodeError("x".into())));
        assert!(!r.matches(&PipelineError::Timeout { stage: "x".into(), elapsed_ms: 1 }));
    }
    #[test] fn route_catch_all() {
        let r = ErrorRoute { match_pattern: "*".into(), route_to_stage: "r".into(), max_retries: 0, backoff_ms: 0, fallback_output: None, retry_count: 0 };
        assert!(r.matches(&PipelineError::InputValidation("x".into())));
        assert!(r.matches(&PipelineError::TranscodeError("x".into())));
    }
    #[test] fn retry_within_limit() {
        let mut r = ErrorRoute { match_pattern: "*".into(), route_to_stage: "r".into(), max_retries: 3, backoff_ms: 100, fallback_output: None, retry_count: 0 };
        assert!(r.can_retry()); assert_eq!(r.retries_left(), 3);
        r.record_retry(); assert!(r.can_retry()); assert_eq!(r.retries_left(), 2);
        r.record_retry(); assert!(r.can_retry()); assert_eq!(r.retries_left(), 1);
        r.record_retry(); assert!(!r.can_retry()); assert_eq!(r.retries_left(), 0);
    }
    #[test] fn retry_exhausted() {
        let mut r = ErrorRoute { match_pattern: "*".into(), route_to_stage: "r".into(), max_retries: 2, backoff_ms: 100, fallback_output: None, retry_count: 0 };
        r.record_retry(); r.record_retry();
        assert!(!r.can_retry()); assert_eq!(r.retries_left(), 0);
    }
    #[test] fn retry_zero_max() {
        let r = ErrorRoute { match_pattern: "*".into(), route_to_stage: "r".into(), max_retries: 0, backoff_ms: 0, fallback_output: None, retry_count: 0 };
        assert!(!r.can_retry()); assert_eq!(r.retries_left(), 0);
    }
    #[test] fn serialize_round_trip() {
        let err = PipelineError::Timeout { stage: "encode".into(), elapsed_ms: 5000 };
        let back: PipelineError = serde_json::from_str(&serde_json::to_string(&err).unwrap()).unwrap();
        assert_eq!(err, back);
    }
    #[test] fn serialize_skips_retry_count() {
        let r = ErrorRoute { match_pattern: "*".into(), route_to_stage: "r".into(), max_retries: 3, backoff_ms: 100, fallback_output: None, retry_count: 5 };
        assert!(!serde_json::to_string(&r).unwrap().contains("retry_count"));
    }
    #[test] fn deserialize_defaults_retry_count_to_zero() {
        let r: ErrorRoute = serde_json::from_str(r#"{"match_pattern":"T","route_to_stage":"s","max_retries":2,"backoff_ms":500,"fallback_output":null}"#).unwrap();
        assert_eq!(r.retry_count, 0);
    }
    #[test] fn stage_port() {
        let p = StagePort { direction: PortDirection::Input, media_type: PortMediaType::Video, description: Some("in".into()) };
        assert_eq!(p.direction, PortDirection::Input); assert_eq!(p.media_type, PortMediaType::Video);
    }
    #[test] fn port_media_type_all() {
        let v = format!("{:?}", vec![PortMediaType::Video,PortMediaType::Audio,PortMediaType::Image,PortMediaType::Json,PortMediaType::Parquet,PortMediaType::Bytes,PortMediaType::Error]);
        for t in &["Video","Audio","Image","Json","Parquet","Bytes","Error"] { assert!(v.contains(t), "missing {t}"); }
    }
}
