//! Bounded engine admission control (BE-137).
//!
//! `/v1/ai/suggest` and `/v1/ai/analyze` used to construct a fresh
//! `EngineService` — and therefore spawn a fresh Stockfish process — for every
//! request, with nothing capping how many could run at once. A single
//! authenticated user could pin every core on the box by sending parallel
//! depth-20 requests.
//!
//! [`EnginePool`] is the admission gate in front of that. Requests take a
//! permit before they are allowed to spawn an engine process, and the permit is
//! released as soon as the search finishes (success *or* failure). Once the
//! pool is saturated, further requests wait up to the configured timeout and
//! then fail fast with [`PoolError::Busy`] instead of piling on more processes.
//!
//! The pool deliberately does **not** keep warm engine processes alive. A
//! long-lived Stockfish worker has to be fed `ucinewgame` between unrelated
//! positions, which is exactly the kind of stateful bug that makes shared
//! engines dangerous; the win here is bounding concurrency, not amortising
//! process startup.

use std::fmt;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Default number of concurrent engine processes.
const DEFAULT_MAX_ENGINES: usize = 4;
/// Default ceiling on how long a request waits for a free slot.
const DEFAULT_ACQUIRE_TIMEOUT_MS: u64 = 5_000;
/// Hard ceiling so a typo in `ENGINE_MAX_CONCURRENCY` cannot disable the pool.
const MAX_ENGINES_CEILING: usize = 64;

#[derive(Debug)]
pub enum PoolError {
    /// Every slot is taken and the waiter ran out of time.
    Busy { capacity: usize },
    /// The pool is shutting down.
    Closed,
}

impl fmt::Display for PoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PoolError::Busy { capacity } => write!(
                f,
                "engine pool saturated: all {capacity} engine slots are in use"
            ),
            PoolError::Closed => write!(f, "engine pool is shutting down"),
        }
    }
}

impl std::error::Error for PoolError {}

/// Point-in-time view of the pool, for the `/v1/ai/health` endpoint.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct PoolStats {
    pub capacity: usize,
    pub available: usize,
    pub in_flight: usize,
    pub total_admitted: u64,
    pub total_rejected: u64,
    pub total_timed_out: u64,
}

struct PoolInner {
    permits: Semaphore,
    capacity: usize,
    acquire_timeout: Duration,
    in_flight: AtomicUsize,
    total_admitted: AtomicU64,
    total_rejected: AtomicU64,
    total_timed_out: AtomicU64,
}

/// Cloneable handle to a process-wide pool. Cloning is cheap and shares the
/// same slots, so every request handler can hold one.
#[derive(Clone)]
pub struct EnginePool {
    inner: Arc<PoolInner>,
}

/// RAII guard. Dropping it — on the happy path, on an error, or when the
/// future is cancelled — returns the slot to the pool.
#[derive(Debug)]
pub struct EnginePermit {
    _permit: OwnedSemaphorePermit,
    inner: Arc<PoolInner>,
}

impl Drop for EnginePermit {
    fn drop(&mut self) {
        self.inner.in_flight.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Default for EnginePool {
    fn default() -> Self {
        Self::from_env()
    }
}

impl EnginePool {
    /// Build a pool from `ENGINE_MAX_CONCURRENCY` and
    /// `ENGINE_ACQUIRE_TIMEOUT_MS`, falling back to conservative defaults.
    pub fn from_env() -> Self {
        let capacity = std::env::var("ENGINE_MAX_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|v| *v > 0)
            .map(|v| v.min(MAX_ENGINES_CEILING))
            .unwrap_or(DEFAULT_MAX_ENGINES);

        let acquire_timeout_ms = std::env::var("ENGINE_ACQUIRE_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_ACQUIRE_TIMEOUT_MS);

        Self::new(capacity, acquire_timeout_ms)
    }

    pub fn new(capacity: usize, acquire_timeout_ms: u64) -> Self {
        let capacity = capacity.clamp(1, MAX_ENGINES_CEILING);
        Self {
            inner: Arc::new(PoolInner {
                permits: Semaphore::new(capacity),
                capacity,
                acquire_timeout: Duration::from_millis(acquire_timeout_ms),
                in_flight: AtomicUsize::new(0),
                total_admitted: AtomicU64::new(0),
                total_rejected: AtomicU64::new(0),
                total_timed_out: AtomicU64::new(0),
            }),
        }
    }

    pub fn capacity(&self) -> usize {
        self.inner.capacity
    }

    pub fn available(&self) -> usize {
        self.inner.permits.available_permits()
    }

    pub fn stats(&self) -> PoolStats {
        PoolStats {
            capacity: self.inner.capacity,
            available: self.available(),
            in_flight: self.inner.in_flight.load(Ordering::Relaxed),
            total_admitted: self.inner.total_admitted.load(Ordering::Relaxed),
            total_rejected: self.inner.total_rejected.load(Ordering::Relaxed),
            total_timed_out: self.inner.total_timed_out.load(Ordering::Relaxed),
        }
    }

    fn admit(&self, permit: OwnedSemaphorePermit) -> EnginePermit {
        self.inner.in_flight.fetch_add(1, Ordering::Relaxed);
        self.inner.total_admitted.fetch_add(1, Ordering::Relaxed);
        EnginePermit {
            _permit: permit,
            inner: Arc::clone(&self.inner),
        }
    }

    /// Wait for an engine slot, up to the configured timeout.
    pub async fn acquire(&self) -> Result<EnginePermit, PoolError> {
        match tokio::time::timeout(
            self.inner.acquire_timeout,
            Arc::clone(&self.inner.permits).acquire_owned(),
        )
        .await
        {
            Ok(Ok(permit)) => Ok(self.admit(permit)),
            // The semaphore is only closed when the pool is being torn down.
            Ok(Err(_)) => Err(PoolError::Closed),
            Err(_) => {
                self.inner.total_timed_out.fetch_add(1, Ordering::Relaxed);
                Err(PoolError::Busy {
                    capacity: self.inner.capacity,
                })
            }
        }
    }

    /// Reject immediately instead of queueing, for call sites that would rather
    /// fail fast than add latency.
    pub fn try_acquire(&self) -> Result<EnginePermit, PoolError> {
        match Arc::clone(&self.inner.permits).try_acquire_owned() {
            Ok(permit) => Ok(self.admit(permit)),
            Err(_) => {
                self.inner.total_rejected.fetch_add(1, Ordering::Relaxed);
                Err(PoolError::Busy {
                    capacity: self.inner.capacity,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[tokio::test]
    async fn never_exceeds_capacity() {
        let pool = EnginePool::new(2, 2_000);
        let peak = Arc::new(AtomicUsize::new(0));
        let live = Arc::new(AtomicUsize::new(0));
        let admitted = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(AtomicBool::new(false));

        let mut handles = Vec::new();
        for _ in 0..16 {
            let pool = pool.clone();
            let peak = Arc::clone(&peak);
            let live = Arc::clone(&live);
            let admitted = Arc::clone(&admitted);
            let release = Arc::clone(&release);
            handles.push(tokio::spawn(async move {
                if let Ok(_permit) = pool.acquire().await {
                    let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    admitted.fetch_add(1, Ordering::SeqCst);
                    while !release.load(Ordering::SeqCst) {
                        tokio::task::yield_now().await;
                    }
                    live.fetch_sub(1, Ordering::SeqCst);
                }
            }));
        }

        // Let every task reach the semaphore.
        while admitted.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
        for _ in 0..100 {
            tokio::task::yield_now().await;
        }

        assert_eq!(
            peak.load(Ordering::SeqCst),
            2,
            "the pool must admit exactly `capacity` holders"
        );

        release.store(true, Ordering::SeqCst);
        for handle in handles {
            let _ = handle.await;
        }
    }

    #[tokio::test]
    async fn permit_is_released_on_drop() {
        let pool = EnginePool::new(1, 50);
        {
            let _permit = pool.acquire().await.expect("first acquire");
            assert_eq!(pool.available(), 0);
            assert_eq!(pool.stats().in_flight, 1);
        }
        assert_eq!(pool.available(), 1, "dropping the guard must free the slot");
        assert_eq!(pool.stats().in_flight, 0);
    }

    #[tokio::test]
    async fn saturating_the_pool_fails_fast_instead_of_spawning() {
        let pool = EnginePool::new(1, 30);
        let _held = pool.acquire().await.expect("first acquire");

        let started = std::time::Instant::now();
        let err = pool.acquire().await.expect_err("second acquire must fail");
        assert!(matches!(err, PoolError::Busy { capacity: 1 }));
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "busy path should not block far past the configured timeout"
        );
        assert_eq!(pool.stats().total_timed_out, 1);
    }

    #[tokio::test]
    async fn try_acquire_rejects_instead_of_queueing() {
        let pool = EnginePool::new(1, 5_000);
        let _held = pool.acquire().await.unwrap();
        let started = std::time::Instant::now();
        assert!(pool.try_acquire().is_err());
        assert!(started.elapsed() < Duration::from_millis(100));
        assert_eq!(pool.stats().total_rejected, 1);
    }

    #[test]
    fn capacity_is_clamped_to_a_sane_range() {
        assert_eq!(EnginePool::new(0, 10).capacity(), 1);
        assert_eq!(EnginePool::new(10_000, 10).capacity(), MAX_ENGINES_CEILING);
    }

    #[tokio::test]
    async fn clones_share_the_same_slots() {
        let a = EnginePool::new(1, 10);
        let b = a.clone();
        let _held = a.acquire().await.expect("acquire via first handle");
        assert_eq!(
            b.available(),
            0,
            "a clone must observe the slot taken through the original"
        );
    }
}