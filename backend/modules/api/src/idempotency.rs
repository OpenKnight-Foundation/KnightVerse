// src/idempotency.rs
// BE-46: Idempotency-Key Header Middleware for Staking, Escrow & Payment Endpoints

use actix_web::{
    body::{to_bytes, BoxBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::{header, Method, StatusCode},
    HttpMessage, HttpResponse,
};
use deadpool_redis::Pool;
use redis::cmd;
use security::jwt::Claims;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::{ready, Future, Ready};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Default TTL for idempotency records in seconds (120s as specified in BE-46)
pub const DEFAULT_IDEMPOTENCY_TTL_SECS: u64 = 120;

/// Header names supported for idempotency key
pub const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";
pub const X_IDEMPOTENCY_KEY_HEADER: &str = "x-idempotency-key";

/// Status of an idempotency key operation in Redis
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IdempotencyStatus {
    Pending,
    Completed,
}

/// Idempotency record stored in Redis / storage backend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdempotencyRecord {
    pub status: IdempotencyStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<Vec<(String, String)>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
}

impl IdempotencyRecord {
    pub fn new_pending() -> Self {
        Self {
            status: IdempotencyStatus::Pending,
            status_code: None,
            headers: None,
            body: None,
        }
    }

    pub fn new_completed(
        status_code: u16,
        headers: Vec<(String, String)>,
        body: String,
    ) -> Self {
        Self {
            status: IdempotencyStatus::Completed,
            status_code: Some(status_code),
            headers: Some(headers),
            body: Some(body),
        }
    }
}

/// Result of attempting to acquire an atomic idempotency lock
pub enum LockResult {
    Acquired,
    Exists(IdempotencyRecord),
    Error,
}

/// Storage backend for idempotency keys (Redis in production, InMemory option for tests)
#[derive(Clone)]
pub enum IdempotencyStorage {
    Redis(Pool),
    InMemory(Arc<Mutex<HashMap<String, (IdempotencyRecord, Instant)>>>),
}

impl IdempotencyStorage {
    /// Attempt atomic lock creation: SET NX EX ttl
    pub async fn try_lock(&self, key: &str, ttl_secs: u64) -> LockResult {
        match self {
            Self::Redis(pool) => {
                let mut conn = match pool.get().await {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("Redis pool error during idempotency lock: {}", e);
                        return LockResult::Error;
                    }
                };

                let pending_record = IdempotencyRecord::new_pending();
                let pending_json = match serde_json::to_string(&pending_record) {
                    Ok(j) => j,
                    Err(e) => {
                        error!("Failed to serialize pending record: {}", e);
                        return LockResult::Error;
                    }
                };

                let set_res: Result<Option<String>, _> = cmd("SET")
                    .arg(key)
                    .arg(&pending_json)
                    .arg("NX")
                    .arg("EX")
                    .arg(ttl_secs)
                    .query_async(&mut conn)
                    .await;

                match set_res {
                    Ok(Some(_)) => LockResult::Acquired,
                    Ok(None) => {
                        // Key exists — fetch its current state
                        let get_res: Result<Option<String>, _> =
                            cmd("GET").arg(key).query_async(&mut conn).await;
                        if let Ok(Some(cached_json)) = get_res {
                            if let Ok(record) = serde_json::from_str::<IdempotencyRecord>(&cached_json) {
                                return LockResult::Exists(record);
                            }
                        }
                        LockResult::Error
                    }
                    Err(e) => {
                        error!("Redis command error during try_lock: {}", e);
                        LockResult::Error
                    }
                }
            }
            Self::InMemory(map_lock) => {
                let mut map = map_lock.lock().await;
                let now = Instant::now();

                // Check if existing key is still valid (unexpired)
                if let Some((record, expires_at)) = map.get(key) {
                    if *expires_at > now {
                        return LockResult::Exists(record.clone());
                    }
                }

                // Acquire lock
                let record = IdempotencyRecord::new_pending();
                map.insert(
                    key.to_string(),
                    (record, now + Duration::from_secs(ttl_secs)),
                );
                LockResult::Acquired
            }
        }
    }

    /// Save completed response payload into storage with TTL
    pub async fn save_completed(
        &self,
        key: &str,
        record: IdempotencyRecord,
        ttl_secs: u64,
    ) {
        match self {
            Self::Redis(pool) => {
                if let Ok(mut conn) = pool.get().await {
                    if let Ok(json_str) = serde_json::to_string(&record) {
                        let res: Result<(), _> = cmd("SET")
                            .arg(key)
                            .arg(&json_str)
                            .arg("EX")
                            .arg(ttl_secs)
                            .query_async(&mut conn)
                            .await;
                        if let Err(e) = res {
                            warn!("Failed to save completed idempotency record in Redis: {}", e);
                        }
                    }
                }
            }
            Self::InMemory(map_lock) => {
                let mut map = map_lock.lock().await;
                map.insert(
                    key.to_string(),
                    (record, Instant::now() + Duration::from_secs(ttl_secs)),
                );
            }
        }
    }

    /// Release/delete pending key on server error (5xx)
    pub async fn release_lock(&self, key: &str) {
        match self {
            Self::Redis(pool) => {
                if let Ok(mut conn) = pool.get().await {
                    let _: Result<(), _> = cmd("DEL").arg(key).query_async(&mut conn).await;
                }
            }
            Self::InMemory(map_lock) => {
                let mut map = map_lock.lock().await;
                map.remove(key);
            }
        }
    }
}

/// Actix-Web middleware for Idempotency-Key support.
#[derive(Clone)]
pub struct IdempotencyMiddleware {
    pub storage: IdempotencyStorage,
    pub ttl_seconds: u64,
}

impl IdempotencyMiddleware {
    /// Create a new `IdempotencyMiddleware` with a Redis connection pool.
    pub fn new(pool: Pool) -> Self {
        Self {
            storage: IdempotencyStorage::Redis(pool),
            ttl_seconds: DEFAULT_IDEMPOTENCY_TTL_SECS,
        }
    }

    /// Create an in-memory `IdempotencyMiddleware` (useful for fast isolated unit tests).
    pub fn in_memory() -> Self {
        Self {
            storage: IdempotencyStorage::InMemory(Arc::new(Mutex::new(HashMap::new()))),
            ttl_seconds: DEFAULT_IDEMPOTENCY_TTL_SECS,
        }
    }

    /// Set custom TTL duration in seconds.
    pub fn with_ttl(mut self, ttl_seconds: u64) -> Self {
        self.ttl_seconds = ttl_seconds;
        self
    }
}

impl<S, B> Transform<S, ServiceRequest> for IdempotencyMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = actix_web::Error;
    type Transform = IdempotencyMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(IdempotencyMiddlewareService {
            service: Rc::new(service),
            storage: self.storage.clone(),
            ttl_seconds: self.ttl_seconds,
        }))
    }
}

pub struct IdempotencyMiddlewareService<S> {
    service: Rc<S>,
    storage: IdempotencyStorage,
    ttl_seconds: u64,
}

impl<S, B> Service<ServiceRequest> for IdempotencyMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let method = req.method().clone();

        // 1. Only intercept mutating requests (POST, PUT, PATCH)
        if method != Method::POST && method != Method::PUT && method != Method::PATCH {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_boxed_body())
            });
        }

        // 2. Extract Idempotency-Key header
        let idempotency_key = req
            .headers()
            .get(IDEMPOTENCY_KEY_HEADER)
            .or_else(|| req.headers().get(X_IDEMPOTENCY_KEY_HEADER))
            .and_then(|h| h.to_str().ok())
            .map(|s| s.trim().to_string());

        let idempotency_key = match idempotency_key {
            Some(k) if !k.is_empty() => k,
            _ => {
                // No idempotency key provided — pass through normally
                let fut = self.service.call(req);
                return Box::pin(async move {
                    let res = fut.await?;
                    Ok(res.map_into_boxed_body())
                });
            }
        };

        // 3. User isolation: Scope by user ID from JWT claims if authenticated
        let user_scope = req
            .extensions()
            .get::<Claims>()
            .map(|claims| format!("user:{}", claims.user_id))
            .unwrap_or_else(|| "anon".to_string());

        let redis_key = format!("idempotency:{}:{}", user_scope, idempotency_key);
        let storage = self.storage.clone();
        let ttl = self.ttl_seconds;
        let service = self.service.clone();

        Box::pin(async move {
            let lock_res = storage.try_lock(&redis_key, ttl).await;

            match lock_res {
                LockResult::Acquired => {
                    // Lock acquired! Proceed with executing underlying request
                    debug!("Acquired idempotency lock for key: {}", redis_key);
                    let (http_req, payload) = req.into_parts();
                    let service_req = ServiceRequest::from_parts(http_req, payload);

                    let fut = service.call(service_req);
                    let res = fut.await?;

                    let status = res.status();
                    let (req_inner, http_res) = res.into_parts();

                    // Clone response headers before consuming body
                    let original_headers: Vec<(header::HeaderName, header::HeaderValue)> = http_res
                        .headers()
                        .iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect();

                    // Extract response headers to cache
                    let mut cached_headers: Vec<(String, String)> = Vec::new();
                    for (header_name, header_val) in &original_headers {
                        if let Ok(val_str) = header_val.to_str() {
                            let name_str = header_name.as_str().to_string();
                            // Skip hop-by-hop headers
                            if name_str != "connection" && name_str != "transfer-encoding" {
                                cached_headers.push((name_str, val_str.to_string()));
                            }
                        }
                    }

                    // Extract response body bytes
                    let body = http_res.into_body();
                    let body_bytes = match to_bytes(body).await {
                        Ok(b) => b,
                        Err(_) => {
                            error!("Failed to read response body for idempotency caching");
                            storage.release_lock(&redis_key).await;
                            return Ok(ServiceResponse::new(
                                req_inner,
                                HttpResponse::InternalServerError().finish(),
                            ));
                        }
                    };

                    let body_str = String::from_utf8_lossy(&body_bytes).to_string();

                    // What NOT to be done: Do NOT cache 5xx server errors as permanent idempotent results
                    if status.is_server_error() {
                        warn!(
                            "Server error {} returned. Releasing idempotency lock for key: {}",
                            status, redis_key
                        );
                        storage.release_lock(&redis_key).await;
                    } else {
                        // Store the completed response payload in storage with remaining TTL
                        let completed_record = IdempotencyRecord::new_completed(
                            status.as_u16(),
                            cached_headers,
                            body_str,
                        );
                        storage.save_completed(&redis_key, completed_record, ttl).await;
                        debug!("Saved completed idempotency record for key: {}", redis_key);
                    }

                    // Reconstruct and return response with boxed body
                    let mut response_builder = HttpResponse::build(status);
                    for (h_name, h_val) in original_headers {
                        response_builder.insert_header((h_name, h_val));
                    }

                    let response = response_builder.body(body_bytes);
                    Ok(ServiceResponse::new(req_inner, response))
                }
                LockResult::Exists(record) => match record.status {
                    IdempotencyStatus::Pending => {
                        // 409 Conflict: Concurrent duplicate request in progress
                        info!(
                            "Concurrent request with duplicate idempotency key in progress: {}",
                            redis_key
                        );
                        let conflict_response = HttpResponse::Conflict().json(
                            serde_json::json!({
                                "error": "Conflict",
                                "code": "OPERATION_IN_PROGRESS",
                                "message": "A request with this Idempotency-Key is currently in progress."
                            }),
                        );
                        Ok(req.into_response(conflict_response))
                    }
                    IdempotencyStatus::Completed => {
                        // Replay cached response directly!
                        info!(
                            "Returning cached idempotent response for key: {}",
                            redis_key
                        );
                        let status_code = record
                            .status_code
                            .and_then(|code| StatusCode::from_u16(code).ok())
                            .unwrap_or(StatusCode::OK);

                        let mut res_builder = HttpResponse::build(status_code);

                        // Restore cached headers
                        if let Some(headers) = record.headers {
                            for (k, v) in headers {
                                res_builder.insert_header((
                                    header::HeaderName::from_bytes(k.as_bytes())
                                        .unwrap_or(header::CONTENT_TYPE),
                                    header::HeaderValue::from_str(&v)
                                        .unwrap_or_else(|_| header::HeaderValue::from_static("application/json")),
                                ));
                            }
                        }

                        // Add indicator header that response was replayed
                        res_builder.insert_header(("Idempotency-Replayed", "true"));

                        let body_content = record.body.unwrap_or_default();
                        let response = res_builder.body(body_content);
                        Ok(req.into_response(response))
                    }
                },
                LockResult::Error => {
                    // Fallback to normal execution if storage fails
                    let res = service.call(req).await?;
                    Ok(res.map_into_boxed_body())
                }
            }
        })
    }
}
