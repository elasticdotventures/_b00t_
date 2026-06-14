//! Postel/DWIW hint system.
//!
//! When b00t normalizes non-canonical input (P2→2, "prd"→content-tag, etc.),
//! it emits a one-time structured hint showing the canonical form + a working
//! example. Hints fire once per (input, canonical) pair per process.
//!
//! Silence all hints: `B00T_POSTEL_HINTS=0`

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

/// Session-scoped set of (input, canonical) pairs already hinted.
static HINTED: OnceLock<Mutex<HashSet<(String, String)>>> = OnceLock::new();

/// Emit a Postel normalization hint once per (input, canonical) pair.
///
/// Skipped when `B00T_POSTEL_HINTS=0`.
/// Output goes to stderr so it doesn't corrupt structured stdout.
pub fn hint(input: &str, canonical: &str, example: &str, tip: &str) {
    if std::env::var("B00T_POSTEL_HINTS")
        .map(|v| v == "0")
        .unwrap_or(false)
    {
        return;
    }
    let key = (input.to_owned(), canonical.to_owned());
    let registry = HINTED.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(mut set) = registry.lock() {
        if set.insert(key) {
            eprintln!(
                "🔁 b00t: '{input}' → '{canonical}' ({tip}); \
                 example: {example}; silence: B00T_POSTEL_HINTS=0"
            );
            crate::otel::record(crate::otel::MetricEvent::PostelNormalization {
                input: input.to_owned(),
                canonical: canonical.to_owned(),
            });
        }
    }
}
