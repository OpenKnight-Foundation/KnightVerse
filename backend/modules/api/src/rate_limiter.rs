use actix_web::{
    body::{BoxBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse, Transform},
    HttpResponse,
};
use deadpool_redis::Pool;
use std::{
    future::{ready, Future, Ready},
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};
use tracing::warn;

/// Redis-backed rate limiter middleware for actix-web.
///
/// Tracks request counts per client IP using Redis INCR + EXPIRE.
/// This ensures rate limit state is shared across all server workers.
///
/// # Example (5 requests/min for login):
/// ```ignore
/// App::new()
///     .service(
///         web::scope("/v1/auth")
///             .wrap(RedisRateLimiter::new(redis_pool, 5, 60))
///             .service(login)
///     )
/// ```
#[derive(Clone)]
pub struct RedisRateLimiter {
    pool: Pool,
    requests_per_window: u64,
    window_seconds: u64,
}

impl RedisRateLimiter {
    /// Create a new `RedisRateLimiter`.
    ///
    /// * `pool` - A `deadpool_redis::Pool` connected to the Redis instance.
    /// * `requests_per_window` - Maximum number of requests allowed within the window.
    /// * `window_seconds` - Duration of the rate limit window in seconds.
    pub fn new(pool: Pool, requests_per_window: u64, window_seconds: u64) -> Self {
        Self {
            pool,
            requests_per_window,
            window_seconds,
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RedisRateLimiter
where
    S: actix_web::dev::Service<
            ServiceRequest,
            Response = ServiceResponse<B>,
            Error = actix_web::Error,
        > + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = actix_web::Error;
    type Transform = RedisRateLimiterMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RedisRateLimiterMiddleware {
            service: Rc::new(service),
            pool: self.pool.clone(),
            requests_per_window: self.requests_per_window,
            window_seconds: self.window_seconds,
        }))
    }
}

pub struct RedisRateLimiterMiddleware<S> {
    service: Rc<S>,
    pool: Pool,
    requests_per_window: u64,
    window_seconds: u64,
}

impl<S, B> actix_web::dev::Service<ServiceRequest> for RedisRateLimiterMiddleware<S>
where
    S: actix_web::dev::Service<
            ServiceRequest,
            Response = ServiceResponse<B>,
            Error = actix_web::Error,
        > + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let pool = self.pool.clone();
        let requests_per_window = self.requests_per_window;
        let window_seconds = self.window_seconds;
        let service = self.service.clone();

        Box::pin(async move {
            // Extract client IP from the peer address
            let client_ip = req
                .peer_addr()
                .map(|addr| addr.ip().to_string())
                .unwrap_or_else(|| "unknown".to_string());

            let redis_key = format!("rl:ip:{}", client_ip);

            // Try to get a Redis connection
            let mut conn = match pool.get().await {
                Ok(c) => c,
                Err(e) => {
                    warn!(
                        "Redis rate limiter connection failed: {}. Allowing request.",
                        e
                    );
                    return service
                        .call(req)
                        .await
                        .map(ServiceResponse::map_into_boxed_body);
                }
            };

            // INCR the key — if it returns 1 (key was just created), set EXPIRE
            let count: u64 = match redis::cmd("INCR")
                .arg(&redis_key)
                .query_async(&mut conn)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    warn!("Redis INCR failed: {}. Allowing request.", e);
                    return service
                        .call(req)
                        .await
                        .map(ServiceResponse::map_into_boxed_body);
                }
            };

            if count == 1 {
                let _: Result<(), _> = redis::cmd("EXPIRE")
                    .arg(&redis_key)
                    .arg(window_seconds)
                    .query_async(&mut conn)
                    .await;
            }

            // Get TTL for the reset header
            let ttl: i64 = redis::cmd("TTL")
                .arg(&redis_key)
                .query_async(&mut conn)
                .await
                .unwrap_or(window_seconds as i64);

            // If over the limit, return 429
            if count > requests_per_window {
                let response = HttpResponse::TooManyRequests()
                    .insert_header(("X-RateLimit-Limit", requests_per_window.to_string()))
                    .insert_header(("X-RateLimit-Remaining", "0"))
                    .insert_header(("X-RateLimit-Reset", ttl.max(0).to_string()))
                    .json(serde_json::json!({
                        "error": "Rate limit exceeded",
                        "message": format!(
                            "Too many requests. Limit: {} requests per {} seconds",
                            requests_per_window, window_seconds
                        ),
                        "code": "RATE_LIMIT_EXCEEDED"
                    }));

                let (req_parts, _) = req.into_parts();
                return Ok(ServiceResponse::new(req_parts, response));
            }

            // Proceed with the request and add rate limit headers
            let remaining = requests_per_window.saturating_sub(count);

            let mut res = service
                .call(req)
                .await
                .map(ServiceResponse::map_into_boxed_body)?;

            res.headers_mut().insert(
                actix_web::http::header::HeaderName::from_static("x-ratelimit-limit"),
                requests_per_window.to_string().parse().unwrap(),
            );
            res.headers_mut().insert(
                actix_web::http::header::HeaderName::from_static("x-ratelimit-remaining"),
                remaining.to_string().parse().unwrap(),
            );
            res.headers_mut().insert(
                actix_web::http::header::HeaderName::from_static("x-ratelimit-reset"),
                ttl.max(0).to_string().parse().unwrap(),
            );

            Ok(res)
        })
    }
}


// ---------------------------------------------------------------------------
// Per-session token bucket for client WebSocket frames (#1138)
// ---------------------------------------------------------------------------

/// Tokens are counted in millionths so refill arithmetic stays in integers.
/// Accumulating fractional tokens in an `f64` drifts, and a limiter that
/// silently gains or loses capacity over a long session is worse than useless.
const TOKEN_SCALE: u64 = 1_000_000;

/// A single-token-bucket rate limiter.
///
/// Unlike the fixed window used for HTTP requests above, a token bucket
/// smooths bursts: a player who fires a premove and a follow-up in the same
/// instant spends burst capacity rather than tripping the limit, while a
/// sustained flood still runs the bucket dry.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    /// Maximum tokens held, i.e. the burst allowance.
    capacity: u64,
    /// Tokens added per second.
    refill_per_sec: u64,
    /// Current tokens, scaled by [`TOKEN_SCALE`].
    tokens: u64,
    last_refill: std::time::Instant,
}

impl TokenBucket {
    pub fn new(refill_per_sec: u64, burst: u64) -> Self {
        let burst = burst.max(1);
        Self {
            capacity: burst.saturating_mul(TOKEN_SCALE),
            refill_per_sec,
            tokens: burst.saturating_mul(TOKEN_SCALE),
            last_refill: std::time::Instant::now(),
        }
    }

    /// Add the tokens earned since the last call, capped at the burst size.
    fn refill(&mut self, now: std::time::Instant) {
        let elapsed_us = now.saturating_duration_since(self.last_refill).as_micros();
        if elapsed_us == 0 {
            return;
        }

        // Tokens are scaled by TOKEN_SCALE, so the product below is already in
        // scaled units: one microsecond of a 10/s bucket earns 10 scaled units,
        // which is 10/TOKEN_SCALE of a token. Dividing by a million here would
        // refill a million times too slowly and lock every session out.
        //
        // u128 throughout: `elapsed_us * refill_per_sec` overflows u64 for a
        // long idle period at a high rate.
        let added = (elapsed_us as u128).saturating_mul(self.refill_per_sec as u128);
        if added > 0 {
            self.tokens = (self.tokens as u128)
                .saturating_add(added)
                .min(self.capacity as u128) as u64;
            self.last_refill = now;
        }
    }

    /// Spend one token. `false` means the frame must be dropped.
    pub fn try_acquire(&mut self, now: std::time::Instant) -> bool {
        self.refill(now);
        if self.tokens >= TOKEN_SCALE {
            self.tokens -= TOKEN_SCALE;
            true
        } else {
            false
        }
    }

    /// Tokens currently available, as a whole number.
    pub fn available(&self) -> u64 {
        self.tokens / TOKEN_SCALE
    }
}

/// Tuning for one WebSocket session's client frame budget.
#[derive(Debug, Clone, Copy)]
pub struct SessionRateLimitConfig {
    /// Sustained client frames per second.
    pub rate_per_sec: u64,
    /// Frames that may arrive at once before the limit engages.
    pub burst: u64,
    /// Consecutive overruns tolerated before a cooldown starts.
    pub max_violations: u32,
    /// How long a session is locked out once it trips.
    pub cooldown: std::time::Duration,
}

impl Default for SessionRateLimitConfig {
    /// The 10 frames/second with a burst of 20 called for by the issue.
    fn default() -> Self {
        Self {
            rate_per_sec: 10,
            burst: 20,
            max_violations: 3,
            cooldown: std::time::Duration::from_secs(30),
        }
    }
}

/// Why a client frame was not forwarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropReason {
    /// Over the frame budget for this session.
    RateExceeded,
    /// Session is serving a cooldown after repeated overruns.
    CooldownActive,
}

/// Outcome of offering one client frame to the limiter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameDecision {
    /// Forward to chess engine validation.
    Accept,
    /// Drop before engine validation; the connection stays up.
    Dropped(DropReason),
    /// Drop and terminate: the session is in cooldown.
    InCooldown {
        retry_after_secs: u64,
    },
}

/// Per-session frame limiter for a WebSocket actor.
///
/// Holds no global state, so each session is independent and nothing needs to be
/// shared between nodes.
#[derive(Debug, Clone)]
pub struct SessionRateLimiter {
    bucket: TokenBucket,
    config: SessionRateLimitConfig,
    consecutive_violations: u32,
    cooldown_until: Option<std::time::Instant>,
}

impl SessionRateLimiter {
    pub fn new(config: SessionRateLimitConfig) -> Self {
        Self {
            bucket: TokenBucket::new(config.rate_per_sec, config.burst),
            config,
            consecutive_violations: 0,
            cooldown_until: None,
        }
    }

    pub fn check(&mut self, now: std::time::Instant) -> FrameDecision {
        if let Some(until) = self.cooldown_until {
            if now < until {
                return FrameDecision::InCooldown {
                    retry_after_secs: until.saturating_duration_since(now).as_secs().max(1),
                };
            }
            // Cooldown served: the session gets a fresh budget.
            self.cooldown_until = None;
            self.consecutive_violations = 0;
            self.bucket = TokenBucket::new(self.config.rate_per_sec, self.config.burst);
        }

        if self.bucket.try_acquire(now) {
            // A clean frame clears the strike count.
            self.consecutive_violations = 0;
            return FrameDecision::Accept;
        }

        record_dropped_frame(DropReason::RateExceeded);
        self.consecutive_violations += 1;

        if self.consecutive_violations >= self.config.max_violations {
            self.cooldown_until = Some(now + self.config.cooldown);
            self.consecutive_violations = 0;
            record_dropped_frame(DropReason::CooldownActive);
            return FrameDecision::InCooldown {
                retry_after_secs: self.config.cooldown.as_secs().max(1),
            };
        }

        FrameDecision::Dropped(DropReason::RateExceeded)
    }

    pub fn consecutive_violations(&self) -> u32 {
        self.consecutive_violations
    }

    pub fn is_in_cooldown(&self, now: std::time::Instant) -> bool {
        self.cooldown_until.map(|until| now < until).unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Per-node spectator admission control (#1140)
// ---------------------------------------------------------------------------

/// Why a spectator was not admitted to this node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionError {
    /// The node is already carrying its share of the spectator load.
    NodeAtCapacity,
    /// This single game already has its spectator cap on this node.
    GameAtCapacity,
}

/// Admission limits applied per node.
#[derive(Debug, Clone, Copy)]
pub struct SpectatorAdmissionConfig {
    /// Spectators this node will carry in total.
    pub max_per_node: usize,
    /// Spectators for a single game on this node.
    pub max_per_game: usize,
}

impl Default for SpectatorAdmissionConfig {
    fn default() -> Self {
        Self {
            max_per_node: 25_000,
            max_per_game: 25_000,
        }
    }
}

#[derive(Debug)]
struct GameEntry {
    count: usize,
    last_seen: std::time::Instant,
}

/// Decides which spectators this node accepts, and which it sheds.
///
/// The Redis fan-out in `redis_broadcast` moves frames between nodes; this
/// decides how many spectators a node takes in the first place. Shedding here
/// is what keeps one node from saturating while others sit idle, and it is the
/// decision an edge node can make locally without a cluster round trip.
#[derive(Debug)]
pub struct SpectatorAdmission {
    config: SpectatorAdmissionConfig,
    games: std::collections::HashMap<String, GameEntry>,
    total: usize,
}

impl SpectatorAdmission {
    pub fn new(config: SpectatorAdmissionConfig) -> Self {
        Self {
            config,
            games: std::collections::HashMap::new(),
            total: 0,
        }
    }

    /// Admit a spectator for `game_id`, or report why it was shed.
    ///
    /// `ttl` bounds how long an idle game's counter is kept: without it a
    /// long-lived node accumulates an entry per game it has ever seen and the
    /// map grows without limit.
    pub fn admit(
        &mut self,
        game_id: &str,
        now: std::time::Instant,
        ttl: std::time::Duration,
    ) -> Result<(), AdmissionError> {
        self.evict_idle(now, ttl);

        let per_game = self.games.get(game_id).map(|entry| entry.count).unwrap_or(0);
        if per_game >= self.config.max_per_game {
            return Err(AdmissionError::GameAtCapacity);
        }
        if self.total >= self.config.max_per_node {
            return Err(AdmissionError::NodeAtCapacity);
        }

        self.games
            .entry(game_id.to_string())
            .and_modify(|entry| {
                entry.count += 1;
                entry.last_seen = now;
            })
            .or_insert(GameEntry {
                count: 1,
                last_seen: now,
            });
        self.total += 1;

        Ok(())
    }

    /// Release a spectator slot when a connection closes.
    pub fn release(&mut self, game_id: &str) {
        // The node total is decremented on every successful release, including
        // when the game's last spectator leaves and its entry is dropped.
        // Returning early on that path leaked the slot and would eventually
        // leave `is_saturated` permanently true.
        let released = match self.games.get_mut(game_id) {
            Some(entry) => {
                entry.count = entry.count.saturating_sub(1);
                if entry.count == 0 {
                    self.games.remove(game_id);
                }
                1
            }
            None => 0,
        };
        self.total = self.total.saturating_sub(released);
    }

    /// Drop games that have been idle past `ttl`, releasing their slots.
    fn evict_idle(&mut self, now: std::time::Instant, ttl: std::time::Duration) {
        let stale: Vec<String> = self
            .games
            .iter()
            .filter(|(_, entry)| now.saturating_duration_since(entry.last_seen) > ttl)
            .map(|(id, _)| id.clone())
            .collect();

        for id in stale {
            if let Some(entry) = self.games.remove(&id) {
                self.total = self.total.saturating_sub(entry.count);
            }
        }
    }

    pub fn total(&self) -> usize {
        self.total
    }

    pub fn for_game(&self, game_id: &str) -> usize {
        self.games.get(game_id).map(|entry| entry.count).unwrap_or(0)
    }

    /// True once the node is full and further spectators should be routed
    /// elsewhere by the load balancer.
    pub fn is_saturated(&self) -> bool {
        self.total >= self.config.max_per_node
    }
}

// ---------------------------------------------------------------------------
// Dropped-frame metrics (#1138)
// ---------------------------------------------------------------------------

/// Frames dropped before engine validation, by reason.
///
/// Registered on the shared `crate::metrics` registry so these series are
/// exported by the existing `/metrics` endpoint.
pub static DROPPED_CLIENT_FRAMES: once_cell::sync::Lazy<prometheus::CounterVec> =
    once_cell::sync::Lazy::new(|| {
        let counter = prometheus::CounterVec::new(
            prometheus::Opts::new(
                "ws_client_frames_dropped_total",
                "Client WebSocket frames dropped before chess engine validation",
            ),
            &["reason"],
        )
        .expect("failed to build ws_client_frames_dropped_total");

        // A duplicate registration means another instance already installed an
        // identically named collector; keeping the local handle is still correct.
        let _ = crate::metrics::Metrics::registry().register(Box::new(counter.clone()));
        counter
    });

fn record_dropped_frame(reason: DropReason) {
    let label = match reason {
        DropReason::RateExceeded => "rate_exceeded",
        DropReason::CooldownActive => "cooldown_active",
    };
    DROPPED_CLIENT_FRAMES.with_label_values(&[label]).inc();
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App, HttpResponse};

    async fn mock_handler() -> HttpResponse {
        HttpResponse::Ok().body("OK")
    }

    /// Helper to create a mock Redis pool for testing.
    /// Returns `None` if Redis is not available.
    async fn get_test_pool() -> Option<Pool> {
        let pool = match deadpool_redis::Config::from_url("redis://localhost:6379")
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        {
            Ok(p) => p,
            Err(_) => return None,
        };

        let mut conn = pool.get().await.ok()?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await.ok()?;

        Some(pool)
    }

    #[actix_web::test]
    async fn test_rate_limit_allows_requests_under_limit() {
        let Some(pool) = get_test_pool().await else {
            eprintln!("Skipping test: Redis not available");
            return;
        };

        let limiter = RedisRateLimiter::new(pool.clone(), 10, 60);

        let app = test::init_service(
            App::new()
                .wrap(limiter)
                .route("/test", web::get().to(mock_handler)),
        )
        .await;

        for _ in 0..5 {
            let req = test::TestRequest::get()
                .uri("/test")
                .peer_addr("127.0.0.1:12345".parse().unwrap())
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200, "Request should succeed under limit");
        }
    }

    #[actix_web::test]
    async fn test_rate_limit_blocks_after_exceeded() {
        let Some(pool) = get_test_pool().await else {
            eprintln!("Skipping test: Redis not available");
            return;
        };

        let cleanup_pool = pool.clone();
        let limiter = RedisRateLimiter::new(pool.clone(), 3, 60);
        let peer = "127.0.0.2:12345".parse().unwrap();

        let app = test::init_service(
            App::new()
                .wrap(limiter)
                .route("/test", web::get().to(mock_handler)),
        )
        .await;

        // Send 3 requests — all should pass
        for i in 0..3 {
            let req = test::TestRequest::get()
                .uri("/test")
                .peer_addr(peer)
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200, "Request {} should succeed", i + 1);
        }

        // 4th request should be rate limited
        let req = test::TestRequest::get()
            .uri("/test")
            .peer_addr(peer)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 429, "Request should be rate limited");

        // Clean up
        if let Ok(mut conn) = cleanup_pool.get().await {
            let _: Result<(), _> = redis::cmd("DEL")
                .arg("rl:ip:127.0.0.2")
                .query_async(&mut conn)
                .await;
        }
    }

    #[actix_web::test]
    async fn test_rate_limit_headers_present() {
        let Some(pool) = get_test_pool().await else {
            eprintln!("Skipping test: Redis not available");
            return;
        };

        let cleanup_pool = pool.clone();
        let limiter = RedisRateLimiter::new(pool.clone(), 5, 60);

        let app = test::init_service(
            App::new()
                .wrap(limiter)
                .route("/test", web::get().to(mock_handler)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/test")
            .peer_addr("127.0.0.3:12345".parse().unwrap())
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.headers().contains_key("X-RateLimit-Limit"));
        assert!(resp.headers().contains_key("X-RateLimit-Remaining"));
        assert!(resp.headers().contains_key("X-RateLimit-Reset"));

        assert_eq!(
            resp.headers()
                .get("X-RateLimit-Limit")
                .unwrap()
                .to_str()
                .unwrap(),
            "5"
        );

        // Clean up
        if let Ok(mut conn) = cleanup_pool.get().await {
            let _: Result<(), _> = redis::cmd("DEL")
                .arg("rl:ip:127.0.0.3")
                .query_async(&mut conn)
                .await;
        }
    }

    #[actix_web::test]
    async fn test_different_ips_have_independent_limits() {
        let Some(pool) = get_test_pool().await else {
            eprintln!("Skipping test: Redis not available");
            return;
        };

        let cleanup_pool = pool.clone();
        let limiter = RedisRateLimiter::new(pool.clone(), 2, 60);

        let app = test::init_service(
            App::new()
                .wrap(limiter)
                .route("/test", web::get().to(mock_handler)),
        )
        .await;

        // Exhaust limit for IP 1
        for _ in 0..2 {
            let req = test::TestRequest::get()
                .uri("/test")
                .peer_addr("10.0.0.1:12345".parse().unwrap())
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200);
        }

        // IP 1 should now be blocked
        let req = test::TestRequest::get()
            .uri("/test")
            .peer_addr("10.0.0.1:12345".parse().unwrap())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 429, "IP 1 should be rate limited");

        // IP 2 should still be allowed
        let req = test::TestRequest::get()
            .uri("/test")
            .peer_addr("10.0.0.2:12345".parse().unwrap())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200, "IP 2 should not be rate limited");

        // Clean up
        if let Ok(mut conn) = cleanup_pool.get().await {
            let _: Result<(), _> = redis::cmd("DEL")
                .arg("rl:ip:10.0.0.1")
                .query_async(&mut conn)
                .await;
            let _: Result<(), _> = redis::cmd("DEL")
                .arg("rl:ip:10.0.0.2")
                .query_async(&mut conn)
                .await;
        }
    }
}
