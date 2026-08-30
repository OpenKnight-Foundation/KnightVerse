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
