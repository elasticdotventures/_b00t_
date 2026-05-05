//! Rate-limiter for concurrent agent delegations.
//!
//! Uses a `tokio::sync::Semaphore` to cap the number of in-flight delegate
//! tasks.  Provides both a non-blocking `try_acquire` and a timeout-backed
//! `acquire_timeout` so callers can queue politely or return "busy" when the
//! system is at capacity.
//!
//! # Default
//!
//! A default maximum of 8 concurrent delegates is used when no explicit limit
//! is provided.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Default maximum number of concurrent agent delegations.
pub const DEFAULT_MAX_CONCURRENT: usize = 8;

/// Rate-limiter for concurrent agent delegations.
///
/// Wraps a `tokio::sync::Semaphore` and exposes high-level acquire methods
/// that are safe to call from any async context.
#[derive(Debug)]
pub struct DelegationLimiter {
    /// Shared semaphore that coordinates access.
    semaphore: Arc<Semaphore>,
    /// Configured maximum (stored for introspection).
    max_concurrent: usize,
}

impl DelegationLimiter {
    /// Create a new limiter with the given maximum number of permits.
    ///
    /// The semaphore is initialised with `max` permits, one per concurrent
    /// delegation slot.
    ///
    /// # Panics
    ///
    /// Panics if `max` is 0 (a semaphore with zero permits is useless here).
    pub fn new(max: usize) -> Self {
        assert!(max > 0, "DelegationLimiter requires at least 1 permit");
        Self {
            semaphore: Arc::new(Semaphore::new(max)),
            max_concurrent: max,
        }
    }

    /// Create a new limiter with the default maximum (8).
    pub fn default() -> Self {
        Self::new(DEFAULT_MAX_CONCURRENT)
    }

    /// Try to acquire a permit without blocking.
    ///
    /// Returns `Some(permit)` if a slot is available, or `None` if the system
    /// is at capacity.  The caller should either queue the work or return a
    /// "busy" response.
    pub fn try_acquire(&self) -> Option<OwnedSemaphorePermit> {
        self.semaphore.clone().try_acquire_owned().ok()
    }

    /// Acquire a permit, waiting up to `timeout`.
    ///
    /// Returns `Some(permit)` if one became available within the timeout, or
    /// `None` if the deadline elapsed before a slot opened up.
    pub async fn acquire_timeout(&self, timeout: Duration) -> Option<OwnedSemaphorePermit> {
        tokio::time::timeout(timeout, self.semaphore.clone().acquire_owned())
            .await
            .ok()?
            .ok()
    }

    /// The configured maximum number of concurrent delegates.
    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }

    /// The number of permits currently available.
    ///
    /// A return value of 0 means the system is at capacity.
    pub fn available_permits(&self) -> usize {
        self.semaphore.available_permits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_new_creates_limiter_with_correct_permits() {
        let limiter = DelegationLimiter::new(4);
        assert_eq!(limiter.max_concurrent(), 4);
        assert_eq!(limiter.available_permits(), 4);
    }

    #[tokio::test]
    async fn test_default_creates_limiter_with_8_permits() {
        let limiter = DelegationLimiter::default();
        assert_eq!(limiter.max_concurrent(), DEFAULT_MAX_CONCURRENT);
        assert_eq!(limiter.available_permits(), DEFAULT_MAX_CONCURRENT);
    }

    #[tokio::test]
    async fn test_try_acquire_succeeds_when_permits_available() {
        let limiter = DelegationLimiter::new(2);
        let permit = limiter.try_acquire();
        assert!(permit.is_some());
        assert_eq!(limiter.available_permits(), 1);
    }

    #[tokio::test]
    async fn test_try_acquire_fails_at_capacity() {
        let limiter = DelegationLimiter::new(1);
        let _p1 = limiter.try_acquire().unwrap();
        assert!(limiter.try_acquire().is_none());
        assert_eq!(limiter.available_permits(), 0);
    }

    #[tokio::test]
    async fn test_acquire_timeout_returns_permit() {
        let limiter = DelegationLimiter::new(2);
        let permit = limiter.acquire_timeout(Duration::from_millis(50)).await;
        assert!(permit.is_some());
        assert_eq!(limiter.available_permits(), 1);
    }

    #[tokio::test]
    async fn test_acquire_timeout_times_out() {
        let limiter = DelegationLimiter::new(1);
        let _p1 = limiter.try_acquire().unwrap();
        let permit = limiter.acquire_timeout(Duration::from_millis(10)).await;
        assert!(permit.is_none());
    }

    #[tokio::test]
    async fn test_permits_are_returned_on_drop() {
        let limiter = DelegationLimiter::new(2);
        assert_eq!(limiter.available_permits(), 2);
        {
            let _p = limiter.try_acquire().unwrap();
            assert_eq!(limiter.available_permits(), 1);
        }
        // Permit dropped — permit returns to semaphore
        assert_eq!(limiter.available_permits(), 2);
    }

    #[tokio::test]
    async fn test_concurrent_delegates_respected() {
        let limiter = Arc::new(DelegationLimiter::new(3));
        let counter = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..10 {
            let lim = limiter.clone();
            let cnt = counter.clone();
            let max = max_seen.clone();
            handles.push(tokio::spawn(async move {
                let _permit = lim.acquire_timeout(Duration::from_secs(2)).await.unwrap();
                let prev = cnt.fetch_add(1, Ordering::SeqCst);
                max.fetch_max(prev + 1, Ordering::SeqCst);
                // Simulate work
                tokio::time::sleep(Duration::from_millis(10)).await;
                cnt.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        // At most 3 concurrent (the semaphore limit)
        assert!(max_seen.load(Ordering::SeqCst) <= 3);
    }

    #[tokio::test]
    async fn test_max_concurrent_getter() {
        let limiter = DelegationLimiter::new(16);
        assert_eq!(limiter.max_concurrent(), 16);
    }

    #[tokio::test]
    async fn test_available_permits_full_to_zero() {
        let limiter = DelegationLimiter::new(3);
        assert_eq!(limiter.available_permits(), 3);
        let _p1 = limiter.try_acquire().unwrap();
        assert_eq!(limiter.available_permits(), 2);
        let _p2 = limiter.try_acquire().unwrap();
        assert_eq!(limiter.available_permits(), 1);
        let _p3 = limiter.try_acquire().unwrap();
        assert_eq!(limiter.available_permits(), 0);
    }
}
